//! Native invocation ownership shared by synchronous consumers and owned continuations.
//! Preparing an activation executes no builtin body; arguments and the diagnostic
//! frame survive until completion, rejection or abandonment.
use super::{NativeArguments, NativeInvocation, NativeInvokeMode, NativeInvokeOutcome};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::NativeFunctionId,
    heap::ContextId,
    object::CallableRef,
    value::{JsValue, Value},
    vm::{Completion, frames::ActiveFrameGuard},
};

pub(in crate::engine::vm) struct PreparedNativeCall {
    pub activation: NativeActivation,
    pub invocation: NativeInvocation,
}

pub(in crate::engine::vm) struct NativeActivation {
    runtime: Option<Runtime>,
    // Retire the non-owning diagnostic descriptor before callable roots on unwind.
    active_frame: Option<ActiveFrameGuard>,
    callable: Option<CallableRef>,
    pub realm: ContextId,
    pub target: NativeFunctionId,
    pub mode: NativeInvokeMode,
    pub arguments: NativeArguments,
}

enum NativeCallableInput<'a> {
    Borrowed(&'a CallableRef),

    Owned(CallableRef),
}

impl NativeCallableInput<'_> {
    fn as_ref(&self) -> &CallableRef {
        match self {
            Self::Borrowed(callable) => callable,

            Self::Owned(callable) => callable,
        }
    }

    fn into_owned(self) -> CallableRef {
        match self {
            Self::Borrowed(callable) => callable.clone(),

            Self::Owned(callable) => callable,
        }
    }
}

enum NativeArgumentInput<'a> {
    Borrowed(&'a [Value]),

    #[cfg(test)]
    Owned(Vec<Value>),
    Internal(Vec<JsValue>),
}

/// Owns the internal edges preparation took until the activation exists.
/// `NativeActivation`'s Drop covers readable arguments after publication;
/// before that, this guard releases both halves on every early return.
struct PendingNativeOwners<'a> {
    runtime: &'a Runtime,
    invocation: Option<NativeInvocation>,
    readable: Vec<JsValue>,
}

impl Drop for PendingNativeOwners<'_> {
    fn drop(&mut self) {
        if let Some(invocation) = self.invocation.take() {
            let _ = invocation.release(self.runtime);
        }
        for value in self.readable.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}

impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn prepare_native_invocation(
        &self,
        callable: &CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: &[Value],
        mode: NativeInvokeMode,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        self.prepare_native_arguments(
            NativeCallableInput::Borrowed(callable),
            realm,
            target,
            min_readable_args,
            invocation,
            NativeArgumentInput::Borrowed(arguments),
            mode,
            false,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(in crate::engine::vm) fn prepare_native_invocation_owned(
        &self,
        callable: CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: Vec<Value>,
        mode: NativeInvokeMode,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        self.prepare_native_arguments(
            NativeCallableInput::Owned(callable),
            realm,
            target,
            min_readable_args,
            invocation,
            NativeArgumentInput::Owned(arguments),
            mode,
            false,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn prepare_native_invocation_jsvalue(
        &self,
        callable: &CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
        mode: NativeInvokeMode,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        self.prepare_native_arguments(
            NativeCallableInput::Borrowed(callable),
            realm,
            target,
            min_readable_args,
            invocation,
            NativeArgumentInput::Internal(arguments),
            mode,
            false,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(in crate::engine::vm) fn prepare_native_continuation_owned(
        &self,
        callable: CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: Vec<Value>,
        mode: NativeInvokeMode,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        self.prepare_native_arguments(
            NativeCallableInput::Owned(callable),
            realm,
            target,
            min_readable_args,
            invocation,
            NativeArgumentInput::Owned(arguments),
            mode,
            true,
            None,
        )
    }

    /// Array iterator-next has no JavaScript arguments. Keep the same owning
    /// activation/publication ABI without entering general argv construction.
    pub(in crate::engine::vm) fn prepare_array_next_owned(
        &self,
        callable: CallableRef,
        realm: ContextId,
        min_readable_args: u8,
        receiver: JsValue,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        let target = NativeFunctionId::ArrayIteratorNext;
        let mode = NativeInvokeMode::IteratorNextRaw;
        let invocation = NativeInvocation::Call {
            this_value: receiver,
        };
        let mut owners = PendingNativeOwners {
            runtime: self,
            invocation: Some(invocation),
            readable: Vec::new(),
        };
        if min_readable_args != 0 {
            return self.prepare_native_continuation_selected(
                callable,
                realm,
                target,
                min_readable_args,
                owners.invocation.take().expect("array next invocation"),
                Vec::new(),
                mode,
                None,
            );
        }
        let publication = super::super::frames::NativePublicationWitness::validate(
            self, &callable, realm, target, 0, mode,
        )?;
        let active_frame = publication.publish(0, 0, true)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_activation_prepared");
        Ok(PreparedNativeCall {
            activation: NativeActivation {
                runtime: Some(self.clone()),
                callable: Some(callable),
                realm,
                target,
                mode,
                arguments: NativeArguments {
                    actual_arg_count: 0,
                    readable: Vec::new(),
                },
                active_frame: Some(active_frame),
            },
            invocation: owners.invocation.take().expect("array next invocation"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn prepare_native_continuation_selected(
        &self,
        callable: CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
        mode: NativeInvokeMode,
        selected: Option<super::super::frames::NativeClassification>,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        self.prepare_native_arguments(
            NativeCallableInput::Owned(callable),
            realm,
            target,
            min_readable_args,
            invocation,
            NativeArgumentInput::Internal(arguments),
            mode,
            true,
            selected,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_native_arguments(
        &self,
        callable_input: NativeCallableInput<'_>,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: NativeArgumentInput<'_>,
        mode: NativeInvokeMode,
        continuation: bool,
        selected: Option<super::super::frames::NativeClassification>,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        #[cfg(feature = "profiling")]
        let _profile_phase = crate::engine::api::profiling::PhaseTimer::start_vm("native.prepare");
        let callable = callable_input.as_ref();
        // Every early return below abandons an already-owned invocation or a
        // partially materialized readable buffer. Keep them under one owner so
        // failure paths release exactly what preparation took.
        let mut owners = PendingNativeOwners {
            runtime: self,
            invocation: Some(invocation),
            readable: Vec::new(),
        };

        let arguments = match arguments {
            NativeArgumentInput::Internal(values) => {
                owners.readable = values;
                None
            }
            other => Some(other),
        };
        let publication = match selected.as_ref() {
            Some(selected) => super::super::frames::NativePublicationWitness::from_classification(
                self,
                callable,
                realm,
                target,
                min_readable_args,
                mode,
                selected,
            )?,
            None => super::super::frames::NativePublicationWitness::validate(
                self,
                callable,
                realm,
                target,
                min_readable_args,
                mode,
            )?,
        };

        let actual_arg_count = match &arguments {
            Some(NativeArgumentInput::Borrowed(values)) => values.len(),
            #[cfg(test)]
            Some(NativeArgumentInput::Owned(values)) => values.len(),
            Some(NativeArgumentInput::Internal(_)) => unreachable!(),
            None => owners.readable.len(),
        };
        let available_arg_count = actual_arg_count.max(usize::from(min_readable_args));
        let (_copied, _before) = match arguments {
            Some(NativeArgumentInput::Borrowed(values)) => {
                owners
                    .readable
                    .try_reserve(available_arg_count)
                    .map_err(|_| {
                        RuntimeError::Invariant("native readable arguments allocation failed")
                    })?;
                for value in values {
                    let converted = self.unroot_value(value)?;
                    owners.readable.push(converted);
                }
                (true, 0)
            }

            #[cfg(test)]
            Some(NativeArgumentInput::Owned(values)) => {
                let before = values.capacity();
                owners.readable = Vec::with_capacity(values.len());
                for value in values {
                    let converted = self.into_jsvalue(value)?;
                    owners.readable.push(converted);
                }
                // All padding allocation precedes publication. Actual arity
                // and every extra argument survive this owning handoff.
                owners
                    .readable
                    .try_reserve(available_arg_count - actual_arg_count)
                    .map_err(|_| {
                        RuntimeError::Invariant("native readable arguments allocation failed")
                    })?;
                (false, before)
            }
            None => {
                let before = owners.readable.capacity();
                owners
                    .readable
                    .try_reserve(available_arg_count - actual_arg_count)
                    .map_err(|_| {
                        RuntimeError::Invariant("native readable arguments allocation failed")
                    })?;
                (false, before)
            }
            Some(NativeArgumentInput::Internal(_)) => unreachable!(),
        };
        while owners.readable.len() < available_arg_count {
            owners
                .readable
                .push(crate::engine::value::JsValue::Undefined);
        }
        #[cfg(feature = "profiling")]
        {
            use crate::engine::api::profiling::{
                record_call_buffer_capacity, record_call_buffer_initialized,
                record_call_buffer_js_value_copies, record_call_buffer_observed,
            };
            record_call_buffer_capacity(
                "native.readable",
                _before,
                owners.readable.capacity(),
                size_of::<JsValue>(),
            );
            if _copied {
                record_call_buffer_js_value_copies(
                    "native.readable",
                    &owners.readable[..actual_arg_count],
                );
            } else {
                record_call_buffer_observed("native.inbound_argv", _before, size_of::<JsValue>());
                // Moving Vec ownership into NativeArguments does not move elements.
                record_call_buffer_observed(
                    "native.readable",
                    owners.readable.capacity(),
                    size_of::<JsValue>(),
                );
            }
            record_call_buffer_initialized(
                "native.readable",
                available_arg_count - actual_arg_count,
            );
        }
        let mut arguments = NativeArguments {
            actual_arg_count,
            readable: std::mem::take(&mut owners.readable),
        };
        // Reservation happens before installing the native scope.
        let active_frame =
            match publication.publish(actual_arg_count, available_arg_count, continuation) {
                Ok(active_frame) => active_frame,
                Err(error) => {
                    owners.readable = std::mem::take(&mut arguments.readable);
                    return Err(error);
                }
            };

        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_activation_prepared");
        Ok(PreparedNativeCall {
            activation: NativeActivation {
                runtime: Some(self.clone()),
                callable: Some(callable_input.into_owned()),
                realm,
                target,
                mode,
                arguments,
                active_frame: Some(active_frame),
            },
            invocation: owners.invocation.take().expect("prepared invocation owner"),
        })
    }
}

impl PreparedNativeCall {
    /// Release the owned invocation edge when a prepared call is abandoned
    /// before its dispatcher consumes it. `NativeActivation`'s own Drop already
    /// releases the readable argument edges.
    pub(in crate::engine::vm) fn release_invocation(&mut self) -> Result<(), RuntimeError> {
        let invocation = std::mem::replace(
            &mut self.invocation,
            NativeInvocation::Getter {
                this_value: JsValue::Undefined,
            },
        );
        invocation.release(
            self.activation
                .runtime
                .as_ref()
                .expect("native runtime present"),
        )
    }
}

impl Drop for NativeActivation {
    /// Release every readable argument edge still owned when the activation is
    /// abandoned without `finish`. `finish` takes the buffer first, so a
    /// completed activation drops an empty vector. Releases are defer-safe and
    /// never run JavaScript.
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.as_ref() {
            for value in self.arguments.readable.drain(..) {
                let _ = runtime.release_jsvalue(value);
            }
        } else {
            debug_assert!(self.arguments.readable.is_empty());
        }
    }
}

impl NativeActivation {
    pub(in crate::engine::vm) fn callable(&self) -> &CallableRef {
        self.callable.as_ref().expect("native callable present")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::engine::vm) fn own_continuation(&mut self) -> Result<(), RuntimeError> {
        self.active_frame
            .as_mut()
            .expect("native activation lost its active frame")
            .mark_native_continuation()
    }

    /// Allocate JS engine errors while this native frame and its selected realm
    /// are still visible. An existing thrown Value must not be re-materialized.
    pub(in crate::engine::vm) fn finish(
        self,
        result: Result<NativeInvokeOutcome, RuntimeError>,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        self.finish_reusing(result).0
    }

    /// Return only empty capacity after errors have been materialized and the
    /// native frame has ended. This also preserves cleanup on a failed finish.
    pub(in crate::engine::vm) fn finish_reusing(
        self,
        result: Result<NativeInvokeOutcome, RuntimeError>,
    ) -> (Result<NativeInvokeOutcome, RuntimeError>, Vec<JsValue>) {
        self.finish_reusing_with(
            result,
            |value| NativeInvokeOutcome::Completion(Completion::Throw(value)),
            |outcome| match outcome {
                NativeInvokeOutcome::Completion(
                    Completion::Return(value) | Completion::Throw(value),
                )
                | NativeInvokeOutcome::IteratorNextRaw { value, .. } => value,
            },
        )
    }

    pub(in crate::engine::vm) fn finish_completion_reusing(
        self,
        result: Result<Completion, RuntimeError>,
    ) -> (Result<Completion, RuntimeError>, Vec<JsValue>) {
        self.finish_reusing_with(result, Completion::Throw, |completion| match completion {
            Completion::Return(value) | Completion::Throw(value) => value,
        })
    }

    #[inline]
    fn finish_reusing_with<T>(
        self,
        result: Result<T, RuntimeError>,
        throw: impl FnOnce(JsValue) -> T,
        into_owned_value: impl FnOnce(T) -> JsValue,
    ) -> (Result<T, RuntimeError>, Vec<JsValue>) {
        match result {
            Ok(value) => self.finish_value(value, into_owned_value),
            Err(error) => self.finish_error(error, throw, into_owned_value),
        }
    }

    // Keep the wide RuntimeError transport out of the successful owner handoff.
    fn finish_value<T>(
        mut self,
        value: T,
        into_owned_value: impl FnOnce(T) -> JsValue,
    ) -> (Result<T, RuntimeError>, Vec<JsValue>) {
        if let Err(error) = self
            .active_frame
            .take()
            .expect("native activation lost its active frame")
            .finish()
        {
            let _ = self
                .runtime
                .as_ref()
                .expect("native runtime present")
                .release_jsvalue(into_owned_value(value));
            return (Err(error), self.release_arguments_reusing());
        }
        (Ok(value), self.release_arguments_reusing())
    }

    #[cold]
    #[inline(never)]
    fn finish_error<T>(
        mut self,
        error: RuntimeError,
        throw: impl FnOnce(JsValue) -> T,
        into_owned_value: impl FnOnce(T) -> JsValue,
    ) -> (Result<T, RuntimeError>, Vec<JsValue>) {
        // Error construction must still see the active native frame and realm.
        let error = match error {
            RuntimeError::Engine(error)
                if NativeErrorKind::from_javascript_error(error.kind()).is_some() =>
            {
                let kind = NativeErrorKind::from_javascript_error(error.kind())
                    .expect("guard proved this is a JavaScript-visible native error");
                match self
                    .runtime
                    .as_ref()
                    .expect("native runtime present")
                    .new_native_error_from_error_jsvalue(self.realm, kind, &error)
                {
                    Ok(value) => return self.finish_value(throw(value), into_owned_value),
                    Err(error) => error,
                }
            }
            error => error,
        };
        let error = match self
            .active_frame
            .take()
            .expect("native activation lost its active frame")
            .finish()
        {
            Ok(()) => error,
            Err(frame_error) => frame_error,
        };
        (Err(error), self.release_arguments_reusing())
    }

    fn release_arguments_reusing(&mut self) -> Vec<JsValue> {
        // finish_value/finish_error already retired the diagnostic frame. Retire
        // the callable in place before argv, leaving the activation empty for
        // its automatic Drop instead of moving the whole record twice.
        debug_assert!(self.active_frame.is_none());
        drop(self.callable.take());
        let runtime = self.runtime.as_ref().expect("native runtime present");
        let mut readable = std::mem::take(&mut self.arguments.readable);
        for value in readable.drain(..) {
            let _ = runtime.release_jsvalue(value);
        }
        readable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::{Context, Error, ErrorKind},
        vm::call::CallableExecution,
    };

    fn prepare(
        runtime: &Runtime,
        context: &mut Context,
        arguments: &[Value],
    ) -> PreparedNativeCall {
        let callable = runtime
            .callable_from_value(context.eval("Reflect.get").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected native")
        };
        runtime
            .prepare_native_invocation(
                &callable,
                realm,
                target,
                min_readable_args,
                NativeInvocation::Call {
                    this_value: JsValue::Undefined,
                },
                arguments,
                NativeInvokeMode::Ordinary,
            )
            .unwrap()
    }

    #[test]
    fn borrowed_native_adaptation_shares_validation_and_preserves_input_owners() {
        use super::super::NativeInvocationAdaptation;
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for (fixture, construct) in [
            ("Reflect.get", false),
            ("Math.min", true),
            ("Array", false),
            ("Map", false),
            ("Map", true),
            (
                "Object.getOwnPropertyDescriptor(Map.prototype,'size').get",
                false,
            ),
            (
                "Object.getOwnPropertyDescriptor(Object.prototype,'__proto__').set",
                false,
            ),
        ] {
            let callable = runtime
                .callable_from_value(context.eval(fixture).unwrap())
                .unwrap();
            let CallableExecution::Native {
                target,
                realm,
                min_readable_args,
            } = runtime.bytecode_for_callable(&callable).unwrap()
            else {
                panic!("native fixture")
            };
            let input = runtime
                .unroot_value(&Value::Object(runtime.new_object(None).unwrap()))
                .unwrap();
            let invocation = if construct {
                NativeInvocation::Construct { new_target: input }
            } else {
                NativeInvocation::Call { this_value: input }
            };
            let prepared = runtime
                .prepare_native_invocation_owned(
                    callable,
                    realm,
                    target,
                    min_readable_args,
                    invocation,
                    Vec::new(),
                    NativeInvokeMode::Ordinary,
                )
                .unwrap();
            fn native_input(value: &NativeInvocation) -> &JsValue {
                match value {
                    NativeInvocation::Call { this_value }
                    | NativeInvocation::Getter { this_value }
                    | NativeInvocation::Setter { this_value } => this_value,
                    NativeInvocation::Construct { new_target } => new_target,
                }
            }
            let borrowed = runtime
                .adapt_native_invocation_borrowed(
                    target,
                    realm,
                    &prepared.invocation,
                    &prepared.activation.arguments,
                )
                .unwrap();
            if fixture == "Reflect.get" {
                let NativeInvocationAdaptation::Invoke(value) = &borrowed else {
                    panic!("expected invocation adaptation");
                };
                assert_eq!(
                    native_input(value.as_ref()),
                    native_input(&prepared.invocation)
                );
            }
            let owned = runtime
                .adapt_native_invocation(
                    target,
                    realm,
                    prepared.invocation.dup(&runtime).unwrap(),
                    &prepared.activation.arguments,
                )
                .unwrap();
            match (borrowed, owned) {
                (
                    NativeInvocationAdaptation::Invoke(borrowed),
                    NativeInvocationAdaptation::Invoke(owned),
                ) => {
                    assert_eq!(
                        std::mem::discriminant(borrowed.as_ref()),
                        std::mem::discriminant(&owned)
                    );
                    assert_eq!(native_input(borrowed.as_ref()), native_input(&owned));
                    borrowed.release(&runtime).unwrap();
                    owned.release(&runtime).unwrap();
                }
                (
                    NativeInvocationAdaptation::Complete(Completion::Throw(throw_a)),
                    NativeInvocationAdaptation::Complete(Completion::Throw(throw_b)),
                ) => {
                    let Value::Object(a) = runtime.root_and_release_jsvalue(throw_a).unwrap()
                    else {
                        panic!("expected thrown object");
                    };
                    let Value::Object(b) = runtime.root_and_release_jsvalue(throw_b).unwrap()
                    else {
                        panic!("expected thrown object");
                    };
                    assert_eq!(
                        runtime.get_prototype_of(&a).unwrap(),
                        runtime.get_prototype_of(&b).unwrap()
                    );
                }
                _ => panic!("borrowed and owned adaptation diverged"),
            }
            let already_adapted = NativeInvocation::Getter {
                this_value: JsValue::Undefined,
            };
            assert!(matches!(
                runtime.adapt_native_invocation_borrowed(
                    target,
                    realm,
                    &already_adapted,
                    &prepared.activation.arguments
                ),
                Err(RuntimeError::Invariant(
                    "native invocation was adapted more than once"
                ))
            ));
            let wrong_arguments = NativeArguments {
                actual_arg_count: usize::MAX,
                readable: Vec::new(),
            };
            assert!(matches!(
                runtime.adapt_native_invocation_borrowed(
                    target,
                    realm,
                    &prepared.invocation,
                    &wrong_arguments
                ),
                Err(RuntimeError::Invariant(
                    "active native frame disagrees with handler arguments"
                ))
            ));
            prepared.invocation.release(&runtime).unwrap();
            prepared
                .activation
                .finish(Ok(NativeInvokeOutcome::Completion(Completion::Return(
                    JsValue::Undefined,
                ))))
                .unwrap();
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    #[cfg(feature = "profiling")]
    fn native_argument_pool_reuses_nested_capacity_and_releases_all_owners() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Reflect.get").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected native")
        };
        let mut slots = crate::engine::vm::stack::SlotStore::new(1024);
        let profile = crate::engine::api::profiling::CostProfile::start();
        for round in 0..20 {
            let mut pending = Vec::new();
            let mut markers = Vec::new();
            for depth in 1..=16 {
                slots.reserve_native_argument_depth(depth).unwrap();
                let mut arguments = slots.take_native_argument_buffer(4).unwrap();
                let marker = runtime.new_object(None).unwrap();
                markers.push(marker.object_id());
                arguments.push(JsValue::Object(marker.into_handle()));
                let prepared = runtime
                    .prepare_native_invocation_jsvalue(
                        &callable,
                        realm,
                        target,
                        min_readable_args,
                        NativeInvocation::Call {
                            this_value: JsValue::Undefined,
                        },
                        arguments,
                        NativeInvokeMode::Ordinary,
                    )
                    .unwrap();
                pending.push(prepared);
            }
            while let Some(prepared) = pending.pop() {
                let result = if round % 2 == 0 {
                    Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                        JsValue::Int(42),
                    )))
                } else {
                    Err(RuntimeError::Invariant("pool error path"))
                };
                prepared.invocation.release(&runtime).unwrap();
                let (result, empty) = prepared.activation.finish_reusing(result);
                assert!(empty.is_empty());
                assert!(empty.capacity() >= 4);
                assert_eq!(result.is_err(), round % 2 != 0);
                slots.recycle_native_argument_buffer(empty);
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            runtime.run_gc().unwrap();
            for marker in markers {
                assert!(runtime.0.state.borrow().heap.object(marker).is_err());
            }
        }
        let costs = profile.snapshot();
        assert_eq!(costs.call_buffers["call.native_argv"].capacity_growths, 16);
        assert_eq!(costs.call_buffers["native.readable"].capacity_growths, 0);
        assert_eq!(costs.call_buffers["native.readable"].values_copied, 0);
        drop(callable);
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none(), "empty pool retained Runtime");
        drop(slots);
    }

    #[test]
    #[cfg(feature = "profiling")]
    fn owned_readable_arguments_keep_buffer_identity_arity_and_padding() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Reflect.get").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected native");
        };
        let marker = runtime.new_object(None).unwrap();
        for actual in [
            vec![],
            vec![Value::Object(marker.clone())],
            vec![
                Value::Object(marker.clone()),
                Value::Int(0),
                Value::Undefined,
                Value::Object(marker.clone()),
            ],
        ] {
            let borrowed = runtime
                .prepare_native_invocation(
                    &callable,
                    realm,
                    target,
                    min_readable_args,
                    NativeInvocation::Call {
                        this_value: JsValue::Undefined,
                    },
                    &actual,
                    NativeInvokeMode::Ordinary,
                )
                .unwrap();
            let expected = borrowed
                .activation
                .arguments
                .readable
                .iter()
                .map(|value| runtime.dup_jsvalue(value).unwrap())
                .collect::<Vec<_>>();
            borrowed.invocation.release(&runtime).unwrap();
            drop(borrowed.activation);
            let count = actual.len();
            let mut owned = Vec::with_capacity(8);
            owned.extend(
                actual
                    .into_iter()
                    .map(|value| runtime.into_jsvalue(value).unwrap()),
            );
            let address = owned.as_ptr();
            let profile = crate::engine::api::profiling::CostProfile::start();
            let prepared = runtime
                .prepare_native_invocation_jsvalue(
                    &callable,
                    realm,
                    target,
                    min_readable_args,
                    NativeInvocation::Call {
                        this_value: JsValue::Undefined,
                    },
                    owned,
                    NativeInvokeMode::Ordinary,
                )
                .unwrap();
            assert_eq!(prepared.activation.arguments.actual_arg_count, count);
            assert_eq!(prepared.activation.arguments.readable, expected);
            assert_eq!(prepared.activation.arguments.readable.as_ptr(), address);
            let costs = profile.snapshot();
            let buffer = &costs.call_buffers["native.readable"];
            assert_eq!(buffer.capacity_growths, 0);
            assert_eq!(buffer.values_copied, 0);
            assert_eq!(buffer.heap_root_copies, 0);
            // Moving the Vec header does not move its internal values.
            assert_eq!(buffer.values_moved, 0);
            assert_eq!(buffer.slots_initialized, (expected.len() - count) as u64);
            prepared.invocation.release(&runtime).unwrap();
            let result = prepared
                .activation
                .finish(Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                    JsValue::Int(42),
                ))))
                .unwrap();
            assert!(matches!(
                result,
                NativeInvokeOutcome::Completion(Completion::Throw(JsValue::Int(42)))
            ));
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            for value in expected {
                runtime.release_jsvalue(value).unwrap();
            }
        }
    }

    #[test]

    fn owning_preparation_keeps_rejection_order_and_argument_domain_errors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Reflect.get").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected native");
        };
        let foreign = Runtime::new();
        let rejected = foreign.prepare_native_invocation_owned(
            callable.clone(),
            realm,
            target,
            min_readable_args + 1,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            vec![],
            NativeInvokeMode::Ordinary,
        );
        assert!(matches!(
            rejected,
            Err(RuntimeError::WrongRuntime("native callable"))
        ));
        assert!(foreign.0.state.borrow().active_frames.is_empty());
        let rejected = runtime.prepare_native_invocation_owned(
            callable.clone(),
            realm,
            target,
            min_readable_args + 1,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            vec![],
            NativeInvokeMode::Ordinary,
        );
        assert!(matches!(
            rejected,
            Err(RuntimeError::Invariant(
                "native invocation metadata changed after snapshot"
            ))
        ));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        let arguments = vec![
            Value::Object(foreign.new_object(None).unwrap()),
            Value::Int(0),
        ];
        let mut errors = Vec::new();
        for owned in [false, true] {
            let rejected = if owned {
                runtime.prepare_native_invocation_owned(
                    callable.clone(),
                    realm,
                    target,
                    min_readable_args,
                    NativeInvocation::Call {
                        this_value: JsValue::Undefined,
                    },
                    arguments.clone(),
                    NativeInvokeMode::Ordinary,
                )
            } else {
                runtime.prepare_native_invocation(
                    &callable,
                    realm,
                    target,
                    min_readable_args,
                    NativeInvocation::Call {
                        this_value: JsValue::Undefined,
                    },
                    &arguments,
                    NativeInvokeMode::Ordinary,
                )
            };
            let error = match rejected {
                Err(error) => error,
                Ok(_) => panic!("foreign argument accepted"),
            };
            errors.push(format!("{error:?}"));
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        assert_eq!(errors[0], errors[1]);
    }

    #[test]
    #[cfg(feature = "profiling")]
    fn readable_buffer_ledger_separates_padding_from_copied_roots() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let marker = runtime.new_object(None).unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        let native = prepare(
            &runtime,
            &mut context,
            &[Value::Object(marker), Value::Int(0)],
        );
        let cost = profile.snapshot();
        let buffer = &cost.call_buffers["native.readable"];
        assert_eq!(buffer.capacity_growths, 1);
        assert_eq!(buffer.values_copied, 2);
        assert_eq!(buffer.heap_root_copies, 1);
        assert_eq!(buffer.immediate_copies, 1);
        assert_eq!(buffer.primitive_rc_copies, 0);
        assert_eq!(
            buffer.slots_initialized,
            native.activation.arguments.readable.len() as u64
        );
        assert_eq!(cost.vm_phases["native.prepare"].cost.attempts, 1);
        assert_eq!(cost.vm_phases["native.prepare"].samples_ns.len(), 1);
    }

    #[test]
    fn prepared_native_activation_owns_all_arguments_without_invoking_the_body() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let target = context
            .eval("var calls=0;new Proxy({}, {get(){calls++;return 1}})")
            .unwrap();
        let extra = runtime.new_object(None).unwrap();
        let extra_id = extra.object_id();
        let native = prepare(
            &runtime,
            &mut context,
            &[
                target,
                Value::Int(0),
                Value::Undefined,
                Value::Object(extra),
            ],
        );
        assert_eq!(native.activation.arguments.actual_arg_count, 4);
        assert_eq!(native.activation.arguments.readable.len(), 4);
        assert_eq!(runtime.0.state.borrow().active_frames.len(), 1);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(extra_id).is_ok());
        assert_eq!(context.eval("calls").unwrap(), Value::Int(0));
        drop(native);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(extra_id).is_err());
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn prepared_native_padding_and_unwind_restore_the_frame() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let native = prepare(&runtime, &mut context, &[]);
        assert_eq!(native.activation.arguments.actual_arg_count, 0);
        assert!(!native.activation.arguments.readable.is_empty());
        assert!(
            native
                .activation
                .arguments
                .readable
                .iter()
                .all(|value| *value == JsValue::Undefined)
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _native = native;
            panic!("abandon native activation");
        }));
        assert!(result.is_err());
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn native_activation_materializes_errors_before_leaving_its_defining_realm() {
        let runtime = Runtime::new();
        let mut caller = runtime.new_context();
        let mut defining = runtime.new_context();
        let expected_prototype = defining.eval("TypeError.prototype").unwrap();
        let native = prepare(&runtime, &mut defining, &[]);
        drop(defining);
        let result = native
            .activation
            .finish(Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "activation failure",
            ))))
            .unwrap();
        let NativeInvokeOutcome::Completion(Completion::Throw(thrown)) = result else {
            panic!("expected TypeError")
        };
        let Value::Object(error) = runtime.root_value(&thrown).unwrap() else {
            panic!("expected TypeError")
        };
        assert_eq!(
            runtime.get_prototype_of(&error).unwrap().map(Value::Object),
            Some(expected_prototype)
        );
        let stack = caller
            .get_property(&error, &runtime.intern_property_key("stack").unwrap())
            .unwrap();
        let Value::String(stack) = stack else {
            panic!("expected captured stack")
        };
        assert!(stack.to_string().contains("get (native)"), "{stack:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        runtime.release_jsvalue(thrown).unwrap();
        let sentinel = runtime.new_object(None).unwrap();
        let sentinel_id = sentinel.object_id();
        let native = prepare(&runtime, &mut caller, &[]);
        let result = native
            .activation
            .finish(Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                runtime.unroot_value(&Value::Object(sentinel)).unwrap(),
            ))))
            .unwrap();
        let NativeInvokeOutcome::Completion(Completion::Throw(value)) = result else {
            panic!("expected sentinel throw")
        };
        assert_eq!(value, JsValue::Object(sentinel_id));
        runtime.release_jsvalue(value).unwrap();
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn native_activation_rejects_foreign_and_changed_metadata_before_registration() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Reflect.get").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected native")
        };
        let wrong_runtime = Runtime::new();
        let result = wrong_runtime.prepare_native_invocation(
            &callable,
            realm,
            target,
            min_readable_args,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &[],
            NativeInvokeMode::Ordinary,
        );
        assert!(matches!(result, Err(RuntimeError::WrongRuntime(_))));
        assert!(wrong_runtime.0.state.borrow().active_frames.is_empty());
        let result = runtime.prepare_native_invocation(
            &callable,
            realm,
            target,
            min_readable_args + 1,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &[],
            NativeInvokeMode::Ordinary,
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Invariant(
                "native invocation metadata changed after snapshot"
            ))
        ));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[cfg(test)]
mod continuation_publication_tests {
    use super::*;
    use crate::engine::vm::call::CallableExecution;

    #[test]
    fn native_continuation_publication_preserves_abi_hidden_flags_and_single_registration() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for (source, mode) in [
            ("Map.prototype.set", NativeInvokeMode::Ordinary),
            ("[].values().next", NativeInvokeMode::IteratorNextRaw),
        ] {
            let callable = runtime
                .callable_from_value(context.eval(source).unwrap())
                .unwrap();
            let CallableExecution::Native {
                target,
                realm,
                min_readable_args,
            } = runtime.bytecode_for_callable(&callable).unwrap()
            else {
                panic!("native");
            };
            let mut call = runtime
                .prepare_native_continuation_owned(
                    callable,
                    realm,
                    target,
                    min_readable_args,
                    NativeInvocation::Call {
                        this_value: JsValue::Undefined,
                    },
                    vec![Value::Int(7)],
                    mode,
                )
                .unwrap();
            {
                let state = runtime.0.state.borrow();
                let frame = state.active_frames.last().unwrap();
                assert!(frame.native_continuation);
                assert_eq!(
                    frame.flags.backtrace_hidden,
                    matches!(mode, NativeInvokeMode::IteratorNextRaw)
                );
                assert_eq!(frame.realm, realm);
            }
            assert_eq!(call.activation.arguments.actual_arg_count, 1);
            assert_eq!(
                call.activation.arguments.readable.len(),
                1usize.max(usize::from(min_readable_args))
            );
            assert!(
                runtime
                    .adapt_native_invocation_borrowed(
                        target,
                        realm,
                        &call.invocation,
                        &call.activation.arguments
                    )
                    .is_ok()
            );
            assert!(matches!(
                call.activation.own_continuation(),
                Err(RuntimeError::Invariant(
                    "native continuation was registered twice"
                ))
            ));
            drop(call);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn promise_resolving_publication_hides_backtrace_frames() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "Promise.withResolvers().resolve",
            "Promise.withResolvers().reject",
        ] {
            let callable = runtime
                .callable_from_value(context.eval(source).unwrap())
                .unwrap();
            let CallableExecution::Native {
                target,
                realm,
                min_readable_args,
            } = runtime.bytecode_for_callable(&callable).unwrap()
            else {
                panic!("native");
            };
            assert!(
                matches!(target, NativeFunctionId::PromiseResolving(_)),
                "{source} was not classified as a promise-resolving function"
            );
            let call = runtime
                .prepare_native_continuation_owned(
                    callable,
                    realm,
                    target,
                    min_readable_args,
                    NativeInvocation::Call {
                        this_value: Value::Undefined,
                    },
                    vec![Value::Int(7)],
                    NativeInvokeMode::Ordinary,
                )
                .unwrap();
            {
                let state = runtime.0.state.borrow();
                let frame = state.active_frames.last().unwrap();
                assert!(frame.native_continuation);
                assert!(
                    frame.flags.backtrace_hidden,
                    "{source} frame must not appear in backtraces"
                );
                assert_eq!(frame.realm, realm);
            }
            drop(call);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn native_continuation_publication_rejects_stale_metadata_before_publishing() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Math.min").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("native");
        };
        let token = runtime.0.state.borrow().next_active_frame_token;
        let result = runtime.prepare_native_continuation_owned(
            callable.clone(),
            realm,
            target,
            min_readable_args.saturating_add(1),
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            Vec::new(),
            NativeInvokeMode::Ordinary,
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Invariant(
                "native invocation metadata changed after snapshot"
            ))
        ));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert_eq!(runtime.0.state.borrow().next_active_frame_token, token);
        let mut legacy = runtime
            .prepare_native_invocation_owned(
                callable,
                realm,
                target,
                min_readable_args,
                NativeInvocation::Call {
                    this_value: JsValue::Undefined,
                },
                Vec::new(),
                NativeInvokeMode::Ordinary,
            )
            .unwrap();
        assert!(
            !runtime
                .0
                .state
                .borrow()
                .active_frames
                .last()
                .unwrap()
                .native_continuation
        );
        legacy.activation.own_continuation().unwrap();
        assert!(
            runtime
                .0
                .state
                .borrow()
                .active_frames
                .last()
                .unwrap()
                .native_continuation
        );
        drop(legacy);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[cfg(test)]
mod publication_witness_tests {
    use super::*;
    use crate::engine::vm::call::CallableExecution;
    use crate::engine::vm::frames::{ActiveFrameKind, NativePublicationWitness};

    #[test]
    fn publication_witness_matches_checked_registration_and_keeps_count_guard() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "Map.prototype.set",
            "Math.min",
            "Reflect.get",
            "Array.prototype.push",
        ] {
            let callable = runtime
                .callable_from_value(context.eval(source).unwrap())
                .unwrap();
            let CallableExecution::Native {
                target,
                realm,
                min_readable_args,
            } = runtime.bytecode_for_callable(&callable).unwrap()
            else {
                panic!("native")
            };
            let count = usize::from(min_readable_args);
            let checked = runtime
                .push_native_active_frame(callable.as_object().clone(), realm, target, 0, count)
                .unwrap();
            let original = *runtime.0.state.borrow().active_frames.last().unwrap();
            checked.finish().unwrap();
            let witness = NativePublicationWitness::validate(
                &runtime,
                &callable,
                realm,
                target,
                min_readable_args,
                NativeInvokeMode::Ordinary,
            )
            .unwrap();
            let published = witness.publish(0, count, false).unwrap();
            let current = *runtime.0.state.borrow().active_frames.last().unwrap();
            assert_eq!(original.function, current.function);
            assert_eq!(original.realm, current.realm);
            assert_eq!(original.native_continuation, current.native_continuation);
            assert!(
                matches!(current.kind, ActiveFrameKind::Native { target: actual_target, actual_arg_count: 0, readable_arg_count } if actual_target == target && readable_arg_count == count)
            );
            published.finish().unwrap();
            let token = runtime.0.state.borrow().next_active_frame_token;
            let witness = NativePublicationWitness::validate(
                &runtime,
                &callable,
                realm,
                target,
                min_readable_args,
                NativeInvokeMode::Ordinary,
            )
            .unwrap();
            assert!(matches!(
                witness.publish(0, count + 1, false),
                Err(RuntimeError::Invariant(
                    "native active frame disagrees with its rooted callable"
                ))
            ));
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            assert_eq!(runtime.0.state.borrow().next_active_frame_token, token);
        }
    }

    #[test]
    fn publication_witness_preserves_foreign_realm_metadata_and_token_error_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let other_context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Reflect.get").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("native")
        };
        let foreign = Runtime::new();
        assert!(matches!(
            NativePublicationWitness::validate(
                &foreign,
                &callable,
                other_context.realm,
                target,
                min_readable_args + 1,
                NativeInvokeMode::Ordinary
            ),
            Err(RuntimeError::WrongRuntime("native callable"))
        ));
        let token = runtime.0.state.borrow().next_active_frame_token;
        for (selected_realm, minimum) in [
            (other_context.realm, min_readable_args),
            (realm, min_readable_args + 1),
        ] {
            assert!(matches!(
                NativePublicationWitness::validate(
                    &runtime,
                    &callable,
                    selected_realm,
                    target,
                    minimum,
                    NativeInvokeMode::Ordinary
                ),
                Err(RuntimeError::Invariant(
                    "native invocation metadata changed after snapshot"
                ))
            ));
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            assert_eq!(runtime.0.state.borrow().next_active_frame_token, token);
        }
        runtime.0.state.borrow_mut().next_active_frame_token = u64::MAX;
        let result = runtime.prepare_native_invocation(
            &callable,
            realm,
            target,
            min_readable_args,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &[],
            NativeInvokeMode::Ordinary,
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Invariant(
                "active-frame token space was exhausted"
            ))
        ));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert_eq!(runtime.0.state.borrow().next_active_frame_token, u64::MAX);
        runtime.0.state.borrow_mut().next_active_frame_token = token;
    }

    #[test]
    fn publication_witness_keeps_native_reentry_throw_and_following_result() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "(function(){var m=new Map(),n=0,o={valueOf(){n++;m.set('x',41);return m.get('x')}};return Math.min(o,99)+n})()",
            "(function(){var marker={},m=new Map(),n=0;try{Math.min({valueOf(){n++;m.set('x',41);throw marker}},0);return 0}catch(e){return e===marker&&n===1?m.get('x')+1:0}})()",
            "(function(){var n=0,a={get length(){n++;return 0},set length(v){}};Array.prototype.push.call(a,41);return a[0]+n})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Int(42));
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }
}

#[cfg(test)]
mod classified_preparation_tests {
    use super::*;
    use crate::engine::vm::{call::CallableExecution, frames::NativeClassification};

    #[test]
    fn selected_native_payload_is_bound_to_its_call_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("Math.min").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("native")
        };
        let selection = NativeClassification::select(&runtime, &callable)
            .unwrap()
            .unwrap();
        let prepared = runtime
            .prepare_native_continuation_selected(
                callable.clone(),
                realm,
                target,
                min_readable_args,
                NativeInvocation::Call {
                    this_value: JsValue::Undefined,
                },
                vec![JsValue::Int(3), JsValue::Int(2)],
                NativeInvokeMode::Ordinary,
                Some(selection),
            )
            .unwrap();
        assert_eq!(prepared.activation.arguments.actual_arg_count, 2);
        prepared
            .activation
            .finish(Ok(NativeInvokeOutcome::Completion(Completion::Return(
                JsValue::Int(2),
            ))))
            .unwrap();
        let selection = NativeClassification::select(&runtime, &callable)
            .unwrap()
            .unwrap();
        let other = runtime
            .callable_from_value(context.eval("Math.max").unwrap())
            .unwrap();
        assert!(matches!(
            runtime.prepare_native_continuation_selected(
                other,
                realm,
                target,
                min_readable_args,
                NativeInvocation::Call {
                    this_value: JsValue::Undefined
                },
                vec![],
                NativeInvokeMode::Ordinary,
                Some(selection)
            ),
            Err(RuntimeError::Invariant(
                "native invocation metadata changed after snapshot"
            ))
        ));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}
