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

pub mod agent;
mod array;
mod atomics;
mod base64;
mod bigint;
mod boolean;
mod buffer;
mod case;
mod collection;
mod date;
mod error;
mod function;
mod generator;
mod global;
mod iterator;
mod iterator_helpers;
mod json;
mod math;
mod number;
mod object;
mod promise;
mod proxy;
mod reflect;
mod regexp;
mod string;
mod symbol;
pub(crate) mod test262;
mod typed_array;
mod uri;

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
    ($($(#[$doc:meta])* $name:ident => $function:path, $js:expr, $length:expr $(, $constructor:ident)?;)*) => {
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

            /// Its `name` property (a getter's without the `get `).
            pub const fn js_name(self) -> &'static str {
                match self {
                    $(Native::$name => $js,)*
                }
            }

            /// Its `length` property.
            pub const fn length(self) -> u32 {
                match self {
                    $(Native::$name => $length,)*
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
    Print => global::print, "print", 1;
    /// `Function.prototype.call`. The loop rewrites a call of it into a call
    /// of its receiver (section 5.2); the native runs only where no call
    /// site is rewritten.
    FunctionCall => function::call, "call", 1;
    /// `Function.prototype.apply`, rewritten by the loop as `call` is.
    FunctionApply => function::apply, "apply", 2;
    /// `%Function.prototype%` itself, which returns `undefined`.
    FunctionPrototype => function::prototype, "", 0;
    FunctionConstructor => function::constructor, "Function", 1, constructor;
    FunctionToString => function::to_string, "toString", 0;
    FunctionBind => function::bind, "bind", 1;
    FunctionHasInstance => function::has_instance, "[Symbol.hasInstance]", 1;
    FunctionFileName => function::file_name, "fileName", 0;
    FunctionLineNumber => function::line_number, "lineNumber", 0;
    FunctionColumnNumber => function::column_number, "columnNumber", 0;
    ThrowTypeError => function::throw_type_error, "", 0;

    ParseInt => global::parse_int, "parseInt", 2;
    ParseFloat => global::parse_float, "parseFloat", 1;
    Eval => crate::vm::compile::eval, "eval", 1;
    IsNaN => global::is_nan, "isNaN", 1;
    IsFinite => global::is_finite, "isFinite", 1;

    ObjectConstructor => object::constructor, "Object", 1, constructor;
    ObjectToString => object::to_string, "toString", 0;
    ObjectToLocaleString => object::to_locale_string, "toLocaleString", 0;
    ObjectValueOf => object::value_of, "valueOf", 0;
    ObjectHasOwnProperty => object::has_own_property, "hasOwnProperty", 1;
    ObjectIsPrototypeOf => object::is_prototype_of, "isPrototypeOf", 1;
    ObjectPropertyIsEnumerable => object::property_is_enumerable, "propertyIsEnumerable", 1;
    ObjectProtoGetter => object::proto_getter, "__proto__", 0;
    ObjectProtoSetter => object::proto_setter, "__proto__", 1;
    ObjectDefineGetter => object::define_getter, "__defineGetter__", 2;
    ObjectDefineSetter => object::define_setter, "__defineSetter__", 2;
    ObjectLookupGetter => object::lookup_getter, "__lookupGetter__", 1;
    ObjectLookupSetter => object::lookup_setter, "__lookupSetter__", 1;
    ObjectAssign => object::assign, "assign", 2;
    ObjectCreate => object::create, "create", 2;
    ObjectDefineProperties => object::define_properties, "defineProperties", 2;
    ObjectDefineProperty => object::define_property, "defineProperty", 3;
    ObjectEntries => object::entries, "entries", 1;
    ObjectFreeze => object::freeze, "freeze", 1;
    ObjectFromEntries => object::from_entries, "fromEntries", 1;
    ObjectGetOwnPropertyDescriptor => object::get_own_property_descriptor, "getOwnPropertyDescriptor", 2;
    ObjectGetOwnPropertyDescriptors => object::get_own_property_descriptors, "getOwnPropertyDescriptors", 1;
    ObjectGetOwnPropertyNames => object::get_own_property_names, "getOwnPropertyNames", 1;
    ObjectGetOwnPropertySymbols => object::get_own_property_symbols, "getOwnPropertySymbols", 1;
    ObjectGetPrototypeOf => object::get_prototype_of, "getPrototypeOf", 1;
    ObjectGroupBy => object::group_by, "groupBy", 2;
    ObjectHasOwn => object::has_own, "hasOwn", 2;
    ObjectIs => object::is, "is", 2;
    ObjectIsExtensible => object::is_extensible, "isExtensible", 1;
    ObjectIsFrozen => object::is_frozen, "isFrozen", 1;
    ObjectIsSealed => object::is_sealed, "isSealed", 1;
    ObjectKeys => object::keys, "keys", 1;
    ObjectPreventExtensions => object::prevent_extensions, "preventExtensions", 1;
    ObjectSeal => object::seal, "seal", 1;
    ObjectSetPrototypeOf => object::set_prototype_of, "setPrototypeOf", 2;
    ObjectValues => object::values, "values", 1;

    ReflectApply => reflect::apply, "apply", 3;
    ReflectConstruct => reflect::construct, "construct", 2;
    ReflectDefineProperty => reflect::define_property, "defineProperty", 3;
    ReflectDeleteProperty => reflect::delete_property, "deleteProperty", 2;
    ReflectGet => reflect::get, "get", 2;
    ReflectGetOwnPropertyDescriptor => reflect::get_own_property_descriptor, "getOwnPropertyDescriptor", 2;
    ReflectGetPrototypeOf => reflect::get_prototype_of, "getPrototypeOf", 1;
    ReflectHas => reflect::has, "has", 2;
    ReflectIsExtensible => reflect::is_extensible, "isExtensible", 1;
    ReflectOwnKeys => reflect::own_keys, "ownKeys", 1;
    ReflectPreventExtensions => reflect::prevent_extensions, "preventExtensions", 1;
    ReflectSet => reflect::set, "set", 3;
    ReflectSetPrototypeOf => reflect::set_prototype_of, "setPrototypeOf", 2;

    ProxyConstructor => proxy::constructor, "Proxy", 2, constructor;
    ProxyRevocable => proxy::revocable, "revocable", 2;
    ProxyRevoke => proxy::revoke, "", 0;

    SymbolConstructor => symbol::constructor, "Symbol", 0, constructor;
    SymbolFor => symbol::for_, "for", 1;
    SymbolKeyFor => symbol::key_for, "keyFor", 1;
    SymbolToString => symbol::to_string, "toString", 0;
    SymbolValueOf => symbol::value_of, "valueOf", 0;
    SymbolDescription => symbol::description, "description", 0;
    SymbolToPrimitive => symbol::value_of, "[Symbol.toPrimitive]", 1;

    ArrayConstructor => array::constructor, "Array", 1, constructor;
    ArrayFrom => array::from, "from", 1;
    ArrayIsArray => array::is_array, "isArray", 1;
    ArrayOf => array::of, "of", 0;
    ArrayAt => array::at, "at", 1;
    ArrayConcat => array::concat, "concat", 1;
    ArrayCopyWithin => array::copy_within, "copyWithin", 2;
    ArrayFill => array::fill, "fill", 1;
    ArrayFind => array::find_, "find", 1;
    ArrayFindIndex => array::find_index, "findIndex", 1;
    ArrayFindLast => array::find_last, "findLast", 1;
    ArrayFindLastIndex => array::find_last_index, "findLastIndex", 1;
    ArrayLastIndexOf => array::last_index_of, "lastIndexOf", 1;
    ArrayPop => array::pop, "pop", 0;
    ArrayPush => array::push, "push", 1;
    ArrayReverse => array::reverse, "reverse", 0;
    ArrayShift => array::shift, "shift", 0;
    ArrayUnshift => array::unshift, "unshift", 1;
    ArraySlice => array::slice, "slice", 2;
    ArraySort => array::sort, "sort", 1;
    ArraySplice => array::splice, "splice", 2;
    ArrayIncludes => array::includes, "includes", 1;
    ArrayIndexOf => array::index_of, "indexOf", 1;
    ArrayJoin => array::join, "join", 1;
    ArrayToString => array::to_string, "toString", 0;
    ArrayToLocaleString => array::to_locale_string, "toLocaleString", 0;
    ArrayEvery => array::every, "every", 1;
    ArraySome => array::some, "some", 1;
    ArrayForEach => array::for_each, "forEach", 1;
    ArrayMap => array::map, "map", 1;
    ArrayFilter => array::filter, "filter", 1;
    ArrayReduce => array::reduce, "reduce", 1;
    ArrayReduceRight => array::reduce_right, "reduceRight", 1;
    ArrayFlat => array::flat, "flat", 0;
    ArrayFlatMap => array::flat_map, "flatMap", 1;
    ArrayToReversed => array::to_reversed, "toReversed", 0;
    ArrayToSorted => array::to_sorted, "toSorted", 1;
    ArrayToSpliced => array::to_spliced, "toSpliced", 2;
    ArrayWith => array::with, "with", 2;
    ArrayValues => iterator::values, "values", 0;
    ArrayKeys => iterator::keys, "keys", 0;
    ArrayEntries => iterator::entries, "entries", 0;
    ArrayIteratorNext => iterator::array_iterator_next, "next", 0;
    IteratorPrototypeIterator => iterator::iterator_prototype_iterator, "[Symbol.iterator]", 0;
    StringIterator => iterator::string_iterator, "[Symbol.iterator]", 0;
    StringIteratorNext => iterator::string_iterator_next, "next", 0;

    StringConstructor => string::constructor, "String", 1, constructor;
    StringFromCharCode => string::from_char_code, "fromCharCode", 1;
    StringFromCodePoint => string::from_code_point, "fromCodePoint", 1;
    StringRaw => string::raw, "raw", 1;
    StringAt => string::at, "at", 1;
    StringCharAt => string::char_at, "charAt", 1;
    StringCharCodeAt => string::char_code_at, "charCodeAt", 1;
    StringCodePointAt => string::code_point_at, "codePointAt", 1;
    StringConcat => string::concat, "concat", 1;
    StringEndsWith => string::ends_with, "endsWith", 1;
    StringIncludes => string::includes, "includes", 1;
    StringIndexOf => string::index_of, "indexOf", 1;
    StringIsWellFormed => string::is_well_formed, "isWellFormed", 0;
    StringLastIndexOf => string::last_index_of, "lastIndexOf", 1;
    StringLocaleCompare => string::locale_compare, "localeCompare", 1;
    StringMatch => string::match_, "match", 1;
    StringMatchAll => string::match_all, "matchAll", 1;
    StringNormalize => string::normalize, "normalize", 0;
    StringPadEnd => string::pad_end, "padEnd", 1;
    StringPadStart => string::pad_start, "padStart", 1;
    StringRepeat => string::repeat, "repeat", 1;
    StringReplace => string::replace, "replace", 2;
    StringReplaceAll => string::replace_all, "replaceAll", 2;
    StringSearch => string::search, "search", 1;
    StringSlice => string::slice, "slice", 2;
    StringSplit => string::split, "split", 2;
    StringStartsWith => string::starts_with, "startsWith", 1;
    StringSubstr => string::substr, "substr", 2;
    StringSubstring => string::substring, "substring", 2;
    StringToLocaleLowerCase => string::to_lower_case, "toLocaleLowerCase", 0;
    StringToLocaleUpperCase => string::to_upper_case, "toLocaleUpperCase", 0;
    StringToLowerCase => string::to_lower_case, "toLowerCase", 0;
    StringToString => string::value_of, "toString", 0;
    StringToUpperCase => string::to_upper_case, "toUpperCase", 0;
    StringToWellFormed => string::to_well_formed, "toWellFormed", 0;
    StringTrim => string::trim, "trim", 0;
    StringTrimEnd => string::trim_end, "trimEnd", 0;
    StringTrimStart => string::trim_start, "trimStart", 0;
    StringValueOf => string::value_of, "valueOf", 0;
    StringAnchor => string::anchor, "anchor", 1;
    StringBig => string::big, "big", 0;
    StringBlink => string::blink, "blink", 0;
    StringBold => string::bold, "bold", 0;
    StringFixed => string::fixed, "fixed", 0;
    StringFontcolor => string::fontcolor, "fontcolor", 1;
    StringFontsize => string::fontsize, "fontsize", 1;
    StringItalics => string::italics, "italics", 0;
    StringLink => string::link, "link", 1;
    StringSmall => string::small, "small", 0;
    StringStrike => string::strike, "strike", 0;
    StringSub => string::sub, "sub", 0;
    StringSup => string::sup, "sup", 0;

    NumberConstructor => number::constructor, "Number", 1, constructor;
    NumberToString => number::to_string, "toString", 1;
    NumberToLocaleString => number::to_locale_string, "toLocaleString", 0;
    NumberValueOf => number::value_of, "valueOf", 0;
    NumberToFixed => number::to_fixed, "toFixed", 1;
    NumberToExponential => number::to_exponential, "toExponential", 1;
    NumberToPrecision => number::to_precision, "toPrecision", 1;
    NumberIsFinite => number::is_finite, "isFinite", 1;
    NumberIsInteger => number::is_integer, "isInteger", 1;
    NumberIsNaN => number::is_nan, "isNaN", 1;
    NumberIsSafeInteger => number::is_safe_integer, "isSafeInteger", 1;

    BooleanConstructor => boolean::constructor, "Boolean", 1, constructor;
    BooleanToString => boolean::to_string, "toString", 0;
    BooleanValueOf => boolean::value_of, "valueOf", 0;

    BigIntConstructor => bigint::constructor, "BigInt", 1, constructor;
    BigIntAsIntN => bigint::as_int_n, "asIntN", 2;
    BigIntAsUintN => bigint::as_uint_n, "asUintN", 2;
    BigIntToString => bigint::to_string, "toString", 0;
    BigIntValueOf => bigint::value_of, "valueOf", 0;

    DecodeUri => uri::decode_uri, "decodeURI", 1;
    DecodeUriComponent => uri::decode_uri_component, "decodeURIComponent", 1;
    EncodeUri => uri::encode_uri, "encodeURI", 1;
    EncodeUriComponent => uri::encode_uri_component, "encodeURIComponent", 1;
    Escape => uri::escape, "escape", 1;
    Unescape => uri::unescape, "unescape", 1;

    MathAbs => math::abs, "abs", 1;
    MathAcos => math::acos, "acos", 1;
    MathAcosh => math::acosh, "acosh", 1;
    MathAsin => math::asin, "asin", 1;
    MathAsinh => math::asinh, "asinh", 1;
    MathAtan => math::atan, "atan", 1;
    MathAtanh => math::atanh, "atanh", 1;
    MathAtan2 => math::atan2, "atan2", 2;
    MathCbrt => math::cbrt, "cbrt", 1;
    MathCeil => math::ceil, "ceil", 1;
    MathClz32 => math::clz32, "clz32", 1;
    MathCos => math::cos, "cos", 1;
    MathCosh => math::cosh, "cosh", 1;
    MathExp => math::exp, "exp", 1;
    MathExpm1 => math::expm1, "expm1", 1;
    MathFloor => math::floor, "floor", 1;
    MathFround => math::fround, "fround", 1;
    MathF16round => math::f16round, "f16round", 1;
    MathHypot => math::hypot, "hypot", 2;
    MathImul => math::imul, "imul", 2;
    MathLog => math::log, "log", 1;
    MathLog1p => math::log1p, "log1p", 1;
    MathLog10 => math::log10, "log10", 1;
    MathLog2 => math::log2, "log2", 1;
    MathMax => math::max, "max", 2;
    MathMin => math::min, "min", 2;
    MathPow => math::pow, "pow", 2;
    MathRandom => math::random, "random", 0;
    MathRound => math::round, "round", 1;
    MathSign => math::sign, "sign", 1;
    MathSin => math::sin, "sin", 1;
    MathSinh => math::sinh, "sinh", 1;
    MathSqrt => math::sqrt, "sqrt", 1;
    MathSumPrecise => math::sum_precise, "sumPrecise", 1;
    MathTan => math::tan, "tan", 1;
    MathTanh => math::tanh, "tanh", 1;
    MathTrunc => math::trunc, "trunc", 1;

    RegExpConstructor => regexp::constructor, "RegExp", 2, constructor;
    RegExpEscape => regexp::escape, "escape", 1;
    RegExpCompile => regexp::compile, "compile", 2;
    RegExpExec => regexp::exec, "exec", 1;
    RegExpTest => regexp::test, "test", 1;
    RegExpToString => regexp::to_string, "toString", 0;
    RegExpSource => regexp::source, "source", 0;
    RegExpFlags => regexp::flags, "flags", 0;
    RegExpHasIndices => regexp::has_indices, "hasIndices", 0;
    RegExpGlobal => regexp::global, "global", 0;
    RegExpIgnoreCase => regexp::ignore_case, "ignoreCase", 0;
    RegExpMultiline => regexp::multiline, "multiline", 0;
    RegExpDotAll => regexp::dot_all, "dotAll", 0;
    RegExpUnicode => regexp::unicode, "unicode", 0;
    RegExpUnicodeSets => regexp::unicode_sets, "unicodeSets", 0;
    RegExpSticky => regexp::sticky, "sticky", 0;
    RegExpSymbolMatch => regexp::symbol_match, "[Symbol.match]", 1;
    RegExpSymbolMatchAll => regexp::symbol_match_all, "[Symbol.matchAll]", 1;
    RegExpSymbolReplace => regexp::symbol_replace, "[Symbol.replace]", 2;
    RegExpSymbolSearch => regexp::symbol_search, "[Symbol.search]", 1;
    RegExpSymbolSplit => regexp::symbol_split, "[Symbol.split]", 2;
    RegExpStringIteratorNext => regexp::string_iterator_next, "next", 0;

    ErrorConstructor => error::error, "Error", 1, constructor;
    EvalErrorConstructor => error::eval_error, "EvalError", 1, constructor;
    RangeErrorConstructor => error::range_error, "RangeError", 1, constructor;
    ReferenceErrorConstructor => error::reference_error, "ReferenceError", 1, constructor;
    SyntaxErrorConstructor => error::syntax_error, "SyntaxError", 1, constructor;
    TypeErrorConstructor => error::type_error, "TypeError", 1, constructor;
    InternalErrorConstructor => error::internal_error, "InternalError", 1, constructor;
    UriErrorConstructor => error::uri_error, "URIError", 1, constructor;
    AggregateErrorConstructor => error::aggregate_error, "AggregateError", 2, constructor;
    ErrorToString => error::to_string, "toString", 0;
    ErrorIsError => error::is_error, "isError", 1;

    PromiseConstructor => promise::constructor, "Promise", 1, constructor;
    PromiseResolveFunction => promise::resolve_function, "", 1;
    PromiseRejectFunction => promise::reject_function, "", 1;
    PromiseCapabilityExecutor => promise::capability_executor, "", 2;
    PromiseThen => promise::then, "then", 2;
    PromiseCatch => promise::catch, "catch", 1;
    PromiseFinally => promise::finally, "finally", 1;
    PromiseThenFinally => promise::then_finally, "", 1;
    PromiseCatchFinally => promise::catch_finally, "", 1;
    PromiseValueThunk => promise::value_thunk, "", 0;
    PromiseThrower => promise::thrower, "", 0;
    PromiseResolveStatic => promise::resolve_static, "resolve", 1;
    PromiseRejectStatic => promise::reject_static, "reject", 1;
    PromiseWithResolvers => promise::with_resolvers, "withResolvers", 0;
    PromiseTry => promise::try_, "try", 1;
    PromiseAll => promise::all, "all", 1;
    PromiseAllSettled => promise::all_settled, "allSettled", 1;
    PromiseAny => promise::any, "any", 1;
    PromiseRace => promise::race, "race", 1;
    PromiseAllResolveElement => promise::all_resolve_element, "", 1;
    PromiseAllSettledResolveElement => promise::all_settled_resolve_element, "", 1;
    PromiseAllSettledRejectElement => promise::all_settled_reject_element, "", 1;
    PromiseAnyRejectElement => promise::any_reject_element, "", 1;
    SpeciesGetter => promise::species_getter, "[Symbol.species]", 0;

    GeneratorFunctionConstructor => generator::generator_function_constructor, "GeneratorFunction", 1, constructor;
    AsyncFunctionConstructor => generator::async_function_constructor, "AsyncFunction", 1, constructor;
    AsyncGeneratorFunctionConstructor => generator::async_generator_function_constructor, "AsyncGeneratorFunction", 1, constructor;
    GeneratorNext => generator::next, "next", 1;
    GeneratorReturn => generator::return_, "return", 1;
    GeneratorThrow => generator::throw, "throw", 1;
    AsyncIteratorPrototypeAsyncIterator => generator::async_iterator_prototype_async_iterator, "[Symbol.asyncIterator]", 0;
    AsyncFromSyncNext => generator::async_from_sync_next, "next", 1;
    AsyncFromSyncReturn => generator::async_from_sync_return, "return", 1;
    AsyncFromSyncThrow => generator::async_from_sync_throw, "throw", 1;
    AsyncFromSyncUnwrap => generator::async_from_sync_unwrap, "", 1;
    AsyncFromSyncCloseRejected => generator::async_from_sync_close_rejected, "", 1;
    AsyncGeneratorNext => crate::vm::async_generator::next, "next", 1;
    AsyncGeneratorReturn => crate::vm::async_generator::return_, "return", 1;
    AsyncGeneratorThrow => crate::vm::async_generator::throw, "throw", 1;

    DateConstructor => date::constructor, "Date", 7, constructor;
    DateNow => date::now_, "now", 0;
    DateParse => date::parse, "parse", 1;
    DateUtc => date::utc, "UTC", 7;
    DateValueOf => date::value_of, "valueOf", 0;
    DateGetTime => date::value_of, "getTime", 0;
    DateToString => date::to_string, "toString", 0;
    DateToDateString => date::to_date_string, "toDateString", 0;
    DateToTimeString => date::to_time_string, "toTimeString", 0;
    DateToUtcString => date::to_utc_string, "toUTCString", 0;
    DateToIsoString => date::to_iso_string, "toISOString", 0;
    DateToLocaleString => date::to_locale_string, "toLocaleString", 0;
    DateToLocaleDateString => date::to_locale_date_string, "toLocaleDateString", 0;
    DateToLocaleTimeString => date::to_locale_time_string, "toLocaleTimeString", 0;
    DateToPrimitive => date::to_primitive, "[Symbol.toPrimitive]", 1;
    DateGetTimezoneOffset => date::get_timezone_offset, "getTimezoneOffset", 0;
    DateGetYear => date::get_year, "getYear", 0;
    DateGetFullYear => date::get_full_year, "getFullYear", 0;
    DateGetUtcFullYear => date::get_utc_full_year, "getUTCFullYear", 0;
    DateGetMonth => date::get_month, "getMonth", 0;
    DateGetUtcMonth => date::get_utc_month, "getUTCMonth", 0;
    DateGetDate => date::get_date, "getDate", 0;
    DateGetUtcDate => date::get_utc_date, "getUTCDate", 0;
    DateGetHours => date::get_hours, "getHours", 0;
    DateGetUtcHours => date::get_utc_hours, "getUTCHours", 0;
    DateGetMinutes => date::get_minutes, "getMinutes", 0;
    DateGetUtcMinutes => date::get_utc_minutes, "getUTCMinutes", 0;
    DateGetSeconds => date::get_seconds, "getSeconds", 0;
    DateGetUtcSeconds => date::get_utc_seconds, "getUTCSeconds", 0;
    DateGetMilliseconds => date::get_milliseconds, "getMilliseconds", 0;
    DateGetUtcMilliseconds => date::get_utc_milliseconds, "getUTCMilliseconds", 0;
    DateGetDay => date::get_day, "getDay", 0;
    DateGetUtcDay => date::get_utc_day, "getUTCDay", 0;
    DateSetMilliseconds => date::set_milliseconds, "setMilliseconds", 1;
    DateSetUtcMilliseconds => date::set_utc_milliseconds, "setUTCMilliseconds", 1;
    DateSetSeconds => date::set_seconds, "setSeconds", 2;
    DateSetUtcSeconds => date::set_utc_seconds, "setUTCSeconds", 2;
    DateSetMinutes => date::set_minutes, "setMinutes", 3;
    DateSetUtcMinutes => date::set_utc_minutes, "setUTCMinutes", 3;
    DateSetHours => date::set_hours, "setHours", 4;
    DateSetUtcHours => date::set_utc_hours, "setUTCHours", 4;
    DateSetDate => date::set_date, "setDate", 1;
    DateSetUtcDate => date::set_utc_date, "setUTCDate", 1;
    DateSetMonth => date::set_month, "setMonth", 2;
    DateSetUtcMonth => date::set_utc_month, "setUTCMonth", 2;
    DateSetFullYear => date::set_full_year, "setFullYear", 3;
    DateSetUtcFullYear => date::set_utc_full_year, "setUTCFullYear", 3;
    DateSetTime => date::set_time, "setTime", 1;
    DateSetYear => date::set_year, "setYear", 1;
    DateToJson => date::to_json, "toJSON", 1;

    JsonIsRawJson => json::is_raw_json, "isRawJSON", 1;
    JsonParse => json::parse, "parse", 2;
    JsonRawJson => json::raw_json, "rawJSON", 1;
    JsonStringify => json::stringify, "stringify", 3;

    MapConstructor => collection::map_constructor, "Map", 0, constructor;
    MapGroupBy => collection::group_by, "groupBy", 2;
    MapSet => collection::map_set, "set", 2;
    MapGet => collection::map_get, "get", 1;
    MapGetOrInsert => collection::map_get_or_insert, "getOrInsert", 2;
    MapGetOrInsertComputed => collection::map_get_or_insert_computed, "getOrInsertComputed", 2;
    MapHas => collection::map_has, "has", 1;
    MapDelete => collection::map_delete, "delete", 1;
    MapClear => collection::map_clear, "clear", 0;
    MapSize => collection::map_size, "size", 0;
    MapForEach => collection::map_for_each, "forEach", 1;
    MapValues => collection::map_values, "values", 0;
    MapKeys => collection::map_keys, "keys", 0;
    MapEntries => collection::map_entries, "entries", 0;
    MapIteratorNext => collection::map_iterator_next, "next", 0;
    SetConstructor => collection::set_constructor, "Set", 0, constructor;
    SetAdd => collection::set_add, "add", 1;
    SetHas => collection::set_has, "has", 1;
    SetDelete => collection::set_delete, "delete", 1;
    SetClear => collection::set_clear, "clear", 0;
    SetSize => collection::set_size, "size", 0;
    SetForEach => collection::set_for_each, "forEach", 1;
    SetIsDisjointFrom => collection::set_is_disjoint_from, "isDisjointFrom", 1;
    SetIsSubsetOf => collection::set_is_subset_of, "isSubsetOf", 1;
    SetIsSupersetOf => collection::set_is_superset_of, "isSupersetOf", 1;
    SetIntersection => collection::set_intersection, "intersection", 1;
    SetDifference => collection::set_difference, "difference", 1;
    SetSymmetricDifference => collection::set_symmetric_difference, "symmetricDifference", 1;
    SetUnion => collection::set_union, "union", 1;
    SetValues => collection::set_values, "values", 0;
    SetEntries => collection::set_entries, "entries", 0;
    SetIteratorNext => collection::set_iterator_next, "next", 0;
    WeakMapConstructor => collection::weak_map_constructor, "WeakMap", 0, constructor;
    WeakMapSet => collection::weak_map_set, "set", 2;
    WeakMapGet => collection::weak_map_get, "get", 1;
    WeakMapGetOrInsert => collection::weak_map_get_or_insert, "getOrInsert", 2;
    WeakMapGetOrInsertComputed => collection::weak_map_get_or_insert_computed, "getOrInsertComputed", 2;
    WeakMapHas => collection::weak_map_has, "has", 1;
    WeakMapDelete => collection::weak_map_delete, "delete", 1;
    WeakSetConstructor => collection::weak_set_constructor, "WeakSet", 0, constructor;
    WeakSetAdd => collection::weak_set_add, "add", 1;
    WeakSetHas => collection::weak_set_has, "has", 1;
    WeakSetDelete => collection::weak_set_delete, "delete", 1;
    WeakRefConstructor => collection::weak_ref_constructor, "WeakRef", 1, constructor;
    WeakRefDeref => collection::weak_ref_deref, "deref", 0;
    FinalizationRegistryConstructor => collection::finalization_registry_constructor, "FinalizationRegistry", 1, constructor;
    FinalizationRegistryRegister => collection::finalization_registry_register, "register", 2;
    FinalizationRegistryUnregister => collection::finalization_registry_unregister, "unregister", 1;

    IteratorConstructor => iterator_helpers::constructor, "Iterator", 0, constructor;
    IteratorConstructorAccessor => iterator_helpers::constructor_accessor, "", 0;
    IteratorToStringTagGetter => iterator_helpers::to_string_tag_getter, "[Symbol.toStringTag]", 0;
    IteratorToStringTagSetter => iterator_helpers::to_string_tag_setter, "[Symbol.toStringTag]", 1;
    IteratorFrom => iterator_helpers::from, "from", 1;
    IteratorConcat => iterator_helpers::concat, "concat", 0;
    WrapForValidNext => iterator_helpers::wrap_next, "next", 0;
    WrapForValidReturn => iterator_helpers::wrap_return, "return", 0;
    IteratorDrop => iterator_helpers::drop, "drop", 1;
    IteratorFilter => iterator_helpers::filter, "filter", 1;
    IteratorFlatMap => iterator_helpers::flat_map, "flatMap", 1;
    IteratorMap => iterator_helpers::map, "map", 1;
    IteratorTake => iterator_helpers::take, "take", 1;
    IteratorEvery => iterator_helpers::every, "every", 1;
    IteratorFind => iterator_helpers::find, "find", 1;
    IteratorForEach => iterator_helpers::for_each, "forEach", 1;
    IteratorSome => iterator_helpers::some, "some", 1;
    IteratorReduce => iterator_helpers::reduce, "reduce", 1;
    IteratorToArray => iterator_helpers::to_array, "toArray", 0;
    IteratorHelperNext => iterator_helpers::helper_next, "next", 0;
    IteratorHelperReturn => iterator_helpers::helper_return, "return", 0;
    IteratorConcatNext => iterator_helpers::concat_next, "next", 0;
    IteratorConcatReturn => iterator_helpers::concat_return, "return", 0;

    ArrayBufferConstructor => buffer::array_buffer_constructor, "ArrayBuffer", 1, constructor;
    ArrayBufferIsView => buffer::is_view, "isView", 1;
    ArrayBufferByteLength => buffer::byte_length, "byteLength", 0;
    ArrayBufferMaxByteLength => buffer::max_byte_length, "maxByteLength", 0;
    ArrayBufferResizable => buffer::resizable, "resizable", 0;
    ArrayBufferDetached => buffer::detached, "detached", 0;
    ArrayBufferResize => buffer::resize, "resize", 1;
    ArrayBufferSlice => buffer::slice, "slice", 2;
    ArrayBufferTransfer => buffer::transfer, "transfer", 0;
    ArrayBufferTransferToFixedLength => buffer::transfer_to_fixed_length, "transferToFixedLength", 0;
    SharedArrayBufferConstructor => buffer::shared_array_buffer_constructor, "SharedArrayBuffer", 1, constructor;
    SharedArrayBufferByteLength => buffer::shared_byte_length, "byteLength", 0;
    SharedArrayBufferGrowable => buffer::growable, "growable", 0;
    SharedArrayBufferMaxByteLength => buffer::shared_max_byte_length, "maxByteLength", 0;
    SharedArrayBufferGrow => buffer::grow, "grow", 1;
    SharedArrayBufferSlice => buffer::shared_slice, "slice", 2;
    DataViewConstructor => buffer::data_view_constructor, "DataView", 1, constructor;
    DataViewBuffer => buffer::data_view_buffer, "buffer", 0;
    DataViewByteLength => buffer::data_view_byte_length, "byteLength", 0;
    DataViewByteOffset => buffer::data_view_byte_offset, "byteOffset", 0;
    DataViewGetInt8 => buffer::get_int8, "getInt8", 1;
    DataViewSetInt8 => buffer::set_int8, "setInt8", 2;
    DataViewGetUint8 => buffer::get_uint8, "getUint8", 1;
    DataViewSetUint8 => buffer::set_uint8, "setUint8", 2;
    DataViewGetInt16 => buffer::get_int16, "getInt16", 1;
    DataViewSetInt16 => buffer::set_int16, "setInt16", 2;
    DataViewGetUint16 => buffer::get_uint16, "getUint16", 1;
    DataViewSetUint16 => buffer::set_uint16, "setUint16", 2;
    DataViewGetInt32 => buffer::get_int32, "getInt32", 1;
    DataViewSetInt32 => buffer::set_int32, "setInt32", 2;
    DataViewGetUint32 => buffer::get_uint32, "getUint32", 1;
    DataViewSetUint32 => buffer::set_uint32, "setUint32", 2;
    DataViewGetFloat16 => buffer::get_float16, "getFloat16", 1;
    DataViewSetFloat16 => buffer::set_float16, "setFloat16", 2;
    DataViewGetFloat32 => buffer::get_float32, "getFloat32", 1;
    DataViewSetFloat32 => buffer::set_float32, "setFloat32", 2;
    DataViewGetFloat64 => buffer::get_float64, "getFloat64", 1;
    DataViewSetFloat64 => buffer::set_float64, "setFloat64", 2;
    DataViewGetBigInt64 => buffer::get_big_int64, "getBigInt64", 1;
    DataViewSetBigInt64 => buffer::set_big_int64, "setBigInt64", 2;
    DataViewGetBigUint64 => buffer::get_big_uint64, "getBigUint64", 1;
    DataViewSetBigUint64 => buffer::set_big_uint64, "setBigUint64", 2;
    TypedArrayConstructor => typed_array::abstract_constructor, "TypedArray", 0, constructor;
    TypedArrayFrom => typed_array::from, "from", 1;
    TypedArrayOf => typed_array::of, "of", 0;
    TypedArrayBuffer => typed_array::buffer_getter, "buffer", 0;
    TypedArrayByteLength => typed_array::byte_length, "byteLength", 0;
    TypedArrayByteOffset => typed_array::byte_offset, "byteOffset", 0;
    TypedArrayLength => typed_array::length, "length", 0;
    TypedArrayToStringTag => typed_array::to_string_tag_getter, "[Symbol.toStringTag]", 0;
    TypedArrayAt => typed_array::at, "at", 1;
    TypedArrayCopyWithin => typed_array::copy_within, "copyWithin", 2;
    TypedArrayEntries => typed_array::entries, "entries", 0;
    TypedArrayEvery => typed_array::every, "every", 1;
    TypedArrayFill => typed_array::fill, "fill", 1;
    TypedArrayFilter => typed_array::filter, "filter", 1;
    TypedArrayFind => typed_array::find, "find", 1;
    TypedArrayFindIndex => typed_array::find_index, "findIndex", 1;
    TypedArrayFindLast => typed_array::find_last, "findLast", 1;
    TypedArrayFindLastIndex => typed_array::find_last_index, "findLastIndex", 1;
    TypedArrayForEach => typed_array::for_each, "forEach", 1;
    TypedArrayIncludes => typed_array::includes, "includes", 1;
    TypedArrayIndexOf => typed_array::index_of, "indexOf", 1;
    TypedArrayJoin => typed_array::join, "join", 1;
    TypedArrayKeys => typed_array::keys, "keys", 0;
    TypedArrayLastIndexOf => typed_array::last_index_of, "lastIndexOf", 1;
    TypedArrayMap => typed_array::map, "map", 1;
    TypedArrayReduce => typed_array::reduce, "reduce", 1;
    TypedArrayReduceRight => typed_array::reduce_right, "reduceRight", 1;
    TypedArrayReverse => typed_array::reverse, "reverse", 0;
    TypedArraySet => typed_array::set_method, "set", 1;
    TypedArraySlice => typed_array::slice, "slice", 2;
    TypedArraySome => typed_array::some, "some", 1;
    TypedArraySort => typed_array::sort, "sort", 1;
    TypedArraySubarray => typed_array::subarray, "subarray", 2;
    TypedArrayToLocaleString => typed_array::to_locale_string, "toLocaleString", 0;
    TypedArrayToReversed => typed_array::to_reversed, "toReversed", 0;
    TypedArrayToSorted => typed_array::to_sorted, "toSorted", 1;
    TypedArrayValues => typed_array::values, "values", 0;
    TypedArrayWith => typed_array::with, "with", 2;
    Int8ArrayConstructor => typed_array::int8_array, "Int8Array", 3, constructor;
    Uint8ArrayConstructor => typed_array::uint8_array, "Uint8Array", 3, constructor;
    Uint8ClampedArrayConstructor => typed_array::uint8_clamped_array, "Uint8ClampedArray", 3, constructor;
    Int16ArrayConstructor => typed_array::int16_array, "Int16Array", 3, constructor;
    Uint16ArrayConstructor => typed_array::uint16_array, "Uint16Array", 3, constructor;
    Int32ArrayConstructor => typed_array::int32_array, "Int32Array", 3, constructor;
    Uint32ArrayConstructor => typed_array::uint32_array, "Uint32Array", 3, constructor;
    Float16ArrayConstructor => typed_array::float16_array, "Float16Array", 3, constructor;
    Float32ArrayConstructor => typed_array::float32_array, "Float32Array", 3, constructor;
    Float64ArrayConstructor => typed_array::float64_array, "Float64Array", 3, constructor;
    BigInt64ArrayConstructor => typed_array::big_int64_array, "BigInt64Array", 3, constructor;
    BigUint64ArrayConstructor => typed_array::big_uint64_array, "BigUint64Array", 3, constructor;
    Uint8ArrayFromBase64 => typed_array::from_base64, "fromBase64", 1;
    Uint8ArrayFromHex => typed_array::from_hex, "fromHex", 1;
    Uint8ArrayToBase64 => typed_array::to_base64, "toBase64", 0;
    Uint8ArraySetFromBase64 => typed_array::set_from_base64, "setFromBase64", 1;
    Uint8ArrayToHex => typed_array::to_hex, "toHex", 0;
    Uint8ArraySetFromHex => typed_array::set_from_hex, "setFromHex", 1;
    AtomicsAdd => atomics::add, "add", 3;
    AtomicsAnd => atomics::and, "and", 3;
    AtomicsCompareExchange => atomics::compare_exchange, "compareExchange", 4;
    AtomicsExchange => atomics::exchange, "exchange", 3;
    AtomicsIsLockFree => atomics::is_lock_free, "isLockFree", 1;
    AtomicsLoad => atomics::load, "load", 2;
    AtomicsNotify => atomics::notify, "notify", 3;
    AtomicsOr => atomics::or, "or", 3;
    AtomicsStore => atomics::store, "store", 3;
    AtomicsSub => atomics::sub, "sub", 3;
    AtomicsWait => atomics::wait, "wait", 4;
    AtomicsXor => atomics::xor, "xor", 3;
    AtomicsPause => atomics::pause, "pause", 0;

    AgentStart => agent::start, "start", 1;
    AgentBroadcast => agent::broadcast, "broadcast", 2;
    AgentGetReport => agent::get_report, "getReport", 0;
    AgentSleep => agent::sleep, "sleep", 1;
    AgentMonotonicNow => agent::monotonic_now, "monotonicNow", 0;
    AgentReceiveBroadcast => agent::receive_broadcast, "receiveBroadcast", 1;
    AgentReport => agent::report, "report", 1;
    AgentLeaving => agent::leaving, "leaving", 0;

    ModuleAsyncFulfilled => crate::vm::module::module_async_fulfilled, "", 1;
    ModuleAsyncRejected => crate::vm::module::module_async_rejected, "", 1;
    ModuleImportFulfilled => crate::vm::module::module_import_fulfilled, "", 1;
    ModuleImportJob => crate::vm::module::module_import_job, "", 0;
    ModuleImportRejected => crate::vm::module::module_import_rejected, "", 1;

    Test262CreateRealm => test262::create_realm, "createRealm", 0;
    Test262Gc => test262::gc, "gc", 0;
    Test262EvalScript => test262::eval_script, "evalScript", 1;
    Test262DetachArrayBuffer => test262::detach_array_buffer, "detachArrayBuffer", 1;
    Test262CodePointRange => test262::code_point_range, "codePointRange", 2;

    ConsoleLog => global::print, "log", 1;
    Load => crate::vm::compile::load, "load", 1;
    Read => crate::vm::compile::read, "read", 1;
    LoadScript => crate::vm::compile::load_script, "__loadScript", 1;
    PerformanceNow => global::performance_now, "now", 0;
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
            ErrorKind::Uri => Native::UriErrorConstructor,
            ErrorKind::Aggregate => Native::AggregateErrorConstructor,
        }
    }
}

// ---- helpers for the natives ----

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
    if units.len() > string::MAX_STRING {
        return Err(vm_error::string_too_long(cx));
    }
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
        return Err(vm_error::type_error(cx, "null or undefined are forbidden"));
    }
    to_string_id(cx, this)
}

/// The receiver as an object (`ToObject(this)`), wrapped and rooted when it
/// is a primitive.
pub(crate) fn this_object(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    if let Some(object) = this.as_object() {
        return Ok(object);
    }
    let object = crate::vm::object::to_object(cx, this)?;
    cx.heap.roots().push(Value::object(object));
    Ok(object)
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
    let key = cx.heap.property_key(name)?;
    define_key(cx, object, key, value, flags)
}

fn define_key(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: crate::PropertyKey,
    value: Value,
    flags: PropertyFlags,
) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(value);
    let result = cx.heap.add_property(object, key, flags, value);
    cx.heap.roots().truncate(mark);
    result
}

/// Defines a function's `length` and then `name` (non-writable,
/// non-enumerable, configurable), as every function has them.
pub(crate) fn define_name_length(
    heap: &mut crate::Heap,
    names: &Names,
    function: ObjectId,
    name: Value,
    length: u32,
) -> Result<(), HeapError> {
    let flags = PropertyFlags::data(false, false, true);
    let mark = heap.roots().mark();
    heap.roots().push(name);
    let result = heap
        .add_property(
            function,
            Names::key(names.length),
            flags,
            Value::int32(length as i32),
        )
        .and_then(|()| heap.add_property(function, Names::key(names.name), flags, name));
    heap.roots().truncate(mark);
    result
}

/// A native function object named `name`, with its `length`, left on the
/// root stack.
fn native_named(
    cx: &mut Cx<'_>,
    native: Native,
    name: &str,
    data: Option<ObjectId>,
) -> Result<ObjectId, HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
    let function = cx
        .heap
        .alloc_native_closure(Some(prototype), native.index(), data)?;
    cx.heap.roots().push(Value::object(function));
    if native.is_constructor() {
        cx.heap
            .set_object_flag(function, crate::heap::object::flags::CONSTRUCTOR);
    }
    let name = if name.is_empty() {
        cx.realm.names.empty
    } else {
        cx.heap.intern(name)?
    };
    let names = cx.realm.names;
    define_name_length(
        cx.heap,
        &names,
        function,
        Value::string(name),
        native.length(),
    )?;
    Ok(function)
}

/// A native function object with its table name and length, left on the
/// root stack.
fn native_function(cx: &mut Cx<'_>, native: Native) -> Result<ObjectId, HeapError> {
    native_named(cx, native, native.js_name(), None)
}

/// A native closure over `data` (an object whose internal slots the native
/// reads through [`CallSite::data`]), named by the table, for natives made
/// at run time (a promise's resolving functions, a revoker).
pub(crate) fn native_closure(
    cx: &mut Cx<'_>,
    native: Native,
    data: ObjectId,
) -> Result<ObjectId, Throw> {
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(data));
    let result = native_named(cx, native, native.js_name(), Some(data));
    cx.heap.roots().truncate(mark);
    result.map_err(|error| vm_error::heap_error(cx, error))
}

/// A data object for [`native_closure`] holding `slots`.
pub(crate) fn native_data(cx: &mut Cx<'_>, slots: &[Value]) -> Result<ObjectId, Throw> {
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            crate::heap::object::InternalTag::NativeData,
            None,
            0,
            slots,
            Default::default(),
            0,
        )
        .map_err(|error| vm_error::heap_error(cx, error))
}

/// Defines each native as a non-enumerable method of `object` under its
/// table name.
fn methods(cx: &mut Cx<'_>, object: ObjectId, list: &[Native]) -> Result<(), HeapError> {
    for &native in list {
        let function = native_function(cx, native)?;
        define(
            cx,
            object,
            native.js_name(),
            Value::object(function),
            hidden(),
        )?;
    }
    Ok(())
}

/// Defines a method keyed by a well-known symbol; its name is the table's
/// (`[Symbol.iterator]`).
fn symbol_method(
    cx: &mut Cx<'_>,
    object: ObjectId,
    symbol: crate::SymbolId,
    native: Native,
    flags: PropertyFlags,
) -> Result<(), HeapError> {
    let function = native_function(cx, native)?;
    define_key(
        cx,
        object,
        crate::PropertyKey::symbol(symbol),
        Value::object(function),
        flags,
    )
}

/// Defines accessor `name` of `object` (non-enumerable, configurable) with
/// a getter native and an optional setter native, named `get name` and
/// `set name`.
fn accessor(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: crate::PropertyKey,
    name: &str,
    getter: Option<Native>,
    setter: Option<Native>,
) -> Result<(), HeapError> {
    let get = match getter {
        Some(native) => Value::object(native_named(cx, native, &format!("get {name}"), None)?),
        None => Value::UNDEFINED,
    };
    let set = match setter {
        Some(native) => Value::object(native_named(cx, native, &format!("set {name}"), None)?),
        None => Value::UNDEFINED,
    };
    cx.heap
        .add_accessor(object, key, PropertyFlags::accessor(false, true), get, set)
}

/// [`accessor`] keyed by a string.
fn getter(cx: &mut Cx<'_>, object: ObjectId, name: &str, native: Native) -> Result<(), HeapError> {
    let key = cx.heap.property_key(name)?;
    accessor(cx, object, key, name, Some(native), None)
}

/// A constructor function, defined on the global object as its table name.
/// Its `prototype` link comes last ([`link_constructor`]), after its static
/// members and the prototype's methods, as QuickJS's `JS_NewCConstructor`
/// orders them.
fn install_constructor(cx: &mut Cx<'_>, native: Native) -> Result<ObjectId, HeapError> {
    let function = native_function(cx, native)?;
    let global = cx.realm.global();
    define(
        cx,
        global,
        native.js_name(),
        Value::object(function),
        hidden(),
    )?;
    Ok(function)
}

/// Defines `function.prototype` (non-writable) and then
/// `prototype.constructor` (QuickJS's `JS_SetConstructor2`), each the last
/// string-keyed property of its object.
fn link_constructor(
    cx: &mut Cx<'_>,
    function: ObjectId,
    prototype: ObjectId,
) -> Result<(), HeapError> {
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
    )
}

/// An ordinary object inheriting from `prototype`, rooted.
fn plain_object(
    cx: &mut Cx<'_>,
    prototype: Option<ObjectId>,
    slots: u32,
) -> Result<ObjectId, HeapError> {
    let object = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, prototype, slots)?;
    cx.heap.roots().push(Value::object(object));
    Ok(object)
}

/// Defines `Symbol.toStringTag` on `object` as `tag` (non-writable,
/// non-enumerable, configurable).
fn to_string_tag(cx: &mut Cx<'_>, object: ObjectId, tag: &str) -> Result<(), HeapError> {
    let tag = cx.heap.intern(tag)?;
    let key = crate::PropertyKey::symbol(cx.realm.symbols.to_string_tag);
    cx.heap.add_property(
        object,
        key,
        PropertyFlags::data(false, false, true),
        Value::string(tag),
    )
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
            Native::ParseInt,
            Native::ParseFloat,
            Native::IsNaN,
            Native::IsFinite,
        ],
    )?;
    let eval = native_function(cx, Native::Eval)?;
    cx.set_intrinsic(Intrinsic::Eval, eval);
    define(cx, global, "eval", Value::object(eval), hidden())?;
    function::install(cx)?;
    object::install(cx)?;
    symbol::install(cx)?;
    reflect::install(cx)?;
    proxy::install(cx)?;
    array::install(cx)?;
    string::install(cx)?;
    iterator::install(cx)?;
    iterator_helpers::install(cx)?;
    generator::install(cx)?;
    generator::install_async_from_sync(cx)?;
    promise::install(cx)?;
    number::install(cx)?;
    boolean::install(cx)?;
    math::install(cx)?;
    bigint::install(cx)?;
    uri::install(cx)?;
    regexp::install(cx)?;
    date::install(cx)?;
    json::install(cx)?;
    collection::install(cx)?;
    buffer::install(cx)?;
    typed_array::install(cx)?;
    atomics::install(cx)?;
    error::install(cx)?;
    regexp::record_flag_cache(cx)?;
    to_string_tag(cx, global, "global")?;
    order_globals(cx, global)?;
    let message = cx.heap.intern("out of memory")?;
    let error = vm_error::new_error(cx, ErrorKind::Internal, Value::string(message))?;
    Ok(error.as_object().unwrap())
}

/// The global object's properties in QuickJS's order: its
/// `JS_AddIntrinsicBasicObjects`, `JS_AddIntrinsicBaseObjects` and the
/// `JS_AddIntrinsic*` calls of `JS_NewContext`, which define them in an
/// order the install's dependencies do not allow.
const GLOBAL_ORDER: &[&str] = &[
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "InternalError",
    "AggregateError",
    "Array",
    "Object",
    "Function",
    "Iterator",
    "parseInt",
    "parseFloat",
    "isNaN",
    "isFinite",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "escape",
    "unescape",
    "Infinity",
    "NaN",
    "undefined",
    "eval",
    "Number",
    "Boolean",
    "String",
    "Math",
    "Reflect",
    "Symbol",
    "globalThis",
    "BigInt",
    "Date",
    "RegExp",
    "JSON",
    "Proxy",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "ArrayBuffer",
    "SharedArrayBuffer",
    "Uint8ClampedArray",
    "Int8Array",
    "Uint8Array",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "BigInt64Array",
    "BigUint64Array",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "DataView",
    "Atomics",
    "Promise",
    "WeakRef",
    "FinalizationRegistry",
];

/// Puts the global object's properties in [`GLOBAL_ORDER`].
fn order_globals(cx: &mut Cx<'_>, global: ObjectId) -> Result<(), HeapError> {
    let mut keys = Vec::with_capacity(GLOBAL_ORDER.len());
    for name in GLOBAL_ORDER {
        keys.push(cx.heap.property_key(name)?);
    }
    debug_assert!(
        keys.iter()
            .all(|&key| cx.heap.lookup_own(global, key).is_some()),
        "every global in GLOBAL_ORDER is installed"
    );
    cx.heap.reorder_dictionary(global, &keys);
    Ok(())
}

/// Defines `print` on the global object.
/// qjs's `print` and `console.log`.
pub(crate) fn install_print(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let global = cx.realm.global();
        let print = native_function(cx, Native::Print)?;
        define(cx, global, "print", Value::object(print), hidden())?;
        let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
        let console = plain_object(cx, Some(prototype), 1)?;
        let log = native_function(cx, Native::ConsoleLog)?;
        define(cx, console, "log", Value::object(log), hidden())?;
        define(cx, global, "console", Value::object(console), hidden())
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// qjs's `--std` helpers the suites' shells use: `load` and `read`.
pub(crate) fn install_std(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let global = cx.realm.global();
        for native in [Native::Load, Native::Read] {
            let function = native_function(cx, native)?;
            define(
                cx,
                global,
                native.js_name(),
                Value::object(function),
                hidden(),
            )?;
        }
        Ok(())
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// qjs's `scriptArgs`: an array of the script's path and arguments.
pub(crate) fn install_script_args(cx: &mut Cx<'_>, args: &[String]) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let array = script_args_array(cx, args)?;
        let global = cx.realm.global();
        define(cx, global, "scriptArgs", Value::object(array), hidden())
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// An array of `args` as strings, rooted.
fn script_args_array(cx: &mut Cx<'_>, args: &[String]) -> Result<ObjectId, HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx.heap.alloc_array(Some(prototype), args.len() as u32)?;
    cx.heap.roots().push(Value::object(array));
    for (index, arg) in args.iter().enumerate() {
        let text = cx.heap.alloc_string(arg)?;
        cx.heap
            .set_element(array, index as u32, Value::string(text))?;
    }
    Ok(array)
}

/// QuickJS's `js_std_add_helpers` as `qjs` runs it: `console` (with `log`),
/// `performance` (with `now`), `scriptArgs` when given, `print`, then
/// `__loadScript`, in that order, each writable, enumerable and
/// configurable (`JS_SetPropertyStr`).
pub(crate) fn install_qjs_helpers(
    cx: &mut Cx<'_>,
    script_args: Option<&[String]>,
) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let global = cx.realm.global();
        let all = PropertyFlags::data(true, true, true);
        let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
        let console = plain_object(cx, Some(prototype), 1)?;
        let log = native_function(cx, Native::ConsoleLog)?;
        define(cx, console, "log", Value::object(log), all)?;
        define(cx, global, "console", Value::object(console), all)?;
        let performance = plain_object(cx, Some(prototype), 1)?;
        let now = native_function(cx, Native::PerformanceNow)?;
        define(cx, performance, "now", Value::object(now), all)?;
        define(cx, global, "performance", Value::object(performance), all)?;
        if let Some(args) = script_args {
            let array = script_args_array(cx, args)?;
            define(cx, global, "scriptArgs", Value::object(array), all)?;
        }
        let print = native_function(cx, Native::Print)?;
        define(cx, global, "print", Value::object(print), all)?;
        let load_script = native_function(cx, Native::LoadScript)?;
        define(cx, global, "__loadScript", Value::object(load_script), all)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// A typed array's length, or a `TypeError` when it is out of bounds
/// (section 6.1 row 9).
pub(crate) fn typed_array_length_or_throw(cx: &mut Cx<'_>, object: ObjectId) -> Result<u64, Throw> {
    match crate::vm::typed::length(cx.heap, object) {
        Some(length) => Ok(length as u64),
        None => Err(vm_error::type_error(
            cx,
            "ArrayBuffer is detached or resized",
        )),
    }
}

/// `$262.detachArrayBuffer(buffer)` (section 6.1 row 9).
pub(crate) fn detach_array_buffer(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    // QuickJS's `JS_DetachArrayBuffer`: anything but an ArrayBuffer is
    // ignored, and the result is `undefined`.
    let buffer = site.arg(cx, 0);
    let is_array_buffer = buffer.as_object().is_some_and(|object| {
        cx.heap.internal_tag(object) == crate::heap::object::InternalTag::ArrayBuffer
    });
    if is_array_buffer {
        buffer::detach(cx, buffer)?;
    }
    Ok(Value::UNDEFINED)
}

pub(crate) use bigint::to_bigint;
pub(crate) use buffer::detach as detach_buffer;
pub(crate) use date::iso_string as date_iso_string;
pub(crate) use generator::async_from_sync_iterator;
pub(crate) use json::json_module_text;
pub(crate) use regexp::RegExpFlagCache;

/// An ordinary object inheriting from `%Object.prototype%`, rooted.
pub(crate) fn new_plain(cx: &mut Cx<'_>, slots: u32) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    cx.heap
        .alloc_object(ObjectKind::Ordinary, Some(prototype), slots)
        .map_err(|error| vm_error::heap_error(cx, error))
}
