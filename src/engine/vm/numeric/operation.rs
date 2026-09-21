//! Numeric operators retain ordered conversion operands across JavaScript callbacks.
use super::{
    NumericValue, add_primitives, bigint_error, bigint_payload, compare_bigint_number,
    jsvalue_number, mixed_numeric_type_error, number_to_int32, number_to_uint32, string_payload,
    string_to_bigint, to_number_jsvalue, to_numeric_primitive, unary_plus_primitive,
};
use crate::engine::{
    api::runtime::Runtime,
    api::{Error, ErrorKind},
    code::bytecode::Instruction,
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
    pub(in crate::engine::vm) fn for_instruction(instruction: &Instruction) -> Option<Self> {
        Some(match instruction {
            Instruction::Neg => Self::Neg,
            Instruction::Plus => Self::Plus,
            Instruction::BitNot => Self::BitNot,
            Instruction::Inc => Self::Inc,
            Instruction::Dec => Self::Dec,
            Instruction::PostInc => Self::PostInc,
            Instruction::PostDec => Self::PostDec,
            Instruction::Add => Self::Add,
            Instruction::Sub => Self::Sub,
            Instruction::Mul => Self::Mul,
            Instruction::Div => Self::Div,
            Instruction::Mod => Self::Mod,
            Instruction::Pow => Self::Pow,
            Instruction::Shl => Self::Shl,
            Instruction::Sar => Self::Sar,
            Instruction::Shr => Self::Shr,
            Instruction::BitAnd => Self::BitAnd,
            Instruction::BitOr => Self::BitOr,
            Instruction::BitXor => Self::BitXor,
            Instruction::Eq => Self::Eq,
            Instruction::Neq => Self::Neq,
            Instruction::Lt => Self::Lt,
            Instruction::Lte => Self::Lte,
            Instruction::Gt => Self::Gt,
            Instruction::Gte => Self::Gte,
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
/// RunSlots borrow; the resident run helper is also such an owning boundary.
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
    HtmlDda {
        value: JsValue,
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
            Phase::EqualityDda(left, right) => {
                let left = std::mem::replace(left, JsValue::Undefined);
                let right = std::mem::replace(right, JsValue::Undefined);
                let _ = self.runtime.release_jsvalue(left);
                return drop_owned(&self.runtime, right);
            }
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
    EqualityDda(JsValue, JsValue),
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
            return equality(runtime, kind, left, right, false);
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
            Phase::EqualityLeft(right) => equality(runtime, kind, value, right, false),
            Phase::EqualityRight(left) => equality(runtime, kind, left, value, false),
            Phase::EqualityDda(..) => {
                Err(Error::internal("HTMLDDA check received a primitive reply"))
            }
        }
    }
    pub(in crate::engine::vm) fn html_dda(
        mut self,
        runtime: &Runtime,
        value: bool,
    ) -> Result<NumericStep, Error> {
        let phase = std::mem::replace(&mut self.0.phase, Phase::Unary);
        let Phase::EqualityDda(left, right) = phase else {
            return Err(Error::internal("HTMLDDA reply lost equality owner"));
        };
        if value {
            equality_complete(runtime, self.0.kind, left, right, true)
        } else {
            equality(runtime, self.0.kind, left, right, true)
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
            JsValue::BigInt(id) => bigint_payload(runtime, *id)
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
        JsValue::BigInt(id) => {
            let result = bigint_payload(runtime, id)
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
                    super::release_primitive_operand(runtime, JsValue::BigInt(id))?;
                    return Err(error);
                }
            };
            let old = JsValue::BigInt(id);
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
            super::allocate_bigint_jsvalue(
                runtime,
                match kind {
                    NumericKind::Sub => left.sub(&right),
                    NumericKind::Mul => left.mul(&right),
                    NumericKind::Div => left.div(&right),
                    NumericKind::Mod => left.rem(&right),
                    NumericKind::Pow => left.pow(&right),
                    NumericKind::Shl => left.shl(&right),
                    NumericKind::Sar => left.shr(&right),
                    NumericKind::BitAnd => left.bit_and(&right),
                    NumericKind::BitOr => left.bit_or(&right),
                    NumericKind::BitXor => left.bit_xor(&right),
                    _ => {
                        return Err(Error::internal(
                            "non-arithmetic operator entered binary Numeric",
                        ));
                    }
                }
                .map_err(bigint_error)?,
            )?
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
        (JsValue::BigInt(left), JsValue::BigInt(right)) => {
            Some(bigint_payload(runtime, *left)?.cmp(&bigint_payload(runtime, *right)?))
        }
        (JsValue::BigInt(left), JsValue::String(right)) => {
            let left = bigint_payload(runtime, *left)?;
            let right = string_payload(runtime, *right)?;
            string_to_bigint(&right).map(|right| left.cmp(&right))
        }
        (JsValue::String(left), JsValue::BigInt(right)) => {
            let left = string_payload(runtime, *left)?;
            let right = bigint_payload(runtime, *right)?;
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
fn equality(
    runtime: &Runtime,
    kind: NumericKind,
    mut left: JsValue,
    mut right: JsValue,
    mut checked_dda: bool,
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
        if !checked_dda {
            if matches!(right, JsValue::Null | JsValue::Undefined) {
                let value = match runtime.dup_jsvalue(&left) {
                    Ok(value) => value,
                    Err(error) => {
                        return Err(equality_error(
                            runtime,
                            left,
                            right,
                            Error::internal(error.to_string()),
                        ));
                    }
                };
                return Ok(NumericStep::HtmlDda {
                    value,
                    resume: NumericResume(Box::new(NumericResumeState {
                        kind,
                        phase: Phase::EqualityDda(left, right),
                        runtime: runtime.clone(),
                    })),
                });
            }
            if matches!(left, JsValue::Null | JsValue::Undefined) {
                let value = match runtime.dup_jsvalue(&right) {
                    Ok(value) => value,
                    Err(error) => {
                        return Err(equality_error(
                            runtime,
                            left,
                            right,
                            Error::internal(error.to_string()),
                        ));
                    }
                };
                return Ok(NumericStep::HtmlDda {
                    value,
                    resume: NumericResume(Box::new(NumericResumeState {
                        kind,
                        phase: Phase::EqualityDda(left, right),
                        runtime: runtime.clone(),
                    })),
                });
            }
        }
        checked_dda = false;
        match (&left, &right) {
            (JsValue::Null, JsValue::Undefined) | (JsValue::Undefined, JsValue::Null) => {
                return equality_complete(runtime, kind, left, right, true);
            }
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
            (JsValue::BigInt(a), JsValue::String(b)) => {
                let payloads = bigint_payload(runtime, *a)
                    .and_then(|a| string_payload(runtime, *b).map(|b| (a, b)));
                let (a, b) = match payloads {
                    Ok(payloads) => payloads,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = string_to_bigint(&b).is_some_and(|b| b == a);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (JsValue::String(a), JsValue::BigInt(b)) => {
                let payloads = string_payload(runtime, *a)
                    .and_then(|a| bigint_payload(runtime, *b).map(|b| (a, b)));
                let (a, b) = match payloads {
                    Ok(payloads) => payloads,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = string_to_bigint(&a).is_some_and(|a| a == b);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (JsValue::BigInt(a), JsValue::Int(_) | JsValue::Float(_)) => {
                let converted = bigint_payload(runtime, *a)
                    .and_then(|a| to_number_jsvalue(runtime, &right).map(|number| (a, number)));
                let (a, number) = match converted {
                    Ok(converted) => converted,
                    Err(error) => return Err(equality_error(runtime, left, right, error)),
                };
                let equal = compare_bigint_number(&a, number) == Some(std::cmp::Ordering::Equal);
                return equality_complete(runtime, kind, left, right, equal);
            }
            (JsValue::Int(_) | JsValue::Float(_), JsValue::BigInt(b)) => {
                let converted = bigint_payload(runtime, *b)
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
            assert!(
                matches!(completion, Completion::Return(Value::Int(42)))
                    || matches!(&completion,Completion::Return(Value::BigInt(value)) if value == &crate::engine::value::bigint::JsBigInt::from(42_i32)),
                "{source}: {completion:?}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }
}
