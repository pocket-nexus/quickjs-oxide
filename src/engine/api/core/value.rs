//! Values: primitives as Rust data copied out of the
//! core, objects and symbols as handles in the core's handle table.

use std::fmt;
use std::rc::Rc;

use oxide_core::Handle;

use super::RuntimeError;
use super::runtime::Shared;
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;

/// A JavaScript value held by the embedder.
#[derive(Clone)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    BigInt(JsBigInt),
    String(JsString),
    Symbol(SymbolRef),
    Object(ObjectRef),
}

impl Value {
    /// The same value; never fails (kept for the current API's callers).
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    /// A number: an `Int` when `value` is an integer in `i32` range other
    /// than `-0`, else a `Float`.
    pub fn number(value: f64) -> Self {
        if value.fract() == 0.0
            && value >= f64::from(i32::MIN)
            && value <= f64::from(i32::MAX)
            && !(value == 0.0 && value.is_sign_negative())
        {
            Value::Int(value as i32)
        } else {
            Value::Float(value)
        }
    }

    /// The numeric value of a number.
    pub const fn as_number(&self) -> Option<f64> {
        match self {
            Value::Int(value) => Some(*value as f64),
            Value::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&ObjectRef> {
        match self {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }

    /// `ToBoolean`, which never runs JavaScript.
    pub fn to_boolean(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Bool(value) => *value,
            Value::Int(value) => *value != 0,
            Value::Float(value) => *value != 0.0 && !value.is_nan(),
            Value::BigInt(value) => !value.is_zero(),
            Value::String(value) => !value.is_empty(),
            // `document.all`-like objects exist only under the Test262 host,
            // whose `IsHTMLDDA` the runtime's own `to_boolean` reports.
            Value::Symbol(_) | Value::Object(_) => true,
        }
    }

    /// `typeof`, for primitives and ordinary objects (a callable object is
    /// `"object"` here; `Runtime::type_of` asks the engine).
    pub const fn type_of(&self) -> &'static str {
        match self {
            Value::Undefined => "undefined",
            Value::Null | Value::Object(_) => "object",
            Value::Bool(_) => "boolean",
            Value::Int(_) | Value::Float(_) => "number",
            Value::BigInt(_) => "bigint",
            Value::String(_) => "string",
            Value::Symbol(_) => "symbol",
        }
    }

    /// `IsStrictlyEqual`.
    pub fn strict_equal(&self, other: &Self) -> bool {
        match (self.as_number(), other.as_number()) {
            (Some(x), Some(y)) => x == y,
            _ => self.same_value(other),
        }
    }

    /// `SameValue`.
    pub fn same_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::BigInt(x), Value::BigInt(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            (Value::Symbol(x), Value::Symbol(y)) => x == y,
            (Value::Object(x), Value::Object(y)) => x == y,
            _ => match (self.as_number(), other.as_number()) {
                (Some(x), Some(y)) => {
                    (x.is_nan() && y.is_nan())
                        || (x == y && x.is_sign_negative() == y.is_sign_negative())
                }
                _ => false,
            },
        }
    }

    /// `SameValueZero`.
    pub fn same_value_zero(&self, other: &Self) -> bool {
        match (self.as_number(), other.as_number()) {
            (Some(x), Some(y)) => (x.is_nan() && y.is_nan()) || x == y,
            _ => self.same_value(other),
        }
    }
}

/// Structural equality: primitives by representation (`Int(1) != Float(1.0)`,
/// as the current API's derived equality), references by identity.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Float(x), Value::Float(y)) => x == y,
            (Value::BigInt(x), Value::BigInt(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            (Value::Symbol(x), Value::Symbol(y)) => x == y,
            (Value::Object(x), Value::Object(y)) => x == y,
            _ => false,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Undefined => f.write_str("Undefined"),
            Value::Null => f.write_str("Null"),
            Value::Bool(value) => write!(f, "Bool({value})"),
            Value::Int(value) => write!(f, "Int({value})"),
            Value::Float(value) => write!(f, "Float({value:?})"),
            Value::BigInt(value) => write!(f, "BigInt({value})"),
            Value::String(value) => write!(f, "String({:?})", value.to_utf8_lossy()),
            Value::Symbol(value) => write!(f, "Symbol({value:?})"),
            Value::Object(value) => write!(f, "Object({value:?})"),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::number(value)
    }
}

impl From<JsString> for Value {
    fn from(value: JsString) -> Self {
        Value::String(value)
    }
}

impl From<ObjectRef> for Value {
    fn from(value: ObjectRef) -> Self {
        Value::Object(value)
    }
}

impl From<CallableRef> for Value {
    fn from(value: CallableRef) -> Self {
        Value::Object(value.0)
    }
}

/// One handle in the core's handle table, released when the last reference
/// drops (deferred to the next entry when the runtime is borrowed).
pub(crate) struct Slot {
    pub(crate) shared: Rc<Shared>,
    pub(crate) handle: Handle,
    /// The held value; it never changes, and objects do not move.
    pub(crate) value: oxide_core::Value,
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.shared.release(self.handle);
    }
}

/// A reference to an object of a runtime.
#[derive(Clone)]
pub struct ObjectRef {
    pub(crate) slot: Rc<Slot>,
}

/// A reference to a symbol of a runtime.
#[derive(Clone)]
pub struct SymbolRef {
    pub(crate) slot: Rc<Slot>,
}

/// An object reference known to be callable.
#[derive(Clone, PartialEq, Debug)]
pub struct CallableRef(pub(crate) ObjectRef);

macro_rules! reference {
    ($name:ident) => {
        impl $name {
            pub fn try_clone(&self) -> Result<Self, RuntimeError> {
                Ok(self.clone())
            }

            /// Whether this reference belongs to `runtime`.
            pub fn belongs_to(&self, runtime: &super::Runtime) -> bool {
                Rc::ptr_eq(&self.slot.shared, &runtime.shared)
            }

            pub(crate) fn core_value(&self) -> oxide_core::Value {
                self.slot.value
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.slot.shared, &other.slot.shared)
                    && self.slot.value == other.slot.value
            }
        }

        impl Eq for $name {}

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.slot.value)
            }
        }
    };
}

reference!(ObjectRef);
reference!(SymbolRef);

impl CallableRef {
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    pub fn as_object(&self) -> &ObjectRef {
        &self.0
    }

    pub fn into_object(self) -> ObjectRef {
        self.0
    }

    pub fn belongs_to(&self, runtime: &super::Runtime) -> bool {
        self.0.belongs_to(runtime)
    }
}

impl std::ops::Deref for CallableRef {
    type Target = ObjectRef;

    fn deref(&self) -> &ObjectRef {
        &self.0
    }
}
