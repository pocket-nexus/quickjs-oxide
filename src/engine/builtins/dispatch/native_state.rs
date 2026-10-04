//! Canonical C-function ABI validation and adaptation under held state.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    builtins::native::{NativeCProto, NativeFunctionId},
    heap::{ContextId, runtime::RuntimeState},
    value::JsValue,
    vm::{
        Completion,
        call::{
            AdaptedNativeInvocation, NativeArguments, NativeInvocation, NativeInvocationAdaptation,
        },
        frames::ActiveFrameKind,
    },
};

fn native_invocation_input(invocation: NativeInvocation) -> JsValue {
    match invocation {
        NativeInvocation::Call { this_value }
        | NativeInvocation::Getter { this_value }
        | NativeInvocation::Setter { this_value } => this_value,
        NativeInvocation::Construct { new_target } => new_target,
    }
}

impl RuntimeState {
    pub(crate) fn adapt_native_invocation_borrowed<'a>(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        target: NativeFunctionId,
        realm: ContextId,
        invocation: &'a NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<NativeInvocationAdaptation<AdaptedNativeInvocation<'a>>, RuntimeError> {
        self.validate_native_invocation(target, realm, arguments)?;
        let unchanged = matches!(
            (target.descriptor().cproto, invocation),
            (
                NativeCProto::Generic
                    | NativeCProto::GenericMagic
                    | NativeCProto::UnaryF64
                    | NativeCProto::BinaryF64
                    | NativeCProto::IteratorNext,
                NativeInvocation::Call { .. },
            ) | (
                NativeCProto::Constructor
                    | NativeCProto::ConstructorMagic
                    | NativeCProto::ConstructorOrFunction
                    | NativeCProto::ConstructorOrFunctionMagic,
                NativeInvocation::Construct { .. },
            )
        );
        if unchanged {
            return Ok(NativeInvocationAdaptation::Invoke(
                AdaptedNativeInvocation::Borrowed(invocation),
            ));
        }
        let owned = invocation.dup_in_state(self)?;
        Ok(
            match self.adapt_native_invocation(poisoned, target, realm, owned, arguments)? {
                NativeInvocationAdaptation::Invoke(invocation) => {
                    NativeInvocationAdaptation::Invoke(AdaptedNativeInvocation::Owned(invocation))
                }
                NativeInvocationAdaptation::Complete(completion) => {
                    NativeInvocationAdaptation::Complete(completion)
                }
            },
        )
    }

    fn validate_native_invocation(
        &self,
        target: NativeFunctionId,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<(), RuntimeError> {
        let frame = self
            .active_frames
            .last()
            .copied()
            .ok_or(RuntimeError::Invariant(
                "native handler ran without an active frame",
            ))?;
        let ActiveFrameKind::Native {
            target: frame_target,
            actual_arg_count,
            readable_arg_count,
        } = frame.kind
        else {
            return Err(RuntimeError::Invariant(
                "native handler was not the top active frame",
            ));
        };
        if frame.realm != realm
            || frame_target != target
            || actual_arg_count != arguments.actual_arg_count
            || readable_arg_count != arguments.readable.len()
        {
            return Err(RuntimeError::Invariant(
                "active native frame disagrees with handler arguments",
            ));
        }
        Ok(())
    }

    pub(crate) fn adapt_native_invocation(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        target: NativeFunctionId,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<NativeInvocationAdaptation, RuntimeError> {
        if let Err(error) = self.validate_native_invocation(target, realm, arguments) {
            let _ = invocation.release_in_state(self, poisoned);
            return Err(error);
        }
        // Some handlers do not inspect their adapted this/new-target input,
        // but keeping it rooted for the full dispatch is part of the ABI.
        let invocation = match (target.descriptor().cproto, invocation) {
            (
                NativeCProto::Generic
                | NativeCProto::GenericMagic
                | NativeCProto::UnaryF64
                | NativeCProto::BinaryF64,
                invocation @ NativeInvocation::Call { .. },
            ) => invocation,
            (
                NativeCProto::Generic
                | NativeCProto::GenericMagic
                | NativeCProto::UnaryF64
                | NativeCProto::BinaryF64,
                invocation @ NativeInvocation::Construct { .. },
            ) => {
                // QuickJS's generic and floating-point ABIs receive
                // new.target in their receiver slot when an embedding
                // independently enables the constructor bit on the native
                // function object. Floating-point argument conversion stays
                // in the handler so abrupt completions keep their defining
                // realm and left-to-right order.
                NativeInvocation::Call {
                    this_value: native_invocation_input(invocation),
                }
            }
            (
                NativeCProto::Constructor | NativeCProto::ConstructorMagic,
                invocation @ NativeInvocation::Construct { .. },
            ) => invocation,
            (
                NativeCProto::Constructor | NativeCProto::ConstructorMagic,
                NativeInvocation::Call { this_value },
            ) => {
                self.release_owned_jsvalue(poisoned, this_value)?;
                let exception = JsValue::Object(self.new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("must be called with new"),
                )?);
                return Ok(NativeInvocationAdaptation::Complete(Completion::Throw(
                    exception,
                )));
            }
            (
                NativeCProto::ConstructorOrFunction | NativeCProto::ConstructorOrFunctionMagic,
                NativeInvocation::Call { this_value },
            ) => {
                self.release_owned_jsvalue(poisoned, this_value)?;
                NativeInvocation::Construct {
                    new_target: crate::engine::value::JsValue::Undefined,
                }
            }
            (
                NativeCProto::ConstructorOrFunction | NativeCProto::ConstructorOrFunctionMagic,
                invocation @ NativeInvocation::Construct { .. },
            ) => invocation,
            (
                NativeCProto::Getter | NativeCProto::GetterMagic,
                invocation @ (NativeInvocation::Call { .. } | NativeInvocation::Construct { .. }),
            ) => NativeInvocation::Getter {
                this_value: native_invocation_input(invocation),
            },
            (
                NativeCProto::Setter | NativeCProto::SetterMagic,
                invocation @ (NativeInvocation::Call { .. } | NativeInvocation::Construct { .. }),
            ) => NativeInvocation::Setter {
                this_value: native_invocation_input(invocation),
            },
            (NativeCProto::IteratorNext, invocation @ NativeInvocation::Call { .. }) => invocation,
            (NativeCProto::IteratorNext, invocation @ NativeInvocation::Construct { .. }) => {
                // Iterator-next functions are non-constructors by default.
                // If an embedder independently enables [[Construct]], QuickJS
                // passes new.target through the same native receiver slot.
                NativeInvocation::Call {
                    this_value: native_invocation_input(invocation),
                }
            }
            (
                _,
                invocation @ (NativeInvocation::Getter { .. } | NativeInvocation::Setter { .. }),
            ) => {
                invocation.release_in_state(self, poisoned)?;
                return Err(RuntimeError::Invariant(
                    "native invocation was adapted more than once",
                ));
            }
        };
        Ok(NativeInvocationAdaptation::Invoke(invocation))
    }
}

impl RuntimeState {
    /// This is a whole registered body-family decision. It is independent of
    /// receiver/argv representations and never tests an input eligibility path.
    pub(crate) fn has_state_native_body(target: NativeFunctionId) -> bool {
        matches!(
            target,
            NativeFunctionId::NumberPredicate(_)
                | NativeFunctionId::MathRandom
                | NativeFunctionId::FunctionPrototype
                | NativeFunctionId::Date(
                    crate::engine::builtins::native::DateNativeKind::Now
                        | crate::engine::builtins::native::DateNativeKind::TimeValue
                        | crate::engine::builtins::native::DateNativeKind::String(_)
                        | crate::engine::builtins::native::DateNativeKind::TimezoneOffset
                        | crate::engine::builtins::native::DateNativeKind::GetField(_)
                )
        )
    }
    pub(crate) fn invoke_state_native_body(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        host: &dyn crate::engine::host::HostServices,
        target: NativeFunctionId,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        match self
            .adapt_native_invocation_borrowed(poisoned, target, realm, invocation, arguments)?
        {
            NativeInvocationAdaptation::Complete(completion) => Ok(completion),
            NativeInvocationAdaptation::Invoke(invocation) => {
                let result = self.dispatch_state_native_body(
                    poisoned,
                    host,
                    target,
                    realm,
                    invocation.as_ref(),
                    arguments,
                );
                invocation.release_in_state(self, poisoned).and(result)
            }
        }
    }
    pub(crate) fn dispatch_state_native_body(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        host: &dyn crate::engine::host::HostServices,
        target: NativeFunctionId,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_state_body");
        match target {
            NativeFunctionId::NumberPredicate(kind) => {
                self.call_number_predicate(kind, invocation, arguments)
            }
            NativeFunctionId::MathRandom => self.call_math_random(realm, invocation),
            NativeFunctionId::FunctionPrototype => Ok(Completion::Return(JsValue::Undefined)),
            NativeFunctionId::Date(kind) => {
                self.call_date_readonly_native(poisoned, host, realm, kind, invocation)
            }
            _ => Err(RuntimeError::Invariant(
                "native body has not migrated to state",
            )),
        }
    }
}
