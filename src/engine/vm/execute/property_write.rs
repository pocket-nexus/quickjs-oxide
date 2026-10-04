//! Whole assignment uses State directly; only actual effects acquire a Query.
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
    let mut owner = RawNativeQuery::from_step(runtime, state, Step::Complete(None));
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
    run_value_set(
        &mut owner,
        segment,
        realm,
        atom,
        strict,
        fallthrough,
        depth,
        &mut None,
    )
}

// State and the independently owned key role stay explicit through publication.
#[allow(clippy::too_many_arguments)]
fn run_value_set(
    owner: &mut RawNativeQuery<'_>,
    segment: &mut FrameExecution<'_>,
    realm: crate::engine::heap::ContextId,
    atom: crate::engine::atom::Atom,
    strict: bool,
    fallthrough: FallthroughPc,
    depth: usize,
    key_owner: &mut Option<crate::engine::value::JsValue>,
) -> Result<Progress, Error> {
    let runtime = owner.runtime;
    owner
        .start_write_in_state(realm)
        .map_err(runtime_error_to_vm_error)?;
    let cycle_published = if matches!(owner.step, Step::SetProgress(_)) {
        owner
            .advance_set_in_state()
            .map_err(runtime_error_to_vm_error)?
    } else {
        false
    };
    if matches!(owner.step, Step::SetReply { .. }) {
        owner
            .finish_write_in_state(atom, strict)
            .map_err(runtime_error_to_vm_error)?;
    }
    if !cycle_published && let Step::Complete(value) = &mut owner.step {
        // The real result remains armed while atom retirement can poison.
        if let Some(key) = key_owner.take() {
            owner
                .state
                .release_owned_jsvalue(&runtime.0.poisoned, key)
                .map_err(runtime_error_to_vm_error)?;
        }
        let completion = value.take().expect("local write completion");
        return finish_progress(
            runtime,
            owner,
            segment,
            fallthrough,
            depth,
            StateNativeProgress::Complete(completion),
        );
    }
    let key_owner = key_owner.take().map(|key| match key {
        crate::engine::value::JsValue::Symbol(index) => {
            crate::engine::atom::Atom::from_raw(index.raw())
        }
        _ => unreachable!("computed write atom owner"),
    });
    // Every nonlocal case carries its already-selected Step. Query is acquired
    // before callbacks/diagnostics/service, without repeating the Set producer.
    owner.query = Some(segment.acquire_write_query(realm, atom, key_owner, strict, depth));
    if cycle_published {
        owner
            .state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .map_err(runtime_error_to_vm_error)?;
    }
    consume(runtime, owner, segment, fallthrough, depth)
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
    finish_progress(runtime, owner, segment, fallthrough, _depth, progress)
}

fn finish_progress(
    runtime: &Runtime,
    owner: &mut RawNativeQuery<'_>,
    segment: &mut FrameExecution<'_>,
    fallthrough: FallthroughPc,
    _depth: usize,
    progress: StateNativeProgress,
) -> Result<Progress, Error> {
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
    if !object_key {
        return primitive_key(runtime, state, segment, realm, strict, depth, fallthrough);
    }
    let query =
        segment.acquire_write_query(realm, crate::engine::atom::Atom::NULL, None, strict, depth);
    let mut owner = RawNativeQuery::from_query(runtime, state, query, Step::Complete(None));
    // Only object-key conversion displaces these actual operands before JS.
    let mut input = crate::engine::vm::proxy_get_driver::WriteKeyInputs::object(realm);
    let key = {
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
        key
    };
    owner.step = Step::Primitive {
        value: Some(key),
        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
        resume: Some(crate::engine::vm::proxy_get_driver::Resume::WriteKey(input)),
    };
    consume(runtime, &mut owner, segment, fallthrough, depth)
}

// The instruction facts and atom owner are distinct from the State lease.
#[allow(clippy::too_many_arguments)]
fn primitive_key(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    realm: crate::engine::heap::ContextId,
    strict: bool,
    depth: usize,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    use crate::engine::{heap::runtime::owned_values::OwnedValueGuard, value::JsValue};
    // Reborrow through this existing outer guard. RawNativeQuery retires its
    // value/base suffix first; the atom follows if cleanup has not poisoned.
    let mut atom_owner = OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Undefined);
    let (state, key_owner) = atom_owner.parts();
    let _ = key_owner.take();
    let mut owner = RawNativeQuery::from_step(runtime, state, Step::Complete(None));
    let key = {
        let turn = segment.frame();
        owner
            .state
            .dup_jsvalue(turn.transaction.peek(1)?)
            .map_err(runtime_error_to_vm_error)?
    };
    owner.step = Step::Primitive {
        value: Some(key),
        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
        resume: Some(crate::engine::vm::proxy_get_driver::Resume::WriteKey(
            crate::engine::vm::proxy_get_driver::WriteKeyInputs::primitive(realm),
        )),
    };
    owner
        .complete_primitive_write_key_in_state(realm)
        .map_err(runtime_error_to_vm_error)?;
    if !matches!(owner.step, Step::WriteOperands { .. }) {
        // A real conversion diagnostic/publication keeps the original window
        // suffix and enters the common protected consumer unchanged.
        owner.query = Some(segment.acquire_write_query(
            realm,
            crate::engine::atom::Atom::NULL,
            None,
            strict,
            depth,
        ));
        return consume(runtime, &mut owner, segment, fallthrough, depth);
    }
    let atom = segment.publish_local_write_operands_in_state(&mut owner, key_owner)?;
    run_value_set(
        &mut owner,
        segment,
        realm,
        atom,
        strict,
        fallthrough,
        depth,
        key_owner,
    )
}
