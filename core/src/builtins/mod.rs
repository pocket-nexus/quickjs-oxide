//! The realm's builtins (new-core phase 3, section 6.2): exactly what the
//! V8 v7 programs call, plus the members a program reaches through them, as
//! leaf natives. One file per builtin object.
//!
//! Every native reads its receiver and arguments through [`CallSite`]. A
//! conversion that may run JavaScript (`ToString` of an object, `ToNumber`
//! through `valueOf`) goes through the re-entrant [`call`](crate::vm::call);
//! a native that holds a value across an allocation roots it first (the
//! phase 1 rule, enforced by `gc-stress`). Natives that call back into
//! JavaScript for their own work (`sort` with a comparator, `replace` with a
//! function) are phase 4's Step/Resume machines and are not here.

mod array;
mod boolean;
mod case;
mod error;
mod function;
mod global;
mod math;
mod number;
mod object;
mod regexp;
mod string;

use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::heap::string::StringRepr;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, StringId, Value};
use crate::vm::error::{self as vm_error, ErrorKind};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, ops};

/// A native's entry point.
pub(crate) type NativeFn = fn(&mut Cx<'_>, CallSite) -> Result<Value, Throw>;

macro_rules! natives {
    ($($(#[$doc:meta])* $name:ident => $function:path $(, $constructor:ident)?;)*) => {
        /// A host function, stored by index in its function record.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum Native {
            $($(#[$doc])* $name,)*
        }

        impl Native {
            /// Every native, in index order.
            pub const ALL: &'static [Native] = &[$(Native::$name,)*];

            #[inline]
            pub(crate) fn function(self) -> NativeFn {
                match self {
                    $(Native::$name => $function,)*
                }
            }

            /// Whether `new` may call it.
            pub const fn is_constructor(self) -> bool {
                match self {
                    $(Native::$name => natives!(@constructor $($constructor)?),)*
                }
            }
        }
    };
    (@constructor constructor) => { true };
    (@constructor) => { false };
}

natives! {
    /// qjs's `print`: the arguments' strings joined by a space, then a
    /// newline.
    Print => global::print;
    /// `Function.prototype.call`. The loop rewrites a call of it into a call
    /// of its receiver (section 5.2); the native runs only where no call
    /// site is rewritten.
    FunctionCall => function::call;
    /// `Function.prototype.apply`, rewritten by the loop as `call` is.
    FunctionApply => function::apply;
    /// `%Function.prototype%` itself, which returns `undefined`.
    FunctionPrototype => function::prototype;
    FunctionConstructor => function::constructor, constructor;
    FunctionToString => function::to_string;

    ParseInt => global::parse_int;
    ParseFloat => global::parse_float;
    IsNaN => global::is_nan;
    IsFinite => global::is_finite;

    ObjectConstructor => object::constructor, constructor;
    ObjectToString => object::to_string;
    ObjectValueOf => object::value_of;
    ObjectHasOwnProperty => object::has_own_property;

    ArrayConstructor => array::constructor, constructor;
    ArrayIsArray => array::is_array;
    ArrayPush => array::push;
    ArrayPop => array::pop;
    ArrayJoin => array::join;
    ArraySlice => array::slice;
    ArrayConcat => array::concat;
    ArrayIndexOf => array::index_of;
    ArrayToString => array::to_string;

    StringConstructor => string::constructor, constructor;
    StringFromCharCode => string::from_char_code;
    StringCharAt => string::char_at;
    StringCharCodeAt => string::char_code_at;
    StringIndexOf => string::index_of;
    StringLastIndexOf => string::last_index_of;
    StringSlice => string::slice;
    StringSubstring => string::substring;
    StringSubstr => string::substr;
    StringConcat => string::concat;
    StringSplit => string::split;
    StringReplace => string::replace;
    StringMatch => string::match_;
    StringToLowerCase => string::to_lower_case;
    StringToUpperCase => string::to_upper_case;
    /// `String.prototype.toString` and `valueOf`.
    StringValueOf => string::value_of;

    NumberConstructor => number::constructor, constructor;
    NumberToString => number::to_string;
    NumberValueOf => number::value_of;

    BooleanConstructor => boolean::constructor, constructor;
    BooleanToString => boolean::to_string;
    BooleanValueOf => boolean::value_of;

    MathAbs => math::abs;
    MathFloor => math::floor;
    MathCeil => math::ceil;
    MathRound => math::round;
    MathMin => math::min;
    MathMax => math::max;
    MathPow => math::pow;
    MathSqrt => math::sqrt;
    MathLog => math::log;
    MathExp => math::exp;
    MathSin => math::sin;
    MathCos => math::cos;
    MathTan => math::tan;
    MathAsin => math::asin;
    MathAcos => math::acos;
    MathAtan => math::atan;
    MathRandom => math::random;

    RegExpConstructor => regexp::constructor, constructor;
    RegExpExec => regexp::exec;
    RegExpTest => regexp::test;
    RegExpToString => regexp::to_string;
    RegExpSource => regexp::source;
    RegExpFlags => regexp::flags;
    RegExpGlobal => regexp::global;
    RegExpIgnoreCase => regexp::ignore_case;
    RegExpMultiline => regexp::multiline;

    ErrorConstructor => error::error, constructor;
    EvalErrorConstructor => error::eval_error, constructor;
    RangeErrorConstructor => error::range_error, constructor;
    ReferenceErrorConstructor => error::reference_error, constructor;
    SyntaxErrorConstructor => error::syntax_error, constructor;
    TypeErrorConstructor => error::type_error, constructor;
    InternalErrorConstructor => error::internal_error, constructor;
    ErrorToString => error::to_string;
}

impl Native {
    #[inline]
    pub(crate) const fn index(self) -> u32 {
        self as u32
    }

    #[inline]
    pub(crate) fn from_index(index: u32) -> Option<Native> {
        Native::ALL.get(index as usize).copied()
    }

    /// The constructor of an error kind.
    pub(crate) const fn error(kind: ErrorKind) -> Native {
        match kind {
            ErrorKind::Error => Native::ErrorConstructor,
            ErrorKind::Eval => Native::EvalErrorConstructor,
            ErrorKind::Range => Native::RangeErrorConstructor,
            ErrorKind::Reference => Native::ReferenceErrorConstructor,
            ErrorKind::Syntax => Native::SyntaxErrorConstructor,
            ErrorKind::Type => Native::TypeErrorConstructor,
            ErrorKind::Internal => Native::InternalErrorConstructor,
        }
    }
}

// ---- helpers for the natives ----

/// Throws `InternalError: unsupported: …` for a phase 4 construct.
pub(crate) fn unsupported(cx: &mut Cx<'_>, construct: &str) -> Throw {
    vm_error::throw(
        cx,
        ErrorKind::Internal,
        &format!("unsupported: {construct}"),
    )
}

/// `ToIntegerOrInfinity`.
pub(crate) fn to_integer(cx: &mut Cx<'_>, value: Value) -> Result<f64, Throw> {
    if let Some(int) = value.as_int32() {
        return Ok(f64::from(int));
    }
    let number = ops::to_number(cx, value)?;
    Ok(if number.is_nan() { 0.0 } else { number.trunc() })
}

/// A relative index (`slice`'s): negative counts from `length`, clamped to
/// `0..=length`.
pub(crate) fn relative(index: f64, length: usize) -> usize {
    let length = length as f64;
    let index = if index < 0.0 {
        (length + index).max(0.0)
    } else {
        index.min(length)
    };
    index as usize
}

/// A string value from UTF-16 units. A single unit is an atom, so `charAt`
/// and one-character slices do not allocate.
pub(crate) fn string_units(cx: &mut Cx<'_>, units: &[u16]) -> Result<Value, Throw> {
    let result = if units.len() == 1 {
        cx.heap.intern_units(units)
    } else if units.is_empty() {
        return Ok(Value::string(cx.realm.names.empty));
    } else {
        cx.heap.alloc_string_units(units)
    };
    result
        .map(Value::string)
        .map_err(|error| vm_error::heap_error(cx, error))
}

/// A string value from a representation already canonical.
pub(crate) fn string_repr(cx: &mut Cx<'_>, repr: StringRepr) -> Result<Value, Throw> {
    if repr.len() <= 1 {
        let units: Vec<u16> = repr.units().collect();
        return string_units(cx, &units);
    }
    cx.heap
        .alloc_string_repr(repr)
        .map(Value::string)
        .map_err(|error| vm_error::heap_error(cx, error))
}

/// A string value from Rust text.
pub(crate) fn string_text(cx: &mut Cx<'_>, text: &str) -> Result<Value, Throw> {
    if text.is_ascii() {
        return string_repr(cx, StringRepr::OneByte(text.as_bytes().into()));
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    string_units(cx, &units)
}

/// `ToString(value)` as a string id. A string made by the conversion is
/// rooted until the native returns (`native::call` cuts the root stack).
pub(crate) fn to_string_id(cx: &mut Cx<'_>, value: Value) -> Result<StringId, Throw> {
    if let Some(id) = value.as_string() {
        return Ok(id);
    }
    let string = ops::to_string(cx, value)?;
    cx.heap.roots().push(string);
    Ok(string.as_string().expect("a string"))
}

/// The receiver of a `String.prototype` method as a string
/// (`RequireObjectCoercible` then `ToString`).
pub(crate) fn this_string(cx: &mut Cx<'_>, site: &CallSite) -> Result<StringId, Throw> {
    let this = site.this(cx);
    if let Some(id) = this.as_string() {
        return Ok(id);
    }
    if this.is_undefined() || this.is_null() {
        return Err(vm_error::type_error(cx, "not an object"));
    }
    to_string_id(cx, this)
}

/// The receiver as an object; a primitive receiver is phase 4's wrapper.
pub(crate) fn this_object(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) => Ok(object),
        None if this.is_undefined() || this.is_null() => {
            Err(vm_error::type_error(cx, "not an object"))
        }
        None => Err(unsupported(cx, "wrapper objects")),
    }
}

// ---- installation ----

fn hidden() -> PropertyFlags {
    PropertyFlags::data(true, false, true)
}

/// Defines data property `name` of `object`; `value` is rooted for the call.
fn define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    name: &str,
    value: Value,
    flags: PropertyFlags,
) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(value);
    let result = cx
        .heap
        .property_key(name)
        .and_then(|key| cx.heap.add_property(object, key, flags, value));
    cx.heap.roots().truncate(mark);
    result
}

/// A native function object, left on the root stack.
fn native_function(cx: &mut Cx<'_>, native: Native) -> Result<ObjectId, HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
    let function = cx
        .heap
        .alloc_native_function(Some(prototype), native.index())?;
    cx.heap.roots().push(Value::object(function));
    Ok(function)
}

/// Defines each `(name, native)` as a non-enumerable method of `object`.
fn methods(cx: &mut Cx<'_>, object: ObjectId, list: &[(&str, Native)]) -> Result<(), HeapError> {
    for &(name, native) in list {
        let function = native_function(cx, native)?;
        define(cx, object, name, Value::object(function), hidden())?;
    }
    Ok(())
}

/// A constructor with its `prototype` (non-writable) and the prototype's
/// `constructor`, defined on the global object as `name`.
fn install_constructor(
    cx: &mut Cx<'_>,
    name: &str,
    native: Native,
    prototype: ObjectId,
) -> Result<ObjectId, HeapError> {
    let function = native_function(cx, native)?;
    let names = cx.realm.names;
    cx.heap.add_property(
        function,
        Names::key(names.prototype),
        PropertyFlags::data(false, false, false),
        Value::object(prototype),
    )?;
    cx.heap.add_property(
        prototype,
        Names::key(names.constructor),
        hidden(),
        Value::object(function),
    )?;
    let global = cx.realm.global();
    define(cx, global, name, Value::object(function), hidden())?;
    Ok(function)
}

/// The realm's globals, constructors and prototypes. Returns the
/// preallocated out-of-memory error.
pub(crate) fn install(cx: &mut Cx<'_>) -> Result<ObjectId, HeapError> {
    let mark = cx.heap.roots().mark();
    let result = install_rooted(cx);
    cx.heap.roots().truncate(mark);
    result
}

fn install_rooted(cx: &mut Cx<'_>) -> Result<ObjectId, HeapError> {
    let global = cx.realm.global();
    let constant = PropertyFlags::data(false, false, false);
    define(cx, global, "globalThis", Value::object(global), hidden())?;
    define(cx, global, "undefined", Value::UNDEFINED, constant)?;
    define(cx, global, "NaN", Value::number(f64::NAN), constant)?;
    define(
        cx,
        global,
        "Infinity",
        Value::number(f64::INFINITY),
        constant,
    )?;
    methods(
        cx,
        global,
        &[
            ("parseInt", Native::ParseInt),
            ("parseFloat", Native::ParseFloat),
            ("isNaN", Native::IsNaN),
            ("isFinite", Native::IsFinite),
        ],
    )?;
    object::install(cx)?;
    function::install(cx)?;
    array::install(cx)?;
    string::install(cx)?;
    number::install(cx)?;
    boolean::install(cx)?;
    math::install(cx)?;
    regexp::install(cx)?;
    error::install(cx)?;
    let message = cx.heap.intern("out of memory")?;
    let error = vm_error::new_error(cx, ErrorKind::Internal, Value::string(message))?;
    Ok(error.as_object().unwrap())
}

/// Defines `print` on the global object.
pub(crate) fn install_print(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = native_function(cx, Native::Print).and_then(|print| {
        let global = cx.realm.global();
        define(cx, global, "print", Value::object(print), hidden())
    });
    cx.heap.roots().truncate(mark);
    result
}

/// Whether `object` is an error (the `ERROR` flag `new_error` sets).
pub(crate) fn is_error(cx: &Cx<'_>, object: ObjectId) -> bool {
    cx.heap
        .header(object)
        .has(crate::heap::object::flags::ERROR)
}

/// An ordinary object inheriting from `%Object.prototype%`, rooted.
pub(crate) fn new_plain(cx: &mut Cx<'_>, slots: u32) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    cx.heap
        .alloc_object(ObjectKind::Ordinary, Some(prototype), slots)
        .map_err(|error| vm_error::heap_error(cx, error))
}
