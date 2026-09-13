//! Capability construction roots its executor until the constructor reply is validated.
use super::{
    Completion, ContextId, NativeConversion, Phase, PromiseResume, PromiseStep,
    RootedPromiseCapability, Runtime, RuntimeError, Value,
};
use crate::engine::vm::call::ConstructorRef;

impl PromiseResume {
    pub(super) fn capability(
        self: Box<Self>,
        runtime: &Runtime,
        constructor: Option<ConstructorRef>,
    ) -> Result<PromiseStep, RuntimeError> {
        let Some(target) = constructor else {
            let capability = runtime.new_default_promise_capability(self.realm)?;
            return self.capability_ready(runtime, NativeConversion::Value(capability));
        };
        let executor = runtime.prepare_promise_capability_executor(self.realm)?;
        Ok(PromiseStep::Construct {
            target,
            arguments: vec![Value::Object(executor.as_object().clone())],
            resume: Box::new(Self {
                realm: self.realm,
                phase: Phase::Capability {
                    executor,
                    after: self,
                },
            }),
        })
    }

    pub(super) fn capability_ready(
        self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<RootedPromiseCapability>,
    ) -> Result<PromiseStep, RuntimeError> {
        let capability = match result {
            NativeConversion::Throw(value) => {
                return Ok(PromiseStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(capability) => capability,
        };
        match self.phase {
            Phase::AggregateCapability {
                constructor,
                iterable,
                kind,
            } => super::aggregate::ready(
                runtime,
                self.realm,
                constructor,
                iterable,
                kind,
                capability,
            ),
            Phase::ConvenienceCapability { kind, arguments } => {
                super::convenience::ready(runtime, self.realm, kind, arguments, capability)
            }
            Phase::StaticCapability { argument, kind } => {
                let target = if kind == crate::engine::builtins::native::PromiseNativeKind::Reject {
                    capability.reject
                } else {
                    capability.resolve
                };
                Ok(PromiseStep::Call {
                    callable: target,
                    receiver: Value::Undefined,
                    arguments: vec![argument],
                    resume: Box::new(Self {
                        realm: self.realm,
                        phase: Phase::ReturnPromise(capability.promise),
                    }),
                })
            }
            Phase::ThenCapability { promise, handlers } => handlers
                .finish(runtime, self.realm, promise, capability)
                .map(PromiseStep::Complete),
            _ => Err(RuntimeError::Invariant(
                "Promise capability reply has wrong phase",
            )),
        }
    }
}

pub(super) fn error(
    runtime: &Runtime,
    realm: ContextId,
    message: &'static str,
) -> Result<PromiseStep, RuntimeError> {
    Ok(PromiseStep::Complete(Completion::Throw(
        runtime.new_native_error(
            realm,
            crate::engine::api::error::NativeErrorKind::Type,
            message,
        )?,
    )))
}
