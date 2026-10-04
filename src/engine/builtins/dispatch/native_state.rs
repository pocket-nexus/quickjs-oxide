//! Canonical C-function ABI validation and adaptation under held state.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    builtins::{
        continuation::NativeStep,
        native::{NativeCProto, NativeFunctionId},
    },
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
                NativeInvocationAdaptation::CyclePublishedComplete(completion) => {
                    NativeInvocationAdaptation::CyclePublishedComplete(completion)
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
                return Ok(NativeInvocationAdaptation::CyclePublishedComplete(
                    Completion::Throw(exception),
                ));
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
            NativeFunctionId::FunctionPrototypeCall
                | NativeFunctionId::FunctionPrototypeApply
                | NativeFunctionId::Reflect(
                    crate::engine::builtins::native::ReflectKind::Apply
                        | crate::engine::builtins::native::ReflectKind::Construct
                )
                | NativeFunctionId::ArrayPrototypePush(_)
                | NativeFunctionId::ArrayPrototypePop(_)
                | NativeFunctionId::NumberPredicate(_)
                | NativeFunctionId::MathRandom
                | NativeFunctionId::FunctionPrototype
                | NativeFunctionId::PrimitivePrototypeValueOf(_)
                | NativeFunctionId::PrimitivePrototypeToString(_)
                | NativeFunctionId::StringPrototypeCharAt(_)
                | NativeFunctionId::StringPrototypeCharCodeAt
                | NativeFunctionId::StringPrototypeCodePointAt
                | NativeFunctionId::StringPrototypeConcat
                | NativeFunctionId::StringPrototypeWellFormed(_)
                | NativeFunctionId::StringPrototypeIterator
                | NativeFunctionId::SymbolPrototypeDescription
                | NativeFunctionId::NumberPrototypeFormat(_)
                | NativeFunctionId::BigIntAsN(_)
                | NativeFunctionId::MathUnary(_)
                | NativeFunctionId::MathBinary(_)
                | NativeFunctionId::MathMinMax(_)
                | NativeFunctionId::MathHypot
                | NativeFunctionId::MathImul
                | NativeFunctionId::MathClz32
                | NativeFunctionId::Date(
                    crate::engine::builtins::native::DateNativeKind::Now
                        | crate::engine::builtins::native::DateNativeKind::Constructor
                        | crate::engine::builtins::native::DateNativeKind::Parse
                        | crate::engine::builtins::native::DateNativeKind::Utc
                        | crate::engine::builtins::native::DateNativeKind::TimeValue
                        | crate::engine::builtins::native::DateNativeKind::String(_)
                        | crate::engine::builtins::native::DateNativeKind::TimezoneOffset
                        | crate::engine::builtins::native::DateNativeKind::GetField(_)
                        | crate::engine::builtins::native::DateNativeKind::SetTime
                        | crate::engine::builtins::native::DateNativeKind::SetField(_)
                        | crate::engine::builtins::native::DateNativeKind::SetYear
                        | crate::engine::builtins::native::DateNativeKind::ToPrimitive
                        | crate::engine::builtins::native::DateNativeKind::ToJson
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
    ) -> Result<NativeStep, RuntimeError> {
        match self
            .adapt_native_invocation_borrowed(poisoned, target, realm, invocation, arguments)?
        {
            NativeInvocationAdaptation::Complete(completion) => {
                Ok(NativeStep::Complete(completion))
            }
            NativeInvocationAdaptation::CyclePublishedComplete(completion) => {
                Ok(NativeStep::CyclePublishedComplete(completion))
            }
            NativeInvocationAdaptation::Invoke(invocation) => {
                let result = self.dispatch_state_native_body(
                    poisoned,
                    host,
                    target,
                    realm,
                    invocation.as_ref(),
                    arguments,
                );
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                let retired = invocation.release_in_state(self, poisoned);
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                retired.and(result)
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
    ) -> Result<NativeStep, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_state_body");
        if let Some(kind) =
            crate::engine::builtins::function::invoke::InvokeKind::for_target(target)
        {
            return crate::engine::builtins::InvokeStep::start_in_state(
                self, poisoned, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::InvokeStep::Complete(completion) => {
                    NativeStep::Complete(completion)
                }
                step => NativeStep::Invoke(step),
            });
        }
        if let Some(kind) = crate::engine::builtins::ArrayMutationKind::for_target(target) {
            return crate::engine::builtins::ArrayMutationStep::start_in_state(
                self, poisoned, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::ArrayMutationStep::Complete(result) => {
                    NativeStep::Complete(result)
                }
                crate::engine::builtins::ArrayMutationStep::CyclePublished(result) => {
                    NativeStep::CyclePublishedComplete(result)
                }
                step => NativeStep::ArrayMutation(step),
            });
        }
        if let Some(kind) = crate::engine::builtins::math::operation::MathKind::for_target(target) {
            return crate::engine::builtins::MathStep::start_in_state(
                self, poisoned, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::MathStep::Complete(completion) => {
                    NativeStep::Complete(completion)
                }
                crate::engine::builtins::MathStep::CyclePublished(completion) => {
                    NativeStep::CyclePublishedComplete(completion)
                }
                step => NativeStep::Math(step),
            });
        }
        if let Some(kind) =
            crate::engine::builtins::primitive::numeric::NumericKind::for_target(target)
        {
            return crate::engine::builtins::NumericStep::start_in_state(
                self, poisoned, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::NumericStep::Complete(completion) => {
                    NativeStep::Complete(completion)
                }
                crate::engine::builtins::NumericStep::CyclePublished(completion) => {
                    NativeStep::CyclePublishedComplete(completion)
                }
                step => NativeStep::Numeric(step),
            });
        }
        if let Some(kind) =
            crate::engine::builtins::primitive::text::ScalarTextKind::for_target(target)
        {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "native_scalar_text_state_body",
            );
            return crate::engine::builtins::ScalarTextStep::start_in_state(
                self, poisoned, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::ScalarTextStep::Complete(completion) => {
                    NativeStep::Complete(completion)
                }
                crate::engine::builtins::ScalarTextStep::CyclePublished(completion) => {
                    NativeStep::CyclePublishedComplete(completion)
                }
                step => NativeStep::ScalarText(step),
            });
        }
        match target {
            NativeFunctionId::NumberPredicate(kind) => self
                .call_number_predicate(kind, invocation, arguments)
                .map(NativeStep::Complete),
            NativeFunctionId::MathRandom => self
                .call_math_random(realm, invocation)
                .map(NativeStep::Complete),
            NativeFunctionId::FunctionPrototype => {
                Ok(NativeStep::Complete(Completion::Return(JsValue::Undefined)))
            }
            NativeFunctionId::Date(
                kind @ (crate::engine::builtins::native::DateNativeKind::Constructor
                | crate::engine::builtins::native::DateNativeKind::Parse
                | crate::engine::builtins::native::DateNativeKind::Utc),
            ) => crate::engine::builtins::DateConstructorStep::start_in_state(
                self, poisoned, host, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::DateConstructorStep::Complete(result) => {
                    NativeStep::Complete(result)
                }
                step => NativeStep::DateConstructor(step),
            }),
            NativeFunctionId::Date(
                kind @ (crate::engine::builtins::native::DateNativeKind::SetTime
                | crate::engine::builtins::native::DateNativeKind::SetField(_)
                | crate::engine::builtins::native::DateNativeKind::SetYear
                | crate::engine::builtins::native::DateNativeKind::ToPrimitive
                | crate::engine::builtins::native::DateNativeKind::ToJson),
            ) => crate::engine::builtins::DatePrototypeStep::start_in_state(
                self, poisoned, host, realm, kind, invocation, arguments,
            )
            .map(|step| match step {
                crate::engine::builtins::DatePrototypeStep::Complete(result) => {
                    NativeStep::Complete(result)
                }
                step => NativeStep::DatePrototype(step),
            }),
            NativeFunctionId::Date(kind) => self.call_date_readonly_native_with_publication(
                poisoned, host, realm, kind, invocation,
            ),
            NativeFunctionId::PrimitivePrototypeValueOf(kind) => {
                self.call_primitive_prototype_value_of(poisoned, realm, kind, invocation)
            }
            NativeFunctionId::SymbolPrototypeDescription => {
                self.call_symbol_prototype_description(poisoned, realm, invocation)
            }
            _ => Err(RuntimeError::Invariant(
                "native body has not migrated to state",
            )),
        }
    }
}

impl crate::engine::api::runtime::Runtime {
    /// Public and unmigrated consumers cross actual Get/Call boundaries here.
    /// The semantic phase and numerical kernel remain the State algorithms.
    pub(crate) fn finish_state_native_body_step(
        &self,
        realm: ContextId,
        step: NativeStep,
    ) -> Result<Completion, RuntimeError> {
        match step {
            NativeStep::Complete(completion) => Ok(completion),
            NativeStep::CyclePublishedComplete(completion) => {
                let _unwind = self.unwind_guard();
                self.0.state.borrow_mut().service_native_completion(
                    &self.0.gc_pressure,
                    &self.0.poisoned,
                    completion,
                )
            }
            NativeStep::Invoke(step) => {
                crate::engine::builtins::function::invoke::finish(self, realm, step)
            }
            NativeStep::ArrayMutation(step) => {
                crate::engine::builtins::array::mutation::finish(self, realm, step)
            }
            NativeStep::Math(step) => {
                crate::engine::builtins::math::operation::finish(self, realm, step)
            }
            NativeStep::ScalarText(step) => {
                crate::engine::builtins::primitive::text::finish(self, realm, step)
            }
            NativeStep::DateConstructor(step) => {
                crate::engine::builtins::date::finish_constructor_operation(self, realm, step)
            }
            NativeStep::DatePrototype(step) => {
                crate::engine::builtins::date::finish_prototype_operation(self, realm, step)
            }
            NativeStep::Numeric(step) => {
                crate::engine::builtins::primitive::numeric::finish(self, realm, step)
            }
            _ => Err(RuntimeError::Invariant(
                "State native body returned an unmigrated domain",
            )),
        }
    }
}

impl RuntimeState {
    /// Service a terminal producer's actual publication with its result armed.
    /// The caller keeps the native input/argv/callee and descriptor owners live
    /// until this returns, then uses the existing ordered activation finisher.
    pub(crate) fn service_native_completion(
        &mut self,
        pressure: &crate::engine::heap::gc_pressure::GcPressure,
        poisoned: &std::cell::Cell<bool>,
        completion: Completion,
    ) -> Result<Completion, RuntimeError> {
        let (value, thrown) = match completion {
            Completion::Return(value) => (value, false),
            Completion::Throw(value) => (value, true),
        };
        let mut owner =
            crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(self, poisoned, value);
        let (state, owner) = owner.parts();
        state.collect_if_requested(pressure, poisoned)?;
        let value = owner.take().expect("published native completion");
        Ok(if thrown {
            Completion::Throw(value)
        } else {
            Completion::Return(value)
        })
    }
}
