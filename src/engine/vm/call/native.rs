//! Native invocation ownership shared by synchronous consumers and owned continuations.
//! Preparing an activation executes no builtin body; arguments and the diagnostic
//! frame survive until completion, rejection or abandonment.
use super::{NativeArguments, NativeInvocation, NativeInvokeMode, NativeInvokeOutcome};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::NativeFunctionId,
    heap::{ContextId, ObjectPayload},
    object::CallableRef,
    value::Value,
    vm::{Completion, frames::ActiveFrameGuard},
};

pub(in crate::engine::vm) struct PreparedNativeCall {
    pub activation: NativeActivation,
    pub invocation: NativeInvocation,
}

pub(in crate::engine::vm) struct NativeActivation {
    pub callable: CallableRef,
    pub realm: ContextId,
    pub target: NativeFunctionId,
    pub mode: NativeInvokeMode,
    pub arguments: NativeArguments,
    active_frame: ActiveFrameGuard,
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
        if !callable.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("native callable"));
        }
        self.0.state.borrow().heap.context(realm)?;

        // The callable root held by the caller owns the native payload and its
        // defining-realm edge for the whole invocation. Revalidate the
        // detached snapshot before recording raw identities in the frame.
        // Class-call and CFunctionData-style internal functions deliberately
        // execute in `realm`, which is the calling realm rather than the
        // separately retained defining realm.
        {
            let state = self.0.state.borrow();
            let object = state.heap.object(callable.as_object().object_id())?;
            let ObjectPayload::NativeFunction { data, .. } = &object.payload else {
                return Err(RuntimeError::Invariant(
                    "native invocation target was not a native function",
                ));
            };
            let defining_realm = data.realm.ok_or(RuntimeError::Invariant(
                "native function lost its defining realm",
            ))?;
            if data.target != target
                || (matches!(mode, NativeInvokeMode::Ordinary)
                    && !target.uses_calling_realm()
                    && defining_realm != realm)
                || data.min_readable_args != min_readable_args
            {
                return Err(RuntimeError::Invariant(
                    "native invocation metadata changed after snapshot",
                ));
            }
            state.heap.context(defining_realm)?;
        }

        let actual_arg_count = arguments.len();
        let available_arg_count = actual_arg_count.max(usize::from(min_readable_args));
        let mut readable = Vec::new();
        readable
            .try_reserve_exact(available_arg_count)
            .map_err(|_| RuntimeError::Invariant("native readable arguments allocation failed"))?;
        readable.extend_from_slice(arguments);
        readable.resize(available_arg_count, Value::Undefined);
        let arguments = NativeArguments {
            actual_arg_count,
            readable,
        };
        let active_frame = match mode {
            NativeInvokeMode::Ordinary => self.push_native_active_frame(
                callable.as_object().clone(),
                realm,
                target,
                actual_arg_count,
                available_arg_count,
            )?,
            NativeInvokeMode::IteratorNextRaw => self.push_native_iterator_next_active_frame(
                callable.as_object().clone(),
                realm,
                target,
                actual_arg_count,
                available_arg_count,
            )?,
        };

        Ok(PreparedNativeCall {
            activation: NativeActivation {
                callable: callable.clone(),
                realm,
                target,
                mode,
                arguments,
                active_frame,
            },
            invocation,
        })
    }
}

impl NativeActivation {
    /// Allocate JS engine errors while this native frame and its selected realm
    /// are still visible. An existing thrown Value must not be re-materialized.
    pub(in crate::engine::vm) fn finish(
        self,
        result: Result<NativeInvokeOutcome, RuntimeError>,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let runtime = &self.active_frame.runtime;
        let result = (|| match result {
            Err(RuntimeError::Engine(error))
                if NativeErrorKind::from_javascript_error(error.kind()).is_some() =>
            {
                let kind = NativeErrorKind::from_javascript_error(error.kind())
                    .expect("guard proved this is a JavaScript-visible native error");
                let value = runtime.new_native_error_from_error(self.realm, kind, &error)?;
                Ok(NativeInvokeOutcome::Completion(Completion::Throw(value)))
            }
            result => result,
        })();
        self.active_frame.finish()?;
        result
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
                    this_value: Value::Undefined,
                },
                arguments,
                NativeInvokeMode::Ordinary,
            )
            .unwrap()
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
                .all(|value| *value == Value::Undefined)
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
        let NativeInvokeOutcome::Completion(Completion::Throw(Value::Object(error))) = result
        else {
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
        let sentinel = runtime.new_object(None).unwrap();
        let native = prepare(&runtime, &mut caller, &[]);
        let result = native
            .activation
            .finish(Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                Value::Object(sentinel.clone()),
            ))))
            .unwrap();
        assert!(
            matches!(result, NativeInvokeOutcome::Completion(Completion::Throw(Value::Object(value))) if value == sentinel)
        );
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
                this_value: Value::Undefined,
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
                this_value: Value::Undefined,
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
