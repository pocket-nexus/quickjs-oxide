//! The class, `super` and private-name opcodes, the object spread copies and
//! the environment references of destructuring. All run out of line through
//! [`class_op`].
//!
//! A class with private methods brands its instances: its home object holds
//! a fresh private symbol under the realm's brand key, and the instance
//! initializer defines that symbol on each instance; a private method or
//! accessor checks the receiver for the brand of its home object.

use super::error::{self, ErrorKind};
use super::object;
use super::ops;
use super::property;
use super::{Cx, Frame, Throw};
use crate::bytecode::{CodeUnit, Insn, Opcode, flags};
use crate::heap::atom::PropertyKey;
use crate::heap::object::{NONE, ObjectKind};
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, ScopeId, Value};

#[inline]
fn reg(cx: &Cx<'_>, frame: &Frame, register: u8) -> Value {
    cx.heap.vm.registers[frame.base as usize + register as usize]
}

#[inline]
fn set_reg(cx: &mut Cx<'_>, frame: &Frame, register: u8, value: Value) {
    cx.heap.vm.registers[frame.base as usize + register as usize] = value;
}

/// Runs one class-family opcode with the root stack cut back afterwards.
#[inline(never)]
pub(super) fn class_op(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    insn: Insn,
) -> Result<(), Throw> {
    let mark = cx.heap.roots().mark();
    let result = class_op_rooted(cx, frame, unit, insn);
    cx.heap.roots().truncate(mark);
    result
}

fn class_op_rooted(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    insn: Insn,
) -> Result<(), Throw> {
    match insn.op {
        Opcode::MakeClass => {
            let (parent, closure) = (reg(cx, frame, insn.a), reg(cx, frame, insn.b));
            let prototype = make_class(cx, parent, closure, insn.c != 0)?;
            // An anonymous `export default class` is named `default`.
            let key = insn.key();
            if let crate::KeyKind::String(name) = key.kind()
                && cx.heap.string_len(name) != 0
            {
                super::interp::set_function_name(cx, function(closure), key, "", false)?;
            }
            set_reg(cx, frame, insn.a, closure);
            set_reg(cx, frame, insn.b, Value::object(prototype));
        }
        Opcode::InstallFields => {
            let constructor = function(reg(cx, frame, insn.a));
            let home = reg(cx, frame, insn.b);
            let initializer = function(reg(cx, frame, insn.c));
            let home = home.as_object().expect("a home object");
            cx.heap.function_record_mut(constructor).fields = initializer.index();
            cx.heap.function_record_mut(initializer).home_object = home.index();
            // The class body has ended: QuickJS's `OP_add_brand` gives the
            // prototype its brand only now, so a computed key evaluated
            // before sees none.
            let record = cx.heap.function_record(initializer);
            if record.bytecode != NONE
                && cx
                    .unit(
                        cx.heap
                            .bytecode_unit(crate::value::BytecodeId(record.bytecode)),
                    )
                    .has(flags::BRAND)
            {
                ensure_brand(cx, home)?;
            }
        }
        Opcode::InitFields => {
            let receiver = reg(cx, frame, insn.a);
            let constructor = function(reg(cx, frame, insn.b));
            let fields = cx.heap.function_record(constructor).fields;
            if fields != NONE {
                run_initializer(cx, ObjectId(fields), receiver)?;
            }
        }
        Opcode::RunStaticInit => {
            let constructor = reg(cx, frame, insn.a);
            let initializer = function(reg(cx, frame, insn.b));
            cx.heap.function_record_mut(initializer).home_object =
                constructor.as_object().expect("a constructor").index();
            run_initializer(cx, initializer, constructor)?;
        }
        Opcode::CallStaticBlock => {
            let block = function(reg(cx, frame, insn.a));
            let home = cx.heap.function_record(frame.function).home_object;
            cx.heap.function_record_mut(block).home_object = home;
            let this = reg(cx, frame, 0);
            super::call(cx, Value::object(block), this, &[])?;
        }
        Opcode::GetSuper => {
            let home = reg(cx, frame, insn.b);
            let prototype = match home.as_object() {
                Some(home) => object::get_prototype_of(cx, home)?,
                None => Value::NULL,
            };
            set_reg(cx, frame, insn.a, prototype);
        }
        Opcode::GetSuperValue => {
            let receiver = reg(cx, frame, insn.b);
            let base = reg(cx, frame, insn.b + 1);
            let key = reg(cx, frame, insn.b + 2);
            let key = ops::to_property_key(cx, key)?;
            let Some(base) = base.as_object() else {
                return Err(super_base_error(cx, base, key));
            };
            // The reference's thisValue is the receiver in the call form
            // too (QuickJS's reads with `base`).
            let value = object::get(cx, base, key, receiver)?;
            set_reg(cx, frame, insn.a, value);
        }
        Opcode::PutSuperValue => {
            let receiver = reg(cx, frame, insn.a);
            let base = reg(cx, frame, insn.a + 1);
            let key = reg(cx, frame, insn.a + 2);
            let value = reg(cx, frame, insn.a + 3);
            // QuickJS rejects the base before converting the key.
            let Some(base) = base.as_object() else {
                return Err(error::type_error(cx, "not an object"));
            };
            let key = ops::to_property_key(cx, key)?;
            // The error reads the key after a setter or trap ran.
            let mark = cx.heap.roots().mark();
            object::root_keys(cx, &[key]);
            let result = object::set(cx, base, key, value, receiver).and_then(|done| {
                if !done && unit.has(flags::STRICT) {
                    return Err(property::set_failed(cx, base, key));
                }
                Ok(())
            });
            cx.heap.roots().truncate(mark);
            result?;
        }
        Opcode::InitDerived => {
            let new_target = reg(cx, frame, insn.b);
            let parent = object::get_prototype_of(cx, frame.function)?;
            if !object::is_constructor(cx, parent) {
                return Err(error::construct_error(cx, parent));
            }
            cx.heap.roots().push(parent);
            let args: Vec<Value> = (0..frame.argc as usize)
                .map(|index| super::arguments::actual_argument(cx, frame, unit, index))
                .collect();
            let result = super::construct(cx, parent, &args, new_target)?;
            set_reg(cx, frame, insn.a, result);
        }
        Opcode::InitThis => {
            if !reg(cx, frame, insn.a).is_empty() {
                return Err(this_bound(cx));
            }
            let value = reg(cx, frame, insn.b);
            set_reg(cx, frame, insn.a, value);
        }
        Opcode::InitThisCell => {
            let cell = cell(reg(cx, frame, insn.a));
            if !cx.heap.scope_get(cell, 0, 0).is_empty() {
                return Err(this_bound(cx));
            }
            let value = reg(cx, frame, insn.b);
            cx.heap.scope_set(cell, 0, 0, value);
        }
        Opcode::InitThisCaptured => {
            let captures = cx
                .heap
                .function_scope(frame.function)
                .expect("a closure with captures");
            let cell = cell(cx.heap.scope_get(captures, 0, insn.imm));
            if !cx.heap.scope_get(cell, 0, 0).is_empty() {
                return Err(this_bound(cx));
            }
            let value = reg(cx, frame, insn.a);
            cx.heap.scope_set(cell, 0, 0, value);
        }
        Opcode::CheckDerivedReturn => {
            let value = reg(cx, frame, insn.a);
            if value.as_object().is_some() {
                return Ok(());
            }
            if !value.is_undefined() {
                return Err(error::type_error(
                    cx,
                    "derived class constructor must return an object or undefined",
                ));
            }
            let this = reg(cx, frame, insn.b);
            if this.is_empty() {
                return Err(error::throw(
                    cx,
                    ErrorKind::Reference,
                    "this is not initialized",
                ));
            }
            set_reg(cx, frame, insn.a, this);
        }
        Opcode::NewPrivateName => {
            let description = match insn.key().kind() {
                crate::heap::atom::KeyKind::String(id) => Some(id),
                _ => None,
            };
            let symbol = cx
                .heap
                .alloc_symbol(description, true)
                .map_err(|error| error::heap_error(cx, error))?;
            set_reg(cx, frame, insn.a, Value::symbol(symbol));
        }
        Opcode::SetHomeObject => {
            let method = function(reg(cx, frame, insn.a));
            let home = reg(cx, frame, insn.b).as_object().expect("a home object");
            cx.heap.function_record_mut(method).home_object = home.index();
        }
        Opcode::NamePrivateMethod => {
            let method = function(reg(cx, frame, insn.a));
            let prefix = match insn.c {
                1 => "get ",
                2 => "set ",
                _ => "",
            };
            super::interp::set_function_name(cx, method, insn.key(), prefix, true)?;
        }
        Opcode::GetPrivate => {
            let (target, name) = (reg(cx, frame, insn.b), reg(cx, frame, insn.c));
            let target = private_target(cx, target)?;
            let key = private_name(cx, name)?;
            match cx.heap.lookup_own(target, key) {
                Some((slot, _)) => {
                    let value = cx.heap.get_slot(target, slot);
                    set_reg(cx, frame, insn.a, value);
                }
                None => return Err(missing_private(cx, key)),
            }
        }
        Opcode::PutPrivate => {
            let (target, value, name) = (
                reg(cx, frame, insn.a),
                reg(cx, frame, insn.b),
                reg(cx, frame, insn.c),
            );
            let target = private_target(cx, target)?;
            let key = private_name(cx, name)?;
            match cx.heap.lookup_own(target, key) {
                Some((slot, _)) => cx.heap.set_slot(target, slot, value),
                None => return Err(missing_private(cx, key)),
            }
        }
        Opcode::DefinePrivate => {
            let (target, value, name) = (
                reg(cx, frame, insn.a),
                reg(cx, frame, insn.b),
                reg(cx, frame, insn.c),
            );
            let target = private_target(cx, target)?;
            let key = private_name(cx, name)?;
            object::materialize(cx, target)?;
            if cx.heap.lookup_own(target, key).is_some() {
                return Err(error::type_error(
                    cx,
                    &format!(
                        "private class field '{}' already exists",
                        property::atom_text(cx.heap, key)
                    ),
                ));
            }
            cx.heap
                .add_property(target, key, PropertyFlags::data(true, false, false), value)
                .map_err(|error| error::heap_error(cx, error))?;
        }
        Opcode::PrivateIn => {
            let (target, name) = (reg(cx, frame, insn.b), reg(cx, frame, insn.c));
            let Some(target) = target.as_object() else {
                return Err(error::type_error(cx, "invalid 'in' operand"));
            };
            let key = private_name(cx, name)?;
            let has = cx.heap.lookup_own(target, key).is_some();
            set_reg(cx, frame, insn.a, Value::bool(has));
        }
        Opcode::GetPrivateMethod => {
            let (target, method) = (reg(cx, frame, insn.b), reg(cx, frame, insn.c));
            check_brand(cx, target, method)?;
            set_reg(cx, frame, insn.a, method);
        }
        Opcode::CallPrivateGetter => {
            let (target, getter) = (reg(cx, frame, insn.b), reg(cx, frame, insn.c));
            check_brand(cx, target, getter)?;
            let value = super::call(cx, getter, target, &[])?;
            set_reg(cx, frame, insn.a, value);
        }
        Opcode::CallPrivateSetter => {
            let (target, value, setter) = (
                reg(cx, frame, insn.a),
                reg(cx, frame, insn.b),
                reg(cx, frame, insn.c),
            );
            check_brand(cx, target, setter)?;
            super::call(cx, setter, target, &[value])?;
        }
        Opcode::PrivateBrandIn => {
            let (target, method) = (reg(cx, frame, insn.b), reg(cx, frame, insn.c));
            let Some(target) = target.as_object() else {
                return Err(error::type_error(cx, "invalid 'in' operand"));
            };
            let has = brand_check(cx, Value::object(target), method)?;
            set_reg(cx, frame, insn.a, Value::bool(has));
        }
        Opcode::CopyData => {
            let target = reg(cx, frame, insn.a)
                .as_object()
                .expect("a literal object");
            // A primitive source is converted (`{..."ab"}` copies 0 and 1;
            // QuickJS spreads nothing from a primitive).
            let source = reg(cx, frame, insn.b);
            object::copy_data_properties(cx, target, source, &[])?;
        }
        Opcode::CopyDataExcluded => {
            let target = reg(cx, frame, insn.a).as_object().expect("a rest object");
            let source = reg(cx, frame, insn.b);
            let excluded = reg(cx, frame, insn.c);
            let excluded = match excluded.as_object() {
                Some(excluded) => object::own_property_keys(cx, excluded)?,
                None => Vec::new(),
            };
            object::copy_data_properties(cx, target, source, &excluded)?;
        }
        Opcode::GlobalRef => {
            let key = insn.key();
            let lexical = cx.realm.intrinsic(Intrinsic::GlobalLexical);
            let environment = if let Some((slot, flags)) = cx.heap.lookup_own(lexical, key) {
                if cx.heap.get_slot(lexical, slot).is_empty() {
                    return Err(binding_error(
                        cx,
                        ErrorKind::Reference,
                        key,
                        " is not initialized",
                    ));
                }
                if !flags.has(PropertyFlags::WRITABLE) {
                    return Err(binding_error(cx, ErrorKind::Type, key, " is read-only"));
                }
                Value::object(lexical)
            } else {
                let global = cx.realm.global();
                if property::has(cx, global, key)? {
                    Value::object(global)
                } else {
                    Value::UNDEFINED
                }
            };
            set_reg(cx, frame, insn.a, environment);
        }
        Opcode::GetRef => {
            let key = insn.key();
            let environment = reg(cx, frame, insn.b);
            let value = match environment.as_object() {
                Some(environment) if property::has(cx, environment, key)? => {
                    property::get(cx, Value::object(environment), key)?
                }
                _ if insn.c != 0 => Value::UNDEFINED,
                _ => {
                    return Err(binding_error(
                        cx,
                        ErrorKind::Reference,
                        key,
                        " is not defined",
                    ));
                }
            };
            set_reg(cx, frame, insn.a, value);
        }
        Opcode::PutRef => {
            let key = insn.key();
            let (environment, value) = (reg(cx, frame, insn.a), reg(cx, frame, insn.b));
            let strict = unit.has(flags::STRICT);
            match environment.as_object() {
                Some(environment) => {
                    property::set(cx, Value::object(environment), key, value, strict)?
                }
                None if strict => {
                    return Err(binding_error(
                        cx,
                        ErrorKind::Reference,
                        key,
                        " is not defined",
                    ));
                }
                None => {
                    let global = cx.realm.global();
                    property::set_on(cx, global, key, value, false)?;
                }
            }
        }
        _ => unreachable!("{} is not a class form", insn.op.name()),
    }
    Ok(())
}

#[inline]
fn function(value: Value) -> ObjectId {
    value.as_object().expect("a function")
}

#[inline]
fn cell(value: Value) -> ScopeId {
    ScopeId(value.as_object().expect("a cell").index())
}

fn private_key(name: Value) -> PropertyKey {
    PropertyKey::symbol(name.as_symbol().expect("a private name"))
}

/// A private name read before its class created it (a computed key that
/// names a later field) is still `undefined`.
fn private_name(cx: &mut Cx<'_>, name: Value) -> Result<PropertyKey, Throw> {
    match name.as_symbol() {
        Some(symbol) => Ok(PropertyKey::symbol(symbol)),
        None => Err(error::type_error(cx, "private class field is not defined")),
    }
}

fn private_target(cx: &mut Cx<'_>, target: Value) -> Result<ObjectId, Throw> {
    match target.as_object() {
        Some(target) => Ok(target),
        None => Err(error::type_error(cx, "not an object")),
    }
}

#[cold]
fn missing_private(cx: &mut Cx<'_>, key: PropertyKey) -> Throw {
    let message = format!(
        "private class field '{}' does not exist",
        property::atom_text(cx.heap, key)
    );
    error::type_error(cx, &message)
}

#[cold]
fn this_bound(cx: &mut Cx<'_>) -> Throw {
    error::throw(
        cx,
        ErrorKind::Reference,
        "'this' can be initialized only once",
    )
}

#[cold]
fn super_base_error(cx: &mut Cx<'_>, base: Value, key: PropertyKey) -> Throw {
    let message = format!(
        "cannot read property '{}' of {}",
        property::atom_text(cx.heap, key),
        if base.is_null() { "null" } else { "undefined" }
    );
    error::type_error(cx, &message)
}

#[cold]
fn binding_error(cx: &mut Cx<'_>, kind: ErrorKind, key: PropertyKey, after: &str) -> Throw {
    // QuickJS quotes the name except in `{name} is not initialized`.
    let message = if after == " is not initialized" {
        error::atom_message(cx.heap, "", key, after)
    } else {
        error::atom_message(cx.heap, "'", key, &format!("'{after}"))
    };
    error::throw_units(cx, kind, &message)
}

/// `DefineClass`: the prototype object of the class made from `closure`,
/// whose parent is `parent` when the class has a heritage.
fn make_class(
    cx: &mut Cx<'_>,
    parent: Value,
    closure: Value,
    heritage: bool,
) -> Result<ObjectId, Throw> {
    let constructor = function(closure);
    let (prototype_parent, constructor_parent) = if heritage {
        if parent.is_null() {
            (None, cx.realm.intrinsic(Intrinsic::FunctionPrototype))
        } else if !object::is_constructor(cx, parent) {
            return Err(error::type_error(cx, "parent class must be constructor"));
        } else {
            let parent_object = parent.as_object().unwrap();
            let names = cx.realm.names;
            let prototype = object::get_property(cx, parent_object, Names::key(names.prototype))?;
            if prototype.as_object().is_none() && !prototype.is_null() {
                return Err(error::type_error(
                    cx,
                    "parent prototype must be an object or null",
                ));
            }
            (prototype.as_object(), parent_object)
        }
    } else {
        (
            Some(cx.realm.intrinsic(Intrinsic::ObjectPrototype)),
            cx.realm.intrinsic(Intrinsic::FunctionPrototype),
        )
    };
    object::materialize(cx, constructor)?;
    let prototype = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, prototype_parent, 4)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(prototype));
    let names = cx.realm.names;
    let heap = &mut *cx.heap;
    heap.set_prototype(constructor, Some(constructor_parent))
        .and_then(|()| {
            heap.add_property(
                constructor,
                Names::key(names.prototype),
                PropertyFlags::data(false, false, false),
                Value::object(prototype),
            )
        })
        .and_then(|()| {
            heap.add_property(
                prototype,
                Names::key(names.constructor),
                PropertyFlags::data(true, false, true),
                closure,
            )
        })
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.function_record_mut(constructor).home_object = prototype.index();
    Ok(prototype)
}

/// Runs a field or static initializer on `receiver`, branding it first when
/// the initializer owns its class's private methods.
fn run_initializer(cx: &mut Cx<'_>, initializer: ObjectId, receiver: Value) -> Result<(), Throw> {
    let record = cx.heap.function_record(initializer);
    if record.bytecode != NONE {
        let unit = cx.unit(
            cx.heap
                .bytecode_unit(crate::value::BytecodeId(record.bytecode)),
        );
        if unit.has(flags::BRAND)
            && record.home_object != NONE
            && let Some(target) = receiver.as_object()
        {
            add_brand(cx, ObjectId(record.home_object), target)?;
        }
    }
    super::call(cx, Value::object(initializer), receiver, &[])?;
    Ok(())
}

/// The brand symbol of `home`, created on first use.
fn ensure_brand(cx: &mut Cx<'_>, home: ObjectId) -> Result<Value, Throw> {
    let key = PropertyKey::symbol(cx.realm.brand);
    if let Some((slot, _)) = cx.heap.lookup_own(home, key) {
        return Ok(cx.heap.get_slot(home, slot));
    }
    object::materialize(cx, home)?;
    let brand = cx
        .heap
        .alloc_symbol(None, true)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::symbol(brand));
    cx.heap
        .add_property(
            home,
            key,
            PropertyFlags::data(false, false, false),
            Value::symbol(brand),
        )
        .map_err(|error| error::heap_error(cx, error))?;
    Ok(Value::symbol(brand))
}

fn add_brand(cx: &mut Cx<'_>, home: ObjectId, target: ObjectId) -> Result<(), Throw> {
    let brand = ensure_brand(cx, home)?;
    let key = private_key(brand);
    object::materialize(cx, target)?;
    if cx.heap.lookup_own(target, key).is_some() {
        return Err(error::type_error(cx, "private method is already present"));
    }
    cx.heap
        .add_property(
            target,
            key,
            PropertyFlags::data(false, false, false),
            Value::UNDEFINED,
        )
        .map_err(|error| error::heap_error(cx, error))
}

/// QuickJS's `JS_CheckBrand`: whether `target` carries the brand of the
/// class whose method is `method`, after its checks in their order.
#[cold]
#[inline(never)]
fn brand_check(cx: &mut Cx<'_>, target: Value, method: Value) -> Result<bool, Throw> {
    let home = method
        .as_object()
        .filter(|&method| cx.heap.object_kind(method) == crate::ObjectKind::Function)
        .map(|method| cx.heap.function_record(method).home_object)
        .filter(|&home| home != NONE);
    let Some(home) = home else {
        return Err(error::type_error(cx, "not an object"));
    };
    let key = PropertyKey::symbol(cx.realm.brand);
    let Some((slot, _)) = cx.heap.lookup_own(ObjectId(home), key) else {
        return Err(error::type_error(cx, "expecting <brand> private field"));
    };
    let brand = cx.heap.get_slot(ObjectId(home), slot);
    let target = private_target(cx, target)?;
    Ok(cx.heap.lookup_own(target, private_key(brand)).is_some())
}

fn check_brand(cx: &mut Cx<'_>, target: Value, method: Value) -> Result<(), Throw> {
    if !brand_check(cx, target, method)? {
        return Err(error::type_error(cx, "invalid brand on object"));
    }
    Ok(())
}
