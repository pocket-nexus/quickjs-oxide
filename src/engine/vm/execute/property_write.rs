//! Whole assignment uses State directly; only actual effects acquire a Query.
use super::{FallthroughPc, FrameCursor};
use crate::engine::{
    api::{Error, RuntimeError, runtime::Runtime},
    heap::runtime::{RuntimeState, owned_values::OwnedValueGuard},
    object::{SetAction, SetProgress, SetProgressGuard},
    value::{JsValue, conversion::property_key::PropertyKeyAtomStep},
    vm::{
        Completion,
        exception::runtime_error_to_vm_error,
        frame::ReturnValue,
        proxy_get_driver::{
            RawNativeQuery, StateNativeProgress, Step, ValueSetStart,
            advance_set_progress_in_state, finish_set_action_in_state, retire_value_set_inputs,
            setter_call_step, start_value_set_in_state,
        },
        stack::FrameExecution,
    },
};

#[cfg(test)]
mod local_guard_tests;

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
    run_local_assignment(
        runtime,
        state,
        segment,
        realm,
        atom,
        strict,
        fallthrough,
        depth,
        false,
        &mut None,
    )
}

/// The instruction prefix, not the receiver/value representation, determines
/// whether the original computed key must be discarded. Both enter one body.
#[allow(clippy::too_many_arguments)]
fn run_local_assignment(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    realm: crate::engine::heap::ContextId,
    atom: crate::engine::atom::Atom,
    strict: bool,
    fallthrough: FallthroughPc,
    depth: usize,
    discard_key: bool,
    key_owner: &mut Option<JsValue>,
) -> Result<Progress, Error> {
    // Nested existing guards preserve value-before-receiver retirement and
    // mark panic poison even before the complete Set producer can take inputs.
    let mut receiver_guard = OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Undefined);
    let (state, receiver) = receiver_guard.parts();
    receiver.take();
    let mut value_guard = OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Undefined);
    let (state, value) = value_guard.parts();
    value.take();
    let result = (|| {
        let discarded = {
            let mut turn = segment.frame();
            *value = Some(turn.transaction.slots().pop().expect("checked write value"));
            let discarded =
                discard_key.then(|| turn.transaction.slots().pop().expect("checked write key"));
            *receiver = Some(
                turn.transaction
                    .slots()
                    .pop()
                    .expect("checked write receiver"),
            );
            discarded
        };
        // Both assignment inputs and the new atom are armed before the original
        // key retires. A fatal release cannot traverse their remaining suffix.
        if let Some(discarded) = discarded {
            state
                .release_owned_jsvalue(&runtime.0.poisoned, discarded)
                .map_err(|_| runtime_error_to_vm_error(RuntimeError::Poisoned))?;
        }
        run_value_set(
            runtime,
            state,
            segment,
            realm,
            atom,
            strict,
            fallthrough,
            depth,
            value,
            receiver,
            key_owner,
        )
    })();
    if result.is_err() {
        retire_value_set_inputs(state, &runtime.0.poisoned, value, receiver)
            .map_err(runtime_error_to_vm_error)?;
        if let Some(key) = key_owner.take() {
            state
                .release_owned_jsvalue(&runtime.0.poisoned, key)
                .map_err(runtime_error_to_vm_error)?;
        }
    }
    result
}

// State and the independently owned key role stay explicit through publication.
#[allow(clippy::too_many_arguments)]
fn run_value_set(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    realm: crate::engine::heap::ContextId,
    atom: crate::engine::atom::Atom,
    strict: bool,
    fallthrough: FallthroughPc,
    depth: usize,
    value: &mut Option<JsValue>,
    receiver: &mut Option<JsValue>,
    key_owner: &mut Option<JsValue>,
) -> Result<Progress, Error> {
    let progress =
        match start_value_set_in_state(state, &runtime.0.poisoned, realm, atom, value, receiver)
            .map_err(runtime_error_to_vm_error)?
        {
            ValueSetStart::Progress(progress) => progress,
            ValueSetStart::Diagnostic(error) => {
                return run_boundary(
                    runtime,
                    state,
                    segment,
                    realm,
                    atom,
                    strict,
                    fallthrough,
                    depth,
                    key_owner,
                    Step::WriteError(Some(error)),
                    false,
                );
            }
        };
    // The actual raw result has no implicit Drop. Only its finite domain guard
    // is initialized on a synchronous operation, not a whole Step/Query owner.
    let mut progress_guard = SetProgressGuard::new(state, &runtime.0.poisoned, progress);
    let result = (|| {
        let (state, progress) = progress_guard.parts();
        let cycle_published = advance_set_progress_in_state(state, &runtime.0.poisoned, progress)
            .map_err(runtime_error_to_vm_error)?;
        if matches!(progress.as_ref(), Some(SetProgress::Waiting { .. })) {
            let step = Step::SetProgress(progress.take());
            return run_boundary(
                runtime,
                state,
                segment,
                realm,
                atom,
                strict,
                fallthrough,
                depth,
                key_owner,
                step,
                cycle_published,
            );
        }
        let action = match progress.take().expect("local Set result") {
            SetProgress::Complete(action) | SetProgress::CyclePublished(action) => action,
            SetProgress::Waiting { .. } => unreachable!(),
        };
        if let SetAction::Call {
            function,
            receiver,
            argument,
        } = action
        {
            // Publish the canonical callback ledger before any collector,
            // classification, frame budget or pending-storage operation.
            let step = setter_call_step(function, receiver, argument);
            return run_boundary(
                runtime,
                state,
                segment,
                realm,
                atom,
                strict,
                fallthrough,
                depth,
                key_owner,
                step,
                cycle_published,
            );
        }
        let completion =
            match finish_set_action_in_state(state, &runtime.0.poisoned, atom, strict, action) {
                Ok(completion) => completion,
                Err(RuntimeError::Engine(error)) => {
                    return run_boundary(
                        runtime,
                        state,
                        segment,
                        realm,
                        atom,
                        strict,
                        fallthrough,
                        depth,
                        key_owner,
                        Step::WriteError(Some(error)),
                        cycle_published,
                    );
                }
                Err(error) => return Err(runtime_error_to_vm_error(error)),
            };
        if cycle_published {
            return run_boundary(
                runtime,
                state,
                segment,
                realm,
                atom,
                strict,
                fallthrough,
                depth,
                key_owner,
                Step::Complete(Some(completion)),
                true,
            );
        }
        let (output, thrown) = match completion {
            Completion::Return(value) => (value, false),
            Completion::Throw(value) => (value, true),
        };
        let mut output_guard = OwnedValueGuard::new(state, &runtime.0.poisoned, output);
        let (state, output) = output_guard.parts();
        if let Some(key) = key_owner.take() {
            state
                .release_owned_jsvalue(&runtime.0.poisoned, key)
                .map_err(runtime_error_to_vm_error)?;
        }
        let value = output.take().expect("local assignment result");
        let completion = if thrown {
            Completion::Throw(value)
        } else {
            Completion::Return(value)
        };
        finish_progress(
            runtime,
            state,
            segment,
            fallthrough,
            depth,
            StateNativeProgress::Complete(completion),
        )
    })();
    if result.is_err() {
        progress_guard.retire().map_err(runtime_error_to_vm_error)?;
    }
    result
}

/// Durable native/Query machinery begins only at an actual selected effect.
#[allow(clippy::too_many_arguments)]
fn run_boundary(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    realm: crate::engine::heap::ContextId,
    atom: crate::engine::atom::Atom,
    strict: bool,
    fallthrough: FallthroughPc,
    depth: usize,
    key_owner: &mut Option<JsValue>,
    step: Step,
    cycle_published: bool,
) -> Result<Progress, Error> {
    let mut owner = RawNativeQuery::from_step(runtime, state, step);
    let key_owner = key_owner.take().map(|key| match key {
        JsValue::Symbol(index) => crate::engine::atom::Atom::from_raw(index.raw()),
        _ => unreachable!("computed write atom owner"),
    });
    owner.query = Some(segment.acquire_write_query(realm, atom, key_owner, strict, depth));
    let result = (|| {
        if cycle_published {
            owner
                .state
                .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                .map_err(runtime_error_to_vm_error)?;
        }
        consume(runtime, &mut owner, segment, fallthrough, depth)
    })();
    if result.is_err() {
        owner.retire().map_err(runtime_error_to_vm_error)?;
        runtime.check_poison().map_err(runtime_error_to_vm_error)?;
    }
    result
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
    finish_progress(runtime, owner.state, segment, fallthrough, _depth, progress)
}

fn finish_progress(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    fallthrough: FallthroughPc,
    _depth: usize,
    progress: StateNativeProgress,
) -> Result<Progress, Error> {
    Ok(match progress {
        StateNativeProgress::Complete(Completion::Return(value)) => {
            state
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
            cursor.commit_owned(state, value)?;
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
    // The converted atom outlives value/base cleanup. The canonical consumed
    // suffix owns the checked duplicate; original operands stay in their window
    // until that suffix succeeds, exactly as the Query primitive child did.
    let mut atom_owner = OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Undefined);
    let (state, key_owner) = atom_owner.parts();
    key_owner.take();
    let key = {
        let turn = segment.frame();
        state
            .dup_jsvalue(turn.transaction.peek(1)?)
            .map_err(runtime_error_to_vm_error)?
    };
    // ToPrimitive of this checked primitive is the canonical identity result;
    // its one consumed suffix is shared with WriteKeyInputs::reply_in_state.
    let atom = match state
        .property_key_from_primitive_jsvalue_with_publication(&runtime.0.poisoned, realm, key)
        .map_err(runtime_error_to_vm_error)?
    {
        PropertyKeyAtomStep::Value(atom) => atom,
        PropertyKeyAtomStep::CyclePublishedThrow(value) => {
            return run_boundary(
                runtime,
                state,
                segment,
                realm,
                crate::engine::atom::Atom::NULL,
                strict,
                fallthrough,
                depth,
                key_owner,
                Step::CyclePublishedComplete(Some(Completion::Throw(value))),
                false,
            );
        }
    };
    *key_owner = Some(JsValue::Symbol(crate::engine::atom::AtomIdx::from_raw(
        atom.raw(),
    )));
    run_local_assignment(
        runtime,
        state,
        segment,
        realm,
        atom,
        strict,
        fallthrough,
        depth,
        true,
        key_owner,
    )
}
