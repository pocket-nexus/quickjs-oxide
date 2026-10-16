//! Phase 2 M1 acceptance: the instruction format, the builder, the
//! publication validator, the disassembler, and the code-store and bytecode
//! attach path. Every test follows the rooting rule, so the suite also passes
//! under `--features gc-stress`.

use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{
    Capture, CodeBuilder, CodeUnit, Format, Handler, Insn, Opcode, ValidateErrorKind, flags,
};
use oxide_core::heap::gc::HeapLimits;
use oxide_core::{CodeStore, PropertyKey, Runtime, Value};

fn layout(registers: u16) -> UnitLayout {
    UnitLayout {
        register_count: registers,
        parameter_count: 2,
        local_count: 2,
        ..UnitLayout::default()
    }
}

/// `function f(p0, p1) { var l0, l1; return; }` with `t0 ..` above.
fn unit(registers: u16, build: impl FnOnce(&mut CodeBuilder)) -> CodeUnit {
    let mut builder = CodeBuilder::new();
    build(&mut builder);
    builder.finish(layout(registers))
}

fn validate(unit: &CodeUnit) -> Result<(), ValidateErrorKind> {
    oxide_core::bytecode::validate::validate(unit, &[]).map_err(|error| error.kind)
}

#[test]
fn instruction_is_eight_bytes() {
    assert_eq!(size_of::<Insn>(), 8);
    assert_eq!(align_of::<Insn>(), 4);
    assert!(Opcode::ALL.len() <= 256);
    for (index, &op) in Opcode::ALL.iter().enumerate() {
        assert_eq!(op as usize, index);
    }
}

/// Builds every opcode through the builder with format-specific operands,
/// publishes the unit and checks each disassembled line.
#[test]
fn every_opcode_round_trips_through_the_disassembler() {
    let key = PropertyKey::index(7).unwrap();
    let mut builder = CodeBuilder::new();
    let mut expected = Vec::new();
    // The spill opcodes need a spill scope: `spill_scope_accesses_validate`.
    let spill = [Opcode::NewSpill, Opcode::LoadSlot, Opcode::StoreSlot];
    for &op in Opcode::ALL.iter().filter(|op| !spill.contains(op)) {
        // Registers: this, p0, p1, l0, l1, t0 = 5, t1 = 6, ...
        let operands = match op.format() {
            Format::None => {
                builder.op(op);
                String::new()
            }
            Format::A => {
                builder.a(op, 5);
                "t0".into()
            }
            Format::AB => {
                builder.ab(op, 3, 1);
                "l0, p0".into()
            }
            Format::ABC => {
                builder.abc(op, 5, 1, 2);
                "t0, p0, p1".into()
            }
            Format::AInt => {
                builder.a_int(op, 3, -5);
                "l0, -5".into()
            }
            Format::ABInt => {
                builder.ab_int(op, 3, 3, 1);
                "l0, l0, 1".into()
            }
            Format::AConst => {
                builder.a_const(op, 6, 1);
                "t1, k1".into()
            }
            Format::Key => {
                builder.key(op, key);
                "[7]".into()
            }
            Format::AKey => {
                builder.a_key(op, 0, key);
                "this, [7]".into()
            }
            Format::ABKey => {
                builder.ab_key(op, 5, 0, key);
                "t0, this, [7]".into()
            }
            Format::ACapture => {
                builder.a_capture(op, 4, 0);
                "l1, c0".into()
            }
            Format::ACount => {
                builder.a_count(op, 5, 2);
                "t0, 2".into()
            }
            Format::ABRange => {
                builder.ab_range(op, 5, 6, 3);
                "t0, [t1..t3]".into()
            }
            Format::AChild => {
                builder.a_child(op, 5, 0);
                "t0, fn0".into()
            }
            Format::Call => {
                builder.call(op, 5, 5, 2);
                "t0, t0, 2".into()
            }
            Format::Jump => {
                builder.jump(op, 0);
                "@0".into()
            }
            Format::AJump => {
                builder.a_jump(op, 1, 0);
                "p0, @0".into()
            }
            Format::ABJump => {
                builder.ab_jump(op, 1, 2, 0);
                "p0, p1, @0".into()
            }
            Format::AShortJump => {
                builder.a_short_jump(op, 1, -300, 0);
                "p0, -300, @0".into()
            }
            Format::AWide => {
                builder.a_wide(op, 5, 300);
                "t0, t295".into()
            }
        };
        expected.push((op, operands));
    }
    builder.op(Opcode::ReturnUndefined);
    let unit = builder.finish(UnitLayout {
        register_count: 400,
        parameter_count: 2,
        local_count: 2,
        constant_count: 3,
        children: Vec::new(),
        captures: vec![Capture::None],
        name: "every".into(),
        ..UnitLayout::default()
    });
    // `MakeClosure fn0` needs one child: publish a leaf first.
    let mut store = CodeStore::default();
    let leaf = store.publish(unit_returning()).unwrap();
    let mut unit = unit;
    unit.children = vec![leaf].into();
    let id = store.publish(unit).unwrap();

    let text = oxide_core::bytecode::disassemble(
        store.get(id),
        &mut oxide_core::bytecode::disasm::key_placeholder,
    );
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some(
            "every: params 2, locals 2, registers 400, constants 3, children 1, this slots 0, \
             cache sites 6"
        )
    );
    for (pc, (op, operands)) in expected.iter().enumerate() {
        let line = lines.next().unwrap();
        let operands = match store.get(id).code[pc].site() {
            Some(site) => format!("{operands}, ic{site}"),
            None => operands.clone(),
        };
        let wanted = format!("  {pc:04}  {:<20}{operands}", op.name());
        assert_eq!(line, wanted.trim_end(), "{}", op.name());
    }
    assert!(lines.next().unwrap().contains("ReturnUndefined"));
    assert_eq!(lines.next(), Some("  capture c0 <- none"));
    assert_eq!(lines.next(), None);
}

fn unit_returning() -> CodeUnit {
    let mut builder = CodeBuilder::new();
    builder.op(Opcode::ReturnUndefined);
    builder.finish(UnitLayout {
        register_count: 1,
        ..UnitLayout::default()
    })
}

#[test]
fn builder_patches_forward_jumps_and_decodes_operands() {
    let mut builder = CodeBuilder::new();
    let exit = builder.a_short_jump(Opcode::JNltI, 3, -2, 0);
    builder.ab_int(Opcode::AddI, 3, 3, i32::MIN);
    builder.a_capture(Opcode::LoadCaptured, 5, 0);
    let end = builder.pc();
    builder.patch_target(exit, end);
    builder.a(Opcode::Return, 3);
    let code = builder.code();
    assert_eq!(code[0].imm, 3);
    assert_eq!(code[0].short(), -2);
    assert_eq!(code[1].int(), i32::MIN);
    assert_eq!(code[2].imm, 0);
    let mut unit = builder.finish(layout(6));
    unit.captures = vec![Capture::None].into();
    assert_eq!(validate(&unit), Ok(()));
    assert_eq!(unit.temp_base(), 5);
    assert_eq!(unit.debug.lines.len(), unit.code.len());
}

#[test]
#[should_panic(expected = "has no jump target")]
fn patching_a_non_jump_panics() {
    let mut builder = CodeBuilder::new();
    let pc = builder.ab(Opcode::Mov, 1, 2);
    builder.patch_target(pc, 0);
}

#[test]
fn validator_accepts_a_minimal_unit() {
    assert_eq!(validate(&unit_returning()), Ok(()));
}

#[test]
fn validator_rejects_each_malformation() {
    use ValidateErrorKind as E;
    let ret = |b: &mut CodeBuilder| {
        b.op(Opcode::ReturnUndefined);
    };
    let cases: Vec<(&str, CodeUnit, ValidateErrorKind)> = vec![
        ("empty", unit(6, |_| {}), E::EmptyCode),
        (
            "falls off the end",
            unit(6, |b| {
                b.ab(Opcode::Mov, 1, 2);
            }),
            E::FallsOffEnd,
        ),
        (
            "conditional jump last",
            unit(6, |b| {
                b.a_jump(Opcode::JTrue, 1, 0);
            }),
            E::FallsOffEnd,
        ),
        ("layout overflow", unit(4, ret), E::LayoutOverflow),
        (
            "register a",
            unit(6, |b| {
                b.a(Opcode::LoadNull, 6);
                ret(b);
            }),
            E::Register(6),
        ),
        (
            "register b",
            unit(6, |b| {
                b.ab(Opcode::Mov, 1, 200);
                ret(b);
            }),
            E::Register(200),
        ),
        (
            "register c",
            unit(6, |b| {
                b.abc(Opcode::Add, 1, 2, 9);
                ret(b);
            }),
            E::Register(9),
        ),
        (
            "compare-jump register",
            unit(6, |b| {
                b.ab_jump(Opcode::JLt, 1, 7, 0);
                ret(b);
            }),
            E::Register(7),
        ),
        (
            "jump target",
            unit(6, |b| {
                b.jump(Opcode::Jmp, 2);
                ret(b);
            }),
            E::Target(2),
        ),
        (
            "short compare-jump target",
            unit(6, |b| {
                b.a_short_jump(Opcode::JEqI, 1, 0, 99);
                ret(b);
            }),
            E::Target(99),
        ),
        (
            "constant",
            unit(6, |b| {
                b.a_const(Opcode::LoadConst, 1, 0);
                ret(b);
            }),
            E::Constant(0),
        ),
        (
            "child",
            unit(6, |b| {
                b.a_child(Opcode::MakeClosure, 1, 0);
                ret(b);
            }),
            E::Child(0),
        ),
        (
            "call window",
            unit(6, |b| {
                b.call(Opcode::Call, 5, 4, 2);
                ret(b);
            }),
            E::CallWindow,
        ),
        (
            "method call window",
            unit(6, |b| {
                b.call(Opcode::CallMethod, 5, 5, 0);
                ret(b);
            }),
            E::CallWindow,
        ),
        (
            "capture",
            unit(6, |b| {
                b.a_capture(Opcode::LoadCaptured, 5, 0);
                ret(b);
            }),
            E::Capture(0),
        ),
        (
            "array range",
            unit(6, |b| {
                b.ab_range(Opcode::NewArray, 5, 5, 2);
                ret(b);
            }),
            E::RegisterRange,
        ),
        (
            "wide register",
            unit(6, |b| {
                b.a_wide(Opcode::LoadWide, 5, 6);
                ret(b);
            }),
            E::WideRegister(6),
        ),
    ];
    for (name, unit, kind) in cases {
        assert_eq!(validate(&unit), Err(kind), "{name}");
    }

    let mut unpublished = unit_returning();
    unpublished.children = vec![CodeStore::default().publish(unit_returning()).unwrap()].into();
    assert_eq!(
        oxide_core::bytecode::validate::validate(&unpublished, &[]).map_err(|e| e.kind),
        Err(E::UnpublishedChild(0))
    );
    assert_eq!(
        oxide_core::bytecode::validate::validate(&unpublished, &[unit_returning()]),
        Ok(())
    );
    let mut capturing = unit_returning();
    capturing.captures = vec![Capture::Register(1)].into();
    assert_eq!(
        oxide_core::bytecode::validate::validate(&unpublished, &[capturing.clone()])
            .map_err(|e| e.kind),
        Err(E::ChildCapture(1)),
        "the parent has one register"
    );
    capturing.captures = vec![Capture::Outer(0)].into();
    assert_eq!(
        oxide_core::bytecode::validate::validate(&unpublished, &[capturing]).map_err(|e| e.kind),
        Err(E::ChildCapture(0)),
        "the parent has no captures"
    );

    let mut lines = unit_returning();
    lines.debug.lines = vec![1, 2].into();
    assert_eq!(validate(&lines), Err(E::LineTable));

    let mut arrow = unit_returning();
    arrow.flags = flags::ARROW | flags::CONSTRUCTOR;
    assert_eq!(validate(&arrow), Err(E::Flags));
}

#[test]
fn validator_checks_handler_ranges_and_nesting() {
    use ValidateErrorKind as E;
    let with = |handlers: &[Handler]| {
        let mut unit = unit(6, |b| {
            for _ in 0..5 {
                b.op(Opcode::Nop);
            }
            b.op(Opcode::ReturnUndefined);
        });
        unit.handlers = handlers.into();
        validate(&unit)
    };
    let h = |start, end, target, register| Handler {
        start,
        end,
        target,
        register,
    };
    assert_eq!(with(&[h(0, 4, 5, 5)]), Ok(()));
    assert_eq!(with(&[h(1, 2, 5, 5), h(0, 4, 5, 5)]), Ok(()), "inner first");
    assert_eq!(with(&[h(0, 2, 5, 5), h(2, 4, 5, 5)]), Ok(()), "disjoint");
    assert_eq!(with(&[h(2, 2, 5, 5)]), Err(E::HandlerRange));
    assert_eq!(with(&[h(0, 7, 5, 5)]), Err(E::HandlerRange));
    assert_eq!(with(&[h(0, 4, 6, 5)]), Err(E::Target(6)));
    assert_eq!(with(&[h(0, 4, 5, 6)]), Err(E::Register(6)));
    assert_eq!(
        with(&[h(0, 4, 5, 5), h(1, 2, 5, 5)]),
        Err(E::HandlerNesting),
        "outer first"
    );
    assert_eq!(
        with(&[h(0, 3, 5, 5), h(2, 4, 5, 5)]),
        Err(E::HandlerNesting),
        "overlap"
    );
}

#[test]
fn publication_validates_and_the_store_walks_trees_in_pre_order() {
    let mut store = CodeStore::default();
    let a = store.publish(unit_returning()).unwrap();
    let b = store.publish(unit_returning()).unwrap();
    let mut middle = unit_returning();
    middle.children = vec![b].into();
    let middle = store.publish(middle).unwrap();
    let mut root = unit_returning();
    root.children = vec![a, middle].into();
    let root = store.publish(root).unwrap();
    assert_eq!(store.tree(root), [root, a, middle, b]);
    let error = store.publish(unit(6, |_| {})).unwrap_err();
    assert_eq!(error.kind, ValidateErrorKind::EmptyCode);
    assert_eq!(store.len(), 4, "a rejected unit is not appended");
}

/// The attach path: a tree of code units instantiated as a function whose
/// bytecode records hold the constants and child links, traced through
/// collections (every allocation collects under `gc-stress`).
#[test]
fn instantiated_functions_trace_constants_and_children() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let mut leaf = unit_returning();
    leaf.constant_count = 1;
    let leaf = runtime.publish(leaf).unwrap();
    let mut root = unit(6, |b| {
        b.a_const(Opcode::LoadConst, 5, 1);
        b.a_child(Opcode::MakeClosure, 5, 0);
        b.op(Opcode::ReturnUndefined);
    });
    root.constant_count = 2;
    root.children = vec![leaf].into();
    let root = runtime.publish(root).unwrap();

    let heap = runtime.heap_mut();
    let mark = heap.roots().mark();
    let root_text = heap.alloc_string("root constant").unwrap();
    heap.roots().push(Value::string(root_text));
    let leaf_text = heap.alloc_string("leaf constant").unwrap();
    heap.roots().truncate(mark);
    // Both strings are unrooted now; `instantiate` roots them for its call.
    let root_constants = [Value::int32(1), Value::string(root_text)];
    let leaf_constants = [Value::string(leaf_text)];
    let function = runtime
        .instantiate(root, &[&root_constants, &leaf_constants])
        .unwrap();
    let handle = runtime.hold(Value::object(function));
    runtime.heap_mut().collect();

    let heap = runtime.heap();
    let bytecode = heap.function_bytecode(function).unwrap();
    assert_eq!(heap.bytecode_unit(bytecode), root);
    assert_eq!(heap.bytecode_constants(bytecode), &root_constants);
    assert_eq!(
        heap.bytecode_constant(bytecode, 1),
        Value::string(root_text)
    );
    let child = heap.bytecode_child(bytecode, 0);
    assert_eq!(heap.bytecode_unit(child), leaf);
    assert_eq!(heap.bytecode_constants(child), &leaf_constants);
    assert_eq!(heap.live_bytecodes(), 2);

    // MakeClosure: a closure over the child record, reachable through the
    // held parent. The closure itself is unrooted and dies.
    let closure = runtime.heap_mut().alloc_closure(None, None, child).unwrap();
    assert_eq!(runtime.heap().function_bytecode(closure), Some(child));
    let closure_handle = runtime.hold(Value::object(closure));
    runtime.release(handle);
    runtime.heap_mut().collect();
    let heap = runtime.heap();
    assert!(!heap.is_live(Value::object(function)));
    assert!(!heap.is_live(Value::string(root_text)));
    assert!(
        heap.is_live(Value::string(leaf_text)),
        "the closure keeps its unit"
    );
    assert_eq!(heap.live_bytecodes(), 1);

    runtime.release(closure_handle);
    runtime.heap_mut().collect();
    let heap = runtime.heap();
    assert!(!heap.is_live(Value::string(leaf_text)));
    assert_eq!(heap.live_bytecodes(), 0);
    assert_eq!(runtime.code().len(), 2, "code units are not collected");
}

#[test]
fn runtime_disassembly_spells_atom_keys() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let key = runtime.heap_mut().property_key("length").unwrap();
    let mut builder = CodeBuilder::new();
    builder.ab_key(Opcode::GetField, 5, 1, key);
    builder.a(Opcode::Return, 5);
    let id = runtime.publish(builder.finish(layout(6))).unwrap();
    let text = runtime.disassemble(id);
    assert!(
        text.contains("GetField            t0, p0, .length"),
        "{text}"
    );
}

/// Phase 3, section 2.1: field sites first in instruction order, then the
/// global forms, then `new`; one entry per site.
#[test]
fn cache_sites_are_numbered_per_unit() {
    let key = PropertyKey::index(7).unwrap();
    let unit = unit(8, |b| {
        b.call(Opcode::New, 5, 5, 0);
        b.a_key(Opcode::GetGlobal, 5, key);
        b.ab_key(Opcode::GetField, 5, 5, key);
        b.a_key(Opcode::SetGlobal, 5, key);
        b.ab_key(Opcode::SetField, 5, 5, key);
        b.op(Opcode::ReturnUndefined);
    });
    let sites: Vec<Option<u32>> = unit.code.iter().map(|insn| insn.site()).collect();
    assert_eq!(sites, [Some(4), Some(2), Some(0), Some(3), Some(1), None]);
    assert_eq!(unit.ic_count, 5);
    assert_eq!(validate(&unit), Ok(()));

    let mut bad = unit.clone();
    bad.code[2].c = 5;
    assert_eq!(validate(&bad), Err(ValidateErrorKind::Site(5)));
    let mut bad = unit;
    bad.code[0].imm = 9;
    assert_eq!(validate(&bad), Err(ValidateErrorKind::Site(9)));
}

/// Past 255 named-property sites the rest share the reserved site 255.
#[test]
fn field_sites_past_255_share_the_reserved_site() {
    let key = PropertyKey::index(7).unwrap();
    let unit = unit(8, |b| {
        for _ in 0..300 {
            b.ab_key(Opcode::GetField, 5, 5, key);
        }
        b.a_key(Opcode::GetGlobal, 5, key);
        b.op(Opcode::ReturnUndefined);
    });
    assert_eq!(unit.code[254].site(), Some(254));
    assert_eq!(unit.code[255].site(), Some(255));
    assert_eq!(unit.code[299].site(), Some(255));
    assert_eq!(unit.code[300].site(), Some(256));
    assert_eq!(unit.ic_count, 257);
    assert_eq!(validate(&unit), Ok(()));
}

/// Publication gives each unit its own range of the cache store.
#[test]
fn publication_reserves_cache_entries() {
    let key = PropertyKey::index(7).unwrap();
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let before = runtime.heap().ics().len();
    let two = |b: &mut CodeBuilder| {
        b.ab_key(Opcode::GetField, 5, 5, key);
        b.ab_key(Opcode::GetField, 5, 5, key);
        b.op(Opcode::ReturnUndefined);
    };
    let first = runtime.publish(unit(8, two)).unwrap();
    let second = runtime.publish(unit(8, two)).unwrap();
    assert_eq!(runtime.code().get(first).ic_base as usize, before);
    assert_eq!(runtime.code().get(second).ic_base as usize, before + 2);
    assert_eq!(runtime.heap().ics().len(), before + 4);
    assert_eq!(runtime.heap().ics().filled(), 0);
}

/// A function with more locals than the window keeps the rest in a spill
/// scope: its accesses must name the unit's spill register and a slot.
#[test]
fn spill_scope_accesses_validate() {
    let spilled = |build: &dyn Fn(&mut CodeBuilder)| {
        let mut builder = CodeBuilder::new();
        build(&mut builder);
        builder.op(Opcode::ReturnUndefined);
        builder.finish(UnitLayout {
            register_count: 8,
            parameter_count: 1,
            local_count: 4,
            spill_register: Some(4),
            spill_slots: 2,
            ..UnitLayout::default()
        })
    };
    let unit = spilled(&|b| {
        b.a_count(Opcode::NewSpill, 4, 2);
        b.ab_int(Opcode::StoreSlot, 5, 4, 1);
        b.ab_int(Opcode::LoadSlot, 6, 4, 0);
    });
    assert_eq!(validate(&unit), Ok(()));
    assert_eq!(unit.temp_base(), 5);
    let text = oxide_core::bytecode::disassemble(
        &unit,
        &mut oxide_core::bytecode::disasm::key_placeholder,
    );
    assert!(text.contains("StoreSlot           t0, spill, 1"), "{text}");
    for bad in [
        spilled(&|b| {
            b.ab_int(Opcode::LoadSlot, 6, 4, 2);
        }),
        spilled(&|b| {
            b.ab_int(Opcode::LoadSlot, 6, 5, 0);
        }),
        spilled(&|b| {
            b.a_count(Opcode::NewSpill, 4, 3);
        }),
    ] {
        assert_eq!(validate(&bad), Err(ValidateErrorKind::Spill));
    }
}
