//! Capability creation precedes callback validation, including Promise.try.
use super::{Phase, PromiseResume, PromiseStep, capability};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::{native::PromiseNativeKind, promise::RootedPromiseCapability};
use crate::engine::heap::ContextId;
use crate::engine::object::ObjectRef;
use crate::engine::value::{JsValue, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{NativeArguments, NativeInvocation},
};

impl PromiseStep {
    pub(in crate::engine::builtins::promise) fn convenience(
        runtime: &Runtime,
        realm: ContextId,
        kind: PromiseNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise convenience received constructor invocation",
            ));
        };
        let JsValue::Object(object_id) = this_value else {
            return capability::error(runtime, realm, "not an object");
        };
        let object = ObjectRef::from_borrowed_handle(runtime.clone(), *object_id)?;
        let constructor =
            match runtime.constructor_from_jsvalue(realm, JsValue::Object(object.into_handle()))? {
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
                NativeConversion::Value(constructor) => constructor,
            };
        let mut resume = Box::new(PromiseResume {
            runtime: runtime.clone(),
            pending_effect: super::PromiseStepPending::default(),
            realm,
            phase: Phase::ConvenienceCapability {
                kind,
                arguments: NativeArguments {
                    readable: Vec::new(),
                    actual_arg_count: arguments.actual_arg_count,
                },
            },
        });
        let Phase::ConvenienceCapability {
            arguments: owned, ..
        } = &mut resume.phase
        else {
            unreachable!();
        };
        owned
            .readable
            .try_reserve_exact(arguments.readable.len())
            .map_err(|_| RuntimeError::Invariant("Promise convenience argv allocation failed"))?;
        for value in &arguments.readable {
            owned.readable.push(runtime.dup_jsvalue(value)?);
        }
        resume.capability(runtime, Some(constructor))
    }
}

pub(super) fn ready(
    runtime: &Runtime,
    realm: ContextId,
    kind: PromiseNativeKind,
    arguments: NativeArguments,
    capability: RootedPromiseCapability,
) -> Result<PromiseStep, RuntimeError> {
    let NativeArguments {
        actual_arg_count,
        readable,
    } = arguments;
    if kind == PromiseNativeKind::WithResolvers {
        for value in readable {
            runtime.release_jsvalue(value)?;
        }
        let RootedPromiseCapability {
            promise,
            resolve,
            reject,
        } = capability;
        let result = runtime.new_ordinary_object_in_realm(realm)?;
        for (name, object) in [
            ("promise", promise),
            ("resolve", resolve.as_object().clone()),
            ("reject", reject.as_object().clone()),
        ] {
            runtime.define_fresh_promise_property(
                &result,
                name,
                JsValue::Object(object.into_handle()),
                "fresh Promise.withResolvers result rejected a data property",
            )?;
        }
        return Ok(PromiseStep::Complete(Completion::Return(JsValue::Object(
            result.into_handle(),
        ))));
    }

    let outcome = match readable.first() {
        Some(callback) => runtime.promise_callable(realm, callback),
        None => Err(RuntimeError::Invariant(
            "Promise.try callback argv was not padded",
        )),
    };
    let callable = match outcome {
        Ok(NativeConversion::Value(callable)) => callable,
        other => {
            for value in readable {
                let _ = runtime.release_jsvalue(value);
            }
            return match other {
                Ok(NativeConversion::Throw(reason)) => {
                    settle(runtime, realm, capability, Completion::Throw(reason))
                }
                Err(error) => Err(error),
                Ok(NativeConversion::Value(_)) => unreachable!(),
            };
        }
    };
    // Consume the saved argv instead of duplicating the callback suffix again.
    let mut call_arguments = readable;
    if !call_arguments.is_empty() {
        runtime.release_jsvalue(call_arguments.remove(0))?;
    }
    for value in call_arguments.drain(actual_arg_count.saturating_sub(1)..) {
        runtime.release_jsvalue(value)?;
    }
    Ok({
        let __pending_field_callable = callable;
        let __pending_field_receiver = JsValue::Undefined;
        let __pending_field_arguments = call_arguments;
        let __pending_field_resume = Box::new(PromiseResume {
            runtime: runtime.clone(),
            pending_effect: super::PromiseStepPending::default(),
            realm,
            phase: Phase::TryCallback(capability),
        });
        PromiseStep::request_call(
            __pending_field_callable,
            __pending_field_receiver,
            __pending_field_arguments,
            __pending_field_resume,
        )
    })
}

pub(super) fn settle(
    runtime: &Runtime,
    realm: ContextId,
    capability: RootedPromiseCapability,
    completion: Completion,
) -> Result<PromiseStep, RuntimeError> {
    let (callable, value) = match completion {
        Completion::Return(value) => (capability.resolve, value),
        Completion::Throw(value) => (capability.reject, value),
    };
    Ok({
        let __pending_field_callable = callable;
        let __pending_field_receiver = JsValue::Undefined;
        let __pending_field_arguments = vec![value];
        let __pending_field_resume = Box::new(PromiseResume {
            runtime: runtime.clone(),
            pending_effect: super::PromiseStepPending::default(),
            realm,
            phase: Phase::ReturnPromise(capability.promise),
        });
        PromiseStep::request_call(
            __pending_field_callable,
            __pending_field_receiver,
            __pending_field_arguments,
            __pending_field_resume,
        )
    })
}
