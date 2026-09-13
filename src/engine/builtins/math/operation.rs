//! Math argument conversion advances in source order while numerical kernels stay pure.
use super::{quickjs_binary, quickjs_max, quickjs_min, quickjs_unary};
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{MathBinaryKind, MathMinMaxKind, MathUnaryKind},
    heap::ContextId,
    value::{Value, conversion::NativeConversion},
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
    #[cfg(feature = "stack-vm")]
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
    Number { value: Value, resume: MathResume },
}
pub(crate) struct MathResume {
    kind: MathKind,
    arguments: std::vec::IntoIter<Value>,
    result: Option<f64>,
    count: usize,
}
impl MathStep {
    pub(crate) fn start(
        _runtime: &Runtime,
        _realm: ContextId,
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
            .ok_or(RuntimeError::Invariant("Math argv was not padded"))?
            .to_vec();
        MathResume {
            kind,
            arguments: values.into_iter(),
            result: None,
            count,
        }
        .next()
    }
}
impl MathResume {
    fn next(mut self) -> Result<MathStep, RuntimeError> {
        if let Some(value) = self.arguments.next() {
            return Ok(MathStep::Number {
                value,
                resume: self,
            });
        }
        let value = match self.kind {
            MathKind::MinMax(kind) if self.result.is_none() => {
                return Ok(MathStep::Complete(Completion::Return(Value::Float(
                    match kind {
                        MathMinMaxKind::Min => f64::INFINITY,
                        MathMinMaxKind::Max => f64::NEG_INFINITY,
                    },
                ))));
            }
            MathKind::Hypot if self.count == 0 => {
                return Ok(MathStep::Complete(Completion::Return(Value::Int(0))));
            }
            MathKind::Hypot if self.count == 1 => self
                .result
                .ok_or(RuntimeError::Invariant("Math hypot result missing"))?
                .abs(),
            _ => self
                .result
                .ok_or(RuntimeError::Invariant("Math result missing"))?,
        };
        Ok(MathStep::Complete(Completion::Return(Value::number(value))))
    }
    pub(crate) fn number(
        mut self,
        result: NativeConversion<f64>,
    ) -> Result<MathStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(MathStep::Complete(Completion::Throw(value)));
            }
        };
        self.result = Some(match self.kind {
            MathKind::Unary(kind) => quickjs_unary(kind, value),
            MathKind::Clz32 => {
                return Ok(MathStep::Complete(Completion::Return(Value::Int(
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
                    return Ok(MathStep::Complete(Completion::Return(Value::Int(
                        i32::from_ne_bytes(product.to_ne_bytes()),
                    ))));
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
        self.next()
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
                resume.number(runtime.native_to_number(realm, &value)?)?
            }
        };
    }
}
