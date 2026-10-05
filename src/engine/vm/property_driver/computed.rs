//! Computed Get borrows its primitive key and completes with one State access.
//! Only a real getter, Proxy or shared-word service transports selected edges.
use super::{
    Error, FallthroughPc, FrameId, JsValue, PropertyProgress, RunningExecution, Runtime,
    runtime_error_to_vm_error,
};
use crate::engine::{
    atom::Atom,
    heap::runtime::{RuntimeState, owned_values::OwnedValueGuard},
    object::{ProxyGetStep, ReadBoundary, StateReadEffect},
    vm::stack::{FrameExecution, FrameSlots, FrameTurn},
};
use std::cell::Cell;

struct KeyGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    atom: Option<Atom>,
    retained: Option<JsValue>,
}

impl Drop for KeyGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        if let Some(value) = self.retained.take()
            && self
                .state
                .release_owned_jsvalue(self.poisoned, value)
                .is_err()
        {
            return;
        }
        if let Some(atom) = self.atom.take()
            && self.state.atoms.release(atom).is_err()
        {
            self.poisoned.set(true);
        }
    }
}

pub(in crate::engine::vm) struct Effect {
    read: Option<StateReadEffect>,
    retained: Option<JsValue>,
}

enum Selection {
    Completed,
    Effect(Effect),
    Throw(JsValue),
}

impl Effect {
    pub(in crate::engine::vm) fn release_owned(self, runtime: &Runtime) {
        drop(EffectGuard {
            runtime,
            effect: Some(self),
        });
    }
}

pub(in crate::engine::vm) enum Progress {
    Completed,
    Boundary,
    Throw,
}

/// Dense misses reuse the current execution access. Only a selected external
/// service/callback goes into the frame's existing resident rare storage.
#[cold]
#[inline(never)]
pub(in crate::engine::vm) fn execute(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    keep_receiver: bool,
    keep_key: bool,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    {
        let FrameTurn {
            mut transaction, ..
        } = segment.frame();
        let slots = transaction.slots();
        if matches!(slots.peek(0)?, JsValue::Object(_))
            && !matches!(slots.peek(1)?, JsValue::Null | JsValue::Undefined)
        {
            return Ok(Progress::Boundary);
        }
    }
    segment.materialize_in_state(state)?;
    let FrameTurn {
        owners,
        executable,
        mut transaction,
        resume_pc,
        pending,
        ..
    } = segment.frame();
    #[cfg(feature = "profiling")]
    let depth = transaction.operand_depth();
    #[cfg(not(feature = "profiling"))]
    let depth = 0;
    let selected = select_in_slots(
        runtime,
        state,
        executable.realm,
        &mut transaction.slots(),
        resume_pc,
        keep_receiver,
        keep_key,
        fallthrough,
        depth,
    );
    if runtime.0.poisoned.get() {
        return Err(selected
            .err()
            .unwrap_or_else(|| Error::internal("runtime is poisoned")));
    }
    match selected {
        Ok(Selection::Completed) => {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "computed_read.completed_in_segment",
            );
            state
                .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                .map_err(runtime_error_to_vm_error)?;
            Ok(Progress::Completed)
        }
        Ok(Selection::Effect(effect)) => {
            if owners
                .rare
                .get()
                .is_some_and(|rare| rare.computed_read.is_some())
            {
                let mut guard = EffectGuardInState {
                    state,
                    runtime,
                    effect: Some(effect),
                };
                guard.release()?;
                return Err(Error::internal("computed read overwrote a selected effect"));
            }
            // Rare is allocated once for the frame and reused. No common data
            // read allocates an operand or continuation transport.
            let mut guard = EffectGuardInState {
                state,
                runtime,
                effect: Some(effect),
            };
            let resident = &mut owners.computed_read;
            *resident = guard.effect.take();
            Ok(Progress::Boundary)
        }
        Ok(Selection::Throw(value)) => {
            debug_assert!(pending.is_none());
            *pending = Some(value);
            Ok(Progress::Throw)
        }
        Err(error) => {
            if runtime.0.poisoned.get() {
                return Err(error);
            }
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            let message = error.native_message().cloned().unwrap_or_else(|| {
                crate::engine::api::error::NativeErrorMessage::from_utf8(error.message())
            });
            let error = state
                .new_native_error_from_message(&runtime.0.poisoned, executable.realm, kind, message)
                .map_err(runtime_error_to_vm_error)?;
            debug_assert!(pending.is_none());
            *pending = Some(JsValue::Object(error));
            Ok(Progress::Throw)
        }
    }
}

struct EffectGuardInState<'a> {
    state: &'a mut RuntimeState,
    runtime: &'a Runtime,
    effect: Option<Effect>,
}
impl EffectGuardInState<'_> {
    fn release(&mut self) -> Result<(), Error> {
        let Some(effect) = self.effect.take() else {
            return Ok(());
        };
        if let Some(read) = effect.read {
            read.release_in_state(self.state, self.runtime)
                .map_err(runtime_error_to_vm_error)?;
        }
        drop(KeyGuard {
            state: self.state,
            poisoned: &self.runtime.0.poisoned,
            atom: None,
            retained: effect.retained,
        });
        Ok(())
    }
}
impl Drop for EffectGuardInState<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.runtime.0.poisoned.set(true);
        }
        if self.runtime.0.poisoned.get() {
            return;
        }
        if self.release().is_err() {
            self.runtime.0.poisoned.set(true);
        }
    }
}

/// This guard exists only after exclusive State access ended. It owns a real
/// selected effect; it never clones Runtime and is absent from synchronous Get.
struct EffectGuard<'a> {
    runtime: &'a Runtime,
    effect: Option<Effect>,
}

impl Drop for EffectGuard<'_> {
    fn drop(&mut self) {
        let Some(effect) = self.effect.take() else {
            return;
        };
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        let mut state = self.runtime.0.state.borrow_mut();
        if let Some(read) = effect.read
            && read.release_in_state(&mut state, self.runtime).is_err()
        {
            self.runtime.0.poisoned.set(true);
            return;
        }
        drop(KeyGuard {
            state: &mut state,
            poisoned: &self.runtime.0.poisoned,
            atom: None,
            retained: effect.retained,
        });
    }
}

#[cold]
#[inline(never)]
pub(super) fn read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    keep_receiver: bool,
    keep_key: bool,
    fallthrough: FallthroughPc,
) -> Result<PropertyProgress, Error> {
    let resident = execution
        .frames
        .current_mut(id)?
        .cold
        .rare
        .get_mut()
        .and_then(|rare| rare.computed_read.take());
    let mut resident = EffectGuard {
        runtime,
        effect: resident,
    };
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let selected = if let Some(effect) = resident.effect.take() {
        Ok(Selection::Effect(effect))
    } else {
        let mut state = runtime.0.state.borrow_mut();
        read_in_state(
            runtime,
            &mut state,
            execution,
            id,
            keep_receiver,
            keep_key,
            fallthrough,
        )
    };
    if runtime.0.poisoned.get() {
        return Err(selected
            .err()
            .unwrap_or_else(|| Error::internal("runtime is poisoned")));
    }
    let effect = match selected {
        Ok(Selection::Completed) => return Ok(PropertyProgress::Completed),
        Ok(Selection::Throw(value)) => {
            return Ok(PropertyProgress::Deferred(super::CallStep::Complete(
                super::Completion::Throw(value),
            )));
        }
        Ok(Selection::Effect(effect)) => effect,
        Err(error) => {
            return super::throw_error(runtime, realm, error).map(PropertyProgress::Deferred);
        }
    };
    let mut effect = EffectGuard {
        runtime,
        effect: Some(effect),
    };
    if matches!(
        effect
            .effect
            .as_ref()
            .and_then(|effect| effect.read.as_ref()),
        Some(StateReadEffect::Shared(_))
    ) {
        let Some(StateReadEffect::Shared(read)) =
            effect.effect.as_mut().and_then(|effect| effect.read.take())
        else {
            unreachable!("selected shared service")
        };
        // Do not acquire the shared backing lock while State is borrowed. The
        // selected effect guard owns its retained key until the lock ends.
        let (element, bytes) = read.read().map_err(runtime_error_to_vm_error)?;
        let mut state = runtime.0.state.borrow_mut();
        let mut key = KeyGuard {
            state: &mut state,
            poisoned: &runtime.0.poisoned,
            atom: None,
            retained: effect
                .effect
                .as_mut()
                .and_then(|effect| effect.retained.take()),
        };
        let value = key
            .state
            .decode_typed_index(element, bytes)
            .map_err(runtime_error_to_vm_error)?;
        let frame = execution.frames.current_mut(id)?;
        let mut transaction = execution.slots.frame_transaction(&mut frame.cold.window)?;
        complete(
            key.state,
            &runtime.0.poisoned,
            &mut transaction.slots(),
            &mut frame.resume_pc,
            keep_receiver,
            &mut key.retained,
            value,
            fallthrough,
        )?;
        return Ok(PropertyProgress::Completed);
    }
    // The source window pins every input until exact effect selection finishes.
    // Consume that window through its current State before publishing the actual
    // callback; no public key/root or replayable read request is constructed.
    let depth = {
        let mut state = runtime.0.state.borrow_mut();
        let frame = execution.frames.current_mut(id)?;
        let depth = execution.slots.depth(&frame.window);
        let mut transaction = execution.slots.frame_transaction(&mut frame.cold.window)?;
        let mut slots = transaction.slots();
        let key = slots.pop()?;
        let mut key = OwnedValueGuard::new(&mut state, &runtime.0.poisoned, key);
        let (state, key) = key.parts();
        let receiver = slots.pop()?;
        let mut receiver = OwnedValueGuard::new(state, &runtime.0.poisoned, receiver);
        let (state, receiver) = receiver.parts();
        if keep_receiver {
            slots.push_pending(receiver)?;
        }
        let retained = &mut effect.effect.as_mut().unwrap().retained;
        if retained.is_some() {
            slots.push_pending(retained)?;
        }
        state
            .release_owned_jsvalue(&runtime.0.poisoned, key.take().unwrap())
            .map_err(runtime_error_to_vm_error)?;
        frame.resume_pc = fallthrough.index();
        depth
    };
    if runtime.0.poisoned.get() {
        return Err(Error::internal("runtime is poisoned"));
    }
    let Effect { read, retained } = effect.effect.take().unwrap();
    debug_assert!(retained.is_none());
    super::super::proxy_get_driver::start_property_state_read(
        runtime,
        execution,
        id,
        read.expect("selected read effect"),
        None,
        depth,
    )
    .map(PropertyProgress::Deferred)
}

#[allow(clippy::too_many_arguments)]
fn read_in_state(
    runtime: &Runtime,
    state: &mut RuntimeState,
    execution: &mut RunningExecution,
    id: FrameId,
    keep_receiver: bool,
    keep_key: bool,
    fallthrough: FallthroughPc,
) -> Result<Selection, Error> {
    execution.frames.materialize_in_state(state)?;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let depth = execution.slots.depth(&frame.window);
    let mut transaction = execution.slots.frame_transaction(&mut frame.cold.window)?;
    let mut slots = transaction.slots();
    select_in_slots(
        runtime,
        state,
        realm,
        &mut slots,
        &mut frame.resume_pc,
        keep_receiver,
        keep_key,
        fallthrough,
        depth,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_in_slots(
    runtime: &Runtime,
    state: &mut RuntimeState,
    realm: crate::engine::heap::ContextId,
    slots: &mut FrameSlots<'_>,
    resume_pc: &mut usize,
    keep_receiver: bool,
    keep_key: bool,
    fallthrough: FallthroughPc,
    depth: usize,
) -> Result<Selection, Error> {
    let receiver = slots.peek(1)?;
    let key = slots.peek(0)?;
    if matches!(receiver, JsValue::Null | JsValue::Undefined) {
        let message = if keep_key
            && !matches!(
                key,
                JsValue::Int(_) | JsValue::String(_) | JsValue::Symbol(_)
            ) {
            "value has no property"
        } else if matches!(receiver, JsValue::Null) {
            "cannot read property of null"
        } else {
            "cannot read property of undefined"
        };
        return Err(Error::new(
            crate::engine::api::error::ErrorKind::Type,
            message,
        ));
    }
    let atom = state
        .property_key_atom_from_primitive(key)
        .map_err(runtime_error_to_vm_error)?;
    let mut key_owner = KeyGuard {
        state,
        poisoned: &runtime.0.poisoned,
        atom: Some(atom),
        retained: None,
    };
    if keep_key {
        key_owner.retained = Some(match key {
            JsValue::Int(_) | JsValue::String(_) | JsValue::Symbol(_) => key_owner
                .state
                .dup_jsvalue(key)
                .map_err(runtime_error_to_vm_error)?,
            value => {
                let text = key_owner.state.primitive_to_js_string(value)?;
                JsValue::String(
                    key_owner
                        .state
                        .heap
                        .allocate_string(text)
                        .map_err(|error| Error::internal(error.to_string()))?,
                )
            }
        });
    }
    let mut boundary = None;
    let value = key_owner
        .state
        .select_value_read_in_state(
            &runtime.0.poisoned,
            realm,
            receiver,
            atom,
            runtime.domain_id(),
            &mut boundary,
            None,
        )
        .map_err(runtime_error_to_vm_error)?;
    let value = match (value, boundary) {
        (Some(value), None) => value,
        (None, Some(ReadBoundary::Absent)) => JsValue::Undefined,
        (None, Some(boundary)) => {
            match crate::engine::object::internal_methods::resolve_read_boundary_in_state(
                runtime,
                key_owner.state,
                realm,
                atom,
                receiver,
                boundary,
            )
            .map_err(runtime_error_to_vm_error)?
            {
                ProxyGetStep::Complete(super::Completion::Return(value)) => value,
                ProxyGetStep::Complete(super::Completion::Throw(value)) => {
                    return Ok(Selection::Throw(value));
                }
                ProxyGetStep::Effect(read) => {
                    return Ok(Selection::Effect(Effect {
                        read: Some(read),
                        retained: key_owner.retained.take(),
                    }));
                }
            }
        }
        _ => {
            return Err(Error::internal(
                "computed read returned an inconsistent selection",
            ));
        }
    };
    complete(
        key_owner.state,
        &runtime.0.poisoned,
        slots,
        resume_pc,
        keep_receiver,
        &mut key_owner.retained,
        value,
        fallthrough,
    )?;
    super::record_read_completion(depth);
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("computed_read.state_completed");
    Ok(Selection::Completed)
}

#[allow(clippy::too_many_arguments)]
fn complete(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    slots: &mut FrameSlots<'_>,
    resume_pc: &mut usize,
    keep_receiver: bool,
    retained: &mut Option<JsValue>,
    value: JsValue,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    let mut value = OwnedValueGuard::new(state, poisoned, value);
    let (state, value) = value.parts();
    let key = slots.pop()?;
    let mut key = OwnedValueGuard::new(state, poisoned, key);
    let (state, key) = key.parts();
    let receiver = slots.pop()?;
    let mut receiver = OwnedValueGuard::new(state, poisoned, receiver);
    let (state, receiver) = receiver.parts();
    state
        .release_owned_jsvalue(poisoned, key.take().expect("input key owner"))
        .map_err(runtime_error_to_vm_error)?;
    if keep_receiver {
        slots.push_pending(receiver)?;
    }
    if retained.is_some() {
        slots.push_pending(retained)?;
    }
    *resume_pc = fallthrough.index();
    slots.push_pending(value)?;
    if let Some(receiver) = receiver.take() {
        state
            .release_owned_jsvalue(poisoned, receiver)
            .map_err(runtime_error_to_vm_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        code::exec_opcode::Opcode, value::Value,
        vm::property_driver::read_completion_tests::read_fixture,
    };

    #[cfg(feature = "profiling")]
    #[test]
    fn synchronous_computed_read_retires_final_receiver_with_current_state() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context.eval("({x:{marker:7}})").unwrap() else {
            panic!("object")
        };
        let receiver = object.object_id();
        let key = runtime
            .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
                "x",
            )))
            .unwrap();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o,k){return o[k]})",
            Opcode::GetArrayElDense,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        execution.slots.push(&mut frame.window, key).unwrap();
        let fallthrough = FallthroughPc::from_decoded(
            frame
                .executable
                .exec
                .decode_published(frame.resume_pc as u32)
                .unwrap(),
        );
        let owners = std::rc::Rc::strong_count(&runtime.0);
        let profile = crate::engine::api::profiling::CostProfile::start();
        let _scope = crate::engine::api::profiling::CoreExecutionScope::enter();
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(
                read_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    id,
                    false,
                    false,
                    fallthrough
                )
                .is_ok_and(|result| matches!(result, Selection::Completed))
            );
            assert!(state.heap.object(receiver).is_err());
            let frame = execution.frames.current_mut(id).unwrap();
            let JsValue::Object(child) = execution.slots.peek(&frame.window, 0).unwrap() else {
                panic!("result owner")
            };
            assert_eq!(state.heap.object_strong_count(*child), Ok(1));
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
            assert!(!runtime.0.deferred_references.has_pending());
        }
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(
            events.get("computed_read.state_completed").copied(),
            Some(1)
        );
        assert_eq!(events.get("query.read.acquired").copied().unwrap_or(0), 0);
        assert_eq!(events.get("core.runtime_clone").copied().unwrap_or(0), 0);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn non_dense_data_read_uses_the_held_state_without_a_rare_record() {
        for source in [
            "({x:7})",
            "new Proxy({x:7},{})",
            "new Proxy(new Proxy({x:7},{}),{})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let Value::Object(object) = context.eval(source).unwrap() else {
                panic!("object")
            };
            let key = runtime
                .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
                    "x",
                )))
                .unwrap();
            let (mut execution, id) = read_fixture(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                Opcode::GetArrayElDense,
            );
            let frame = execution.frames.current_mut(id).unwrap();
            execution
                .slots
                .push(&mut frame.window, JsValue::Object(object.into_handle()))
                .unwrap();
            execution.slots.push(&mut frame.window, key).unwrap();
            assert!(frame.cold.rare.get().is_none());
            let profile = crate::engine::api::profiling::CostProfile::start();
            let _core = crate::engine::api::profiling::CoreExecutionScope::enter();
            let owners = std::rc::Rc::strong_count(&runtime.0);
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                crate::engine::vm::execute::execute_frame_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    id
                )
                .unwrap(),
                crate::engine::vm::execute::VmAction::Complete
            ));
            assert!(matches!(execution.pending, Some(JsValue::Int(7))));
            assert!(
                execution
                    .frames
                    .current_mut(id)
                    .unwrap()
                    .cold
                    .rare
                    .get()
                    .is_none()
            );
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
            assert!(!runtime.0.deferred_references.has_pending());
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("computed_read.completed_in_segment"), Some(&1));
            assert_eq!(events.get("query.read.acquired").copied().unwrap_or(0), 0);
            assert_eq!(events.get("core.runtime_clone").copied().unwrap_or(0), 0);
        }
    }

    #[test]
    fn kept_computed_key_preserves_direct_int_and_normalizes_float_zero() {
        for (key, normalized) in [
            (JsValue::Int(0), Value::Int(0)),
            (
                JsValue::Float(-0.0),
                Value::String(crate::engine::value::JsString::from_static("0")),
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let Value::Object(object) = context.eval("({'0':7})").unwrap() else {
                panic!("object")
            };
            let (mut execution, id) = read_fixture(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]++})",
                Opcode::GetArrayEl3Dense,
            );
            let frame = execution.frames.current_mut(id).unwrap();
            execution
                .slots
                .push(&mut frame.window, JsValue::Object(object.into_handle()))
                .unwrap();
            execution.slots.push(&mut frame.window, key).unwrap();
            let fallthrough = FallthroughPc::from_decoded(
                frame
                    .executable
                    .exec
                    .decode_published(frame.resume_pc as u32)
                    .unwrap(),
            );
            assert!(matches!(
                read(&runtime, &mut execution, id, true, true, fallthrough).unwrap(),
                PropertyProgress::Completed
            ));
            let frame = execution.frames.current_mut(id).unwrap();
            assert_eq!(execution.slots.depth(&frame.window), 3);
            assert_eq!(
                execution.slots.peek(&frame.window, 0).unwrap(),
                &JsValue::Int(7)
            );
            let key = runtime
                .dup_jsvalue(execution.slots.peek(&frame.window, 1).unwrap())
                .unwrap();
            assert_eq!(runtime.root_and_release_jsvalue(key).unwrap(), normalized);
        }
    }
}
