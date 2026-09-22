pub(super) mod operation;
use crate::engine::{
    api::runtime::Runtime,
    api::{Error, ErrorKind},
    heap::{BigIntId, StringId},
    value::{
        JsString, JsValue,
        bigint::{BigIntError, JsBigInt},
    },
};
use num_bigint::BigInt;
use num_traits::FromPrimitive;

pub(in crate::engine::vm) enum NumericValue {
    Number(f64),
    BigInt(JsBigInt),
}

/// Read one string node's payload. The borrowed value keeps its edge.
pub(in crate::engine::vm) fn string_payload(
    runtime: &Runtime,
    id: StringId,
) -> Result<JsString, Error> {
    Ok(runtime
        .0
        .state
        .borrow()
        .heap
        .string(id)
        .map_err(|error| Error::internal(error.to_string()))?
        .clone())
}

/// Read one BigInt node's payload. The borrowed value keeps its edge.
pub(in crate::engine::vm) fn bigint_payload(
    runtime: &Runtime,
    id: BigIntId,
) -> Result<JsBigInt, Error> {
    Ok(runtime
        .0
        .state
        .borrow()
        .heap
        .bigint(id)
        .map_err(|error| Error::internal(error.to_string()))?
        .clone())
}

pub(in crate::engine::vm) fn bigint_value_payload(
    runtime: &Runtime,
    value: &JsValue,
) -> Result<JsBigInt, Error> {
    match value {
        JsValue::ShortBigInt(value) => Ok(JsBigInt::from(*value)),
        JsValue::BigInt(id) => bigint_payload(runtime, *id),
        _ => Err(Error::internal("BigInt payload requested for another type")),
    }
}

/// Run a pure BigInt kernel while the original operand edges keep its borrowed
/// payloads alive. The kernel must not access Runtime: publication and owner
/// release happen after this shared heap borrow ends. Inline pairs need no heap.
fn with_bigint_operands(
    runtime: &Runtime,
    left: &JsValue,
    right: &JsValue,
    kernel: impl FnOnce(&JsBigInt, &JsBigInt) -> Result<JsBigInt, Error>,
) -> Result<JsBigInt, Error> {
    if let (JsValue::ShortBigInt(left), JsValue::ShortBigInt(right)) = (left, right) {
        return kernel(&JsBigInt::from(*left), &JsBigInt::from(*right));
    }
    let state = runtime.0.state.borrow();
    let left_short;
    let right_short;
    // The caller's operands are owning edges, so both nodes are live for the
    // whole shared borrow; use the trusted read instead of re-validating.
    let left = match left {
        JsValue::ShortBigInt(value) => {
            left_short = JsBigInt::from(*value);
            &left_short
        }
        JsValue::BigInt(id) => state.heap.bigint_fast(*id),
        _ => return Err(Error::internal("BigInt kernel received another type")),
    };
    let right = match right {
        JsValue::ShortBigInt(value) => {
            right_short = JsBigInt::from(*value);
            &right_short
        }
        JsValue::BigInt(id) => state.heap.bigint_fast(*id),
        _ => return Err(Error::internal("BigInt kernel received another type")),
    };
    kernel(left, right)
}

/// Finish an operation that consumes both input edges. Only this consuming
/// boundary may transfer a unique operand node to the newly computed result;
/// the borrowed add kernel must leave its caller's values unchanged.
fn finish_bigint_operands(
    runtime: &Runtime,
    left: JsValue,
    right: JsValue,
    result: Result<JsBigInt, Error>,
) -> Result<JsValue, Error> {
    let mut result = match result {
        Ok(result) => Some(result),
        Err(error) => {
            let released_left = release_primitive_operand(runtime, left);
            let released_right = release_primitive_operand(runtime, right);
            released_left?;
            released_right?;
            return Err(error);
        }
    };
    let reuse: Result<Option<bool>, Error> = if result.as_ref().and_then(JsBigInt::as_i64).is_none()
    {
        (|| {
            let mut state = runtime.0.state.borrow_mut();
            for (is_left, value) in [(true, &left), (false, &right)] {
                if let JsValue::BigInt(id) = value {
                    // Aliased operands share one node; its first uniqueness
                    // check already rejected reuse for both edges.
                    if !is_left && matches!(&left, JsValue::BigInt(left) if left == id) {
                        continue;
                    }
                    if let Some(slot) = state
                        .heap
                        .unique_bigint_mut(*id)
                        .map_err(|error| Error::internal(error.to_string()))?
                    {
                        *slot = result.take().expect("fresh BigInt result");
                        return Ok(Some(is_left));
                    }
                }
            }
            Ok(None)
        })()
    } else {
        Ok(None)
    };
    match reuse {
        Ok(Some(true)) => {
            if let Err(error) = release_primitive_operand(runtime, right) {
                let _ = release_primitive_operand(runtime, left);
                return Err(error);
            }
            Ok(left)
        }
        Ok(Some(false)) => {
            if let Err(error) = release_primitive_operand(runtime, left) {
                let _ = release_primitive_operand(runtime, right);
                return Err(error);
            }
            Ok(right)
        }
        reuse => {
            let released_left = release_primitive_operand(runtime, left);
            let released_right = release_primitive_operand(runtime, right);
            released_left?;
            released_right?;
            reuse?;
            allocate_bigint_jsvalue(runtime, result.expect("unpublished BigInt result"))
        }
    }
}

/// Publish a freshly produced string payload as an owned internal value.
/// Concatenation and primitive formatting are genuine string creation points.
pub(in crate::engine::vm) fn allocate_string_jsvalue(
    runtime: &Runtime,
    string: JsString,
) -> Result<JsValue, Error> {
    let id = runtime
        .0
        .state
        .borrow_mut()
        .heap
        .allocate_string(string)
        .map_err(|error| Error::internal(error.to_string()))?;
    Ok(JsValue::String(id))
}

/// Publish a freshly produced BigInt payload as an owned internal value.
/// BigInt arithmetic results are genuine BigInt creation points.
pub(in crate::engine::vm) fn allocate_bigint_jsvalue(
    runtime: &Runtime,
    bigint: JsBigInt,
) -> Result<JsValue, Error> {
    if let Some(value) = bigint.as_i64() {
        return Ok(JsValue::ShortBigInt(value));
    }
    let id = runtime
        .0
        .state
        .borrow_mut()
        .heap
        .allocate_bigint(bigint)
        .map_err(|error| Error::internal(error.to_string()))?;
    #[cfg(debug_assertions)]
    if std::env::var("QJS_TRACE_BIGINT_ID")
        .is_ok_and(|value| format!("{id:?}").contains(&format!("index: {value},")))
    {
        eprintln!(
            "[alloc-b] {id:?}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
    }
    Ok(JsValue::BigInt(id))
}

/// Representation-only `ToNumber` for internal values. Object conversion must
/// be routed through a context; Symbol and BigInt conversion throw here.
pub(crate) fn to_number_jsvalue(runtime: &Runtime, value: &JsValue) -> Result<f64, Error> {
    Ok(match value {
        JsValue::Undefined => f64::NAN,
        JsValue::Null => 0.0,
        JsValue::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        JsValue::Int(value) => f64::from(*value),
        JsValue::Float(value) => *value,
        JsValue::String(id) => {
            crate::engine::value::string_to_number(&string_payload(runtime, *id)?)
        }
        JsValue::BigInt(_) | JsValue::ShortBigInt(_) => {
            return Err(Error::new(
                ErrorKind::Type,
                "cannot convert bigint to number",
            ));
        }
        JsValue::Symbol(_) => {
            return Err(Error::new(
                ErrorKind::Type,
                "cannot convert symbol to number",
            ));
        }
        JsValue::Object(_) => {
            return Err(Error::internal(
                "object ToNumber requires an execution context",
            ));
        }
    })
}

/// Primitive `ToString` payload for internal values (no object conversion).
pub(crate) fn to_js_string_jsvalue(runtime: &Runtime, value: &JsValue) -> Result<JsString, Error> {
    Ok(match value {
        JsValue::String(id) => string_payload(runtime, *id)?,
        JsValue::Undefined => JsString::from_static("undefined"),
        JsValue::Null => JsString::from_static("null"),
        JsValue::Bool(true) => JsString::from_static("true"),
        JsValue::Bool(false) => JsString::from_static("false"),
        JsValue::Int(value) => JsString::from_owned_latin1(value.to_string().into_bytes()),
        JsValue::Float(value) => {
            JsString::from_owned_latin1(crate::engine::value::number_to_string(*value).into_bytes())
        }
        JsValue::ShortBigInt(value) => JsString::from_owned_latin1(value.to_string().into_bytes()),
        JsValue::BigInt(id) => {
            let bigint = bigint_payload(runtime, *id)?;
            if bigint.exceeds_allocation_limit() {
                return Err(Error::new(
                    ErrorKind::Range,
                    "BigInt is too large to allocate",
                ));
            }
            JsString::from_owned_latin1(bigint.to_string().into_bytes())
        }
        JsValue::Symbol(_) => {
            return Err(Error::new(
                ErrorKind::Type,
                "cannot convert symbol to string",
            ));
        }
        JsValue::Object(_) => {
            return Err(Error::internal(
                "object ToPrimitive requires an execution context",
            ));
        }
    })
}

pub(in crate::engine::vm) fn to_numeric_primitive(
    runtime: &Runtime,
    value: &JsValue,
) -> Result<NumericValue, Error> {
    match value {
        JsValue::BigInt(id) => Ok(NumericValue::BigInt(bigint_payload(runtime, *id)?)),
        JsValue::ShortBigInt(value) => Ok(NumericValue::BigInt(JsBigInt::from(*value))),
        value => Ok(NumericValue::Number(to_number_jsvalue(runtime, value)?)),
    }
}

/// OP_plus after ToPrimitive: preserve numeric tags and its specific BigInt
/// diagnostic. This step cannot invoke user code.
pub(in crate::engine::vm) fn unary_plus_primitive(
    runtime: &Runtime,
    value: JsValue,
) -> Result<JsValue, Error> {
    if value.is_bigint() {
        release_primitive_operand(runtime, value)?;
        return Err(Error::new(ErrorKind::Type, "bigint argument with unary +"));
    }
    if matches!(value, JsValue::Int(_) | JsValue::Float(_)) {
        return Ok(value);
    }
    let result = jsvalue_number(to_number_jsvalue(runtime, &value)?);
    release_primitive_operand(runtime, value)?;
    Ok(result)
}

/// ECMAScript `ToInt32`, matching QuickJS's modulo-2^32 conversion for every
/// finite IEEE-754 input and its zero result for NaN and infinities.
pub(in crate::engine::vm) fn number_to_int32(value: f64) -> i32 {
    crate::engine::value::number::to_int32(value)
}

pub(in crate::engine::vm) fn number_to_uint32(value: f64) -> u32 {
    u32::from_ne_bytes(number_to_int32(value).to_ne_bytes())
}

/// Compact a numeric payload into the internal number representation.
pub(in crate::engine::vm) fn jsvalue_number(value: f64) -> JsValue {
    jsvalue_from_number(crate::engine::value::number::operations::Number::compact(
        value,
    ))
}

/// Project an already-compacted numeric representation without recompacting.
pub(in crate::engine::vm) fn jsvalue_from_number(
    number: crate::engine::value::number::operations::Number,
) -> JsValue {
    match number {
        crate::engine::value::number::operations::Number::Int(value) => JsValue::Int(value),
        crate::engine::value::number::operations::Number::Float(value) => JsValue::Float(value),
    }
}

pub(in crate::engine::vm) fn compare_bigint_number(
    bigint: &JsBigInt,
    number: f64,
) -> Option<std::cmp::Ordering> {
    if number.is_nan() {
        return None;
    }
    if number == f64::INFINITY {
        return Some(std::cmp::Ordering::Less);
    }
    if number == f64::NEG_INFINITY {
        return Some(std::cmp::Ordering::Greater);
    }

    let truncated = BigInt::from_f64(number.trunc())?;
    let ordering = bigint.to_bigint().cmp(&truncated);
    if !ordering.is_eq() {
        return Some(ordering);
    }
    if number.fract().is_sign_positive() && number.fract() != 0.0 {
        Some(std::cmp::Ordering::Less)
    } else if number.fract().is_sign_negative() && number.fract() != 0.0 {
        Some(std::cmp::Ordering::Greater)
    } else {
        Some(std::cmp::Ordering::Equal)
    }
}

pub(in crate::engine::vm) fn string_to_bigint(
    value: &crate::engine::value::JsString,
) -> Option<JsBigInt> {
    let text = String::from_utf16(&value.utf16_units().collect::<Vec<_>>()).ok()?;
    JsBigInt::parse_js_string(&text).ok()
}

pub(in crate::engine::vm) fn mixed_numeric_type_error() -> Error {
    Error::new(ErrorKind::Type, "cannot convert bigint to number")
}

pub(in crate::engine::vm) fn bigint_error(error: BigIntError) -> Error {
    let message = match error {
        BigIntError::ShiftTooLarge => "BigInt is too large to allocate".to_owned(),
        error => error.to_string(),
    };
    Error::new(ErrorKind::Range, message)
}

/// Addition after both operands have completed ToPrimitive, in order.
pub(in crate::engine::vm) fn add_primitives(
    runtime: &Runtime,
    left: JsValue,
    right: JsValue,
) -> Result<JsValue, Error> {
    if left.is_bigint() && right.is_bigint() {
        let result = with_bigint_operands(runtime, &left, &right, |left, right| {
            left.add(right).map_err(bigint_error)
        });
        return finish_bigint_operands(runtime, left, right, result);
    }
    let mut reused_left = false;
    let result = if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
        (|| {
            if let JsValue::String(id) = &left {
                let suffix = match &right {
                    JsValue::String(id) => string_payload(runtime, *id)?,
                    value => to_js_string_jsvalue(runtime, value)?,
                };
                // The consumed arena edge can be the result owner. Do not clone
                // its payload before checking uniqueness: that extra Rc alone
                // disables the pre-existing concat_owned in-place algorithm.
                let appended = if runtime.0.deferred_references.has_pending() {
                    false
                } else {
                    let mut state = runtime.0.state.borrow_mut();
                    match state
                        .heap
                        .unique_string_mut(*id)
                        .map_err(|error| Error::internal(error.to_string()))?
                    {
                        // Preserve concat_owned's empty-string representation
                        // rules, including empty UTF-16 plus Latin1.
                        Some(string) if !string.is_empty() && !suffix.is_empty() => {
                            string.try_concat_in_place(&suffix)?
                        }
                        _ => false,
                    }
                };
                if appended {
                    reused_left = true;
                    return Ok(JsValue::String(*id));
                }
                let prefix = string_payload(runtime, *id)?;
                return allocate_string_jsvalue(runtime, prefix.concat_owned(&suffix)?);
            }
            let left = match &left {
                JsValue::String(id) => string_payload(runtime, *id)?,
                value => to_js_string_jsvalue(runtime, value)?,
            };
            let right = match &right {
                JsValue::String(id) => string_payload(runtime, *id)?,
                value => to_js_string_jsvalue(runtime, value)?,
            };
            allocate_string_jsvalue(runtime, left.concat_owned(&right)?)
        })()
    } else {
        add_primitives_ref(runtime, &left, &right)
    };
    if !reused_left {
        release_primitive_operand(runtime, left)?;
    }
    release_primitive_operand(runtime, right)?;
    result
}

fn release_primitive_operand(runtime: &Runtime, value: JsValue) -> Result<(), Error> {
    runtime
        .release_jsvalue(value)
        .map_err(|error| Error::internal(error.to_string()))
}

/// Same primitive kernel with owners retained by the caller. No user code can
/// execute; callers may borrow frame locals without cloning temporary roots.
pub(in crate::engine::vm) fn add_primitives_ref(
    runtime: &Runtime,
    left: &JsValue,
    right: &JsValue,
) -> Result<JsValue, Error> {
    if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
        // Perform primitive formatting in the original left-to-right order
        // before borrowing state. Existing String payloads remain borrowed,
        // as in the pre-handle Cow::Borrowed path; they need no temporary Rc.
        let formatted_left = if matches!(left, JsValue::String(_)) {
            None
        } else {
            Some(to_js_string_jsvalue(runtime, left)?)
        };
        let formatted_right = if matches!(right, JsValue::String(_)) {
            None
        } else {
            Some(to_js_string_jsvalue(runtime, right)?)
        };
        let result = {
            let state = runtime.0.state.borrow();
            let left = match left {
                JsValue::String(id) => state
                    .heap
                    .string(*id)
                    .map_err(|error| Error::internal(error.to_string()))?,
                _ => formatted_left.as_ref().expect("formatted left primitive"),
            };
            let right = match right {
                JsValue::String(id) => state
                    .heap
                    .string(*id)
                    .map_err(|error| Error::internal(error.to_string()))?,
                _ => formatted_right.as_ref().expect("formatted right primitive"),
            };
            left.try_concat(right).map_err(Error::from)?
        };
        return allocate_string_jsvalue(runtime, result);
    }
    match (left, right) {
        (left, right) if left.is_bigint() && right.is_bigint() => {
            let result = with_bigint_operands(runtime, left, right, |left, right| {
                left.add(right).map_err(bigint_error)
            })?;
            allocate_bigint_jsvalue(runtime, result)
        }
        (left, right) if left.is_bigint() => {
            to_number_jsvalue(runtime, right)?;
            Err(mixed_numeric_type_error())
        }
        (left, right) if right.is_bigint() => {
            to_number_jsvalue(runtime, left)?;
            Err(mixed_numeric_type_error())
        }
        (left, right) => {
            let left = to_number_jsvalue(runtime, left)?;
            let right = to_number_jsvalue(runtime, right)?;
            Ok(jsvalue_number(left + right))
        }
    }
}

#[cfg(test)]
#[path = "numeric/string_tests.rs"]
mod string_tests;
