//! `Function`: `%Function.prototype%` (callable, returning `undefined`) and
//! its `call`, `apply`, `bind`, `toString` and `Symbol.hasInstance`, and
//! `%ThrowTypeError%`. `new Function` compiles source through the compile
//! host (section 5.5).

use super::{Native, define_name_length, install_constructor, methods, string_text, symbol_method};
use crate::heap::HeapError;
use crate::heap::object::{InternalTag, ObjectKind, flags};
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, arguments, error, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
    let names = cx.realm.names;
    define_name_length(cx.heap, &names, prototype, Value::string(names.empty), 0)?;
    let function = install_constructor(cx, Native::FunctionConstructor, prototype)?;
    cx.set_intrinsic(Intrinsic::Function, function);
    methods(
        cx,
        prototype,
        &[
            Native::FunctionApply,
            Native::FunctionBind,
            Native::FunctionCall,
            Native::FunctionToString,
        ],
    )?;
    let has_instance = cx.realm.symbols.has_instance;
    symbol_method(
        cx,
        prototype,
        has_instance,
        Native::FunctionHasInstance,
        PropertyFlags::data(false, false, false),
    )?;
    // `%ThrowTypeError%`: frozen, nameless, length 0.
    let thrower = super::native_function(cx, Native::ThrowTypeError)?;
    for key in [names.length, names.name] {
        cx.heap.set_property_flags(
            thrower,
            Names::key(key),
            PropertyFlags::data(false, false, false),
        )?;
    }
    cx.heap.clear_object_flag(thrower, flags::EXTENSIBLE);
    cx.set_intrinsic(Intrinsic::ThrowTypeError, thrower);
    for name in [names.caller, names.arguments] {
        cx.heap.add_accessor(
            prototype,
            Names::key(name),
            PropertyFlags::accessor(false, true),
            Value::object(thrower),
            Value::object(thrower),
        )?;
    }
    Ok(())
}

pub(super) fn prototype(_: &mut Cx<'_>, _: CallSite) -> Result<Value, Throw> {
    Ok(Value::UNDEFINED)
}

/// `%ThrowTypeError%`. As in QuickJS, reading `caller` or `arguments` of a
/// sloppy function that has a `prototype` gives `undefined` (ES5
/// compatibility); everything else throws.
pub(super) fn throw_type_error(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    use crate::bytecode::flags as unit;
    let this = site.this(cx);
    let sloppy = this.as_object().is_some_and(|object| {
        cx.heap.object_kind(object) == crate::ObjectKind::Function
            && cx.heap.function_bytecode(object).is_some_and(|bytecode| {
                let unit_id = cx.heap.bytecode_unit(bytecode);
                let code = cx.code.get(unit_id);
                !code.has(unit::STRICT) && code.has(unit::PROTOTYPE)
            })
    });
    if sloppy && site.argc == 0 {
        return Ok(Value::UNDEFINED);
    }
    Err(error::type_error(cx, "invalid property access"))
}

pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    crate::vm::compile::function_constructor(cx, site, crate::vm::compile::FunctionKind::Normal)
}

/// `Function.prototype.toString`: the source text of a bytecode function
/// (section 5.5), QuickJS's native form for the rest.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if !ops::is_callable(cx.heap, this) {
        return Err(error::type_error(cx, "not a function"));
    }
    let object = this.as_object().unwrap();
    if let Some(text) = crate::vm::compile::function_source(cx, object) {
        return super::string_units(cx, &text);
    }
    string_text(cx, "function () {\n    [native code]\n}")
}

/// `f.call(thisArg, …)` outside a rewritten call site.
pub(super) fn call(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.this(cx);
    let this = site.arg(cx, 0);
    let args: Vec<Value> = (1..site.argc).map(|index| site.arg(cx, index)).collect();
    if !ops::is_callable(cx.heap, function) {
        return Err(error::type_error(cx, "not a function"));
    }
    crate::vm::call(cx, function, this, &args)
}

/// `f.apply(thisArg, list)` outside a rewritten call site.
pub(super) fn apply(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.this(cx);
    let this = site.arg(cx, 0);
    let list = site.arg(cx, 1);
    if !ops::is_callable(cx.heap, function) {
        return Err(error::type_error(cx, "not a function"));
    }
    if list.is_undefined() || list.is_null() {
        return crate::vm::call(cx, function, this, &[]);
    }
    let mark = cx.heap.roots().mark();
    let result = arguments::list_from_array_like(cx, list).and_then(|start| {
        let args = cx.heap.roots().values()[start..].to_vec();
        crate::vm::call(cx, function, this, &args)
    });
    cx.heap.roots().truncate(mark);
    result
}

/// `Function.prototype.bind(thisArg, …args)`: a bound function whose
/// `length` is the target's less the bound arguments and whose `name` is
/// `bound ` and the target's.
pub(super) fn bind(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let target = site.this(cx);
    if !ops::is_callable(cx.heap, target) {
        return Err(error::type_error(cx, "bind: not a function"));
    }
    let target_object = target.as_object().unwrap();
    let mut slots = vec![target, site.arg(cx, 0)];
    slots.extend((1..site.argc).map(|index| site.arg(cx, index)));
    let prototype = internal::get_prototype_of(cx, target_object)?;
    let callable = cx.heap.header(target_object).flags & (flags::CALLABLE | flags::CONSTRUCTOR);
    let bound = cx
        .heap
        .alloc_internal(
            ObjectKind::BoundFunction,
            InternalTag::None,
            prototype.as_object(),
            2,
            &slots,
            Default::default(),
            callable,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(bound));
    let names = cx.realm.names;
    let mut length = 0.0;
    let length_key = Names::key(names.length);
    if internal::has_own_property(cx, target_object, length_key)? {
        let target_length = internal::get_property(cx, target_object, length_key)?;
        if let Some(number) = target_length.as_f64() {
            if number == f64::INFINITY {
                length = f64::INFINITY;
            } else if number.is_finite() {
                let integer = number.trunc();
                length = (integer - site.argc.saturating_sub(1) as f64).max(0.0);
            }
        }
    }
    let name = internal::get_property(cx, target_object, Names::key(names.name))?;
    let mut text: Vec<u16> = "bound ".encode_utf16().collect();
    if let Some(name) = name.as_string() {
        text.extend(cx.heap.string(name).repr().units());
    }
    let name = super::string_units(cx, &text)?;
    cx.heap.roots().push(name);
    let property = PropertyFlags::data(false, false, true);
    let heap = &mut *cx.heap;
    let length_value = crate::vm::number::number(length);
    heap.add_property(bound, length_key, property, length_value)
        .and_then(|()| heap.add_property(bound, Names::key(names.name), property, name))
        .map_err(|error| error::heap_error(cx, error))?;
    Ok(Value::object(bound))
}

/// `Function.prototype[Symbol.hasInstance](value)`.
pub(super) fn has_instance(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.this(cx);
    let value = site.arg(cx, 0);
    internal::ordinary_has_instance(cx, function, value).map(Value::bool)
}
