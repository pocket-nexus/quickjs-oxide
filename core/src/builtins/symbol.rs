//! `Symbol`: the constructor (callable, not constructible), the registry
//! (`for`, `keyFor`), the well-known symbols, and `Symbol.prototype`.

use super::{Native, define, getter, install_constructor, methods, symbol_method, to_string_tag};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::runtime::Intrinsic;
use crate::shape::PropertyFlags;
use crate::value::{SymbolId, Value};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let prototype = super::plain_object(cx, Some(object_prototype), 8)?;
    cx.set_intrinsic(Intrinsic::SymbolPrototype, prototype);
    let symbol = install_constructor(cx, Native::SymbolConstructor, prototype)?;
    cx.set_intrinsic(Intrinsic::Symbol, symbol);
    methods(cx, symbol, &[Native::SymbolFor, Native::SymbolKeyFor])?;
    let constant = PropertyFlags::data(false, false, false);
    for (name, id) in cx.realm.symbols.all() {
        define(cx, symbol, name, Value::symbol(id), constant)?;
    }
    methods(
        cx,
        prototype,
        &[Native::SymbolToString, Native::SymbolValueOf],
    )?;
    getter(cx, prototype, "description", Native::SymbolDescription)?;
    let to_primitive = cx.realm.symbols.to_primitive;
    symbol_method(
        cx,
        prototype,
        to_primitive,
        Native::SymbolToPrimitive,
        PropertyFlags::data(false, false, true),
    )?;
    to_string_tag(cx, prototype, "Symbol")
}

/// `Symbol(description)`; `new Symbol` throws.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.construct {
        return Err(error::type_error(cx, "Symbol is not a constructor"));
    }
    let description = site.arg(cx, 0);
    let description = if description.is_undefined() {
        None
    } else {
        Some(super::to_string_id(cx, description)?)
    };
    cx.heap
        .alloc_symbol(description, false)
        .map(Value::symbol)
        .map_err(|error| error::heap_error(cx, error))
}

/// `Symbol.for(key)`.
pub(super) fn for_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let key = site.arg(cx, 0);
    let key = super::to_string_id(cx, key)?;
    let units: Vec<u16> = cx.heap.string(key).repr().units().collect();
    if let Some(&symbol) = cx.host.symbol_registry.get(&units) {
        return Ok(Value::symbol(symbol));
    }
    let description = cx
        .heap
        .intern_units(&units)
        .map_err(|error| error::heap_error(cx, error))?;
    let symbol = cx
        .heap
        .alloc_symbol(Some(description), false)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.add_fixed_root(Value::symbol(symbol));
    cx.host.symbol_registry.insert(units, symbol);
    Ok(Value::symbol(symbol))
}

/// `Symbol.keyFor(symbol)`.
pub(super) fn key_for(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let Some(symbol) = site.arg(cx, 0).as_symbol() else {
        return Err(error::type_error(cx, "not a symbol"));
    };
    let registered = cx.host.symbol_registry.values().any(|&id| id == symbol);
    if !registered {
        return Ok(Value::UNDEFINED);
    }
    match cx.heap.symbol_description(symbol) {
        Some(description) => Ok(Value::string(description)),
        None => Ok(Value::UNDEFINED),
    }
}

/// `thisSymbolValue`.
fn this_symbol(cx: &mut Cx<'_>, site: &CallSite) -> Result<SymbolId, Throw> {
    let this = site.this(cx);
    if let Some(symbol) = this.as_symbol() {
        return Ok(symbol);
    }
    if let Some(object) = this.as_object()
        && cx.heap.object_kind(object) == ObjectKind::SymbolWrapper
    {
        return Ok(cx.heap.internal_slot(object, 0).as_symbol().unwrap());
    }
    Err(error::type_error(cx, "not a symbol"))
}

/// `SymbolDescriptiveString`: `Symbol(description)`.
pub(crate) fn descriptive_string(cx: &mut Cx<'_>, symbol: SymbolId) -> Result<Value, Throw> {
    let mut text: Vec<u16> = "Symbol(".encode_utf16().collect();
    if let Some(description) = cx.heap.symbol_description(symbol) {
        text.extend(cx.heap.string(description).repr().units());
    }
    text.push(u16::from(b')'));
    super::string_units(cx, &text)
}

pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let symbol = this_symbol(cx, &site)?;
    descriptive_string(cx, symbol)
}

pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_symbol(cx, &site).map(Value::symbol)
}

/// `get Symbol.prototype.description`.
pub(super) fn description(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let symbol = this_symbol(cx, &site)?;
    Ok(cx
        .heap
        .symbol_description(symbol)
        .map_or(Value::UNDEFINED, Value::string))
}
