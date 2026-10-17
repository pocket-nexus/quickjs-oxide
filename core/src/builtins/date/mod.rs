//! `Date` (phase 4, section 6.1 row 6): the constructor and its statics
//! (`now`, `parse`, `UTC`), and `Date.prototype` with the Annex B `getYear`,
//! `setYear` and `toGMTString`. The calendar, formatting and parsing kernels
//! are the old engine's, which follow QuickJS 2026-06-04; the clock and the
//! time zone come from the host (section 6.3).

mod calendar;
mod format;
mod parse;

use self::calendar::{DateFields, DateInputFields, get_date_fields, set_date_fields};
use self::calendar::{set_date_fields_checked, time_clip};
use self::format::format_date_string;
use self::parse::{ParsedDateString, parse_date_string};
use super::{Native, define, install_constructor, methods, string_text, symbol_method};
use crate::heap::HeapError;
use crate::heap::object::{InternalTag, ObjectKind};
use crate::runtime::Intrinsic;
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, number, ops};

const DEFAULT_DATE_FIELDS: DateInputFields = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
const MS_PER_MINUTE: f64 = 60_000.0;

/// The string a `Date.prototype` formatting method produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DateStringMethod {
    String,
    DateString,
    TimeString,
    UtcString,
    IsoString,
    LocaleString,
    LocaleDateString,
    LocaleTimeString,
}

impl DateStringMethod {
    /// Whether the fields are decomposed in host local time.
    const fn uses_local_time(self) -> bool {
        !matches!(self, Self::UtcString | Self::IsoString)
    }
}

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let prototype = super::plain_object(cx, Some(object_prototype), 48)?;
    cx.set_intrinsic(Intrinsic::DatePrototype, prototype);
    let date = install_constructor(cx, Native::DateConstructor, prototype)?;
    cx.set_intrinsic(Intrinsic::Date, date);
    methods(
        cx,
        date,
        &[Native::DateNow, Native::DateParse, Native::DateUtc],
    )?;
    methods(cx, prototype, &[Native::DateValueOf, Native::DateToString])?;
    let to_primitive = cx.realm.symbols.to_primitive;
    symbol_method(
        cx,
        prototype,
        to_primitive,
        Native::DateToPrimitive,
        PropertyFlags::data(false, false, true),
    )?;
    // `toGMTString` is the same function object as `toUTCString`.
    let utc = super::native_function(cx, Native::DateToUtcString)?;
    define(
        cx,
        prototype,
        "toUTCString",
        Value::object(utc),
        super::hidden(),
    )?;
    define(
        cx,
        prototype,
        "toGMTString",
        Value::object(utc),
        super::hidden(),
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::DateToIsoString,
            Native::DateToDateString,
            Native::DateToTimeString,
            Native::DateToLocaleString,
            Native::DateToLocaleDateString,
            Native::DateToLocaleTimeString,
            Native::DateGetTimezoneOffset,
            Native::DateGetTime,
            Native::DateGetYear,
            Native::DateGetFullYear,
            Native::DateGetUtcFullYear,
            Native::DateGetMonth,
            Native::DateGetUtcMonth,
            Native::DateGetDate,
            Native::DateGetUtcDate,
            Native::DateGetHours,
            Native::DateGetUtcHours,
            Native::DateGetMinutes,
            Native::DateGetUtcMinutes,
            Native::DateGetSeconds,
            Native::DateGetUtcSeconds,
            Native::DateGetMilliseconds,
            Native::DateGetUtcMilliseconds,
            Native::DateGetDay,
            Native::DateGetUtcDay,
            Native::DateSetTime,
            Native::DateSetMilliseconds,
            Native::DateSetUtcMilliseconds,
            Native::DateSetSeconds,
            Native::DateSetUtcSeconds,
            Native::DateSetMinutes,
            Native::DateSetUtcMinutes,
            Native::DateSetHours,
            Native::DateSetUtcHours,
            Native::DateSetDate,
            Native::DateSetUtcDate,
            Native::DateSetMonth,
            Native::DateSetUtcMonth,
            Native::DateSetYear,
            Native::DateSetFullYear,
            Native::DateSetUtcFullYear,
            Native::DateToJson,
        ],
    )
}

// ---- the host clock and time zone ----

fn now(cx: &mut Cx<'_>) -> f64 {
    cx.host.hooks.now_ms().trunc()
}

fn tz<'a>(cx: &'a mut Cx<'_>) -> impl FnMut(i64) -> i32 + 'a {
    move |instant| cx.host.hooks.local_tz_offset_minutes(instant as f64)
}

fn fields_of(cx: &mut Cx<'_>, value: f64, local: bool, force: bool) -> Option<DateFields> {
    get_date_fields(value, local, force, tz(cx))
}

// ---- the constructor and statics ----

/// The time value of a genuine Date, or `None`.
fn date_value(cx: &Cx<'_>, value: Value) -> Option<f64> {
    let object = value.as_object()?;
    (cx.heap.object_kind(object) == ObjectKind::Date)
        .then(|| cx.heap.internal_slot(object, 0).as_f64().unwrap())
}

/// The `Date.parse` value of `parsed`: the explicit offset applies after
/// the inner TimeClip and is not clipped again, as in QuickJS.
fn parsed_date_value(cx: &mut Cx<'_>, parsed: Option<ParsedDateString>) -> f64 {
    let Some(parsed) = parsed else {
        return f64::NAN;
    };
    let mut fields = [0.0; 7];
    for (target, source) in fields.iter_mut().zip(parsed.fields) {
        *target = f64::from(source);
    }
    let value = set_date_fields(&fields, parsed.is_local, tz(cx));
    value - f64::from(parsed.fields[8]) * MS_PER_MINUTE
}

fn parse_value(cx: &mut Cx<'_>, string: Value) -> f64 {
    let units = ops::units(cx.heap, string.as_string().unwrap());
    let parsed = parse_date_string(&units);
    parsed_date_value(cx, parsed)
}

/// The up to seven numeric fields of `Date(y, m, …)` and `Date.UTC`,
/// converted left to right.
fn argument_fields(cx: &mut Cx<'_>, site: &CallSite) -> Result<DateInputFields, Throw> {
    let mut fields = DEFAULT_DATE_FIELDS;
    for (index, field) in fields.iter_mut().enumerate().take(site.argc) {
        let value = site.arg(cx, index);
        *field = ops::to_number(cx, value)?;
    }
    Ok(fields)
}

fn format(cx: &mut Cx<'_>, value: f64, method: DateStringMethod) -> Result<Value, Throw> {
    let fields = fields_of(cx, value, method.uses_local_time(), false);
    match format_date_string(fields.as_ref(), method) {
        Ok(text) => string_text(cx, &text),
        Err(_) => Err(error::range_error(cx, "Date value is NaN")),
    }
}

/// The ISO form of a time value for the value printer; `None` when it is
/// invalid.
pub(crate) fn iso_string(cx: &mut Cx<'_>, value: f64) -> Option<String> {
    let method = DateStringMethod::IsoString;
    let fields = fields_of(cx, value, method.uses_local_time(), false);
    format_date_string(fields.as_ref(), method).ok()
}

/// `Date(...)`: a call returns the current time as a string; `new Date`
/// makes a Date object.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if !site.construct {
        let value = now(cx);
        return format(cx, value, DateStringMethod::String);
    }
    let value = match site.argc {
        0 => now(cx),
        1 => {
            let argument = site.arg(cx, 0);
            match date_value(cx, argument) {
                Some(value) => value,
                None => {
                    let primitive = ops::to_primitive(cx, argument, ops::Hint::Default)?;
                    if primitive.as_string().is_some() {
                        parse_value(cx, primitive)
                    } else {
                        ops::to_number(cx, primitive)?
                    }
                }
            }
        }
        _ => {
            let fields = argument_fields(cx, &site)?;
            set_date_fields_checked(fields, true, tz(cx))
        }
    };
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::DatePrototype)?;
    cx.heap
        .alloc_internal(
            ObjectKind::Date,
            InternalTag::None,
            Some(prototype),
            0,
            &[Value::number(time_clip(value))],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

pub(super) fn now_(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    let value = now(cx);
    Ok(number::number(value))
}

pub(super) fn parse(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = site.arg(cx, 0);
    let string = ops::to_string(cx, string)?;
    let value = parse_value(cx, string);
    Ok(number::number(value))
}

pub(super) fn utc(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.argc == 0 {
        return Ok(Value::number(f64::NAN));
    }
    let fields = argument_fields(cx, &site)?;
    let value = set_date_fields_checked(fields, false, tz(cx));
    Ok(number::number(value))
}

// ---- Date.prototype ----

/// `thisTimeValue`: the receiver and its time value.
fn this_time(cx: &mut Cx<'_>, site: &CallSite) -> Result<(ObjectId, f64), Throw> {
    let this = site.this(cx);
    match date_value(cx, this) {
        Some(value) => Ok((this.as_object().unwrap(), value)),
        None => Err(error::type_error(cx, "not a Date object")),
    }
}

fn store(cx: &mut Cx<'_>, object: ObjectId, value: f64) -> Value {
    cx.heap.internal_mut(object).slots[0] = Value::number(value);
    number::number(value)
}

pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_time(cx, &site).map(|(_, value)| number::number(value))
}

macro_rules! string_methods {
    ($($function:ident => $method:ident;)*) => {
        $(
            pub(super) fn $function(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                let (_, value) = this_time(cx, &site)?;
                format(cx, value, DateStringMethod::$method)
            }
        )*
    };
}

string_methods! {
    to_string => String;
    to_date_string => DateString;
    to_time_string => TimeString;
    to_utc_string => UtcString;
    to_iso_string => IsoString;
    to_locale_string => LocaleString;
    to_locale_date_string => LocaleDateString;
    to_locale_time_string => LocaleTimeString;
}

/// The getters: the field index in the decomposition, local time, and
/// `getYear`'s 1900 offset.
macro_rules! getters {
    ($($function:ident => $field:expr, $local:expr, $legacy:expr;)*) => {
        $(
            pub(super) fn $function(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                let (_, value) = this_time(cx, &site)?;
                let Some(fields) = fields_of(cx, value, $local, false) else {
                    return Ok(Value::number(f64::NAN));
                };
                let offset = if $legacy { 1900.0 } else { 0.0 };
                Ok(number::number(fields[$field] - offset))
            }
        )*
    };
}

getters! {
    get_year => 0, true, true;
    get_full_year => 0, true, false;
    get_utc_full_year => 0, false, false;
    get_month => 1, true, false;
    get_utc_month => 1, false, false;
    get_date => 2, true, false;
    get_utc_date => 2, false, false;
    get_hours => 3, true, false;
    get_utc_hours => 3, false, false;
    get_minutes => 4, true, false;
    get_utc_minutes => 4, false, false;
    get_seconds => 5, true, false;
    get_utc_seconds => 5, false, false;
    get_milliseconds => 6, true, false;
    get_utc_milliseconds => 6, false, false;
    get_day => 7, true, false;
    get_utc_day => 7, false, false;
}

pub(super) fn get_timezone_offset(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (_, value) = this_time(cx, &site)?;
    if value.is_nan() {
        return Ok(Value::number(f64::NAN));
    }
    let offset = cx.host.hooks.local_tz_offset_minutes(value.trunc());
    Ok(Value::int32(offset))
}

/// A field setter over fields `first..end`: the fields are decomposed
/// first (forced for the full-year setters, which recover an invalid date),
/// then the arguments are converted, then the date is rebuilt.
fn set_fields(
    cx: &mut Cx<'_>,
    site: &CallSite,
    first: usize,
    end: usize,
    local: bool,
) -> Result<Value, Throw> {
    let (object, value) = this_time(cx, site)?;
    let fields = fields_of(cx, value, local, first == 0);
    let decomposed = fields.is_some();
    let mut all_finite = decomposed;
    let mut fields = fields.unwrap_or([0.0; 9]);
    let count = site.argc.min(end - first);
    for index in 0..count {
        let argument = site.arg(cx, index);
        let number = ops::to_number(cx, argument)?;
        all_finite &= number.is_finite();
        fields[first + index] = number.trunc();
    }
    if !decomposed {
        return Ok(Value::number(f64::NAN));
    }
    let value = if all_finite && site.argc > 0 {
        let input: DateInputFields = fields[..7].try_into().unwrap();
        set_date_fields(&input, local, tz(cx))
    } else {
        f64::NAN
    };
    Ok(store(cx, object, value))
}

macro_rules! setters {
    ($($function:ident => $first:expr, $end:expr, $local:expr;)*) => {
        $(
            pub(super) fn $function(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                set_fields(cx, &site, $first, $end, $local)
            }
        )*
    };
}

setters! {
    set_milliseconds => 6, 7, true;
    set_utc_milliseconds => 6, 7, false;
    set_seconds => 5, 7, true;
    set_utc_seconds => 5, 7, false;
    set_minutes => 4, 7, true;
    set_utc_minutes => 4, 7, false;
    set_hours => 3, 7, true;
    set_utc_hours => 3, 7, false;
    set_date => 2, 3, true;
    set_utc_date => 2, 3, false;
    set_month => 1, 3, true;
    set_utc_month => 1, 3, false;
    set_full_year => 0, 3, true;
    set_utc_full_year => 0, 3, false;
}

pub(super) fn set_time(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, _) = this_time(cx, &site)?;
    let argument = site.arg(cx, 0);
    let value = ops::to_number(cx, argument)?;
    Ok(store(cx, object, time_clip(value)))
}

/// Annex B `setYear`: years 0 to 99 mean 1900 to 1999.
pub(super) fn set_year(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, _) = this_time(cx, &site)?;
    let argument = site.arg(cx, 0);
    let mut year = ops::to_number(cx, argument)?;
    let current = date_value(cx, Value::object(object)).unwrap_or(f64::NAN);
    if year.is_finite() {
        year = year.trunc();
        if (0.0..100.0).contains(&year) {
            year += 1900.0;
        }
    }
    let mut fields = fields_of(cx, current, true, true).expect("a forced decomposition");
    fields[0] = year;
    let value = if year.is_finite() {
        let input: DateInputFields = fields[..7].try_into().unwrap();
        set_date_fields(&input, true, tz(cx))
    } else {
        f64::NAN
    };
    Ok(store(cx, object, value))
}

/// `[Symbol.toPrimitive](hint)`: `OrdinaryToPrimitive` with `"default"`
/// meaning `"string"`.
pub(super) fn to_primitive(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = this.as_object() else {
        return Err(error::type_error(cx, "not an object"));
    };
    let hint = site.arg(cx, 0);
    let hint = match hint.as_string().map(|hint| cx.heap.string_text(hint)) {
        Some(text) if text == "number" || text == "integer" => ops::Hint::Number,
        Some(text) if text == "string" || text == "default" => ops::Hint::String,
        _ => return Err(error::type_error(cx, "invalid hint")),
    };
    ops::ordinary_to_primitive(cx, object, hint)
}

/// `toJSON`: generic; `null` for a non-finite time value, otherwise the
/// receiver's `toISOString()`.
pub(super) fn to_json(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let object = internal::to_object(cx, this)?;
    cx.heap.roots().push(Value::object(object));
    let primitive = ops::to_primitive(cx, Value::object(object), ops::Hint::Number)?;
    if let Some(number) = primitive.as_f64()
        && !number.is_finite()
    {
        return Ok(Value::NULL);
    }
    let key = cx
        .heap
        .property_key("toISOString")
        .map_err(|error| error::heap_error(cx, error))?;
    internal::invoke(cx, Value::object(object), key, &[])
}
