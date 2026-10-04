//! Whole assignment owns its operands in the canonical raw Query.
use super::{FallthroughPc, FrameCursor};
use crate::engine::{
    api::{Error, runtime::Runtime},
    heap::runtime::RuntimeState,
    vm::{
        Completion,
        exception::runtime_error_to_vm_error,
        frame::ReturnValue,
        proxy_get_driver::{RawNativeQuery, StateNativeProgress, Step},
        stack::FrameExecution,
    },
};

pub(super) enum Progress {
    Completed,
    Entered,
    Boundary,
    Throw,
}

pub(super) fn complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    next_operation: &mut u64,
    index: Option<u32>,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    let Some(index) = index else {
        return computed(runtime, state, segment, next_operation, fallthrough);
    };
    let (realm, atom, strict, depth) = {
        let turn = segment.frame();
        turn.transaction.peek(0)?;
        turn.transaction.peek(1)?;
        let atom = turn
            .executable
            .property_key_atoms
            .as_ref()
            .and_then(|atoms| atoms.get(index as usize))
            .copied()
            .filter(|atom| !atom.is_null())
            .ok_or_else(|| Error::internal("property write has no linked key"))?;
        (
            turn.executable.realm,
            atom,
            turn.executable.metadata.strict,
            turn.transaction.depth(),
        )
    };
    let query = segment.acquire_write_query(realm, atom, None, strict, depth);
    let mut owner = RawNativeQuery::from_query(runtime, state, query, Step::Complete(None));
    {
        let mut turn = segment.frame();
        // Both offsets were checked before any no-Drop input moves.
        let value = turn.transaction.slots().pop().expect("checked write value");
        let receiver = turn
            .transaction
            .slots()
            .pop()
            .expect("checked write receiver");
        owner.step = Step::ValueSet {
            atom,
            value: Some(value),
            receiver: Some(receiver),
        };
    }
    consume(runtime, &mut owner, segment, fallthrough, depth)
}

fn consume(
    runtime: &Runtime,
    owner: &mut RawNativeQuery<'_>,
    segment: &mut FrameExecution<'_>,
    fallthrough: FallthroughPc,
    _depth: usize,
) -> Result<Progress, Error> {
    let mut return_to = segment.named_getter_return_target();
    return_to.value_use = ReturnValue::Discard;
    let progress = segment.consume_native_query(owner, return_to, fallthrough)?;
    Ok(match progress {
        StateNativeProgress::Complete(Completion::Return(value)) => {
            owner
                .state
                .release_owned_jsvalue(&runtime.0.poisoned, value)
                .map_err(runtime_error_to_vm_error)?;
            *segment.frame().resume_pc = fallthrough.index();
            #[cfg(feature = "profiling")]
            {
                crate::engine::api::profiling::record_owned_instruction(_depth);
                crate::engine::api::profiling::record_owned_execution_event(
                    "core.internal_property_write",
                );
            }
            Progress::Completed
        }
        StateNativeProgress::Complete(Completion::Throw(value)) => {
            let turn = segment.frame();
            let mut cursor = FrameCursor::new(
                turn.transaction,
                turn.fault_pc,
                turn.resume_pc,
                &runtime.0.poisoned,
            );
            cursor.commit_owned(owner.state, value)?;
            Progress::Throw
        }
        StateNativeProgress::Published | StateNativeProgress::PublishedThrow => {
            return Err(Error::internal("write used read operand publication"));
        }
        StateNativeProgress::Entered => Progress::Entered,
        StateNativeProgress::Boundary => Progress::Boundary,
    })
}

fn computed(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    next_operation: &mut u64,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    let (realm, strict, depth, object_key) = {
        let turn = segment.frame();
        turn.transaction.peek(0)?;
        let object = matches!(
            turn.transaction.peek(1)?,
            crate::engine::value::JsValue::Object(_)
        );
        turn.transaction.peek(2)?;
        (
            turn.executable.realm,
            turn.executable.metadata.strict,
            turn.transaction.depth(),
            object,
        )
    };
    // Only the original object-key conversion issues this actual operation
    // identity, before surrendering any operand. Primitive keys keep their
    // original window owners until the common suffix has succeeded.
    if object_key {
        *next_operation = next_operation
            .checked_add(1)
            .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
    }
    let query =
        segment.acquire_write_query(realm, crate::engine::atom::Atom::NULL, None, strict, depth);
    let mut owner = RawNativeQuery::from_query(runtime, state, query, Step::Complete(None));
    let (key, input) = if object_key {
        // Allocate the actual displaced-operand record before moving any raw
        // caller input. Primitive keys keep a thin continuation and no box.
        let mut input = crate::engine::vm::proxy_get_driver::WriteKeyInputs::object(realm);
        let mut turn = segment.frame();
        let value = turn
            .transaction
            .slots()
            .pop()
            .expect("checked object-key write value");
        let key = turn
            .transaction
            .slots()
            .pop()
            .expect("checked object write key");
        let base = turn
            .transaction
            .slots()
            .pop()
            .expect("checked object-key write base");
        let operands = input.operands.as_mut().expect("object-key operand record");
        operands.base = Some(base);
        operands.value = Some(value);
        (key, input)
    } else {
        let turn = segment.frame();
        let key = owner
            .state
            .dup_jsvalue(turn.transaction.peek(1)?)
            .map_err(runtime_error_to_vm_error)?;
        (
            key,
            crate::engine::vm::proxy_get_driver::WriteKeyInputs::primitive(realm),
        )
    };
    owner.step = Step::Primitive {
        value: Some(key),
        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
        resume: Some(crate::engine::vm::proxy_get_driver::Resume::WriteKey(input)),
    };
    consume(runtime, &mut owner, segment, fallthrough, depth)
}
