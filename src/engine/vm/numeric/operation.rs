//! Numeric operators retain ordered conversion operands across JavaScript callbacks.
use super::{
    NumericValue, add_primitives, bigint_error, bigint_value_payload, compare_bigint_number,
    jsvalue_number, mixed_numeric_type_error, number_to_int32, number_to_uint32, string_payload,
    string_to_bigint, to_number_jsvalue, to_numeric_primitive, unary_plus_primitive,
};
use crate::engine::{
    api::runtime::Runtime,
    api::{Error, ErrorKind},
    code::exec_opcode::Opcode,
    value::JsValue,
    vm::{Completion, ToPrimitiveHint},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::vm) enum NumericKind {
    Neg,
    Plus,
    BitNot,
    Inc,
    Dec,
    PostInc,
    PostDec,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Shl,
    Sar,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}
impl NumericKind {
    pub(in crate::engine::vm) fn for_opcode(opcode: Opcode) -> Option<Self> {
        Some(match opcode {
            Opcode::Neg => Self::Neg,
            Opcode::Plus => Self::Plus,
            Opcode::BitNot => Self::BitNot,
            Opcode::Inc => Self::Inc,
            Opcode::Dec => Self::Dec,
            Opcode::PostInc => Self::PostInc,
            Opcode::PostDec => Self::PostDec,
            Opcode::Add => Self::Add,
            Opcode::Sub => Self::Sub,
            Opcode::Mul => Self::Mul,
            Opcode::Div => Self::Div,
            Opcode::Mod => Self::Mod,
            Opcode::Pow => Self::Pow,
            Opcode::Shl => Self::Shl,
            Opcode::Sar => Self::Sar,
            Opcode::Shr => Self::Shr,
            Opcode::BitAnd => Self::BitAnd,
            Opcode::BitOr => Self::BitOr,
            Opcode::BitXor => Self::BitXor,
            Opcode::Eq => Self::Eq,
            Opcode::Neq => Self::Neq,
            Opcode::Lt => Self::Lt,
            Opcode::Lte => Self::Lte,
            Opcode::Gt => Self::Gt,
            Opcode::Gte => Self::Gte,
            _ => return None,
        })
    }

    pub(in crate::engine::vm) fn unary(self) -> bool {
        matches!(
            self,
            Self::Neg
                | Self::Plus
                | Self::BitNot
                | Self::Inc
                | Self::Dec
                | Self::PostInc
                | Self::PostDec
        )
    }
    pub(in crate::engine::vm) fn primitive_arithmetic(self) -> bool {
        !self.comparison() && !matches!(self, Self::Eq | Self::Neq)
    }
    fn comparison(self) -> bool {
        matches!(self, Self::Lt | Self::Lte | Self::Gt | Self::Gte)
    }
}
pub(in crate::engine::vm) struct NumericOutput {
    pub value: JsValue,
    pub previous: Option<JsValue>,
}
impl NumericOutput {
    fn value(value: JsValue) -> Self {
        Self {
            value,
            previous: None,
        }
    }
    fn into_step(self) -> NumericStep {
        NumericStep::Complete {
            value: self.value,
            previous: self.previous,
        }
    }
}

/// Primitive arithmetic uses the same conversion and operator kernels as resumes.
/// Parsing, allocation and final primitive-owner release require an ended
/// FrameSlots borrow; the resident run helper is also such an owning boundary.
pub(in crate::engine::vm) fn primitive_output(
    runtime: &Runtime,
    kind: NumericKind,
    left: JsValue,
    right: Option<JsValue>,
) -> Result<NumericOutput, Error> {
    if kind.unary() {
        return unary_output(runtime, kind, left);
    }
    let right = right.ok_or_else(|| Error::internal("binary numeric operator lost RHS"))?;
    if kind == NumericKind::Add {
        return add_primitives(runtime, left, right).map(NumericOutput::value);
    }
    if left.is_bigint() && right.is_bigint() {
        let result = super::with_bigint_operands(runtime, &left, &right, |left, right| {
            bigint_binary(kind, left, right)
        });
        return super::finish_bigint_operands(runtime, left, right, result)
            .map(NumericOutput::value);
    }
    let converted_left = to_numeric_primitive(runtime, &left);
    let converted_right = to_numeric_primitive(runtime, &right);
    super::release_primitive_operand(runtime, left)?;
    super::release_primitive_operand(runtime, right)?;
    binary(runtime, kind, converted_left?, converted_right?).map(NumericOutput::value)
}

pub(in crate::engine::vm) enum NumericStep {
    Complete {
        value: JsValue,
        previous: Option<JsValue>,
    },
    Throw(JsValue),
    Primitive {
        value: JsValue,
        hint: ToPrimitiveHint,
        resume: NumericResume,
    },
}
pub(in crate::engine::vm) struct NumericResume(Box<NumericResumeState>);
impl std::ops::Deref for NumericResume {
    type Target = NumericResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for NumericResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<NumericResume>() <= 8);
pub(in crate::engine::vm) struct NumericResumeState {
    kind: NumericKind,
    phase: Phase,
    runtime: Runtime,
}
impl Drop for NumericResumeState {
    fn drop(&mut self) {
        let owned = match &mut self.phase {
            Phase::Unary | Phase::RightNumeric(_) => return,
            Phase::Left(value)
            | Phase::RightPrimitive(value)
            | Phase::EqualityLeft(value)
            | Phase::EqualityRight(value) => std::mem::replace(value, JsValue::Undefined),
        };
        drop_owned(&self.runtime, owned);
    }
}
fn drop_owned(runtime: &Runtime, value: JsValue) {
    let _ = runtime.release_jsvalue(value);
}
enum Phase {
    Unary,
    Left(JsValue),
    RightPrimitive(JsValue),
    RightNumeric(NumericValue),
    EqualityLeft(JsValue),
    EqualityRight(JsValue),
}
impl NumericStep {
    pub(in crate::engine::vm) fn start(
        runtime: &Runtime,
        kind: NumericKind,
        left: JsValue,
        right: Option<JsValue>,
    ) -> Result<Self, Error> {
        if kind.unary() {
            return primitive(
                runtime,
                left,
                ToPrimitiveHint::Number,
                NumericResume(Box::new(NumericResumeState {
                    kind,
                    phase: Phase::Unary,
                    runtime: runtime.clone(),
                })),
            );
        }
        let right = right.ok_or_else(|| Error::internal("binary numeric operator lost RHS"))?;
        if matches!(kind, NumericKind::Eq | NumericKind::Neq) {
            return equality(runtime, kind, left, right);
        }
        primitive(
            runtime,
            left,
            if kind == NumericKind::Add {
                ToPrimitiveHint::Default
            } else {
                ToPrimitiveHint::Number
            },
            NumericResume(Box::new(NumericResumeState {
                kind,
                phase: Phase::Left(right),
                runtime: runtime.clone(),
            })),
        )
    }
}
fn primitive(
    runtime: &Runtime,
    value: JsValue,
    hint: ToPrimitiveHint,
    resume: NumericResume,
) -> Result<NumericStep, Error> {
    if matches!(value, JsValue::Object(_)) {
        Ok(NumericStep::Primitive {
            value,
            hint,
            resume,
        })
    } else {
        resume.resume(runtime, Completion::Return(value))
    }
}
fn complete(value: JsValue) -> NumericStep {
    NumericStep::Complete {
        value,
        previous: None,
    }
}
impl NumericResume {
    pub(in crate::engine::vm) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<NumericStep, Error> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(NumericStep::Throw(value)),
        };
        if matches!(value, JsValue::Object(_)) {
            return Err(Error::internal(
                "numeric ToPrimitive reply returned an object",
            ));
        }
        let kind = self.0.kind;
        let phase = std::mem::replace(&mut self.0.phase, Phase::Unary);
        match phase {
            Phase::Unary => unary(runtime, kind, value),
            Phase::Left(right) => {
                // Arithmetic converts the left primitive to Numeric before starting
                // the right callback. Relational comparison converts both primitives first.
                let hint = if kind == NumericKind::Add {
                    ToPrimitiveHint::Default
                } else {
                    ToPrimitiveHint::Number
                };
                if kind == NumericKind::Add || kind.comparison() {
                    return primitive(
                        runtime,
                        right,
                        hint,
                        NumericResume(Box::new(NumericResumeState {
                            kind,
                            phase: Phase::RightPrimitive(value),
                            runtime: runtime.clone(),
                        })),
                    );
                }
                let converted = to_numeric_primitive(runtime, &value);
                if let Err(error) = super::release_primitive_operand(runtime, value) {
                    super::release_primitive_operand(runtime, right)?;
                    return Err(error);
                }
                let converted = match converted {
                    Ok(converted) => converted,
                    Err(error) => {
                        super::release_primitive_operand(runtime, right)?;
                        return Err(error);
                    }
                };
                primitive(
                    runtime,
                    right,
                    hint,
                    NumericResume(Box::new(NumericResumeState {
                        kind,
                        phase: Phase::RightNumeric(converted),
                        runtime: runtime.clone(),
                    })),
                )
            }
            Phase::RightPrimitive(left) => {
                if kind == NumericKind::Add {
                    return Ok(complete(add_primitives(runtime, left, value)?));
                }
                let compared = compare(runtime, kind, &left, &value);
                super::release_primitive_operand(runtime, left)?;
                super::release_primitive_operand(runtime, value)?;
                Ok(complete(JsValue::Bool(compared?)))
            }
            Phase::RightNumeric(left) => {
                let converted = to_numeric_primitive(runtime, &value);
                super::release_primitive_operand(runtime, value)?;
                Ok(complete(binary(runtime, kind, left, converted?)?))
            }
            Phase::EqualityLeft(right) => equality(runtime, kind, value, right),
            Phase::EqualityRight(left) => equality(runtime, kind, left, value),
        }
    }
}
fn unary(runtime: &Runtime, kind: NumericKind, value: JsValue) -> Result<NumericStep, Error> {
    unary_output(runtime, kind, value).map(NumericOutput::into_step)
}
fn unary_output(
    runtime: &Runtime,
    kind: NumericKind,
    value: JsValue,
) -> Result<NumericOutput, Error> {
    if kind == NumericKind::Plus {
        return Ok(NumericOutput::value(unary_plus_primitive(runtime, value)?));
    }
    if kind == NumericKind::Neg {
        if let Some(number) = value.as_number_repr() {
            return Ok(NumericOutput::value(match number.negate() {
                crate::engine::value::number::operations::Number::Int(value) => JsValue::Int(value),
                crate::engine::value::number::operations::Number::Float(value) => {
                    JsValue::Float(value)
                }
            }));
        }
        let result = match &value {
            bigint if bigint.is_bigint() => bigint_value_payload(runtime, bigint)
                .and_then(|payload| payload.neg().map_err(bigint_error))
                .and_then(|payload| super::allocate_bigint_jsvalue(runtime, payload)),
            other => to_number_jsvalue(runtime, other).map(|number| jsvalue_number(-number)),
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                super::release_primitive_operand(runtime, value)?;
                return Err(error);
            }
        };
        super::release_primitive_operand(runtime, value)?;
        return Ok(NumericOutput::value(result));
    }
    if kind == NumericKind::BitNot {
        let result = match to_numeric_primitive(runtime, &value) {
            Ok(NumericValue::BigInt(payload)) => payload
                .bit_not()
                .map_err(bigint_error)
                .and_then(|payload| super::allocate_bigint_jsvalue(runtime, payload)),
            Ok(NumericValue::Number(number)) => Ok(JsValue::Int(!number_to_int32(number))),
            Err(error) => Err(error),
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                super::release_primitive_operand(runtime, value)?;
                return Err(error);
            }
        };
        super::release_primitive_operand(runtime, value)?;
        return Ok(NumericOutput::value(result));
    }
    let increment = matches!(kind, NumericKind::Inc | NumericKind::PostInc);
    let postfix = matches!(kind, NumericKind::PostInc | NumericKind::PostDec);
    if let Some(number) = value.as_number_repr() {
        return Ok(NumericOutput {
            value: super::jsvalue_from_number(number.update(increment)),
            previous: postfix.then_some(value),
        });
    }
    match value {
        value if value.is_bigint() => {
            let result = bigint_value_payload(runtime, &value)
                .and_then(|payload| {
                    if increment {
                        payload.add(&crate::engine::value::bigint::JsBigInt::from(1_i32))
                    } else {
                        payload.update_decrement()
                    }
                    .map_err(bigint_error)
                })
                .and_then(|payload| super::allocate_bigint_jsvalue(runtime, payload));
            let next = match result {
                Ok(next) => next,
                Err(error) => {
                    super::release_primitive_operand(runtime, value)?;
                    return Err(error);
                }
            };
            let old = value;
            let previous = if postfix {
                Some(old)
            } else {
                super::release_primitive_operand(runtime, old)?;
                None
            };
            Ok(NumericOutput {
                value: next,
                previous,
            })
        }
        value => {
            let old = match to_number_jsvalue(runtime, &value) {
                Ok(old) => old,
                Err(error) => {
                    super::release_primitive_operand(runtime, value)?;
                    return Err(error);
                }
            };
            super::release_primitive_operand(runtime, value)?;
            Ok(NumericOutput {
                value: jsvalue_number(if increment { old + 1.0 } else { old - 1.0 }),
                previous: postfix.then(|| jsvalue_number(old)),
            })
        }
    }
}
fn bigint_binary(
    kind: NumericKind,
    left: &crate::engine::value::bigint::JsBigInt,
    right: &crate::engine::value::bigint::JsBigInt,
) -> Result<crate::engine::value::bigint::JsBigInt, Error> {
    match kind {
        NumericKind::Sub => left.sub(right),
        NumericKind::Mul => left.mul(right),
        NumericKind::Div => left.div(right),
        NumericKind::Mod => left.rem(right),
        NumericKind::Pow => left.pow(right),
        NumericKind::Shl => left.shl(right),
        NumericKind::Sar => left.shr(right),
        NumericKind::BitAnd => left.bit_and(right),
        NumericKind::BitOr => left.bit_or(right),
        NumericKind::BitXor => left.bit_xor(right),
        NumericKind::Shr => {
            return Err(Error::new(
                ErrorKind::Type,
                "bigint operands are forbidden for >>>",
            ));
        }
        _ => {
            return Err(Error::internal(
                "non-arithmetic operator entered binary Numeric",
            ));
        }
    }
    .map_err(bigint_error)
}
fn binary(
    runtime: &Runtime,
    kind: NumericKind,
    left: NumericValue,
    right: NumericValue,
) -> Result<JsValue, Error> {
    if kind == NumericKind::Shr {
        let (NumericValue::Number(left), NumericValue::Number(right)) = (left, right) else {
            return Err(Error::new(
                ErrorKind::Type,
                "bigint operands are forbidden for >>>",
            ));
        };
        return Ok(jsvalue_number(f64::from(
            number_to_uint32(left) >> (number_to_uint32(right) & 0x1f),
        )));
    }
    Ok(match (left, right) {
        (NumericValue::BigInt(left), NumericValue::BigInt(right)) => {
            super::allocate_bigint_jsvalue(runtime, bigint_binary(kind, &left, &right)?)?
        }
        (NumericValue::Number(left), NumericValue::Number(right)) => jsvalue_number(match kind {
            NumericKind::Sub => left - right,
            NumericKind::Mul => left * right,
            NumericKind::Div => left / right,
            NumericKind::Mod => left % right,
            NumericKind::Pow => crate::engine::value::number::pow(left, right),
            NumericKind::Shl => {
                f64::from(number_to_int32(left).wrapping_shl(number_to_uint32(right) & 0x1f))
            }
            NumericKind::Sar => {
                f64::from(number_to_int32(left) >> (number_to_uint32(right) & 0x1f))
            }
            NumericKind::BitAnd => f64::from(number_to_int32(left) & number_to_int32(right)),
            NumericKind::BitOr => f64::from(number_to_int32(left) | number_to_int32(right)),
            NumericKind::BitXor => f64::from(number_to_int32(left) ^ number_to_int32(right)),
            _ => {
                return Err(Error::internal(
                    "non-arithmetic operator entered binary Numeric",
                ));
            }
        }),
        _ => return Err(mixed_numeric_type_error()),
    })
}
fn compare(
    runtime: &Runtime,
    kind: NumericKind,
    left: &JsValue,
    right: &JsValue,
) -> Result<bool, Error> {
    let ordering = match (left, right) {
        (JsValue::String(left), JsValue::String(right)) => {
            let left = string_payload(runtime, *left)?;
            let right = string_payload(runtime, *right)?;
            Some(left.utf16_units().cmp(right.utf16_units()))
        }
        (left, right) if left.is_bigint() && right.is_bigint() => {
            Some(bigint_value_payload(runtime, left)?.cmp(&bigint_value_payload(runtime, right)?))
        }
        (left, JsValue::String(right)) if left.is_bigint() => {
            let left = bigint_value_payload(runtime, left)?;
            let right = string_payload(runtime, *right)?;
            string_to_bigint(&right).map(|right| left.cmp(&right))
        }
        (JsValue::String(left), right) if right.is_bigint() => {
            let left = string_payload(runtime, *left)?;
            let right = bigint_value_payload(runtime, right)?;
            string_to_bigint(&left).map(|left| left.cmp(&right))
        }
        _ => match (
            to_numeric_primitive(runtime, left)?,
            to_numeric_primitive(runtime, right)?,
        ) {
            (NumericValue::BigInt(left), NumericValue::BigInt(right)) => Some(left.cmp(&right)),
            (NumericValue::BigInt(left), NumericValue::Number(right)) => {
                compare_bigint_number(&left, right)
            }
            (NumericValue::Number(left), NumericValue::BigInt(right)) => {
                compare_bigint_number(&right, left).map(std::cmp::Ordering::reverse)
            }
            (NumericValue::Number(left), NumericValue::Number(right)) => left.partial_cmp(&right),
        },
    };
    Ok(ordering.is_some_and(|ordering| match kind {
        NumericKind::Lt => ordering.is_lt(),
        NumericKind::Lte => ordering.is_le(),
        NumericKind::Gt => ordering.is_gt(),
        NumericKind::Gte => ordering.is_ge(),
        _ => false,
    }))
}
fn equal_result(kind: NumericKind, equal: bool) -> NumericStep {
    complete(JsValue::Bool(equal != (kind == NumericKind::Neq)))
}
fn equality_complete(
    runtime: &Runtime,
    kind: NumericKind,
    left: JsValue,
    right: JsValue,
    equal: bool,
) -> Result<NumericStep, Error> {
    let left = super::release_primitive_operand(runtime, left);
    let right = super::release_primitive_operand(runtime, right);
    left?;
    right?;
    Ok(equal_result(kind, equal))
}
fn equality_error(runtime: &Runtime, left: JsValue, right: JsValue, error: Error) -> Error {
    if let Err(release) = super::release_primitive_operand(runtime, left) {
        return release;
    }
    if let Err(release) = super::release_primitive_operand(runtime, right) {
        return release;
    }
    error
}
/// Elide an Object temporary only when its checked retain/release pair cannot
/// overflow, make the count immortal, or drain older cleanup. Other heap kinds
/// keep their existing pair instead of adding a new leaf/atom count interface.
fn nullish_html_dda(
    runtime: &Runtime,
    value: &JsValue,
) -> Result<bool, crate::engine::api::runtime_error::RuntimeError> {
    let needs_temporary = match value {
        JsValue::Object(id) => {
            let state = runtime.0.state.borrow();
            runtime.0.deferred_references.has_pending()
                || state.heap.has_pending_zero_cleanup()
                || state.heap.object_strong_count(*id)? >= u32::MAX - 1
        }
        JsValue::String(_) | JsValue::BigInt(_) | JsValue::Symbol(_) => true,
        _ => false,
    };
    let temporary = if needs_temporary {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "numeric_nullish_temporary_checkpoint",
        );
        Some(runtime.dup_jsvalue(value)?)
    } else {
        None
    };
    let equal = runtime.value_is_html_dda_jsvalue(value);
    if let Some(temporary) = temporary {
        runtime.release_jsvalue(temporary)?;
    }
    equal
}

fn equality(
    runtime: &Runtime,
    kind: NumericKind,
    mut left: JsValue,
    mut right: JsValue,
) -> Result<NumericStep, Error> {
    loop {
        match runtime.strict_equal_jsvalue(&left, &right) {
            Ok(true) => return equality_complete(runtime, kind, left, right, true),
            Ok(false) => {}
            Err(error) => {
                return Err(equality_error(
                    runtime,
                    left,
                    right,
                    Error::internal(error.to_string()),
                ));
            }
        }
        // Nullish equality never performs ToPrimitive, including for proxies.
        // IsHTMLDDA is an identity-local bit, so query it synchronously while
        // the original operands still own their full generational handles.
        let other = if matches!(right, JsValue::Null | JsValue::Undefined) {
            Some(&left)
        } else if matches!(left, JsValue::Null | JsValue::Undefined) {
            Some(&right)
        } else {
            None
        };
        if let Some(other) = other {
            let equal = if matches!(other, JsValue::Null | JsValue::Undefined) {
                Ok(true)
            } else {
                nullish_html_dda(runtime, other)
            };
            return match equal {
                Ok(equal) => {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "numeric_nullish_equality_direct",
                    );
                    equality_complete(runtime, kind, left, right, equal)
                }
                Err(error) => Err(equality_error(
                    runtime,
                    left,
                    right,
                    Error::internal(error.to_string()),
                )),
            };
        }
        match (&left, &right) {
            (JsValue::Int(_) | JsValue::Float(_), JsValue::String(_)) => {
                let number = match to_number_jsvalue(runtime, &right) {
                    Ok(number) => number,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let old = std::mem::replace(&mut right, jsvalue_number(number));
                if let Err(error) = runtime.release_jsvalue(old) {
                    return Err(equality_error(
                        runtime,
                        left,
                        right,
                        Error::internal(error.to_string()),
                    ));
                }
            }
            (JsValue::String(_), JsValue::Int(_) | JsValue::Float(_)) => {
                let number = match to_number_jsvalue(runtime, &left) {
                    Ok(number) => number,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let old = std::mem::replace(&mut left, jsvalue_number(number));
                if let Err(error) = runtime.release_jsvalue(old) {
                    return Err(equality_error(
                        runtime,
                        left,
                        right,
                        Error::internal(error.to_string()),
                    ));
                }
            }
            (a, JsValue::String(b)) if a.is_bigint() => {
                let payloads = bigint_value_payload(runtime, a)
                    .and_then(|a| string_payload(runtime, *b).map(|b| (a, b)));
                let (a, b) = match payloads {
                    Ok(payloads) => payloads,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = string_to_bigint(&b).is_some_and(|b| b == a);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (JsValue::String(a), b) if b.is_bigint() => {
                let payloads = string_payload(runtime, *a)
                    .and_then(|a| bigint_value_payload(runtime, b).map(|b| (a, b)));
                let (a, b) = match payloads {
                    Ok(payloads) => payloads,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = string_to_bigint(&a).is_some_and(|a| a == b);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (a, JsValue::Int(_) | JsValue::Float(_)) if a.is_bigint() => {
                let converted = bigint_value_payload(runtime, a)
                    .and_then(|a| to_number_jsvalue(runtime, &right).map(|number| (a, number)));
                let (a, number) = match converted {
                    Ok(converted) => converted,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = compare_bigint_number(&a, number) == Some(std::cmp::Ordering::Equal);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (JsValue::Int(_) | JsValue::Float(_), b) if b.is_bigint() => {
                let converted = bigint_value_payload(runtime, b)
                    .and_then(|b| to_number_jsvalue(runtime, &left).map(|number| (b, number)));
                let (b, number) = match converted {
                    Ok(converted) => converted,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = compare_bigint_number(&b, number) == Some(std::cmp::Ordering::Equal);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (JsValue::Bool(_), _) => {
                let number = match to_number_jsvalue(runtime, &left) {
                    Ok(number) => number,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                left = jsvalue_number(number);
            }
            (_, JsValue::Bool(_)) => {
                let number = match to_number_jsvalue(runtime, &right) {
                    Ok(number) => number,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                right = jsvalue_number(number);
            }
            (
                JsValue::Object(_),
                JsValue::Int(_)
                | JsValue::Float(_)
                | JsValue::BigInt(_)
                | JsValue::ShortBigInt(_)
                | JsValue::String(_)
                | JsValue::Symbol(_),
            ) => {
                return primitive(
                    runtime,
                    left,
                    ToPrimitiveHint::Default,
                    NumericResume(Box::new(NumericResumeState {
                        kind,
                        phase: Phase::EqualityLeft(right),
                        runtime: runtime.clone(),
                    })),
                );
            }
            (
                JsValue::Int(_)
                | JsValue::Float(_)
                | JsValue::BigInt(_)
                | JsValue::ShortBigInt(_)
                | JsValue::String(_)
                | JsValue::Symbol(_),
                JsValue::Object(_),
            ) => {
                return primitive(
                    runtime,
                    right,
                    ToPrimitiveHint::Default,
                    NumericResume(Box::new(NumericResumeState {
                        kind,
                        phase: Phase::EqualityRight(left),
                        runtime: runtime.clone(),
                    })),
                );
            }
            _ => return equality_complete(runtime, kind, left, right, false),
        }
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use crate::engine::{
        api::{profiling::CostProfile, runtime::Runtime},
        value::Value,
        vm::Completion,
    };

    #[test]
    fn numeric_callbacks_preserve_order_abrupt_completion_and_postfix_values_without_bridges() {
        for source in [
            "(function(){var log='';var a={[Symbol.toPrimitive](h){log+='L'+h;return 6}},b={[Symbol.toPrimitive](h){log+='R'+h;return 2}};return function(){var result=a-b;return result===4&&log==='LnumberRnumber'?42:0}})()",
            "(function(){var log='';var symbol=Symbol(),a={valueOf(){log+='L';return symbol}},b={valueOf(){log+='R';return 1}};return function(){try{a*b}catch(e){return log==='L'?42:0}return 0}})()",
            "(function(){var log='';var symbol=Symbol(),a={valueOf(){log+='L';return symbol}},b={valueOf(){log+='R';return 1}};return function(){try{a<b}catch(e){return log==='LR'?42:0}return 0}})()",
            "(function(){var log='';var a={valueOf(){log+='L';return 1n}},b={valueOf(){log+='R';return 1}};return function(){try{a/b}catch(e){return log==='LR'?42:0}return 0}})()",
            "(function(){var n=0,a={[Symbol.toPrimitive](h){if(h!=='number')throw 99;n++;return '6'}};return function(){var v=a,old=v++;return old===6&&v===7&&n===1?42:0}})()",
            "(function(){var n=0,a={[Symbol.toPrimitive](h){if(h!=='number')throw 99;n++;return 6n}};return function(){var v=a,old=v--;return old===6n&&v===5n&&n===1?42:0}})()",
            "(function(){var n=0,a={[Symbol.toPrimitive](h){if(h!=='default')throw 99;n++;return '42'}};return function(){return a==42&&n===1?42:0}})()",
            "(function(){var a={valueOf(){return 40n}},b={valueOf(){return 2n}};return function(){return a|b}})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callable = runtime
                .callable_from_value(context.eval(source).unwrap())
                .unwrap();
            let profile = CostProfile::start();
            let completion = runtime
                .call_internal(context.realm, &callable, Value::Undefined, &[])
                .unwrap();
            let _costs = profile.snapshot();
            let Completion::Return(value) = completion else {
                panic!("{source}: {completion:?}");
            };
            let value = runtime.root_and_release_jsvalue(value).unwrap();
            assert!(
                matches!(value, Value::Int(42))
                    || matches!(&value, Value::BigInt(value) if value == &crate::engine::value::bigint::JsBigInt::from(42_i32)),
                "{source}: {value:?}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }
}

#[cfg(test)]
mod borrowed_bigint_tests {
    use super::*;
    use crate::engine::value::{Value, bigint::JsBigInt};

    #[test]
    fn consuming_bigint_reuses_only_unique_edges_without_mutating_public_payloads() {
        let runtime = Runtime::new();
        let payload = JsBigInt::parse_js_string("170141183460469231731687303715884105729").unwrap();
        let public = payload.clone();
        let input = runtime
            .into_jsvalue(Value::BigInt(payload.clone()))
            .unwrap();
        let JsValue::BigInt(original_id) = input else {
            panic!("expected heap BigInt")
        };
        let output = primitive_output(
            &runtime,
            NumericKind::Mul,
            input,
            Some(JsValue::ShortBigInt(2)),
        )
        .unwrap();
        assert!(matches!(output.value, JsValue::BigInt(id) if id == original_id));
        let actual = runtime.root_and_release_jsvalue(output.value).unwrap();
        assert!(
            matches!(actual, Value::BigInt(value) if value == payload.mul(&JsBigInt::from(2_i32)).unwrap())
        );
        assert_eq!(
            public.to_string(),
            "170141183460469231731687303715884105729"
        );

        // A borrowed add cannot repurpose even a sole arena edge.
        let input = runtime
            .into_jsvalue(Value::BigInt(payload.clone()))
            .unwrap();
        let output =
            super::super::add_primitives_ref(&runtime, &input, &JsValue::ShortBigInt(1)).unwrap();
        let original = runtime.root_and_release_jsvalue(input).unwrap();
        runtime.release_jsvalue(output).unwrap();
        assert!(matches!(original, Value::BigInt(value) if value == payload));

        // An external arena edge prohibits reuse and must retain the old value.
        let input = runtime
            .into_jsvalue(Value::BigInt(payload.clone()))
            .unwrap();
        let saved = runtime.dup_jsvalue(&input).unwrap();
        let output = primitive_output(
            &runtime,
            NumericKind::Mul,
            input,
            Some(JsValue::ShortBigInt(2)),
        )
        .unwrap();
        assert!(
            !matches!((&saved, &output.value), (JsValue::BigInt(a), JsValue::BigInt(b)) if a == b)
        );
        runtime.release_jsvalue(output.value).unwrap();
        let saved = runtime.root_and_release_jsvalue(saved).unwrap();
        assert!(matches!(saved, Value::BigInt(value) if value == payload));
    }

    #[test]
    fn borrowed_bigint_kernels_preserve_values_tags_errors_and_operand_owners() {
        let runtime = Runtime::new();
        for source in [
            "7",
            "9223372036854775808",
            "-170141183460469231731687303715884105729",
        ] {
            let left = JsBigInt::parse_js_string(source).unwrap();
            for right in [
                JsBigInt::from(2_i32),
                JsBigInt::from(0_i32),
                JsBigInt::from(-1_i32),
            ] {
                for kind in [
                    NumericKind::Add,
                    NumericKind::Sub,
                    NumericKind::Mul,
                    NumericKind::Div,
                    NumericKind::Mod,
                    NumericKind::Pow,
                    NumericKind::Shl,
                    NumericKind::Sar,
                    NumericKind::Shr,
                    NumericKind::BitAnd,
                    NumericKind::BitOr,
                    NumericKind::BitXor,
                ] {
                    let expected = if kind == NumericKind::Add {
                        left.add(&right).map_err(bigint_error)
                    } else {
                        bigint_binary(kind, &left, &right)
                    };
                    let input_left = runtime.into_jsvalue(Value::BigInt(left.clone())).unwrap();
                    let input_right = runtime.into_jsvalue(Value::BigInt(right.clone())).unwrap();
                    let actual = primitive_output(&runtime, kind, input_left, Some(input_right));
                    match (actual, expected) {
                        (Ok(actual), Ok(expected)) => {
                            assert!(actual.previous.is_none());
                            let actual = runtime.root_and_release_jsvalue(actual.value).unwrap();
                            let Value::BigInt(actual) = actual else {
                                panic!("non-BigInt result")
                            };
                            assert_eq!(actual, expected, "{source} {kind:?} {right}");
                            assert_eq!(actual.as_i64(), expected.as_i64());
                        }
                        (Err(actual), Err(expected)) => {
                            assert_eq!(actual.kind(), expected.kind());
                            assert_eq!(actual.to_string(), expected.to_string());
                        }
                        (actual, expected) => {
                            if let Ok(actual) = actual {
                                runtime.release_jsvalue(actual.value).unwrap();
                            }
                            panic!(
                                "result mismatch: {source} {kind:?} {right}; expected {expected:?}"
                            );
                        }
                    }
                }
            }
        }
        // Both operands may be separate owned edges to exactly the same node.
        let payload = JsBigInt::parse_js_string("170141183460469231731687303715884105729").unwrap();
        for kind in [NumericKind::Add, NumericKind::Mul] {
            let left = runtime
                .into_jsvalue(Value::BigInt(payload.clone()))
                .unwrap();
            let right = runtime.dup_jsvalue(&left).unwrap();
            let actual = primitive_output(&runtime, kind, left, Some(right)).unwrap();
            let actual = runtime.root_and_release_jsvalue(actual.value).unwrap();
            let expected = if kind == NumericKind::Add {
                payload.add(&payload)
            } else {
                payload.mul(&payload)
            }
            .unwrap();
            assert!(matches!(actual, Value::BigInt(actual) if actual == expected));
        }
        // Runtime teardown checks that successful and abrupt paths drained all edges.
    }
}

#[cfg(test)]
mod nullish_equality_tests {
    use super::*;
    use crate::engine::value::Value;

    #[test]
    fn nullish_equality_preserves_proxy_identity_and_conversion_effects() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let result = context.eval(r#"(() => {
            let calls = 0;
            const object = { [Symbol.toPrimitive]() { calls++; return null; } };
            const proxy = new Proxy(object, { get() { calls++; throw 42; } });
            const revoked = Proxy.revocable({}, {}); revoked.revoke();
            const values = [0, 1, NaN, false, true, '', 'null', 0n,
                            123456789012345678901234567890n, Symbol(), object, proxy, revoked.proxy];
            for (const value of values) {
                if (value == null || null == value || value == undefined || undefined == value)
                    return false;
                if (!(value != null && null != value && value != undefined && undefined != value))
                    return false;
            }
            if (!(null == undefined && undefined == null) || null != undefined) return false;
            if (calls !== 0) return false;
            const number = { valueOf() { calls++; return 1; } };
            if (!(number == 1) || calls !== 1) return false;
            try { proxy == 1; return false; } catch (error) { return error === 42 && calls === 2; }
        })()"#).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn nullish_equality_consumes_final_object_owner() {
        for kind in [NumericKind::Eq, NumericKind::Neq] {
            let runtime = Runtime::new();
            let object = runtime.new_object(None).unwrap().into_handle();
            let step =
                NumericStep::start(&runtime, kind, JsValue::Object(object), Some(JsValue::Null))
                    .unwrap();
            assert!(
                matches!(step, NumericStep::Complete { value: JsValue::Bool(value), previous: None }
                if value == (kind == NumericKind::Neq))
            );
            assert!(runtime.0.state.borrow().heap.object(object).is_err());
        }
    }

    #[test]
    fn nullish_equality_keeps_pending_cleanup_before_final_operand_release() {
        for reverse in [false, true] {
            let runtime = Runtime::new();
            let pending = runtime.new_object(None).unwrap();
            let pending_id = pending.object_id();
            let operand = runtime.new_object(None).unwrap().into_handle();
            {
                let _borrow = runtime.0.state.borrow();
                drop(pending);
            }
            assert!(runtime.0.deferred_references.has_pending());
            let value = JsValue::Object(operand);
            let (left, right) = if reverse {
                (JsValue::Null, value)
            } else {
                (value, JsValue::Null)
            };
            let step = NumericStep::start(&runtime, NumericKind::Eq, left, Some(right)).unwrap();
            assert!(matches!(
                step,
                NumericStep::Complete {
                    value: JsValue::Bool(false),
                    previous: None
                }
            ));
            assert!(!runtime.0.deferred_references.has_pending());
            assert!(runtime.0.state.borrow().heap.object(pending_id).is_err());
            assert!(runtime.0.state.borrow().heap.object(operand).is_err());
            // Object arena reuse is LIFO. The original checkpoint reclaims
            // pending first and operand second, so operand's slot is next.
            let next = runtime.new_object(None).unwrap();
            assert_eq!(next.object_id().debug_index(), operand.debug_index());
            assert_ne!(next.object_id(), operand);
        }
    }

    #[test]
    fn nullish_equality_preserves_heap_retain_saturation_and_overflow() {
        use crate::engine::heap::RawId;
        for source in ["({})", "'heap string'", "123456789012345678901234567890n"] {
            for count in [u32::MAX - 2, u32::MAX - 1, u32::MAX] {
                let runtime = Runtime::new();
                let mut context = runtime.new_context();
                let value = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
                let id = match value {
                    JsValue::Object(id) => RawId::Object(id),
                    JsValue::String(id) => RawId::String(id),
                    JsValue::BigInt(id) => RawId::BigInt(id),
                    _ => panic!("expected heap-backed operand"),
                };
                runtime
                    .0
                    .state
                    .borrow_mut()
                    .heap
                    .set_strong_count_for_test(id, count);
                let result = nullish_html_dda(&runtime, &value);
                let actual = runtime.0.state.borrow().heap.strong_count(id).unwrap();
                // Restore a real owner count before assertions/drop, including
                // error cases, so a failed assertion cannot poison teardown.
                runtime
                    .0
                    .state
                    .borrow_mut()
                    .heap
                    .set_strong_count_for_test(id, 1);
                runtime.release_jsvalue(value).unwrap();
                if count == u32::MAX {
                    assert!(result.is_err(), "{source}: checked retain must overflow");
                    assert_eq!(actual, count);
                } else {
                    assert!(!result.unwrap());
                    assert_eq!(
                        actual,
                        if count == u32::MAX - 1 {
                            u32::MAX
                        } else {
                            count
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn nullish_equality_preserves_symbol_checked_count_without_immortality() {
        for count in [u32::MAX - 2, u32::MAX - 1, u32::MAX] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let value = runtime
                .into_jsvalue(context.eval("Symbol('nullish')").unwrap())
                .unwrap();
            let JsValue::Symbol(index) = value else {
                panic!("expected symbol");
            };
            runtime
                .0
                .state
                .borrow()
                .atoms
                .set_ref_count_for_test(index, count);
            let result = nullish_html_dda(&runtime, &value);
            let actual = {
                let state = runtime.0.state.borrow();
                state
                    .atoms
                    .resolve(state.atoms.brand(index).unwrap())
                    .unwrap()
                    .ref_count
            };
            runtime
                .0
                .state
                .borrow()
                .atoms
                .set_ref_count_for_test(index, 1);
            runtime.release_jsvalue(value).unwrap();
            assert_eq!(actual, Some(count));
            if count == u32::MAX {
                assert!(result.is_err());
            } else {
                // Unlike heap nodes, MAX is not an immortal atom count:
                // the successful temporary decrement restores MAX - 1.
                assert!(!result.unwrap());
            }
        }
    }

    #[cfg(feature = "test262-host")]
    #[test]
    fn nullish_equality_reads_html_dda_from_exact_object_identity() {
        let runtime = Runtime::new();
        let object = runtime.new_object(None).unwrap();
        runtime.set_object_is_html_dda(&object).unwrap();
        for kind in [NumericKind::Eq, NumericKind::Neq] {
            for reverse in [false, true] {
                for nullish in [JsValue::Null, JsValue::Undefined] {
                    let value = JsValue::Object(object.clone().into_handle());
                    let (left, right) = if reverse {
                        (nullish, value)
                    } else {
                        (value, nullish)
                    };
                    let step = NumericStep::start(&runtime, kind, left, Some(right)).unwrap();
                    assert!(
                        matches!(step, NumericStep::Complete { value: JsValue::Bool(value), previous: None }
                        if value == (kind == NumericKind::Eq))
                    );
                }
            }
        }
        assert!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object(object.object_id())
                .is_ok()
        );
    }
}
