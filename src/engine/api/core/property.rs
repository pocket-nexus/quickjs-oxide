//! Property keys and descriptors (phase 6, section 2.7), in the current
//! API's shapes.

use super::RuntimeError;
use super::value::{CallableRef, SymbolRef, Value};
use crate::engine::value::JsString;

/// A property key: a string, an array index or a symbol. Made by
/// [`Runtime::intern_property_key`](super::Runtime::intern_property_key) or
/// from a [`SymbolRef`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyKey(pub(crate) Key);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    String(JsString),
    Index(u32),
    Symbol(SymbolRef),
}

impl PropertyKey {
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    /// The key of the string `text` (an index when `text` is one's
    /// canonical form, as `ToPropertyKey` reads it).
    pub fn from_js_string(text: JsString) -> Self {
        let units: Vec<u16> = text.utf16_units().collect();
        match canonical_index(&units) {
            Some(index) => PropertyKey(Key::Index(index)),
            None => PropertyKey(Key::String(text)),
        }
    }

    pub fn index(index: u32) -> Self {
        if index < u32::MAX {
            PropertyKey(Key::Index(index))
        } else {
            PropertyKey(Key::String(JsString::from_fresh_decimal_u32(index)))
        }
    }

    pub fn as_symbol(&self) -> Option<&SymbolRef> {
        match &self.0 {
            Key::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }

    /// The key as a string; `None` for a symbol.
    pub fn to_js_string(&self) -> Option<JsString> {
        match &self.0 {
            Key::String(text) => Some(text.clone()),
            Key::Index(index) => Some(JsString::from_fresh_decimal_u32(*index)),
            Key::Symbol(_) => None,
        }
    }
}

/// `CanonicalNumericIndexString` restricted to array indices: digits without
/// a leading zero, below `2^32 - 1`.
fn canonical_index(units: &[u16]) -> Option<u32> {
    if units.is_empty() || units.len() > 10 || (units.len() > 1 && units[0] == u16::from(b'0')) {
        return None;
    }
    let mut value: u64 = 0;
    for &unit in units {
        let digit = unit.checked_sub(u16::from(b'0')).filter(|&d| d < 10)?;
        value = value * 10 + u64::from(digit);
    }
    (value < u64::from(u32::MAX)).then_some(value as u32)
}

impl From<SymbolRef> for PropertyKey {
    fn from(symbol: SymbolRef) -> Self {
        PropertyKey(Key::Symbol(symbol))
    }
}

impl TryFrom<&SymbolRef> for PropertyKey {
    type Error = RuntimeError;

    fn try_from(symbol: &SymbolRef) -> Result<Self, RuntimeError> {
        Ok(PropertyKey(Key::Symbol(symbol.clone())))
    }
}

/// A descriptor field: absent, or present with a value (`Present(undefined)`
/// and `Absent` mean different things).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DescriptorField<T> {
    #[default]
    Absent,
    Present(T),
}

impl<T> DescriptorField<T> {
    pub fn try_map<U, E>(
        &self,
        map: impl FnOnce(&T) -> Result<U, E>,
    ) -> Result<DescriptorField<U>, E> {
        match self {
            Self::Absent => Ok(DescriptorField::Absent),
            Self::Present(value) => map(value).map(DescriptorField::Present),
        }
    }

    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    pub const fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }

    pub const fn as_ref(&self) -> DescriptorField<&T> {
        match self {
            Self::Absent => DescriptorField::Absent,
            Self::Present(value) => DescriptorField::Present(value),
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> DescriptorField<U> {
        match self {
            Self::Absent => DescriptorField::Absent,
            Self::Present(value) => DescriptorField::Present(map(value)),
        }
    }

    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Absent => None,
            Self::Present(value) => Some(value),
        }
    }
}

impl<T> From<Option<T>> for DescriptorField<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Present(value),
            None => Self::Absent,
        }
    }
}

/// A present `get` or `set` field: a function or `undefined`.
#[derive(Clone, Debug, PartialEq)]
pub enum AccessorValue {
    Undefined,
    Callable(CallableRef),
}

impl AccessorValue {
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }
}

/// A property descriptor for `[[DefineOwnProperty]]`, possibly partial.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrdinaryPropertyDescriptor {
    pub value: DescriptorField<Value>,
    pub writable: DescriptorField<bool>,
    pub get: DescriptorField<AccessorValue>,
    pub set: DescriptorField<AccessorValue>,
    pub enumerable: DescriptorField<bool>,
    pub configurable: DescriptorField<bool>,
}

impl OrdinaryPropertyDescriptor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    /// A data property with every attribute present.
    pub fn data(value: Value, writable: bool, enumerable: bool, configurable: bool) -> Self {
        OrdinaryPropertyDescriptor {
            value: DescriptorField::Present(value),
            writable: DescriptorField::Present(writable),
            enumerable: DescriptorField::Present(enumerable),
            configurable: DescriptorField::Present(configurable),
            ..Self::default()
        }
    }
}

/// A complete own property, as `[[GetOwnProperty]]` reports it.
#[derive(Clone, Debug, PartialEq)]
pub enum CompleteOrdinaryPropertyDescriptor {
    Data {
        value: Value,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    },
    Accessor {
        get: Option<CallableRef>,
        set: Option<CallableRef>,
        enumerable: bool,
        configurable: bool,
    },
}

impl CompleteOrdinaryPropertyDescriptor {
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    pub const fn enumerable(&self) -> bool {
        match self {
            Self::Data { enumerable, .. } | Self::Accessor { enumerable, .. } => *enumerable,
        }
    }

    pub const fn configurable(&self) -> bool {
        match self {
            Self::Data { configurable, .. } | Self::Accessor { configurable, .. } => *configurable,
        }
    }
}

/// The well-known symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WellKnownSymbol {
    ToPrimitive,
    Iterator,
    Match,
    MatchAll,
    Replace,
    Search,
    Split,
    ToStringTag,
    IsConcatSpreadable,
    HasInstance,
    Species,
    Unscopables,
    AsyncIterator,
}

impl WellKnownSymbol {
    pub const ALL: [Self; 13] = [
        Self::ToPrimitive,
        Self::Iterator,
        Self::Match,
        Self::MatchAll,
        Self::Replace,
        Self::Search,
        Self::Split,
        Self::ToStringTag,
        Self::IsConcatSpreadable,
        Self::HasInstance,
        Self::Species,
        Self::Unscopables,
        Self::AsyncIterator,
    ];

    pub const fn description(self) -> &'static str {
        match self {
            Self::ToPrimitive => "Symbol.toPrimitive",
            Self::Iterator => "Symbol.iterator",
            Self::Match => "Symbol.match",
            Self::MatchAll => "Symbol.matchAll",
            Self::Replace => "Symbol.replace",
            Self::Search => "Symbol.search",
            Self::Split => "Symbol.split",
            Self::ToStringTag => "Symbol.toStringTag",
            Self::IsConcatSpreadable => "Symbol.isConcatSpreadable",
            Self::HasInstance => "Symbol.hasInstance",
            Self::Species => "Symbol.species",
            Self::Unscopables => "Symbol.unscopables",
            Self::AsyncIterator => "Symbol.asyncIterator",
        }
    }

    /// The property name on `Symbol` (`"iterator"`).
    pub fn name(self) -> &'static str {
        &self.description()["Symbol.".len()..]
    }
}
