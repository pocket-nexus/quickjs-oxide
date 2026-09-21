//! Math argument conversion advances in source order while numerical kernels stay pure.
//!
//! Owned primitive calls borrow NativeActivation argv and allocate no Math argv.
//! At the first object, only the remaining suffix becomes continuation-owned;
//! NativeActivation retains the original arguments across callbacks. Profiling
//! distinguishes these two starts, without claiming native argument storage or
//! every primitive conversion is allocation-free.
use super::{quickjs_binary, quickjs_max, quickjs_min, quickjs_unary};

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{MathBinaryKind, MathMinMaxKind, MathUnaryKind},
    heap::ContextId,
    value::{JsValue, conversion::NativeConversion, number::operations::Number},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum MathKind {
    Unary(MathUnaryKind),
    Binary(MathBinaryKind),
    MinMax(MathMinMaxKind),
    Hypot,
    Imul,
    Clz32,
}
impl MathKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::MathUnary(kind) => Self::Unary(kind),
            NativeFunctionId::MathBinary(kind) => Self::Binary(kind),
            NativeFunctionId::MathMinMax(kind) => Self::MinMax(kind),
            NativeFunctionId::MathHypot => Self::Hypot,
            NativeFunctionId::MathImul => Self::Imul,
            NativeFunctionId::MathClz32 => Self::Clz32,
            _ => return None,
        })
    }
}
pub(crate) enum MathStep {
    Complete(Completion),
    Number { value: JsValue, resume: MathResume },
}
pub(crate) struct MathResume(Box<MathResumeState>);
impl std::ops::Deref for MathResume {
    type Target = MathResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for MathResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<MathResume>() <= 8);
pub(crate) struct MathResumeState {
    kind: MathKind,
    arguments: std::collections::VecDeque<JsValue>,
    owner: Option<Runtime>,
    result: Option<f64>,
    count: usize,
}
impl Drop for MathResumeState {
    fn drop(&mut self) {
        if let Some(runtime) = &self.owner {
            for value in self.arguments.drain(..) {
                let _ = runtime.release_jsvalue(value);
            }
        } else {
            debug_assert!(self.arguments.is_empty());
        }
    }
}
impl MathStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: MathKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant("Math requires generic invocation"));
        }
        let count = match kind {
            MathKind::Unary(_) | MathKind::Clz32 => 1,
            MathKind::Binary(_) | MathKind::Imul => 2,
            _ => arguments.actual_arg_count,
        };
        let values = arguments
            .readable
            .get(..count)
            .ok_or(RuntimeError::Invariant("Math argv was not padded"))?;

        {
            let mut resume = MathResumeState {
                kind,
                arguments: std::collections::VecDeque::new(),
                owner: None,
                result: None,
                count,
            };
            for (index, value) in values.iter().enumerate() {
                if matches!(value, JsValue::Object(_)) {
                    // The native activation owns original argv. A suspended
                    // continuation needs only the not-yet-converted suffix.
                    resume
                        .arguments
                        .try_reserve(values.len() - index)
                        .map_err(|_| {
                            RuntimeError::Invariant("Math remaining argv allocation failed")
                        })?;
                    resume.owner = Some(runtime.clone());
                    for remaining_value in &values[index..] {
                        resume
                            .arguments
                            .push_back(runtime.dup_jsvalue(remaining_value)?);
                    }
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "math_remaining_arguments_owned",
                    );
                    return MathResume(Box::new(resume)).next();
                }
                // Object arguments above retain the shared waiting protocol.
                // NativeActivation already owns this primitive: borrow it in
                // the same conversion kernel used by NumberStep completion.
                let result = runtime.number_from_primitive_jsvalue(realm, value)?;
                if let Some(completion) = resume.accept_number(runtime, result)? {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "math_completed_without_argument_storage",
                    );
                    return Ok(Self::Complete(completion));
                }
            }
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "math_completed_without_argument_storage",
            );
            resume.finish()
        }
    }
}
impl MathResume {
    fn next(mut self) -> Result<MathStep, RuntimeError> {
        if let Some(value) = self.0.arguments.pop_front() {
            return Ok(MathStep::Number {
                value,
                resume: self,
            });
        }
        self.0.finish()
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<MathStep, RuntimeError> {
        if let Some(completion) = self.accept_number(runtime, result)? {
            Ok(MathStep::Complete(completion))
        } else {
            self.next()
        }
    }
}
impl MathResumeState {
    fn finish(self) -> Result<MathStep, RuntimeError> {
        let value = match self.kind {
            MathKind::MinMax(kind) if self.result.is_none() => {
                return Ok(MathStep::Complete(Completion::Return(JsValue::Float(
                    match kind {
                        MathMinMaxKind::Min => f64::INFINITY,
                        MathMinMaxKind::Max => f64::NEG_INFINITY,
                    },
                ))));
            }
            MathKind::Hypot if self.count == 0 => {
                return Ok(MathStep::Complete(Completion::Return(JsValue::Int(0))));
            }
            MathKind::Hypot if self.count == 1 => self
                .result
                .ok_or(RuntimeError::Invariant("Math hypot result missing"))?
                .abs(),
            _ => self
                .result
                .ok_or(RuntimeError::Invariant("Math result missing"))?,
        };
        Ok(MathStep::Complete(Completion::Return(
            Number::compact(value).into(),
        )))
    }
    /// One numerical accumulation kernel for immediate and suspended inputs.
    fn accept_number(
        &mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<Option<Completion>, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Some(Completion::Throw(runtime.into_jsvalue(value)?)));
            }
        };
        self.result = Some(match self.kind {
            MathKind::Unary(kind) => quickjs_unary(kind, value),
            MathKind::Clz32 => {
                return Ok(Some(Completion::Return(JsValue::Int(
                    Runtime::to_uint32_number(value).leading_zeros() as i32,
                ))));
            }
            MathKind::Binary(kind) => {
                if let Some(left) = self.result {
                    quickjs_binary(kind, left, value)
                } else {
                    value
                }
            }
            MathKind::Imul => {
                if let Some(left) = self.result {
                    let product = Runtime::to_uint32_number(left)
                        .wrapping_mul(Runtime::to_uint32_number(value));
                    return Ok(Some(Completion::Return(JsValue::Int(i32::from_ne_bytes(
                        product.to_ne_bytes(),
                    )))));
                } else {
                    value
                }
            }
            MathKind::MinMax(kind) => {
                if let Some(left) = self.result {
                    if left.is_nan() {
                        left
                    } else if value.is_nan() {
                        value
                    } else {
                        match kind {
                            MathMinMaxKind::Min => quickjs_min(left, value),
                            MathMinMaxKind::Max => quickjs_max(left, value),
                        }
                    }
                } else {
                    value
                }
            }
            MathKind::Hypot => {
                if let Some(left) = self.result {
                    left.hypot(value)
                } else {
                    value
                }
            }
        });
        Ok(None)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: MathStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            MathStep::Complete(result) => return Ok(result),
            MathStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
        };
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests;

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<MathStep>() <= 64);
