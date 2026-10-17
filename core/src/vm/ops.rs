//! The loop's slow paths: type conversions and the operators past their
//! `Int32` and double fast paths. Each takes its operands as values that are
//! still in their registers (so rooted) and roots any intermediate it holds
//! across a second allocation or a call back into JavaScript.

use std::cmp::Ordering;

use super::bigint::{Big, BigError};
use super::error;
use super::number;
use super::property;
use super::{Cx, Throw};
use crate::heap::Heap;
use crate::heap::atom::PropertyKey;
use crate::heap::object::{NONE, flags};
use crate::heap::string::StringRepr;
use crate::runtime::Names;
use crate::value::{Kind, ObjectId, StringId, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hint {
    Default,
    Number,
    String,
}

/// Runs `body` with the root stack restored afterwards.
#[inline]
fn scoped<T>(cx: &mut Cx<'_>, body: impl FnOnce(&mut Cx<'_>) -> T) -> T {
    let mark = cx.heap.roots().mark();
    let result = body(cx);
    cx.heap.roots().truncate(mark);
    result
}

#[inline]
fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

pub(crate) fn heap_error(cx: &Cx<'_>, error: crate::HeapError) -> Throw {
    error::heap_error(cx, error)
}

// ---- predicates ----

#[inline]
pub(crate) fn is_callable(heap: &Heap, value: Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| heap.header(object).has(flags::CALLABLE))
}

#[inline(never)]
pub(crate) fn to_boolean(heap: &Heap, value: Value) -> bool {
    match value.kind() {
        Kind::Bool => value == Value::TRUE,
        Kind::Int32 => value.as_int32() != Some(0),
        Kind::Number => {
            let number = value.as_f64().unwrap();
            number != 0.0 && !number.is_nan()
        }
        Kind::Undefined | Kind::Null | Kind::Empty => false,
        Kind::String => heap.string_len(value.as_string().unwrap()) != 0,
        Kind::BigInt => !heap.bigint(value.as_bigint().unwrap()).limbs.is_empty(),
        Kind::Object => !is_html_dda(heap, value),
        Kind::Symbol => true,
    }
}

/// Whether `value` is Test262's `IsHTMLDDA` object.
#[inline]
pub(crate) fn is_html_dda(heap: &Heap, value: Value) -> bool {
    heap.html_dda
        && value.as_object().is_some_and(|object| {
            heap.object_kind(object) == crate::ObjectKind::Builtin
                && heap.internal_tag(object) == crate::heap::object::InternalTag::HtmlDda
        })
}

#[inline(never)]
pub(crate) fn type_of(cx: &Cx<'_>, value: Value) -> Value {
    let names = &cx.realm.names;
    let name = match value.kind() {
        Kind::Number | Kind::Int32 => names.number,
        Kind::Bool => names.boolean,
        Kind::Undefined | Kind::Empty => names.undefined,
        Kind::Null => names.object,
        Kind::String => names.string,
        Kind::Symbol => names.symbol,
        Kind::BigInt => names.bigint,
        Kind::Object => {
            if is_html_dda(cx.heap, value) {
                names.undefined
            } else if is_callable(cx.heap, value) {
                names.function
            } else {
                names.object
            }
        }
    };
    Value::string(name)
}

// ---- strings ----

pub(crate) fn units(heap: &Heap, string: StringId) -> Vec<u16> {
    heap.string(string).repr().units().collect()
}

pub(crate) fn string_text(heap: &Heap, value: Value) -> String {
    heap.string_text(value.as_string().expect("a string"))
}

fn string_eq(heap: &Heap, a: StringId, b: StringId) -> bool {
    a == b || heap.string(a).repr() == heap.string(b).repr()
}

fn string_cmp(heap: &Heap, a: StringId, b: StringId) -> Ordering {
    let (a, b) = (&heap.string(a).repr(), &heap.string(b).repr());
    a.units().cmp(b.units())
}

fn alloc_text(cx: &mut Cx<'_>, text: &str) -> Result<Value, Throw> {
    cx.heap
        .alloc_string(text)
        .map(Value::string)
        .map_err(|error| heap_error(cx, error))
}

pub(crate) fn alloc_units(cx: &mut Cx<'_>, units: &[u16]) -> Result<Value, Throw> {
    cx.heap
        .alloc_string_units(units)
        .map(Value::string)
        .map_err(|error| heap_error(cx, error))
}

/// The shortest concatenation made a rope.
const ROPE_MIN: usize = 64;

/// QuickJS's `JS_STRING_LEN_MAX`.
pub(crate) const MAX_STRING_LEN: usize = (1 << 30) - 1;

/// `a + b` for two strings.
///
/// A result of at least [`ROPE_MIN`] units is a rope (section 3.3); a
/// shorter one is copied, which keeps short keys flat.
pub(crate) fn concat(cx: &mut Cx<'_>, a: StringId, b: StringId) -> Result<Value, Throw> {
    let (left_len, right_len) = (cx.heap.string_len(a), cx.heap.string_len(b));
    if right_len == 0 {
        return Ok(Value::string(a));
    }
    if left_len == 0 {
        return Ok(Value::string(b));
    }
    if left_len + right_len >= ROPE_MIN {
        if left_len + right_len > MAX_STRING_LEN {
            return Err(error::string_too_long(cx));
        }
        return cx
            .heap
            .alloc_rope(a, b)
            .map(Value::string)
            .map_err(|error| heap_error(cx, error));
    }
    let (left, right) = (cx.heap.string(a).repr(), cx.heap.string(b).repr());
    // Two one-byte strings join as bytes; the result is canonical as is.
    if let (StringRepr::OneByte(left), StringRepr::OneByte(right)) = (left, right) {
        let mut joined = Vec::with_capacity(left.len() + right.len());
        joined.extend_from_slice(left);
        joined.extend_from_slice(right);
        return cx
            .heap
            .alloc_string_repr(StringRepr::OneByte(joined.into()))
            .map(Value::string)
            .map_err(|error| heap_error(cx, error));
    }
    let mut joined = Vec::with_capacity(left.len() + right.len());
    joined.extend(left.units());
    joined.extend(right.units());
    alloc_units(cx, &joined)
}

// ---- conversions ----

/// `ToPrimitive`: `valueOf` then `toString` (or the reverse for a string
/// hint), the first that returns a primitive.
pub(crate) fn to_primitive(cx: &mut Cx<'_>, value: Value, hint: Hint) -> Result<Value, Throw> {
    let Some(object) = value.as_object() else {
        return Ok(value);
    };
    if !cx.heap.vm.enter_native() {
        return Err(error::stack_overflow(cx));
    }
    let result = scoped(cx, |cx| {
        root(cx, value);
        to_primitive_object(cx, object, hint)
    });
    cx.heap.vm.leave_native();
    result
}

fn to_primitive_object(cx: &mut Cx<'_>, object: ObjectId, hint: Hint) -> Result<Value, Throw> {
    let names = cx.realm.names;
    // `@@toPrimitive` first (Date and Symbol prototypes have one).
    let key = crate::runtime::WellKnown::key(cx.realm.symbols.to_primitive);
    let exotic = match property::lookup(cx.heap, &names, object, key) {
        property::Lookup::Miss => Value::UNDEFINED,
        property::Lookup::Hit(property::Found::Data(value)) => value,
        _ => property::get(cx, Value::object(object), key)?,
    };
    if !exotic.is_undefined() && !exotic.is_null() {
        if !is_callable(cx.heap, exotic) {
            return Err(error::type_error(cx, "not a function"));
        }
        let hint = match hint {
            Hint::Default => names.default,
            Hint::Number => names.number,
            Hint::String => names.string,
        };
        let result = super::call(cx, exotic, Value::object(object), &[Value::string(hint)])?;
        if result.as_object().is_some() {
            return Err(error::type_error(cx, "toPrimitive"));
        }
        return Ok(result);
    }
    ordinary_to_primitive(cx, object, hint)
}

/// `OrdinaryToPrimitive(object, hint)`.
pub(crate) fn ordinary_to_primitive(
    cx: &mut Cx<'_>,
    object: ObjectId,
    hint: Hint,
) -> Result<Value, Throw> {
    let names = cx.realm.names;
    let order = if hint == Hint::String {
        [names.to_string, names.value_of]
    } else {
        [names.value_of, names.to_string]
    };
    for name in order {
        let method = property::get_value(cx, object, Names::key(name))?;
        if is_callable(cx.heap, method) {
            let result = super::call(cx, method, Value::object(object), &[])?;
            if result.as_object().is_none() {
                return Ok(result);
            }
        }
    }
    Err(error::type_error(cx, "toPrimitive"))
}

/// `Error.prototype.toString`.
pub(crate) fn error_text(cx: &mut Cx<'_>, object: ObjectId) -> Result<Value, Throw> {
    let names = cx.realm.names;
    let part = |cx: &mut Cx<'_>, key: StringId, default: &str| -> Result<Vec<u16>, Throw> {
        let value = property::get_value(cx, object, Names::key(key))?;
        if value.is_undefined() {
            return Ok(default.encode_utf16().collect());
        }
        let text = to_string(cx, value)?;
        Ok(units(cx.heap, text.as_string().unwrap()))
    };
    let name = part(cx, names.name, "Error")?;
    let message = part(cx, names.message, "")?;
    let mut text = name;
    if !text.is_empty() && !message.is_empty() {
        text.extend(": ".encode_utf16());
    }
    text.extend(message);
    alloc_units(cx, &text)
}

/// `ToString`; the result is a string value.
pub(crate) fn to_string(cx: &mut Cx<'_>, value: Value) -> Result<Value, Throw> {
    let names = &cx.realm.names;
    match value.kind() {
        Kind::String => Ok(value),
        Kind::Int32 => alloc_text(cx, &value.as_int32().unwrap().to_string()),
        Kind::Number => alloc_text(cx, &number::to_string(value.as_f64().unwrap())),
        Kind::Bool => Ok(Value::string(if value == Value::TRUE {
            names.true_
        } else {
            names.false_
        })),
        Kind::Undefined | Kind::Empty => Ok(Value::string(names.undefined)),
        Kind::Null => Ok(Value::string(names.null)),
        Kind::Symbol => Err(error::type_error(cx, "cannot convert symbol to string")),
        Kind::BigInt => {
            let text = big(cx.heap, value).to_string_radix10();
            alloc_text(cx, &text)
        }
        Kind::Object => {
            let primitive = to_primitive(cx, value, Hint::String)?;
            to_string(cx, primitive)
        }
    }
}

/// `ToString` as Rust text.
pub(crate) fn display(cx: &mut Cx<'_>, value: Value) -> Result<String, Throw> {
    let text = to_string(cx, value)?;
    Ok(string_text(cx.heap, text))
}

/// `ToNumber`.
pub(crate) fn to_number(cx: &mut Cx<'_>, value: Value) -> Result<f64, Throw> {
    match value.kind() {
        Kind::Int32 | Kind::Number => Ok(value.as_f64().unwrap()),
        Kind::Undefined | Kind::Empty => Ok(f64::NAN),
        Kind::Null => Ok(0.0),
        Kind::Bool => Ok(if value == Value::TRUE { 1.0 } else { 0.0 }),
        Kind::String => {
            let repr = &cx.heap.string(value.as_string().unwrap()).repr();
            Ok(number::parse(&repr.units().collect::<Vec<_>>()))
        }
        Kind::Symbol => Err(error::type_error(cx, "cannot convert symbol to number")),
        Kind::BigInt => Err(error::type_error(cx, "cannot convert bigint to number")),
        Kind::Object => {
            let primitive = to_primitive(cx, value, Hint::Number)?;
            to_number(cx, primitive)
        }
    }
}

/// `ToNumeric`: a number value or a BigInt.
pub(crate) fn to_numeric(cx: &mut Cx<'_>, value: Value) -> Result<Value, Throw> {
    if value.is_number() {
        return Ok(value);
    }
    let primitive = to_primitive(cx, value, Hint::Number)?;
    if primitive.as_bigint().is_some() {
        return Ok(primitive);
    }
    Ok(number::number(to_number(cx, primitive)?))
}

/// `ToPropertyKey`.
#[inline(never)]
pub(crate) fn to_property_key(cx: &mut Cx<'_>, value: Value) -> Result<PropertyKey, Throw> {
    match value.kind() {
        Kind::Int32 => {
            let int = value.as_int32().unwrap();
            if let Some(key) = u32::try_from(int).ok().and_then(PropertyKey::index) {
                return Ok(key);
            }
            key_from_text(cx, &int.to_string())
        }
        Kind::String => {
            let id = value.as_string().unwrap();
            let record = cx.heap.string(id);
            if record.is_atom {
                return Ok(if record.array_index == NONE {
                    PropertyKey::atom(id)
                } else {
                    PropertyKey::index(record.array_index).unwrap_or(PropertyKey::atom(id))
                });
            }
            let units = units(cx.heap, id);
            cx.heap
                .property_key_units(&units)
                .map_err(|error| heap_error(cx, error))
        }
        Kind::Symbol => Ok(PropertyKey::symbol(value.as_symbol().unwrap())),
        Kind::Object => {
            let primitive = to_primitive(cx, value, Hint::String)?;
            to_property_key(cx, primitive)
        }
        _ => {
            let text = to_string(cx, value)?;
            to_property_key(cx, text)
        }
    }
}

fn key_from_text(cx: &mut Cx<'_>, text: &str) -> Result<PropertyKey, Throw> {
    cx.heap
        .property_key(text)
        .map_err(|error| heap_error(cx, error))
}

// ---- BigInt ----

pub(crate) fn big(heap: &Heap, value: Value) -> Big {
    let record = heap.bigint(value.as_bigint().expect("a BigInt"));
    Big::new(record.negative, record.limbs.to_vec())
}

fn big_value(cx: &mut Cx<'_>, value: Result<Big, BigError>) -> Result<Value, Throw> {
    match value {
        Ok(value) => cx
            .heap
            .alloc_bigint(value.negative, &value.limbs)
            .map(Value::bigint)
            .map_err(|error| heap_error(cx, error)),
        Err(BigError::DivisionByZero) => Err(error::range_error(cx, "BigInt division by zero")),
        Err(BigError::NegativeExponent) => Err(error::range_error(cx, "BigInt negative exponent")),
        Err(BigError::TooLarge) => Err(error::range_error(cx, "BigInt is too large to allocate")),
        Err(BigError::PowTooLarge) => Err(error::range_error(cx, "BigInt is too large")),
    }
}

/// A BigInt and a number in one operator: QuickJS converts the BigInt to a
/// number and fails (`>>>` refuses any BigInt operand first).
#[cold]
#[inline(never)]
fn mixed(cx: &mut Cx<'_>, op: Arith) -> Throw {
    if op == Arith::Shr {
        return shr_bigint(cx);
    }
    error::type_error(cx, "cannot convert bigint to number")
}

#[cold]
#[inline(never)]
fn shr_bigint(cx: &mut Cx<'_>) -> Throw {
    error::type_error(cx, "bigint operands are forbidden for >>>")
}

// ---- arithmetic ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arith {
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
}

/// `x + y` past the number fast paths: string concatenation or numeric
/// addition after `ToPrimitive`.
#[inline(never)]
pub(crate) fn add(cx: &mut Cx<'_>, x: Value, y: Value) -> Result<Value, Throw> {
    if let (Some(a), Some(b)) = (x.as_string(), y.as_string()) {
        return concat(cx, a, b);
    }
    scoped(cx, |cx| {
        let px = to_primitive(cx, x, Hint::Default)?;
        root(cx, px);
        let py = to_primitive(cx, y, Hint::Default)?;
        root(cx, py);
        if px.as_string().is_some() || py.as_string().is_some() {
            let sx = to_string(cx, px)?;
            root(cx, sx);
            let sy = to_string(cx, py)?;
            return concat(cx, sx.as_string().unwrap(), sy.as_string().unwrap());
        }
        numeric(cx, Arith::Add, px, py)
    })
}

/// A binary numeric operator on any operands.
#[inline(never)]
pub(crate) fn arith(cx: &mut Cx<'_>, op: Arith, x: Value, y: Value) -> Result<Value, Throw> {
    if op == Arith::Add {
        return add(cx, x, y);
    }
    scoped(cx, |cx| {
        let a = to_numeric(cx, x)?;
        root(cx, a);
        let b = to_numeric(cx, y)?;
        root(cx, b);
        numeric(cx, op, a, b)
    })
}

/// `op` on two primitives (converted with `ToNumeric` here).
fn numeric(cx: &mut Cx<'_>, op: Arith, x: Value, y: Value) -> Result<Value, Throw> {
    let a = to_numeric(cx, x)?;
    let b = to_numeric(cx, y)?;
    match (a.as_bigint().is_some(), b.as_bigint().is_some()) {
        (false, false) => Ok(number_op(op, a.as_f64().unwrap(), b.as_f64().unwrap())),
        (true, true) => {
            let (a, b) = (big(cx.heap, a), big(cx.heap, b));
            let result = match op {
                Arith::Add => a.add(&b),
                Arith::Sub => a.sub(&b),
                Arith::Mul => a.mul(&b),
                Arith::Div => a.div_rem(&b).map(|(quotient, _)| quotient),
                Arith::Mod => a.div_rem(&b).map(|(_, remainder)| remainder),
                Arith::Pow => a.pow(&b),
                Arith::Shl => a.shl(&b),
                Arith::Sar => a.shl(&b.neg()),
                Arith::Shr => return Err(shr_bigint(cx)),
                Arith::BitAnd => Ok(a.bit_and(&b)),
                Arith::BitOr => Ok(a.bit_or(&b)),
                Arith::BitXor => Ok(a.bit_xor(&b)),
            };
            big_value(cx, result)
        }
        _ => Err(mixed(cx, op)),
    }
}

pub(crate) fn number_op(op: Arith, a: f64, b: f64) -> Value {
    let int = |value: i32| Value::int32(value);
    match op {
        Arith::Add => number::number(a + b),
        Arith::Sub => number::number(a - b),
        Arith::Mul => number::number(a * b),
        Arith::Div => number::number(a / b),
        Arith::Mod => number::number(a % b),
        Arith::Pow => number::number(number::pow(a, b)),
        Arith::Shl => int(number::to_int32(a).wrapping_shl(number::to_uint32(b) & 31)),
        Arith::Sar => int(number::to_int32(a) >> (number::to_uint32(b) & 31)),
        Arith::Shr => {
            let shifted = number::to_uint32(a) >> (number::to_uint32(b) & 31);
            number::number(f64::from(shifted))
        }
        Arith::BitAnd => int(number::to_int32(a) & number::to_int32(b)),
        Arith::BitOr => int(number::to_int32(a) | number::to_int32(b)),
        Arith::BitXor => int(number::to_int32(a) ^ number::to_int32(b)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unary {
    Neg,
    Plus,
    ToNumeric,
    Inc,
    Dec,
    BitNot,
}

#[inline(never)]
pub(crate) fn unary(cx: &mut Cx<'_>, op: Unary, x: Value) -> Result<Value, Throw> {
    if op == Unary::Plus {
        let value = to_numeric(cx, x)?;
        let Some(number) = value.as_f64() else {
            return Err(error::type_error(cx, "bigint argument with unary +"));
        };
        return Ok(number::number(number));
    }
    let value = to_numeric(cx, x)?;
    if value.as_bigint().is_some() {
        let value = big(cx.heap, value);
        let one = Big::from_i64(1);
        let result = match op {
            Unary::Neg => Ok(value.neg()),
            Unary::ToNumeric => Ok(value),
            Unary::Inc => value.add(&one),
            Unary::Dec => value.sub(&one),
            Unary::BitNot => Ok(value.bit_not()),
            Unary::Plus => unreachable!(),
        };
        return big_value(cx, result);
    }
    let number = value.as_f64().unwrap();
    Ok(match op {
        Unary::Neg => number::number(-number),
        Unary::ToNumeric => value,
        Unary::Inc => number::number(number + 1.0),
        Unary::Dec => number::number(number - 1.0),
        Unary::BitNot => Value::int32(!number::to_int32(number)),
        Unary::Plus => unreachable!(),
    })
}

// ---- comparison ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Relation {
    Lt,
    Lte,
    Gt,
    Gte,
}

/// The value of `x op y`: `false` when the comparison is undefined (NaN).
#[inline(never)]
pub(crate) fn relation(cx: &mut Cx<'_>, op: Relation, x: Value, y: Value) -> Result<bool, Throw> {
    Ok(match op {
        Relation::Lt => less(cx, x, y, true)? == Some(true),
        Relation::Gt => less(cx, y, x, false)? == Some(true),
        Relation::Lte => less(cx, y, x, false)? == Some(false),
        Relation::Gte => less(cx, x, y, true)? == Some(false),
    })
}

/// `IsLessThan(x, y, LeftFirst)`: `None` for undefined.
fn less(cx: &mut Cx<'_>, x: Value, y: Value, left_first: bool) -> Result<Option<bool>, Throw> {
    scoped(cx, |cx| {
        let (px, py) = if left_first {
            let px = to_primitive(cx, x, Hint::Number)?;
            root(cx, px);
            (px, to_primitive(cx, y, Hint::Number)?)
        } else {
            let py = to_primitive(cx, y, Hint::Number)?;
            root(cx, py);
            (to_primitive(cx, x, Hint::Number)?, py)
        };
        root(cx, px);
        root(cx, py);
        if let (Some(a), Some(b)) = (px.as_string(), py.as_string()) {
            return Ok(Some(string_cmp(cx.heap, a, b) == Ordering::Less));
        }
        let parse_big = |cx: &Cx<'_>, value: Value| Big::parse(&string_text(cx.heap, value));
        if px.as_bigint().is_some() && py.as_string().is_some() {
            return Ok(parse_big(cx, py).map(|b| big(cx.heap, px).cmp(&b) == Ordering::Less));
        }
        if px.as_string().is_some() && py.as_bigint().is_some() {
            return Ok(parse_big(cx, px).map(|a| a.cmp(&big(cx.heap, py)) == Ordering::Less));
        }
        let a = to_numeric(cx, px)?;
        let b = to_numeric(cx, py)?;
        Ok(match (a.as_bigint().is_some(), b.as_bigint().is_some()) {
            (false, false) => {
                let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                if a.is_nan() || b.is_nan() {
                    None
                } else {
                    Some(a < b)
                }
            }
            (true, true) => Some(big(cx.heap, a).cmp(&big(cx.heap, b)) == Ordering::Less),
            (true, false) => compare_big_number(&big(cx.heap, a), b.as_f64().unwrap())
                .map(|order| order == Ordering::Less),
            (false, true) => compare_big_number(&big(cx.heap, b), a.as_f64().unwrap())
                .map(|order| order == Ordering::Greater),
        })
    })
}

/// The order of a BigInt against a number, `None` for NaN.
fn compare_big_number(a: &Big, b: f64) -> Option<Ordering> {
    if b.is_nan() {
        return None;
    }
    if b.is_infinite() {
        return Some(if b > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let floor = b.floor();
    let order = a.cmp(&Big::from_f64(floor).expect("finite integral"));
    Some(if order == Ordering::Equal && floor != b {
        Ordering::Less
    } else {
        order
    })
}

/// `IsStrictlyEqual`.
pub(crate) fn strict_eq(heap: &Heap, x: Value, y: Value) -> bool {
    if x.is_number() && y.is_number() {
        return x.as_f64() == y.as_f64();
    }
    if let (Some(a), Some(b)) = (x.as_string(), y.as_string()) {
        return string_eq(heap, a, b);
    }
    if let (Some(a), Some(b)) = (x.as_bigint(), y.as_bigint()) {
        return heap.bigint(a) == heap.bigint(b);
    }
    x == y
}

/// `SameValueZero` over the heap.
pub(crate) fn same_value_zero_heap(heap: &Heap, x: Value, y: Value) -> bool {
    if let (Some(a), Some(b)) = (x.as_f64(), y.as_f64()) {
        return a == b || (a.is_nan() && b.is_nan());
    }
    strict_eq(heap, x, y)
}

/// `IsLooselyEqual`.
#[inline(never)]
pub(crate) fn loose_eq(cx: &mut Cx<'_>, x: Value, y: Value) -> Result<bool, Throw> {
    let (kx, ky) = (class(x), class(y));
    if kx == ky {
        return Ok(kx == Class::Nullish || strict_eq(cx.heap, x, y));
    }
    use Class as C;
    Ok(match (kx, ky) {
        (C::Nullish, C::Object) => is_html_dda(cx.heap, y),
        (C::Object, C::Nullish) => is_html_dda(cx.heap, x),
        (C::Nullish, _) | (_, C::Nullish) => false,
        (C::Number, C::String) => {
            let b = to_number(cx, y)?;
            x.as_f64() == Some(b)
        }
        (C::String, C::Number) => {
            let a = to_number(cx, x)?;
            Some(a) == y.as_f64()
        }
        (C::BigInt, C::String) => match Big::parse(&string_text(cx.heap, y)) {
            Some(b) => big(cx.heap, x) == b,
            None => false,
        },
        (C::String, C::BigInt) => return loose_eq(cx, y, x),
        (C::Bool, _) => {
            let a = number::number(to_number(cx, x)?);
            return loose_eq(cx, a, y);
        }
        (_, C::Bool) => {
            let b = number::number(to_number(cx, y)?);
            return loose_eq(cx, x, b);
        }
        (C::Object, C::Number | C::String | C::BigInt | C::Symbol) => {
            let primitive = to_primitive(cx, x, Hint::Default)?;
            return scoped(cx, |cx| {
                root(cx, primitive);
                loose_eq(cx, primitive, y)
            });
        }
        (C::Number | C::String | C::BigInt | C::Symbol, C::Object) => {
            let primitive = to_primitive(cx, y, Hint::Default)?;
            return scoped(cx, |cx| {
                root(cx, primitive);
                loose_eq(cx, x, primitive)
            });
        }
        (C::BigInt, C::Number) => {
            compare_big_number(&big(cx.heap, x), y.as_f64().unwrap()) == Some(Ordering::Equal)
        }
        (C::Number, C::BigInt) => {
            compare_big_number(&big(cx.heap, y), x.as_f64().unwrap()) == Some(Ordering::Equal)
        }
        _ => false,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Nullish,
    Number,
    String,
    Bool,
    Symbol,
    BigInt,
    Object,
}

fn class(value: Value) -> Class {
    match value.kind() {
        Kind::Undefined | Kind::Null | Kind::Empty => Class::Nullish,
        Kind::Number | Kind::Int32 => Class::Number,
        Kind::String => Class::String,
        Kind::Bool => Class::Bool,
        Kind::Symbol => Class::Symbol,
        Kind::BigInt => Class::BigInt,
        Kind::Object => Class::Object,
    }
}

/// `x instanceof f`: `f[Symbol.hasInstance]` when it is not the intrinsic
/// one, else `OrdinaryHasInstance`.
#[inline(never)]
pub(crate) fn instance_of(cx: &mut Cx<'_>, x: Value, f: Value) -> Result<bool, Throw> {
    let Some(function) = f.as_object() else {
        return Err(error::type_error(cx, "invalid 'instanceof' right operand"));
    };
    // A constructor closure with its cached shape: `prototype` is its third
    // slot and `@@hasInstance` the intrinsic one (below).
    let shape = cx.heap.shape_of(function);
    if cx.heap.closure_shapes.contains(&Some(shape))
        && cx.heap.shape(shape).width == 3
        && cx.heap.prototype_of_shape(shape)
            == Some(
                cx.realm
                    .intrinsic(crate::runtime::Intrinsic::FunctionPrototype),
            )
        && let Some(prototype) = cx.heap.get_slot(function, 2).as_object()
    {
        let Some(object) = x.as_object() else {
            return Ok(false);
        };
        return inherits(cx, f, object, prototype);
    }
    let key = crate::runtime::WellKnown::key(cx.realm.symbols.has_instance);
    // A closure with only its own `length`, `name` and `prototype` inherits
    // `@@hasInstance` from `%Function.prototype%` (non-writable and
    // non-configurable there): no lookup needed.
    let function_prototype = cx
        .realm
        .intrinsic(crate::runtime::Intrinsic::FunctionPrototype);
    let plain_closure = cx.heap.object_kind(function) == crate::ObjectKind::Function
        && cx.heap.prototype_of(function) == Some(function_prototype)
        && cx
            .heap
            .closure_shapes
            .contains(&Some(cx.heap.shape_of(function)));
    let handler = if plain_closure {
        Value::UNDEFINED
    } else {
        match property::lookup(cx.heap, &cx.realm.names, function, key) {
            property::Lookup::Hit(property::Found::Data(handler)) => handler,
            property::Lookup::Miss => Value::UNDEFINED,
            _ => property::get(cx, f, key)?,
        }
    };
    let intrinsic = plain_closure
        || handler.as_object().is_some_and(|handler| {
            cx.heap.object_kind(handler) == crate::ObjectKind::Function
                && cx.heap.function_record(handler).native
                    == crate::builtins::Native::FunctionHasInstance.index()
        });
    if !intrinsic && !handler.is_undefined() && !handler.is_null() {
        if !is_callable(cx.heap, handler) {
            return Err(error::type_error(cx, "not a function"));
        }
        let result = super::call(cx, handler, f, &[x])?;
        return Ok(to_boolean(cx.heap, result));
    }
    if !is_callable(cx.heap, f) {
        return Err(error::type_error(cx, "invalid 'instanceof' right operand"));
    }
    let Some(object) = x.as_object() else {
        return Ok(false);
    };
    if cx.heap.object_kind(function) != crate::ObjectKind::Function {
        return super::object::ordinary_has_instance(cx, f, x);
    }
    let names = cx.realm.names;
    // The constructor's own data `prototype` is read straight from its slot
    // (phase 3 M5, on Earley-Boyer's `instanceof` tests).
    let prototype = match cx.heap.lookup_own(function, Names::key(names.prototype)) {
        Some((slot, flags)) if !flags.has(crate::shape::PropertyFlags::ACCESSOR) => {
            cx.heap.get_slot(function, slot)
        }
        _ => property::get_value(cx, function, Names::key(names.prototype))?,
    };
    let Some(prototype) = prototype.as_object() else {
        return Err(error::type_error(
            cx,
            "operand 'prototype' property is not an object",
        ));
    };
    inherits(cx, f, object, prototype)
}

/// Whether `prototype` is on the chain of `object` (a proxy on the way
/// hands the rest to `OrdinaryHasInstance`).
#[inline]
fn inherits(
    cx: &mut Cx<'_>,
    f: Value,
    mut object: ObjectId,
    prototype: ObjectId,
) -> Result<bool, Throw> {
    loop {
        if cx.heap.object_kind(object) == crate::ObjectKind::Proxy {
            return super::object::ordinary_has_instance(cx, f, Value::object(object));
        }
        let Some(next) = cx.heap.prototype_of(object) else {
            return Ok(false);
        };
        if next == prototype {
            return Ok(true);
        }
        object = next;
    }
}

/// `key in object`.
#[inline(never)]
pub(crate) fn has_property(cx: &mut Cx<'_>, key: Value, object: Value) -> Result<bool, Throw> {
    let Some(target) = object.as_object() else {
        return Err(error::type_error(cx, "invalid 'in' operand"));
    };
    let key = to_property_key(cx, key)?;
    property::has(cx, target, key)
}
