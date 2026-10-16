//! Phase 2 M3/M4: the loop on hand-built units, for the opcodes no compiled
//! program lowers to (the root crate's corpus runs the others on both
//! cores), and the runtime's call API. Passes under `--features gc-stress`.

use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{CodeBuilder, CodeUnitId, Opcode, flags};
use oxide_core::heap::gc::HeapLimits;
use oxide_core::{ErrorKind, PropertyKey, Runtime, Value};

fn runtime() -> Runtime {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    runtime.install_print().unwrap();
    runtime.capture_output();
    runtime
}

fn publish(
    runtime: &mut Runtime,
    layout: UnitLayout,
    build: impl FnOnce(&mut CodeBuilder),
) -> CodeUnitId {
    let mut builder = CodeBuilder::new();
    build(&mut builder);
    runtime.publish(builder.finish(layout)).unwrap()
}

fn layout(registers: u16) -> UnitLayout {
    UnitLayout {
        register_count: registers,
        flags: flags::STRICT,
        ..UnitLayout::default()
    }
}

/// Runs a unit tree with no constants as a script.
fn run(runtime: &mut Runtime, root: CodeUnitId) -> Result<Value, oxide_core::Throw> {
    let tables: Vec<&[Value]> = runtime.code().tree(root).iter().map(|_| &[][..]).collect();
    let function = runtime.instantiate(root, &tables).unwrap();
    let handle = runtime.hold(Value::object(function));
    let result = runtime.run_script(function);
    runtime.release(handle);
    result
}

fn key(runtime: &mut Runtime, name: &str) -> PropertyKey {
    runtime.heap_mut().property_key(name).unwrap()
}

#[test]
fn wide_registers_shuttle_above_the_window() {
    let mut runtime = runtime();
    let root = publish(&mut runtime, layout(300), |b| {
        b.a_int(Opcode::LoadInt, 1, 7);
        b.a_wide(Opcode::StoreWide, 1, 290);
        b.a_int(Opcode::LoadInt, 1, 0);
        b.a_wide(Opcode::LoadWide, 2, 290);
        b.a(Opcode::Return, 2);
    });
    assert_eq!(run(&mut runtime, root), Ok(Value::int32(7)));
}

#[test]
fn return_undefined_and_nop() {
    let mut runtime = runtime();
    let root = publish(&mut runtime, layout(1), |b| {
        b.op(Opcode::Nop);
        b.op(Opcode::ReturnUndefined);
    });
    assert_eq!(run(&mut runtime, root), Ok(Value::UNDEFINED));
}

/// `t3 = t2 + IsUndefined(x) * 1 + IsNull(x) * 2 + …` over one value.
fn tests_of(value: impl Fn(&mut CodeBuilder)) -> i32 {
    let mut runtime = runtime();
    let root = publish(&mut runtime, layout(6), |b| {
        value(b);
        b.a_int(Opcode::LoadInt, 2, 0);
        for (bit, op) in [
            Opcode::IsUndefined,
            Opcode::IsNull,
            Opcode::TypeOfIsUndefined,
            Opcode::TypeOfIsFunction,
        ]
        .into_iter()
        .enumerate()
        {
            b.ab(op, 3, 1);
            let skip = b.a_jump(Opcode::JFalse, 3, 0);
            b.ab_int(Opcode::AddI, 2, 2, 1 << bit);
            let next = b.pc();
            b.patch_target(skip, next);
        }
        let skip = b.a_jump(Opcode::JUndefinedOrNull, 1, 0);
        b.ab_int(Opcode::AddI, 2, 2, 16);
        let next = b.pc();
        b.patch_target(skip, next);
        b.a(Opcode::Return, 2);
    });
    run(&mut runtime, root).unwrap().as_int32().unwrap()
}

#[test]
fn fused_nullish_and_typeof_tests() {
    assert_eq!(
        tests_of(|b| {
            b.a(Opcode::LoadUndefined, 1);
        }),
        0b0101
    );
    assert_eq!(
        tests_of(|b| {
            b.a(Opcode::LoadNull, 1);
        }),
        0b0010
    );
    assert_eq!(
        tests_of(|b| {
            b.a_int(Opcode::LoadInt, 1, 0);
        }),
        0b10000
    );
    assert_eq!(
        tests_of(|b| {
            b.a(Opcode::LoadFunction, 1);
        }),
        0b11000
    );
}

#[test]
fn delete_field_removes_an_own_property() {
    let mut runtime = runtime();
    let x = key(&mut runtime, "x");
    let root = publish(&mut runtime, layout(6), |b| {
        b.a_count(Opcode::NewObject, 1, 1);
        b.a_int(Opcode::LoadInt, 2, 5);
        b.ab_key(Opcode::DefineField, 1, 2, x);
        b.ab_key(Opcode::DeleteField, 3, 1, x);
        b.ab_key(Opcode::GetField, 4, 1, x);
        b.ab(Opcode::IsUndefined, 4, 4);
        b.abc(Opcode::StrictEq, 5, 3, 4);
        b.a(Opcode::Return, 5);
    });
    assert_eq!(run(&mut runtime, root), Ok(Value::TRUE));
}

/// `child(p0) = p0 * 2 + this.k`, called by a method tail call from the
/// root on `{k: 1}`.
#[test]
fn tail_calls_return_the_callee_result() {
    let mut runtime = runtime();
    let k = key(&mut runtime, "k");
    let child = publish(
        &mut runtime,
        UnitLayout {
            register_count: 4,
            parameter_count: 1,
            flags: flags::STRICT,
            ..UnitLayout::default()
        },
        |b| {
            b.ab_int(Opcode::MulI, 2, 1, 2);
            b.ab_key(Opcode::GetField, 3, 0, k);
            b.abc(Opcode::Add, 2, 2, 3);
            b.a(Opcode::Return, 2);
        },
    );
    let mut root_layout = layout(6);
    root_layout.children = vec![child];
    let root = publish(&mut runtime, root_layout, |b| {
        // receiver {k: 1} in t0, callee in t1, argument in t2.
        b.a_count(Opcode::NewObject, 1, 1);
        b.a_int(Opcode::LoadInt, 2, 1);
        b.ab_key(Opcode::DefineField, 1, 2, k);
        b.a_child(Opcode::MakeClosure, 2, 0);
        b.a_int(Opcode::LoadInt, 3, 20);
        b.call(Opcode::TailCallMethod, 0, 1, 1);
    });
    assert_eq!(run(&mut runtime, root), Ok(Value::int32(41)));
}

#[test]
fn tail_call_with_an_undefined_receiver_throws_from_the_callee() {
    let mut runtime = runtime();
    let k = key(&mut runtime, "k");
    let child = publish(
        &mut runtime,
        UnitLayout {
            register_count: 3,
            parameter_count: 1,
            flags: flags::STRICT,
            ..UnitLayout::default()
        },
        |b| {
            b.ab_key(Opcode::GetField, 2, 0, k);
            b.a(Opcode::Return, 2);
        },
    );
    let mut root_layout = layout(4);
    root_layout.children = vec![child];
    let root = publish(&mut runtime, root_layout, |b| {
        b.a_child(Opcode::MakeClosure, 1, 0);
        b.a_int(Opcode::LoadInt, 2, 1);
        b.call(Opcode::TailCall, 0, 1, 1);
    });
    let thrown = run(&mut runtime, root).unwrap_err();
    let text = runtime.display(thrown.0).unwrap();
    assert_eq!(text, "TypeError: cannot read property 'k' of undefined");
    assert_eq!(runtime.vm_stack().depth(), 0);
}

#[test]
fn tail_call_to_a_native() {
    let mut runtime = runtime();
    let print = key(&mut runtime, "print");
    let root = publish(&mut runtime, layout(4), |b| {
        b.a_key(Opcode::GetGlobal, 1, print);
        b.a_int(Opcode::LoadInt, 2, 9);
        b.call(Opcode::TailCall, 0, 1, 1);
    });
    assert_eq!(run(&mut runtime, root), Ok(Value::UNDEFINED));
    assert_eq!(runtime.take_output(), "9\n");
}

#[test]
fn calls_from_rust_pass_this_and_arguments() {
    let mut runtime = runtime();
    let root = publish(
        &mut runtime,
        UnitLayout {
            register_count: 4,
            parameter_count: 2,
            flags: flags::STRICT,
            ..UnitLayout::default()
        },
        |b| {
            b.abc(Opcode::Sub, 3, 1, 2);
            b.a(Opcode::Return, 3);
        },
    );
    let function = runtime.instantiate(root, &[&[]]).unwrap();
    let handle = runtime.hold(Value::object(function));
    let result = runtime.call(
        Value::object(function),
        Value::UNDEFINED,
        &[Value::int32(10), Value::number(0.5)],
    );
    assert_eq!(result, Ok(Value::number(9.5)));
    // Missing arguments are undefined.
    let result = runtime.call(
        Value::object(function),
        Value::UNDEFINED,
        &[Value::int32(1)],
    );
    assert!(result.unwrap().as_f64().unwrap().is_nan());
    runtime.release(handle);
    let thrown = runtime
        .call(Value::int32(3), Value::UNDEFINED, &[])
        .unwrap_err();
    assert_eq!(
        runtime.display(thrown.0).unwrap(),
        "TypeError: not a function"
    );
}

#[test]
fn the_register_limit_is_a_stack_overflow() {
    let mut runtime = runtime();
    runtime.set_stack_limit(4096);
    // f() { return f(); } in a sloppy function looked up as a global.
    let f = key(&mut runtime, "f");
    let child = publish(
        &mut runtime,
        UnitLayout {
            register_count: 2,
            ..UnitLayout::default()
        },
        |b| {
            b.a_key(Opcode::GetGlobal, 1, f);
            b.call(Opcode::Call, 1, 1, 0);
            b.a(Opcode::Return, 1);
        },
    );
    let mut root_layout = layout(3);
    root_layout.children = vec![child];
    let root = publish(&mut runtime, root_layout, |b| {
        b.a_child(Opcode::MakeClosure, 1, 0);
        b.a_key(Opcode::DefineGlobal, 1, f);
        b.call(Opcode::Call, 1, 1, 0);
        b.a(Opcode::Return, 1);
    });
    let thrown = run(&mut runtime, root).unwrap_err();
    assert_eq!(
        runtime.display(thrown.0).unwrap(),
        "InternalError: stack overflow"
    );
    assert_eq!(runtime.vm_stack().depth(), 0);
    assert!(runtime.vm_stack().registers_reserved() <= 4096 + 512);
}

#[test]
fn error_kinds_have_their_prototypes() {
    let mut runtime = runtime();
    for kind in ErrorKind::ALL {
        let name = key(&mut runtime, kind.name());
        let message = key(&mut runtime, "m");
        let root = publish(&mut runtime, layout(4), |b| {
            b.a_key(Opcode::GetGlobal, 1, name);
            b.ab(Opcode::Mov, 2, 1);
            b.a_key(Opcode::GetGlobalOrUndefined, 3, message);
            b.call(Opcode::New, 1, 1, 1);
            b.a(Opcode::Return, 1);
        });
        let error = run(&mut runtime, root).unwrap();
        let handle = runtime.hold(error);
        assert_eq!(runtime.display(error).unwrap(), kind.name());
        runtime.release(handle);
    }
}
