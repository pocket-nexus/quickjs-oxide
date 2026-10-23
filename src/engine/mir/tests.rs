use super::dump::dump;
use super::ir::{
    Args, BlockId, Function, Handler, HandlerId, Imm, Insn, Phi, Pos, Terminator, Value, ValueInfo,
};
use super::op::{ImmKind, LOWERING_ONLY, Op};
use super::verify::verify;
use crate::engine::hir::schema::FunctionRef;
use oxide_core::bytecode::{Format, Opcode};

#[test]
fn every_core_opcode_is_named_once_or_left_to_the_lowering() {
    for &opcode in Opcode::ALL {
        let named: Vec<Op> = Op::ALL
            .iter()
            .copied()
            .filter(|op| op.core().contains(&opcode))
            .collect();
        let lowering = LOWERING_ONLY.contains(&opcode);
        assert!(
            named.len() + usize::from(lowering) == 1,
            "{} is named by {named:?} and lowering-only: {lowering}",
            opcode.name()
        );
    }
}

#[test]
fn immediates_match_the_core_formats() {
    for &op in Op::ALL {
        let format = op.core()[0].format();
        let ok = match op.imm() {
            ImmKind::None => !matches!(
                format,
                Format::AInt
                    | Format::ABInt
                    | Format::AConst
                    | Format::Key
                    | Format::AKey
                    | Format::ABKey
                    | Format::ACapture
                    | Format::AChild
            ),
            // `IterCall` selects `return` or `throw` by its immediate.
            ImmKind::Int => matches!(format, Format::AInt | Format::ABInt | Format::ABC),
            ImmKind::Const => matches!(format, Format::AConst),
            ImmKind::Key | ImmKind::KeyFlags => {
                matches!(format, Format::Key | Format::AKey | Format::ABKey)
            }
            ImmKind::Flags => matches!(format, Format::ACount | Format::ABC),
            ImmKind::Capture => matches!(format, Format::ACapture),
            ImmKind::Child => matches!(format, Format::AChild),
        };
        assert!(
            ok,
            "{} has immediate {:?} for {:?}",
            op.name(),
            op.imm(),
            format
        );
    }
}

fn insn(op: Op, args: &[Value], result: Option<Value>, imm: Imm) -> Insn {
    Insn {
        op,
        args: Args::from_slice(args),
        results: [result, None],
        imm,
        pos: Pos::NONE,
    }
}

// `let i = 0; while (i < n) i = i + 1; return i;` with `n` the parameter.
fn counting_loop() -> Function {
    let mut f = Function::new(FunctionRef(0));
    let this = f.new_value(ValueInfo::default());
    let n = f.new_value(ValueInfo::default());
    f.params = vec![this, n];
    let zero = f.new_value(ValueInfo::default());
    let i = f.new_value(ValueInfo::default());
    let one = f.new_value(ValueInfo::default());
    let next = f.new_value(ValueInfo::default());
    let head = f.new_block();
    let body = f.new_block();
    let exit = f.new_block();
    f.blocks[0]
        .insns
        .push(insn(Op::ConstInt, &[], Some(zero), Imm::Int(0)));
    f.blocks[0].term = Terminator::Jump(head);
    f.block_mut(head).phis.push(Phi {
        result: i,
        args: vec![zero, next],
    });
    f.block_mut(head).term = Terminator::BranchCmp {
        op: super::ir::CmpOp::Lt,
        l: i,
        r: n,
        then: body,
        else_: exit,
    };
    f.block_mut(body).loop_depth = 1;
    f.block_mut(body)
        .insns
        .push(insn(Op::ConstInt, &[], Some(one), Imm::Int(1)));
    f.block_mut(body)
        .insns
        .push(insn(Op::Add, &[i, one], Some(next), Imm::None));
    f.block_mut(body).term = Terminator::Jump(head);
    f.block_mut(exit).term = Terminator::Return(i);
    f.recompute_preds();
    f
}

#[test]
fn a_loop_with_a_phi_verifies_and_dumps() {
    let f = counting_loop();
    verify(&f).unwrap();
    assert_eq!(
        dump(&f),
        "mir fn #0 params(v0, v1)\n\
         b0:\n  v2 = ConstInt #0\n  jump b1\n\
         b1: preds b0 b2\n  v3 = phi v2, v5\n  branch.Lt v3 v1 b2 b3\n\
         b2: preds b1 loop 1\n  v4 = ConstInt #1\n  v5 = Add v3, v4\n  jump b1\n\
         b3: preds b1\n  return v3\n"
    );
    assert_eq!(
        f.reverse_postorder(),
        vec![BlockId(0), BlockId(1), BlockId(3), BlockId(2)]
    );
}

#[test]
fn a_use_before_its_definition_is_rejected() {
    let mut f = counting_loop();
    // Return the loop body's sum from the exit, which the body does not
    // dominate.
    f.blocks[3].term = Terminator::Return(Value(5));
    assert!(verify(&f).unwrap_err().0.contains("v5"));
}

#[test]
fn phis_must_match_their_predecessors() {
    let mut f = counting_loop();
    f.blocks[1].phis[0].args.pop();
    assert!(verify(&f).unwrap_err().0.contains("operands"));
}

// try { x = f() } catch (e) { return <value> }: the call's result is
// defined in the region's body and is not available in the catch block.
fn try_catch(returned_from_catch: fn(Value, Value) -> Value) -> Function {
    let mut f = Function::new(FunctionRef(0));
    let this = f.new_value(ValueInfo::default());
    let callee = f.new_value(ValueInfo::default());
    f.params = vec![this, callee];
    let before = f.new_value(ValueInfo::default());
    let called = f.new_value(ValueInfo::default());
    let exception = f.new_value(ValueInfo::default());
    let region = f.new_block();
    let catch = f.new_block();
    f.handlers.push(Handler {
        parent: None,
        catch,
    });
    f.blocks[0]
        .insns
        .push(insn(Op::Undefined, &[], Some(before), Imm::None));
    f.blocks[0].term = Terminator::Jump(region);
    f.block_mut(region).handler = Some(HandlerId(0));
    f.block_mut(region)
        .insns
        .push(insn(Op::Call, &[callee], Some(called), Imm::None));
    f.block_mut(region).term = Terminator::Return(called);
    f.block_mut(catch).catch = Some(exception);
    f.block_mut(catch).term = Terminator::Return(returned_from_catch(before, called));
    f.recompute_preds();
    f
}

#[test]
fn a_catch_block_sees_values_from_before_its_region_only() {
    verify(&try_catch(|before, _| before)).unwrap();
    assert!(verify(&try_catch(|_, called| called)).is_err());
}

#[test]
fn a_value_is_defined_once() {
    let mut f = counting_loop();
    f.blocks[2].insns[0].results[0] = Some(Value(2));
    assert!(verify(&f).unwrap_err().0.contains("twice"));
}
