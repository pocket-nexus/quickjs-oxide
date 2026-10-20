//! Conversions between JavaScript values and Rust types, and host functions
//! over converted arguments.

use std::any::Any;
use std::rc::Rc;

use super::RuntimeError;
use super::function::{HostFunction, Scope};
use super::ops::{Js, JsResult};
use super::property::PropertyKey;
use super::value::{CallableRef, ObjectRef, Value};
use crate::engine::api::error::NativeErrorKind;
use crate::engine::value::JsString;

/// A Rust value made from a JavaScript value, as the specification's
/// conversions make it (`ToNumber`, `ToString`, …). A conversion that may
/// run JavaScript runs it through `js`.
pub trait FromJs: Sized {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self>;
}

/// A JavaScript value made from a Rust value.
pub trait IntoJs {
    fn into_js(self, js: &mut impl Js) -> JsResult<Value>;
}

fn type_error(js: &mut impl Js, message: &str) -> RuntimeError {
    js.throw_error(NativeErrorKind::Type, message)
}

/// `ToInt32` of a number.
fn int32(number: f64) -> i32 {
    uint32(number) as i32
}

/// `ToUint32` of a number.
fn uint32(number: f64) -> u32 {
    if !number.is_finite() {
        return 0;
    }
    number.trunc().rem_euclid(4_294_967_296.0) as u32
}

impl FromJs for Value {
    fn from_js(_: &mut impl Js, value: Value) -> JsResult<Self> {
        Ok(value)
    }
}

/// `ToBoolean`.
impl FromJs for bool {
    fn from_js(_: &mut impl Js, value: Value) -> JsResult<Self> {
        Ok(value.to_boolean())
    }
}

/// `ToNumber`.
impl FromJs for f64 {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        match value {
            Value::Int(value) => Ok(f64::from(value)),
            Value::Float(value) => Ok(value),
            other => js.to_number(&other),
        }
    }
}

/// `ToInt32`.
impl FromJs for i32 {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        match value {
            Value::Int(value) => Ok(value),
            other => f64::from_js(js, other).map(int32),
        }
    }
}

/// `ToUint32`.
impl FromJs for u32 {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        match value {
            Value::Int(value) => Ok(value as u32),
            other => f64::from_js(js, other).map(uint32),
        }
    }
}

/// `ToString`.
impl FromJs for JsString {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        match value {
            Value::String(text) => Ok(text),
            other => js.to_js_string(&other),
        }
    }
}

/// `ToString`, a lone surrogate replaced by U+FFFD.
impl FromJs for String {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        JsString::from_js(js, value).map(|text| text.to_utf8_lossy())
    }
}

/// `undefined` and `null` are `None`.
impl<T: FromJs> FromJs for Option<T> {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        match value {
            Value::Undefined | Value::Null => Ok(None),
            other => T::from_js(js, other).map(Some),
        }
    }
}

/// An array-like: its `length` (`ToLength`), then each index; a non-object
/// is a `TypeError`.
impl<T: FromJs> FromJs for Vec<T> {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        let Value::Object(object) = value else {
            return Err(type_error(js, "not an object"));
        };
        let key = PropertyKey::from_js_string(JsString::from_static("length"));
        let length = js.get_property(&object, &key)?;
        let length = f64::from_js(js, length)?;
        // `ToLength`.
        let length = if length.is_nan() || length <= 0.0 {
            0.0
        } else {
            length.trunc().min(9_007_199_254_740_991.0)
        };
        let mut items = Vec::new();
        let mut index = 0.0;
        while index < length {
            let key = if index < f64::from(u32::MAX) {
                PropertyKey::index(index as u32)
            } else {
                PropertyKey::from_js_string(JsString::try_from_utf8(&index.to_string())?)
            };
            let item = js.get_property(&object, &key)?;
            items.push(T::from_js(js, item)?);
            index += 1.0;
        }
        Ok(items)
    }
}

/// An object; anything else is a `TypeError`.
impl FromJs for ObjectRef {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        match value {
            Value::Object(object) => Ok(object),
            _ => Err(type_error(js, "not an object")),
        }
    }
}

/// A callable object; anything else is a `TypeError`.
impl FromJs for CallableRef {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        if let Value::Object(object) = &value
            && let Some(callable) = js.as_callable(object)?
        {
            return Ok(callable);
        }
        Err(type_error(js, "not a function"))
    }
}

/// The value a host object holds; another value is a `TypeError`.
impl<T: Any> FromJs for Rc<T> {
    fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self> {
        if let Value::Object(object) = &value
            && let Some(data) = js.host_data::<T>(object)?
        {
            return Ok(data);
        }
        let name = std::any::type_name::<T>();
        let name = name.rsplit("::").next().unwrap_or(name);
        Err(type_error(js, &format!("{name} object expected")))
    }
}

impl IntoJs for Value {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(self)
    }
}

/// `undefined`.
impl IntoJs for () {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::Undefined)
    }
}

impl IntoJs for bool {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::Bool(self))
    }
}

impl IntoJs for i32 {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::Int(self))
    }
}

impl IntoJs for u32 {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::number(f64::from(self)))
    }
}

impl IntoJs for f64 {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::number(self))
    }
}

impl IntoJs for JsString {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::String(self))
    }
}

impl IntoJs for String {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::String(JsString::try_from_utf8(&self)?))
    }
}

impl IntoJs for &str {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::String(JsString::try_from_utf8(self)?))
    }
}

impl IntoJs for ObjectRef {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::Object(self))
    }
}

impl IntoJs for CallableRef {
    fn into_js(self, _: &mut impl Js) -> JsResult<Value> {
        Ok(Value::from(self))
    }
}

/// `None` is `undefined`.
impl<T: IntoJs> IntoJs for Option<T> {
    fn into_js(self, js: &mut impl Js) -> JsResult<Value> {
        match self {
            Some(value) => value.into_js(js),
            None => Ok(Value::Undefined),
        }
    }
}

/// An array of the converted items.
impl<T: IntoJs> IntoJs for Vec<T> {
    fn into_js(self, js: &mut impl Js) -> JsResult<Value> {
        let items = self
            .into_iter()
            .map(|item| item.into_js(js))
            .collect::<JsResult<Vec<_>>>()?;
        js.new_array_from_values(items).map(Value::Object)
    }
}

/// The value, or the error to throw.
impl<T: IntoJs> IntoJs for Result<T, RuntimeError> {
    fn into_js(self, js: &mut impl Js) -> JsResult<Value> {
        self.and_then(|value| value.into_js(js))
    }
}

/// Host functions over converted arguments: a closure over the [`Scope`]
/// and up to eight arguments, each converted by [`FromJs`] (a missing one
/// from `undefined`, extra ones ignored), whose result converts by
/// [`IntoJs`].
macro_rules! typed_host_functions {
    ($(($($arg:ident),+))*) => {$(
        impl<F, R, $($arg),+> HostFunction<($($arg,)+)> for F
        where
            F: Fn(&mut Scope<'_>, $($arg),+) -> JsResult<R> + 'static,
            $($arg: FromJs,)+
            R: IntoJs,
        {
            #[allow(non_snake_case)]
            fn call(&self, scope: &mut Scope<'_>) -> JsResult<Value> {
                let mut index = 0;
                $(
                    let value = scope.arg(index);
                    let $arg = $arg::from_js(scope, value)?;
                    index += 1;
                )+
                let _ = index;
                self(scope, $($arg),+)?.into_js(scope)
            }
        }
    )*};
}

typed_host_functions! {
    (A0)
    (A0, A1)
    (A0, A1, A2)
    (A0, A1, A2, A3)
    (A0, A1, A2, A3, A4)
    (A0, A1, A2, A3, A4, A5)
    (A0, A1, A2, A3, A4, A5, A6)
    (A0, A1, A2, A3, A4, A5, A6, A7)
}
