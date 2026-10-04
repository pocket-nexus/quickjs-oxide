//! Actual static-read consumers borrow the executable's key owner.
use super::{PropertyProgress, ReadKey, read_completion_tests::read_fixture, read_progress};
use crate::engine::{
    api::{Runtime, Value},
    atom::{Atom, AtomIdx},
    code::exec_opcode::Opcode,
    value::JsValue,
    vm::{
        execute::{VmAction, execute_frame},
        execution::RunningExecution,
        frame::FrameId,
    },
};

fn selected_static_read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
) -> (u32, bool, super::FallthroughPc, Atom) {
    let VmAction::GetField {
        index,
        keep_receiver,
        fallthrough,
    } = execute_frame(runtime, execution, id).unwrap()
    else {
        panic!("fixture must select a static property read");
    };
    let frame = execution.frames.current_mut(id).unwrap();
    let atom = frame.executable.property_key_atoms.as_ref().unwrap()[index as usize];
    (index, keep_receiver, fallthrough, atom)
}

#[test]
fn primitive_static_read_never_adds_a_runtime_or_linked_atom_owner() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    drop(context.eval("for(const p of [Number.prototype,Boolean.prototype,String.prototype,BigInt.prototype,Symbol.prototype])Object.defineProperty(p,'linkedRawKeyWitness',{value:7,configurable:true})").unwrap());
    for input in [
        "23",
        "23.5",
        "true",
        "'abcd'",
        "17n",
        "1000000000000000000000000000000n",
        "Symbol('probe')",
    ] {
        let value = context.eval(input).unwrap();
        let value = runtime.into_jsvalue(value).unwrap();
        // A named String.prototype data value already completes inside the
        // existing cache. A static destructuring index exercises the actual
        // String allocation fallback instead of pretending that cache hit is
        // a property-driver consumer.
        let string_index = input == "'abcd'";
        let source = if string_index {
            "(function(o){const {'0':c}=o;return c})"
        } else {
            "(function(o){return o.linkedRawKeyWitness})"
        };
        let opcode = if string_index {
            Opcode::GetField2Cached
        } else {
            Opcode::GetFieldCached
        };
        let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
        let frame = execution.frames.current_mut(id).unwrap();
        execution.slots.push(&mut frame.window, value).unwrap();
        let (index, keep_receiver, fallthrough, atom) =
            selected_static_read(&runtime, &mut execution, id);
        let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
        let atom_owners = runtime
            .0
            .state
            .borrow()
            .atoms
            .resolve(atom)
            .unwrap()
            .ref_count;
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert!(
            matches!(
                read_progress(
                    &runtime,
                    &mut execution,
                    id,
                    ReadKey::Static(index),
                    keep_receiver,
                    fallthrough
                )
                .unwrap(),
                PropertyProgress::Completed
            ),
            "{input}"
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(
            frame.cold.rare.get().is_none(),
            "{input} created a frame key cache"
        );
        let result = execution.slots.peek(&frame.window, 0).unwrap();
        if string_index {
            let JsValue::String(id) = result else {
                panic!("String index must produce a String");
            };
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .string(*id)
                    .unwrap()
                    .to_utf8_lossy(),
                "a"
            );
        } else {
            assert_eq!(result, &JsValue::Int(7));
        }
        assert_eq!(
            std::rc::Rc::strong_count(&runtime.0),
            runtime_owners,
            "{input}"
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .atoms
                .resolve(atom)
                .unwrap()
                .ref_count,
            atom_owners,
            "{input}"
        );
        #[cfg(feature = "profiling")]
        assert!(
            !profile
                .snapshot()
                .owned_execution_events
                .contains_key("core.runtime_clone"),
            "{input}"
        );
    }
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn borrowed_linked_key_at_max_succeeds_without_promotion_and_stays_live() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    drop(
        context
            .eval("Number.prototype.linkedMaxKeyWitness=19")
            .unwrap(),
    );
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.linkedMaxKeyWitness})",
        Opcode::GetFieldCached,
    );
    let frame = execution.frames.current_mut(id).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Int(3))
        .unwrap();
    let (index, keep_receiver, fallthrough, atom) =
        selected_static_read(&runtime, &mut execution, id);
    let count = runtime
        .0
        .state
        .borrow()
        .atoms
        .resolve(atom)
        .unwrap()
        .ref_count
        .unwrap();
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(AtomIdx::from_raw(atom.raw()), u32::MAX);
    let result = read_progress(
        &runtime,
        &mut execution,
        id,
        ReadKey::Static(index),
        keep_receiver,
        fallthrough,
    );
    let actual = runtime
        .0
        .state
        .borrow()
        .atoms
        .resolve(atom)
        .unwrap()
        .ref_count;
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(AtomIdx::from_raw(atom.raw()), count);
    assert!(matches!(result.unwrap(), PropertyProgress::Completed));
    assert_eq!(actual, Some(u32::MAX));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Int(19)
    );
    assert!(frame.cold.rare.get().is_none());
    assert!(!runtime.is_poisoned());
}

#[test]
fn proxy_key_promotion_failure_retires_selected_effect_and_preserves_input() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy({},{get(){throw 'must not run'}})")
        .unwrap()
    else {
        panic!("Proxy")
    };
    let object = proxy.object_id();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.linkedProxyKeyWitness})",
        Opcode::GetFieldCached,
    );
    let frame = execution.frames.current_mut(id).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Object(proxy.into_handle()))
        .unwrap();
    let (index, keep_receiver, fallthrough, atom) =
        selected_static_read(&runtime, &mut execution, id);
    let count = runtime
        .0
        .state
        .borrow()
        .atoms
        .resolve(atom)
        .unwrap()
        .ref_count
        .unwrap();
    let object_owners = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(object)
        .unwrap();
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(AtomIdx::from_raw(atom.raw()), u32::MAX);
    let result = read_progress(
        &runtime,
        &mut execution,
        id,
        ReadKey::Static(index),
        keep_receiver,
        fallthrough,
    );
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(AtomIdx::from_raw(atom.raw()), count);
    assert!(result.is_err());
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Object(object)
    );
    assert!(frame.cold.rare.get().is_none());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(object)
            .unwrap(),
        object_owners
    );
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn getter_handoff_rejection_retires_the_selected_raw_symbol_receiver() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(getter) = context.eval("var linkedSymbolGetter=function(){return 1};Object.defineProperty(Symbol.prototype,'linkedReceiverKeyWitness',{get:linkedSymbolGetter,configurable:true});linkedSymbolGetter").unwrap() else { panic!("getter") };
    let getter_id = getter.object_id();
    let value = context.eval("Symbol('raw receiver')").unwrap();
    let receiver = runtime.into_jsvalue(value).unwrap();
    let JsValue::Symbol(receiver_atom) = &receiver else {
        panic!("Symbol")
    };
    let receiver_atom = *receiver_atom;
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.linkedReceiverKeyWitness})",
        Opcode::GetFieldCached,
    );
    let frame = execution.frames.current_mut(id).unwrap();
    execution.slots.push(&mut frame.window, receiver).unwrap();
    let (index, keep_receiver, fallthrough, _) = selected_static_read(&runtime, &mut execution, id);
    let (count, getter_owners) = {
        let state = runtime.0.state.borrow();
        let atom = state.atoms.brand(receiver_atom).unwrap();
        (
            state.atoms.resolve(atom).unwrap().ref_count.unwrap(),
            state.heap.object_strong_count(getter_id).unwrap(),
        )
    };
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(receiver_atom, u32::MAX - 1);
    let result = read_progress(
        &runtime,
        &mut execution,
        id,
        ReadKey::Static(index),
        keep_receiver,
        fallthrough,
    );
    let remaining = {
        let state = runtime.0.state.borrow();
        state
            .atoms
            .resolve(state.atoms.brand(receiver_atom).unwrap())
            .unwrap()
            .ref_count
    };
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(receiver_atom, count);
    assert!(result.is_err());
    // Initial getter selection successfully checked-retained this Symbol up
    // to MAX; the second preservation retain rejects. Atom release has no
    // immortal sentinel, so this proves the raw receiver role was retired.
    assert_eq!(remaining, Some(u32::MAX - 1));
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(getter_id)
            .unwrap(),
        getter_owners
    );
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Symbol(receiver_atom)
    );
    assert!(frame.cold.rare.get().is_none());
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}
