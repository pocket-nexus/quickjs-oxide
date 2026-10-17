//! The main loop (section 4.2): one `match` on the typed opcode, registers
//! read as `registers[base + r]`, inline `Int32` and double fast paths, and
//! `#[inline(never)]` slow paths in [`ops`] and [`property`]. Calls, returns
//! and `new` on bytecode functions stay in the loop (section 4.3); a throw
//! unwinds through the handler tables (section 4.4).

use std::cmp::Ordering;

use super::arguments;
use super::error::{self, ErrorKind};
use super::ic::{self, FIELD_NO_SITE, GLOBAL_NO_SITE, IcEntry, IcKind};
use super::native::{self, CallSite};
use super::number;
use super::ops::{self, Arith, Relation, Unary};
use super::property;
use super::regexp;
use super::typed;
use super::{Cx, Frame, SPAN, Throw, frame_flags};
use crate::builtins::Native;
use crate::bytecode::opcode::global as global_flags;
use crate::bytecode::{Capture, CodeStore, CodeUnit, CodeUnitId, Insn, Opcode, flags};
use crate::heap::Heap;
use crate::heap::atom::PropertyKey;
use crate::heap::object::{NONE, ObjectKind};
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{BytecodeId, ObjectId, ScopeId, Value};

/// What a call's callee value is.
enum Callee {
    Bytecode(ObjectId, BytecodeId),
    Native(Native),
    NotCallable,
}

#[inline]
fn callee(heap: &Heap, value: Value) -> Callee {
    let Some(object) = value.as_object() else {
        return Callee::NotCallable;
    };
    if heap.object_kind(object) != ObjectKind::Function {
        return Callee::NotCallable;
    }
    let record = heap.function_record(object);
    if record.bytecode != NONE {
        Callee::Bytecode(object, BytecodeId(record.bytecode))
    } else {
        match Native::from_index(record.native) {
            Some(native) => Callee::Native(native),
            None => Callee::NotCallable,
        }
    }
}

/// Pushes a frame for a bytecode function whose window starts at `base`,
/// where `this` and `frame.argc` arguments already are. Clears the rest of
/// the window (missing parameters, locals, temporaries) and coerces a sloppy
/// function's `undefined` or `null` receiver to the global object. A
/// function that reads its actual arguments keeps the ones past its
/// parameters above its registers, where `NewArguments` and `Rest` find
/// them (section 5.1).
#[inline]
fn enter(cx: &mut Cx<'_>, unit: &CodeUnit, frame: Frame) -> Result<(), Throw> {
    enter_frame(cx, unit, frame)
}

/// [`enter`], inlined into the loop's call.
#[inline(always)]
fn enter_frame(cx: &mut Cx<'_>, unit: &CodeUnit, frame: Frame) -> Result<(), Throw> {
    let base = frame.base as usize;
    let argc = frame.argc as usize;
    let parameters = unit.parameter_count as usize;
    let registers_end = base + unit.register_count as usize;
    let end = frame_end(&frame, unit) as usize;
    if !cx.heap.vm.reserve(end) {
        return Err(error::stack_overflow(cx));
    }
    if end > registers_end {
        keep_extra_arguments(cx, base + 1 + parameters..base + 1 + argc, registers_end);
    }
    let registers = &mut cx.heap.vm.registers;
    let first = base + 1 + argc.min(parameters);
    registers[first..registers_end].fill(Value::UNDEFINED);
    if !unit.has(flags::STRICT) {
        let this = registers[base];
        if this.is_undefined() || this.is_null() {
            registers[base] = Value::object(cx.realm.global());
        } else if this.as_object().is_none() {
            box_receiver(cx, base, end)?;
        }
    }
    cx.heap.vm.frames.push(frame);
    cx.heap.vm.top = end as u32;
    Ok(())
}

/// A sloppy function's primitive receiver becomes its wrapper object; the
/// window is rooted for the allocation.
#[cold]
#[inline(never)]
fn box_receiver(cx: &mut Cx<'_>, base: usize, end: usize) -> Result<(), Throw> {
    let top = cx.heap.vm.top;
    cx.heap.vm.top = top.max(end as u32);
    let this = cx.heap.vm.registers[base];
    let result = super::object::to_object(cx, this);
    cx.heap.vm.top = top;
    cx.heap.vm.registers[base] = Value::object(result?);
    Ok(())
}

/// Sets `top` for a frame that resumes after a return or an unwind. The
/// registers between the finished frame's end and the resuming frame's are
/// dead (they lie above the call's base) but were not roots while the frame
/// ran, so a collection since may have freed what they name: the part at or
/// above `stale_from` is cleared before it becomes roots again.
#[inline]
fn raise_top(cx: &mut Cx<'_>, end: u32) {
    let vm = &mut cx.heap.vm;
    if end > vm.top && vm.stale_from < end {
        let from = vm.top.max(vm.stale_from);
        clear_registers(cx, from as usize..end as usize);
        cx.heap.vm.stale_from = end;
    }
    cx.heap.vm.top = end;
}

#[inline(never)]
fn clear_registers(cx: &mut Cx<'_>, range: std::ops::Range<usize>) {
    cx.heap.vm.registers[range].fill(Value::UNDEFINED);
}

/// Moves the arguments past a function's parameters above its registers.
#[cold]
#[inline(never)]
fn keep_extra_arguments(cx: &mut Cx<'_>, extras: std::ops::Range<usize>, to: usize) {
    cx.heap.vm.registers.copy_within(extras, to);
}

/// The end of a frame's registers: its window, plus the extra arguments a
/// function that reads them keeps above it.
#[inline]
pub(super) fn frame_end(frame: &Frame, unit: &CodeUnit) -> u32 {
    let mut end = frame.base + u32::from(unit.register_count);
    if unit.has(flags::ARGUMENTS) {
        end += frame.argc.saturating_sub(u32::from(unit.parameter_count));
    }
    end
}

/// The opcodes of the phase 3 surface that no hot path runs: accessor and
/// spill-scope accesses, `arguments`, rest parameters, `new.target`,
/// regular-expression literals, `for-in` setup, spread elements and
/// unsupported constructs. Kept out of the loop, which they would otherwise
/// grow past its size gate; each writes its own result register.
#[inline(never)]
fn rare_op(cx: &mut Cx<'_>, frame: &Frame, unit: &CodeUnit, insn: Insn) -> Result<(), Throw> {
    let base = frame.base as usize;
    let reg = |cx: &Cx<'_>, register: u8| cx.heap.vm.registers[base + register as usize];
    let (register, value) = match insn.op {
        Opcode::DefineGetter | Opcode::DefineSetter => {
            let object = reg(cx, insn.a).as_object().expect("a literal object");
            let function = reg(cx, insn.b);
            let setter = insn.op == Opcode::DefineSetter;
            let flags = PropertyFlags::accessor(true, true);
            return property::define_accessor(cx, object, insn.key(), function, setter, flags);
        }
        Opcode::NewSpill => (insn.a, new_spill(cx, insn.imm)?),
        Opcode::ClearCell => {
            let cell = cell(reg(cx, insn.a));
            cx.heap.scope_set(cell, 0, 0, Value::EMPTY);
            return Ok(());
        }
        Opcode::LoadSlot => {
            let spill = cell(reg(cx, insn.b));
            (insn.a, cx.heap.scope_get(spill, 0, insn.imm))
        }
        Opcode::StoreSlot => {
            let (value, spill) = (reg(cx, insn.a), cell(reg(cx, insn.b)));
            cx.heap.scope_set(spill, 0, insn.imm, value);
            return Ok(());
        }
        Opcode::NewArguments => {
            let mapped = insn.imm != 0;
            (insn.a, arguments::new_arguments(cx, frame, unit, mapped)?)
        }
        Opcode::Rest => (insn.a, arguments::rest(cx, frame, unit, insn.imm as usize)?),
        Opcode::LoadNewTarget => {
            let target = if frame.flags & frame_flags::CONSTRUCT != 0 {
                Value::object(frame.function)
            } else {
                Value::UNDEFINED
            };
            (insn.a, target)
        }
        Opcode::NewRegExp => (insn.a, regexp::new_regexp(cx, frame.bytecode, insn.imm)?),
        Opcode::ForInStart => {
            let value = reg(cx, insn.b);
            (insn.a, property::for_in_start(cx, value)?)
        }
        Opcode::AppendSpread => {
            let (array, index, source) = (reg(cx, insn.a), reg(cx, insn.b), reg(cx, insn.c));
            (insn.b, arguments::append_spread(cx, array, index, source)?)
        }
        Opcode::ThrowInternal => {
            let message = property::key_text(cx.heap, insn.key());
            return Err(error::throw(cx, ErrorKind::Internal, &message));
        }
        Opcode::CheckCtor => {
            if reg(cx, insn.a).is_undefined() {
                return Err(error::type_error(
                    cx,
                    "class constructors must be invoked with 'new'",
                ));
            }
            return Ok(());
        }
        Opcode::ToObject => {
            let value = reg(cx, insn.b);
            let object = super::object::to_object(cx, value)?;
            (insn.a, Value::object(object))
        }
        Opcode::SetProto => {
            let (object, prototype) = (reg(cx, insn.a), reg(cx, insn.b));
            let object = object.as_object().expect("an object");
            if prototype.as_object().is_some() || prototype.is_null() {
                super::object::ordinary_set_prototype_of(cx, object, prototype)?;
            }
            return Ok(());
        }
        Opcode::DefineMethod => {
            let (object, function) = (reg(cx, insn.a), reg(cx, insn.b));
            let object = object.as_object().expect("an object");
            return define_method(cx, object, insn.key(), function, u32::from(insn.c));
        }
        Opcode::DefineMethodComputed => {
            let (object, key, function) = (reg(cx, insn.a), reg(cx, insn.b), reg(cx, insn.c));
            let object = object.as_object().expect("an object");
            let key = ops::to_property_key(cx, key)?;
            return define_method(cx, object, key, function, insn.imm);
        }
        Opcode::SetFunctionName => {
            let (function, key) = (reg(cx, insn.a), reg(cx, insn.b));
            let Some(function) = function.as_object() else {
                return Ok(());
            };
            let key = ops::to_property_key(cx, key)?;
            return set_function_name(cx, function, key, "", false);
        }
        Opcode::ThrowType => {
            let message = property::key_text(cx.heap, insn.key());
            return Err(error::type_error(cx, &message));
        }
        Opcode::ThrowReference => {
            let message = property::key_text(cx.heap, insn.key());
            return Err(error::throw(cx, ErrorKind::Reference, &message));
        }
        Opcode::ImportCall => {
            let (specifier, options) = (reg(cx, insn.b), reg(cx, insn.c));
            (
                insn.a,
                super::module::import(cx, frame.function, specifier, options)?,
            )
        }
        Opcode::LoadHomeObject => {
            let home = cx.heap.function_record(frame.function).home_object;
            let home = if home == NONE {
                Value::UNDEFINED
            } else {
                Value::object(ObjectId(home))
            };
            (insn.a, home)
        }
        _ => unreachable!("{} is not a rare form", insn.op.name()),
    };
    cx.heap.vm.registers[base + register as usize] = value;
    Ok(())
}

/// `DefineMethod`: a method, getter or setter of a literal or a class,
/// with `object` as its home object and its name from the key.
#[inline(never)]
pub(super) fn define_method(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    function: Value,
    method_flags: u32,
) -> Result<(), Throw> {
    use crate::bytecode::opcode::method;
    let function_object = function.as_object().expect("a method closure");
    if cx.heap.object_kind(function_object) == ObjectKind::Function {
        cx.heap.function_record_mut(function_object).home_object = object.index();
    }
    let prefix = if method_flags & method::GETTER != 0 {
        "get "
    } else if method_flags & method::SETTER != 0 {
        "set "
    } else {
        ""
    };
    set_function_name(cx, function_object, key, prefix, true)?;
    let enumerable = method_flags & method::ENUMERABLE != 0;
    let descriptor = if method_flags & method::GETTER != 0 {
        super::object::Descriptor {
            get: Some(function),
            enumerable: Some(enumerable),
            configurable: Some(true),
            ..Default::default()
        }
    } else if method_flags & method::SETTER != 0 {
        super::object::Descriptor {
            set: Some(function),
            enumerable: Some(enumerable),
            configurable: Some(true),
            ..Default::default()
        }
    } else {
        super::object::Descriptor::data(function, true, enumerable, true)
    };
    super::object::define_property_or_throw(cx, object, key, &descriptor)
}

/// `SetFunctionName(function, key, prefix)`: the key's text (a symbol's
/// description in brackets) after `prefix`. Unless `always`, only a
/// function whose `name` is absent or empty is named.
#[inline(never)]
pub(crate) fn set_function_name(
    cx: &mut Cx<'_>,
    function: ObjectId,
    key: PropertyKey,
    prefix: &str,
    always: bool,
) -> Result<(), Throw> {
    let names = cx.realm.names;
    let name_key = Names::key(names.name);
    // The common case: an anonymous closure named by an atom. Its own
    // `name` slot holds the empty atom with the attributes the definition
    // gives, so the slot takes the atom as is.
    if prefix.is_empty()
        && let crate::heap::atom::KeyKind::String(id) = key.kind()
        && let Some((slot, flags)) = cx.heap.lookup_own(function, name_key)
        && flags == PropertyFlags::data(false, false, true)
        && cx.heap.get_slot(function, slot) == Value::string(names.empty)
    {
        cx.heap.set_slot(function, slot, Value::string(id));
        return Ok(());
    }
    if !always {
        match super::object::ordinary_get_own(cx, function, name_key) {
            None => {}
            Some(descriptor)
                if descriptor
                    .value
                    .and_then(Value::as_string)
                    .is_some_and(|name| cx.heap.string_len(name) == 0) => {}
            Some(_) => return Ok(()),
        }
    }
    let mut text: Vec<u16> = prefix.encode_utf16().collect();
    match key.kind() {
        crate::heap::atom::KeyKind::Index(index) => text.extend(index.to_string().encode_utf16()),
        crate::heap::atom::KeyKind::String(id) => text.extend(cx.heap.string(id).repr().units()),
        crate::heap::atom::KeyKind::Symbol(symbol) => {
            if let Some(description) = cx.heap.symbol_description(symbol) {
                text.push(u16::from(b'['));
                text.extend(cx.heap.string(description).repr().units());
                text.push(u16::from(b']'));
            }
        }
    }
    let name = crate::builtins::string_units(cx, &text)?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(name);
    let descriptor = super::object::Descriptor::data(name, false, false, true);
    let result = super::object::define_property_or_throw(cx, function, name_key, &descriptor);
    cx.heap.roots().truncate(mark);
    result
}

/// Rewrites a call of `Function.prototype.call` or `apply` (section 5.2):
/// the receiver `function` becomes the callee, the window moves onto the
/// `thisArg`, and `apply`'s list replaces it from the next register. Repeats
/// while the new callee is one of the two. Returns the callee, the window,
/// `this` and the argument count.
#[inline(never)]
fn rewrite_call(
    cx: &mut Cx<'_>,
    base: usize,
    mut window: usize,
    mut function: Value,
    mut this: Value,
    mut argc: usize,
) -> Result<(Callee, Value, usize, Value, usize), Throw> {
    loop {
        let target = callee(cx.heap, function);
        let Callee::Native(native @ (Native::FunctionCall | Native::FunctionApply)) = target else {
            return Ok((target, function, window, this, argc));
        };
        function = this;
        window += 1;
        let at = base + window;
        argc = if native == Native::FunctionCall {
            arguments::call_window(cx, at, argc)?
        } else {
            arguments::apply_window(cx, at, argc)?
        };
        this = cx.heap.vm.registers[at];
    }
}

/// Calls a native with its `this` in register `at` and its arguments after
/// it. The arguments may reach past the caller's registers (an `apply`), so
/// they are made roots for the call; the caller restores `top`.
#[inline]
#[allow(clippy::too_many_arguments)]
fn call_native(
    cx: &mut Cx<'_>,
    native: Native,
    at: usize,
    this: Value,
    argc: usize,
    construct: bool,
    function: Value,
    new_target: Value,
) -> Result<Value, Throw> {
    // A construction's window holds new.target in its first register;
    // a native constructor reads it from the site instead.
    cx.heap.vm.registers[at] = this;
    let end = (at + 1 + argc) as u32;
    cx.heap.vm.top = cx.heap.vm.top.max(end);
    native::call(
        cx,
        native,
        CallSite {
            this: at,
            args: at + 1,
            argc,
            construct,
            function,
            new_target,
        },
    )
}

/// A construction's receiver and `new.target` for a bytecode callee
/// (section 4.5): a derived constructor starts without one. `site` is the
/// `New` instruction's cache, used only when `new.target` is the callee.
#[inline(never)]
fn construct_this(
    cx: &mut Cx<'_>,
    object: ObjectId,
    function: Value,
    unit: &CodeUnit,
    at: usize,
    site: Option<usize>,
) -> Result<(Value, Value), Throw> {
    if !unit.has(flags::CONSTRUCTOR) {
        return Err(error::type_error(cx, "not a constructor"));
    }
    let new_target = cx.heap.vm.registers[at];
    if unit.has(flags::DERIVED) {
        return Ok((Value::UNDEFINED, new_target));
    }
    let site = site.filter(|_| new_target == function);
    let receiver = construct_receiver(cx, object, new_target, unit.this_slots, site)?;
    Ok((receiver, new_target))
}

/// `ToPropertyKeyOf`'s conversion of an object key: a `null` or
/// `undefined` base throws first.
#[inline(never)]
fn to_property_key_of(cx: &mut Cx<'_>, key: Value, base: Value) -> Result<PropertyKey, Throw> {
    if base.is_null() || base.is_undefined() {
        let base = if base.is_null() { "null" } else { "undefined" };
        return Err(error::type_error(
            cx,
            &format!("cannot read property of {base}"),
        ));
    }
    ops::to_property_key(cx, key)
}

/// A call or construction from the loop of a callee that is not bytecode:
/// a native, an exotic callable, or neither.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn call_other(
    cx: &mut Cx<'_>,
    target: Callee,
    function: Value,
    at: usize,
    this: Value,
    argc: usize,
    construct: bool,
    pc: usize,
) -> Result<Value, Throw> {
    let new_target = if construct {
        cx.heap.vm.registers[at]
    } else {
        Value::UNDEFINED
    };
    match target {
        Callee::Native(native) if !construct || native.is_constructor() => {
            let site = (cx.heap.vm.frames.len() as u32, pc as u32);
            let saved = std::mem::replace(&mut cx.heap.vm.native_site, site);
            let result = call_native(cx, native, at, this, argc, construct, function, new_target);
            cx.heap.vm.native_site = saved;
            result
        }
        Callee::Native(_) => Err(error::type_error(cx, "not a constructor")),
        _ => {
            let callable = function.as_object().is_some_and(|object| {
                let flag = if construct {
                    crate::heap::object::flags::CONSTRUCTOR
                } else {
                    crate::heap::object::flags::CALLABLE
                };
                cx.heap.header(object).has(flag)
            });
            if !callable {
                return Err(error::type_error(
                    cx,
                    if construct {
                        "not a constructor"
                    } else {
                        "not a function"
                    },
                ));
            }
            call_exotic(cx, function, at, this, argc, construct, new_target)
        }
    }
}

/// A call or construction of a callable that is neither bytecode nor a
/// native (a bound function or a proxy), from the loop: the arguments are
/// copied out of the window and the callee runs through the re-entrant
/// entry points.
#[cold]
#[inline(never)]
fn call_exotic(
    cx: &mut Cx<'_>,
    function: Value,
    at: usize,
    this: Value,
    argc: usize,
    construct: bool,
    new_target: Value,
) -> Result<Value, Throw> {
    let end = (at + 1 + argc) as u32;
    cx.heap.vm.top = cx.heap.vm.top.max(end);
    let args: Vec<Value> = cx.heap.vm.registers[at + 1..at + 1 + argc].to_vec();
    if construct {
        super::construct(cx, function, &args, new_target)
    } else {
        super::call(cx, function, this, &args)
    }
}

/// Writes `new.target` into the register a unit that reads it reserves:
/// its first local, right after the parameters (section 4.5).
#[inline]
fn write_new_target(cx: &mut Cx<'_>, base: usize, unit: &CodeUnit, new_target: Value) {
    if unit.has(flags::NEW_TARGET) {
        cx.heap.vm.registers[base + 1 + unit.parameter_count as usize] = new_target;
    }
}

/// Runs `function` with `this` in `registers[base]` and `argc` arguments
/// after it; [`super::call`] put them there.
pub(super) fn call_from_rust(
    cx: &mut Cx<'_>,
    function: Value,
    base: usize,
    argc: usize,
) -> Result<Value, Throw> {
    match callee(cx.heap, function) {
        Callee::Bytecode(object, bytecode) => {
            let store: &CodeStore = cx.code;
            let unit_id = cx.heap.bytecode_unit(bytecode);
            let unit = store.get(unit_id);
            let frame = Frame {
                unit: unit_id,
                function: object,
                bytecode,
                base: base as u32,
                return_pc: 0,
                return_dst: 0,
                argc: argc as u32,
                flags: frame_flags::ENTRY,
            };
            if !cx.heap.vm.enter_native() {
                return Err(error::stack_overflow(cx));
            }
            let result = enter(cx, unit, frame).and_then(|()| run(cx));
            cx.heap.vm.leave_native();
            result
        }
        Callee::Native(native) => native::call(
            cx,
            native,
            CallSite {
                this: base,
                args: base + 1,
                argc,
                construct: false,
                function,
                new_target: Value::UNDEFINED,
            },
        ),
        Callee::NotCallable => call_exotic_from_rust(cx, function, base, argc),
    }
}

/// [`call_from_rust`] for a bound function or a callable proxy.
#[inline(never)]
fn call_exotic_from_rust(
    cx: &mut Cx<'_>,
    function: Value,
    base: usize,
    argc: usize,
) -> Result<Value, Throw> {
    let Some(object) = function.as_object() else {
        return Err(not_a_function(cx, function));
    };
    let this = cx.heap.vm.registers[base];
    match cx.heap.object_kind(object) {
        ObjectKind::BoundFunction => {
            if !cx.heap.vm.enter_native() {
                return Err(error::stack_overflow(cx));
            }
            let internal = cx.heap.internal(object);
            let target = internal.slots[0];
            let bound_this = internal.slots[1];
            let mut args = internal.slots[2..].to_vec();
            args.extend_from_slice(&cx.heap.vm.registers[base + 1..base + 1 + argc]);
            let result = super::call(cx, target, bound_this, &args);
            cx.heap.vm.leave_native();
            result
        }
        ObjectKind::Proxy
            if cx
                .heap
                .header(object)
                .has(crate::heap::object::flags::CALLABLE) =>
        {
            let args = cx.heap.vm.registers[base + 1..base + 1 + argc].to_vec();
            super::proxy::call(cx, object, this, &args)
        }
        // Test262's `IsHTMLDDA` returns `null`.
        _ if super::ops::is_html_dda(cx.heap, function) => Ok(Value::NULL),
        _ => Err(not_a_function(cx, function)),
    }
}

#[cold]
#[inline(never)]
fn not_a_function(cx: &mut Cx<'_>, _function: Value) -> Throw {
    error::type_error(cx, "not a function")
}

/// Constructs `function` with `argc` arguments from `registers[base + 1]`
/// (register `base` is free for the receiver) and `new_target`;
/// [`super::construct`] put them there.
pub(super) fn construct_from_rust(
    cx: &mut Cx<'_>,
    function: Value,
    base: usize,
    argc: usize,
    new_target: Value,
) -> Result<Value, Throw> {
    match callee(cx.heap, function) {
        Callee::Bytecode(object, bytecode) => {
            let store: &CodeStore = cx.code;
            let unit_id = cx.heap.bytecode_unit(bytecode);
            let unit = store.get(unit_id);
            if !unit.has(flags::CONSTRUCTOR) {
                return Err(error::type_error(cx, "not a constructor"));
            }
            let receiver = if unit.has(flags::DERIVED) {
                Value::UNDEFINED
            } else {
                let prototype = super::object::prototype_from_constructor(
                    cx,
                    new_target,
                    Intrinsic::ObjectPrototype,
                )?;
                alloc_receiver(cx, Value::object(prototype), u32::from(unit.this_slots))?
            };
            cx.heap.vm.registers[base] = receiver;
            let frame = Frame {
                unit: unit_id,
                function: object,
                bytecode,
                base: base as u32,
                return_pc: 0,
                return_dst: 0,
                argc: argc as u32,
                flags: frame_flags::ENTRY | frame_flags::CONSTRUCT,
            };
            if !cx.heap.vm.enter_native() {
                return Err(error::stack_overflow(cx));
            }
            let result = enter(cx, unit, frame).and_then(|()| {
                write_new_target(cx, base, unit, new_target);
                run(cx)
            });
            cx.heap.vm.leave_native();
            result
        }
        Callee::Native(native) if native.is_constructor() => native::call(
            cx,
            native,
            CallSite {
                this: base,
                args: base + 1,
                argc,
                construct: true,
                function,
                new_target,
            },
        ),
        Callee::Native(_) => Err(error::type_error(cx, "not a constructor")),
        Callee::NotCallable => construct_exotic(cx, function, base, argc, new_target),
    }
}

/// [`construct_from_rust`] for a bound function or a proxy.
#[inline(never)]
fn construct_exotic(
    cx: &mut Cx<'_>,
    function: Value,
    base: usize,
    argc: usize,
    new_target: Value,
) -> Result<Value, Throw> {
    let Some(object) = function.as_object().filter(|&object| {
        cx.heap
            .header(object)
            .has(crate::heap::object::flags::CONSTRUCTOR)
    }) else {
        return Err(error::type_error(cx, "not a constructor"));
    };
    let args = cx.heap.vm.registers[base + 1..base + 1 + argc].to_vec();
    match cx.heap.object_kind(object) {
        ObjectKind::BoundFunction => {
            if !cx.heap.vm.enter_native() {
                return Err(error::stack_overflow(cx));
            }
            let internal = cx.heap.internal(object);
            let target = internal.slots[0];
            let mut all = internal.slots[2..].to_vec();
            all.extend_from_slice(&args);
            let new_target = if new_target == function {
                target
            } else {
                new_target
            };
            let result = super::construct(cx, target, &all, new_target);
            cx.heap.vm.leave_native();
            result
        }
        ObjectKind::Proxy => super::proxy::construct(cx, object, &args, new_target),
        _ => Err(error::type_error(cx, "not a constructor")),
    }
}

/// A `kind` error whose message names the key in an instruction's `imm`.
#[cold]
#[inline(never)]
fn key_error(cx: &mut Cx<'_>, kind: ErrorKind, insn: Insn, before: &str, after: &str) -> Throw {
    let message = format!("{before}{}{after}", property::key_text(cx.heap, insn.key()));
    error::throw(cx, kind, &message)
}

/// A new cell (a one-slot scope object) holding `value`, which must be
/// rooted by the caller.
#[inline(never)]
fn new_cell(cx: &mut Cx<'_>, value: Value) -> Result<Value, Throw> {
    let cell = cx
        .heap
        .alloc_scope(None, 1)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.scope_set(cell, 0, 0, value);
    Ok(Value::object(cell.object()))
}

/// `NewSpill`: a scope whose slots hold the locals past the window, all
/// `undefined` as a frame's locals start.
#[inline(never)]
fn new_spill(cx: &mut Cx<'_>, slots: u32) -> Result<Value, Throw> {
    let spill = cx
        .heap
        .alloc_scope(None, slots)
        .map_err(|error| error::heap_error(cx, error))?;
    for slot in 0..slots {
        cx.heap.scope_set(spill, 0, slot, Value::UNDEFINED);
    }
    Ok(Value::object(spill.object()))
}

#[inline]
fn cell(value: Value) -> ScopeId {
    ScopeId(value.as_object().expect("a cell").index())
}

/// The cell of capture `index` of the running function.
#[inline(always)]
fn captured_cell(heap: &Heap, function: ObjectId, index: u32) -> ScopeId {
    let captures = heap
        .function_scope(function)
        .expect("a closure with captures");
    cell(heap.scope_get(captures, 0, index))
}

/// `MakeClosure`: a function over child `index` of the running unit, with the
/// cells its capture list names, and a `prototype` object for a constructor.
#[inline(never)]
fn make_closure(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    index: u32,
) -> Result<Value, Throw> {
    let store: &CodeStore = cx.code;
    let child = cx.heap.bytecode_child(frame.bytecode, index);
    let child_unit = store.get(unit.children[index as usize]);
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let scope = if child_unit.captures.is_empty() {
            None
        } else {
            let count = child_unit.captures.len() as u32;
            let scope = cx
                .heap
                .alloc_scope(None, count)
                .map_err(|error| error::heap_error(cx, error))?;
            for (slot, capture) in child_unit.captures.iter().enumerate() {
                let value = match *capture {
                    Capture::None => Value::UNDEFINED,
                    Capture::Register(register) => {
                        cx.heap.vm.registers[frame.base as usize + register as usize]
                    }
                    Capture::Slot(slot) => {
                        let spill = cx.heap.vm.registers
                            [frame.base as usize + unit.spill_register as usize];
                        cx.heap.scope_get(cell(spill), 0, u32::from(slot))
                    }
                    Capture::Outer(outer) => {
                        let captures = cx.heap.function_scope(frame.function).expect("captures");
                        cx.heap.scope_get(captures, 0, u32::from(outer))
                    }
                };
                cx.heap.scope_set(scope, 0, slot as u32, value);
            }
            cx.heap.roots().push(Value::object(scope.object()));
            Some(scope)
        };
        closure_function(cx, child, child_unit, scope).map(Value::object)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// The function object of a closure over `scope` for bytecode record
/// `child` of unit `child_unit`, with its `length`, `name` and `prototype`;
/// rooted.
fn closure_function(
    cx: &mut Cx<'_>,
    child: BytecodeId,
    child_unit: &CodeUnit,
    scope: Option<ScopeId>,
) -> Result<ObjectId, Throw> {
    let generator = child_unit.has(flags::GENERATOR);
    let asynchronous = child_unit.has(flags::ASYNC);
    let (prototype, cache) = match (generator, asynchronous) {
        (false, false) if child_unit.has(flags::PROTOTYPE) => (Intrinsic::FunctionPrototype, 1),
        (false, false) => (Intrinsic::FunctionPrototype, 0),
        (false, true) => (Intrinsic::AsyncFunctionPrototype, 2),
        (true, false) => (Intrinsic::GeneratorFunctionPrototype, 3),
        (true, true) => (Intrinsic::AsyncGeneratorFunctionPrototype, 4),
    };
    let function_prototype = cx.realm.intrinsic(prototype);
    let function = cx
        .heap
        .alloc_closure(Some(function_prototype), scope, child)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(function));
    if child_unit.has(flags::CONSTRUCTOR) {
        cx.heap
            .set_object_flag(function, crate::heap::object::flags::CONSTRUCTOR);
    }
    let names = cx.realm.names;
    let name = Value::string(child_unit.name_atom.unwrap_or(names.empty));
    let length = Value::int32(i32::from(child_unit.length));
    // The `prototype` object, created before the shape is applied so the
    // function stays in a consistent state at the safepoint.
    let instance = if child_unit.has(flags::PROTOTYPE) {
        let object_prototype = cx.realm.intrinsic(match (generator, asynchronous) {
            (true, false) => Intrinsic::GeneratorPrototype,
            (true, true) => Intrinsic::AsyncGeneratorPrototype,
            _ => Intrinsic::ObjectPrototype,
        });
        let heap = &mut *cx.heap;
        let instance = heap
            .alloc_object(ObjectKind::Ordinary, Some(object_prototype), 1)
            .and_then(|instance| {
                if !generator {
                    heap.add_property(
                        instance,
                        Names::key(names.constructor),
                        PropertyFlags::data(true, false, true),
                        Value::object(function),
                    )?;
                }
                Ok(instance)
            })
            .map_err(|error| error::heap_error(cx, error))?;
        // Rooted like the function: the shape's slow path allocates.
        cx.heap.roots().push(Value::object(instance));
        Some(Value::object(instance))
    } else {
        None
    };
    let values = [length, name, instance.unwrap_or(Value::UNDEFINED)];
    let count = if instance.is_some() { 3 } else { 2 };
    match cx.heap.closure_shapes[cache] {
        Some(shape) if cx.heap.prototype_of_shape(shape) == Some(function_prototype) => {
            cx.heap
                .apply_closure_shape(function, shape, &values[..count]);
        }
        _ => {
            let heap = &mut *cx.heap;
            let mut result = crate::builtins::define_name_length(
                heap,
                &names,
                function,
                name,
                child_unit.length.into(),
            );
            if let (Ok(()), Some(instance)) = (result, instance) {
                result = heap.add_property(
                    function,
                    Names::key(names.prototype),
                    PropertyFlags::data(true, false, false),
                    instance,
                );
            }
            result.map_err(|error| error::heap_error(cx, error))?;
            if !cx.heap.is_dictionary(function) {
                cx.heap.closure_shapes[cache] = Some(cx.heap.shape_of(function));
            }
        }
    }
    Ok(function)
}

/// A closure for child `index` of `root`'s unit whose captures are all of
/// `root`'s own (a module's hoisted function declarations, created while
/// linking).
pub(crate) fn closure_over_captures(
    cx: &mut Cx<'_>,
    root: ObjectId,
    index: u32,
) -> Result<ObjectId, Throw> {
    let store: &CodeStore = cx.code;
    let bytecode = cx
        .heap
        .function_bytecode(root)
        .expect("a bytecode function");
    let unit = store.get(cx.heap.bytecode_unit(bytecode));
    let child = cx.heap.bytecode_child(bytecode, index);
    let child_unit = store.get(unit.children[index as usize]);
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let scope = if child_unit.captures.is_empty() {
            None
        } else {
            let count = child_unit.captures.len() as u32;
            let scope = cx
                .heap
                .alloc_scope(None, count)
                .map_err(|error| error::heap_error(cx, error))?;
            let captures = cx.heap.function_scope(root);
            for (slot, capture) in child_unit.captures.iter().enumerate() {
                let value = match (*capture, captures) {
                    (Capture::Outer(outer), Some(captures)) => {
                        cx.heap.scope_get(captures, 0, u32::from(outer))
                    }
                    (Capture::None, _) => Value::UNDEFINED,
                    _ => {
                        return Err(error::throw(
                            cx,
                            ErrorKind::Internal,
                            "a hoisted function captures a local",
                        ));
                    }
                };
                cx.heap.scope_set(scope, 0, slot as u32, value);
            }
            cx.heap.roots().push(Value::object(scope.object()));
            Some(scope)
        };
        closure_function(cx, child, child_unit, scope)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `New`'s receiver (phase 3, section 2.5): an ordinary object inheriting
/// from the constructor's `prototype`, sized by the site's learned slot count.
/// A hit is a shape compare on the constructor and a load of its `prototype`
/// slot, read live; a miss looks it up and refills the entry with the
/// constructor's static `this.x =` count.
#[inline]
fn construct_receiver(
    cx: &mut Cx<'_>,
    constructor: ObjectId,
    new_target: Value,
    this_slots: u16,
    site: Option<usize>,
) -> Result<Value, Throw> {
    let Some(site) = site else {
        if new_target != Value::object(constructor) {
            let prototype = super::object::prototype_from_constructor(
                cx,
                new_target,
                Intrinsic::ObjectPrototype,
            )?;
            return alloc_receiver(cx, Value::object(prototype), u32::from(this_slots));
        }
        let names = cx.realm.names;
        let prototype = property::get_value(cx, constructor, Names::key(names.prototype))?;
        return alloc_receiver(cx, prototype, u32::from(this_slots));
    };
    let entry = cx.heap.ics.entries[site];
    let heap = &*cx.heap;
    let shape = heap.shape_of(constructor).index();
    let (prototype, slots) = if shape == entry.w0 {
        let prototype = heap.get_slot(constructor, entry.w1);
        cx.heap.ics.count_hit(IcKind::Construct);
        (prototype, entry.w2)
    } else {
        let names = cx.realm.names;
        let key = Names::key(names.prototype);
        let slots = u32::from(this_slots);
        match heap.lookup_own(constructor, key) {
            Some((slot, property))
                if !property.has(PropertyFlags::ACCESSOR) && !heap.is_dictionary(constructor) =>
            {
                let prototype = heap.get_slot(constructor, slot);
                let fill = IcEntry {
                    w0: shape,
                    w1: slot,
                    w2: slots,
                    w3: NONE,
                };
                let filled = cx.heap.ics.record(site, fill);
                cx.heap.ics.count_miss(site, filled);
                (prototype, slots)
            }
            _ => {
                cx.heap.ics.count_miss(site, false);
                (property::get_value(cx, constructor, key)?, slots)
            }
        }
    };
    alloc_receiver(cx, prototype, slots)
}

/// An ordinary object inheriting from `prototype` (`%Object.prototype%`
/// when it is not an object) with room for `slots` properties.
fn alloc_receiver(cx: &mut Cx<'_>, prototype: Value, slots: u32) -> Result<Value, Throw> {
    let prototype = prototype
        .as_object()
        .unwrap_or_else(|| cx.realm.intrinsic(Intrinsic::ObjectPrototype));
    cx.heap
        .alloc_object(
            ObjectKind::Ordinary,
            Some(prototype),
            slots.max(MIN_OPEN_SLOTS),
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

/// The slots a constructed receiver or an empty literal reserves at least:
/// such objects usually gain properties after creation (a tree node's
/// children), and a transition into a free slot is a cache hit where a full
/// range is a miss and a copy (phase 3 M5, on Splay).
const MIN_OPEN_SLOTS: u32 = 4;

/// The most receiver slots a construct entry learns: an object past the
/// dictionary threshold leaves the transition tree anyway.
const MAX_LEARNED_SLOTS: u32 = 2 * crate::shape::DICTIONARY_THRESHOLD;

/// Section 2.5's learning: when a constructor's frame returns to the `New`
/// that entered it, the site's entry keeps the larger of its slot count and
/// the slots the receiver grew to, so the next construction reserves what
/// chained constructors added.
#[inline(never)]
fn learn_receiver_size(cx: &mut Cx<'_>, done: &Frame) {
    if done.flags & frame_flags::ENTRY != 0 {
        return;
    }
    // A receiver that fits the slots every receiver gets teaches nothing:
    // checked first, so a constructor that stays small pays only this.
    let Some(receiver) = cx.heap.vm.registers[done.base as usize].as_object() else {
        return;
    };
    let width = cx.heap.shape(cx.heap.shape_of(receiver)).width;
    if width <= MIN_OPEN_SLOTS {
        return;
    }
    let caller = cx.heap.vm.frames.last().expect("a caller");
    let unit = cx.code.get(caller.unit);
    let insn = unit.code[done.return_pc as usize - 1];
    if insn.op != Opcode::New {
        return;
    }
    let site = unit.ic_base as usize + insn.imm as usize;
    let mut entry = cx.heap.ics.entries[site];
    if !entry.is_filled() || cx.heap.is_dictionary(receiver) {
        return;
    }
    if width > entry.w2 {
        entry.w2 = width.min(MAX_LEARNED_SLOTS);
        cx.heap.ics.entries[site] = entry;
    }
}

/// `GetGlobal` and `GetGlobalOrUndefined` past the cache (section 2.4): a
/// binding of the global lexical environment (section 5.7) first, then an
/// own data property of the global object; both fill the entry with the
/// global generation and the holder. A lexical binding in the TDZ throws
/// and is not cached; anything else (an accessor, a global found on
/// `%Object.prototype%`) is read uncached.
#[inline(never)]
fn get_global(cx: &mut Cx<'_>, insn: Insn, site: usize) -> Result<Value, Throw> {
    let key = insn.key();
    let global = cx.realm.global();
    let lexical = cx.realm.intrinsic(Intrinsic::GlobalLexical);
    let heap = &*cx.heap;
    for holder in [lexical, global] {
        if let Some((slot, property)) = heap.lookup_own(holder, key)
            && !property.has(PropertyFlags::ACCESSOR)
        {
            let value = heap.get_slot(holder, slot);
            if value.is_empty() {
                cx.heap.ics.count_miss(site, false);
                return Err(key_error(
                    cx,
                    ErrorKind::Reference,
                    insn,
                    "",
                    " is not initialized",
                ));
            }
            let entry = global_entry(heap, holder, slot, property);
            let filled = insn.site() != Some(GLOBAL_NO_SITE) && cx.heap.ics.record(site, entry);
            cx.heap.ics.count_miss(site, filled);
            return Ok(value);
        }
    }
    cx.heap.ics.count_miss(site, false);
    match property::lookup(cx.heap, &cx.realm.names, global, key) {
        property::Lookup::Hit(found) => property::value_of(cx, Some(found), Value::object(global)),
        property::Lookup::Exotic(holder) => {
            if super::object::has_property(cx, holder, key)? {
                super::object::get(cx, holder, key, Value::object(global))
            } else if insn.op == Opcode::GetGlobalOrUndefined {
                Ok(Value::UNDEFINED)
            } else {
                Err(key_error(
                    cx,
                    ErrorKind::Reference,
                    insn,
                    "'",
                    "' is not defined",
                ))
            }
        }
        property::Lookup::Miss if insn.op == Opcode::GetGlobalOrUndefined => Ok(Value::UNDEFINED),
        property::Lookup::Miss => Err(key_error(
            cx,
            ErrorKind::Reference,
            insn,
            "'",
            "' is not defined",
        )),
    }
}

/// `SetGlobal` past the cache: a lexical binding first (its TDZ and
/// `const` checked), else the uncached write to the global object; then a
/// fill when the binding is writable data.
#[inline(never)]
fn set_global(
    cx: &mut Cx<'_>,
    insn: Insn,
    value: Value,
    strict: bool,
    site: usize,
) -> Result<(), Throw> {
    let key = insn.key();
    let global = cx.realm.global();
    let lexical = cx.realm.intrinsic(Intrinsic::GlobalLexical);
    if let Some((slot, property)) = cx.heap.lookup_own(lexical, key) {
        if cx.heap.get_slot(lexical, slot).is_empty() {
            cx.heap.ics.count_miss(site, false);
            return Err(key_error(
                cx,
                ErrorKind::Reference,
                insn,
                "",
                " is not initialized",
            ));
        }
        if !property.has(PropertyFlags::WRITABLE) {
            cx.heap.ics.count_miss(site, false);
            return Err(key_error(cx, ErrorKind::Type, insn, "'", "' is read-only"));
        }
        cx.heap.set_slot(lexical, slot, value);
        let entry = global_entry(cx.heap, lexical, slot, property);
        let filled = insn.site() != Some(GLOBAL_NO_SITE) && cx.heap.ics.record(site, entry);
        cx.heap.ics.count_miss(site, filled);
        return Ok(());
    }
    if strict && !property::has(cx, global, key)? {
        return Err(key_error(
            cx,
            ErrorKind::Reference,
            insn,
            "'",
            "' is not defined",
        ));
    }
    property::set_on(cx, global, key, value, strict)?;
    let heap = &mut *cx.heap;
    let filled = match heap.lookup_own(global, key) {
        Some((slot, property))
            if insn.site() != Some(GLOBAL_NO_SITE)
                && property.has(PropertyFlags::WRITABLE)
                && !property.has(PropertyFlags::ACCESSOR) =>
        {
            let entry = global_entry(heap, global, slot, property);
            heap.ics.record(site, entry)
        }
        _ => false,
    };
    heap.ics.count_miss(site, filled);
    Ok(())
}

/// A global entry: the generation, the slot, the flags and the holder (the
/// global object or the lexical environment).
fn global_entry(heap: &Heap, holder: ObjectId, slot: u32, property: PropertyFlags) -> IcEntry {
    IcEntry {
        w0: heap.global_generation,
        w1: slot,
        w2: u32::from(property.bits()),
        w3: holder.index(),
    }
}

/// Initializes a script-level binding (`PutVarInit`): a lexical binding
/// leaving its TDZ, or a `var` or function on the global object.
#[inline(never)]
fn define_global(
    cx: &mut Cx<'_>,
    key: PropertyKey,
    value: Value,
    overwrite: bool,
    configurable: bool,
) -> Result<(), Throw> {
    let lexical = cx.realm.intrinsic(Intrinsic::GlobalLexical);
    if let Some((slot, _)) = cx.heap.lookup_own(lexical, key) {
        cx.heap.set_slot(lexical, slot, value);
        return Ok(());
    }
    let global = cx.realm.global();
    let heap = &mut *cx.heap;
    if let Some((slot, property)) = heap.lookup_own(global, key) {
        if overwrite {
            if property.has(PropertyFlags::ACCESSOR) || property.has(PropertyFlags::CONFIGURABLE) {
                // `CreateGlobalFunctionBinding` over a configurable property.
                let descriptor = super::object::Descriptor::data(value, true, true, configurable);
                super::object::define_property_or_throw(cx, global, key, &descriptor)?;
            } else {
                heap.set_slot(global, slot, value);
            }
        }
        return Ok(());
    }
    if !heap
        .header(global)
        .has(crate::heap::object::flags::EXTENSIBLE)
    {
        let message = format!("cannot define variable '{}'", property::key_text(heap, key));
        return Err(error::type_error(cx, &message));
    }
    heap.add_property(
        global,
        key,
        PropertyFlags::data(true, true, configurable),
        value,
    )
    .map_err(|error| error::heap_error(cx, error))
}

/// `delete name` of a global: a lexical binding is not deletable.
#[inline(never)]
fn delete_global(cx: &mut Cx<'_>, key: PropertyKey) -> Result<Value, Throw> {
    let lexical = cx.realm.intrinsic(Intrinsic::GlobalLexical);
    if cx.heap.lookup_own(lexical, key).is_some() {
        return Ok(Value::FALSE);
    }
    let global = Value::object(cx.realm.global());
    property::delete(cx, global, key, false).map(Value::bool)
}

/// The declaration forms and checks of a script's prologue (section 5.7).
#[inline(never)]
fn declare_global(cx: &mut Cx<'_>, insn: Insn) -> Result<(), Throw> {
    let key = insn.key();
    let lexical = cx.realm.intrinsic(Intrinsic::GlobalLexical);
    let global = cx.realm.global();
    match insn.op {
        Opcode::DeclareGlobal => {
            let configurable = insn.b & global_flags::EVAL != 0;
            define_global(cx, key, Value::UNDEFINED, false, configurable)
        }
        Opcode::DeclareGlobalLet | Opcode::DeclareGlobalConst => {
            let writable = insn.op == Opcode::DeclareGlobalLet;
            cx.heap
                .add_property(
                    lexical,
                    key,
                    PropertyFlags::data(writable, true, false),
                    Value::EMPTY,
                )
                .map_err(|error| error::heap_error(cx, error))
        }
        Opcode::CheckGlobalVar => {
            if cx.heap.lookup_own(lexical, key).is_some() {
                return Err(key_error(
                    cx,
                    ErrorKind::Syntax,
                    insn,
                    "redeclaration of '",
                    "'",
                ));
            }
            let declarable = match cx.heap.lookup_own(global, key) {
                None => cx
                    .heap
                    .header(global)
                    .has(crate::heap::object::flags::EXTENSIBLE),
                Some(_) if insn.b & global_flags::FUNCTION == 0 => true,
                Some((_, property)) => {
                    property.has(PropertyFlags::CONFIGURABLE)
                        || (!property.has(PropertyFlags::ACCESSOR)
                            && property.has(PropertyFlags::WRITABLE)
                            && property.has(PropertyFlags::ENUMERABLE))
                }
            };
            if !declarable {
                return Err(key_error(
                    cx,
                    ErrorKind::Type,
                    insn,
                    "cannot define variable '",
                    "'",
                ));
            }
            Ok(())
        }
        _ => {
            let restricted = cx.heap.lookup_own(lexical, key).is_some()
                || cx
                    .heap
                    .lookup_own(global, key)
                    .is_some_and(|(_, property)| !property.has(PropertyFlags::CONFIGURABLE));
            if restricted {
                return Err(key_error(
                    cx,
                    ErrorKind::Syntax,
                    insn,
                    "redeclaration of '",
                    "'",
                ));
            }
            Ok(())
        }
    }
}

/// The cache site of a global form, in `b | c << 8`.
#[inline(always)]
fn global_site(insn: Insn) -> usize {
    insn.b as usize | (insn.c as usize) << 8
}

/// The running frame's registers. The register file keeps [`SPAN`] values
/// reserved past `top`, so the window can be taken at any frame's base.
type Window = [Value; SPAN];

#[inline(always)]
fn window(registers: &mut [Value], base: usize) -> &mut Window {
    (&mut registers[base..base + SPAN])
        .try_into()
        .expect("a reserved window")
}

/// Runs the top frame from its start until the entry frame returns.
fn run(cx: &mut Cx<'_>) -> Result<Value, Throw> {
    run_at(cx, 0)
}

/// Starts a resumed entry frame (section 4.3) by throwing `thrown` at the
/// instruction before `pc`: its handler takes it, or the frame is popped
/// and the throw returned.
pub(super) fn run_throwing(cx: &mut Cx<'_>, pc: usize, thrown: Value) -> Result<Value, Throw> {
    let frame = *cx.heap.vm.frames.last().expect("an entry frame");
    let unit = cx.code.get(frame.unit);
    let at = pc.saturating_sub(1);
    if let Some(handler) = unit
        .handlers
        .iter()
        .find(|handler| handler.start as usize <= at && at < handler.end as usize)
    {
        cx.heap.vm.registers[frame.base as usize + handler.register as usize] = thrown;
        return run_at(cx, handler.target as usize);
    }
    cx.heap.vm.frames.pop();
    Err(Throw(thrown))
}

/// Runs the top frame from instruction `pc` until the entry frame returns.
///
/// `regs` is the running frame's window. It borrows the register file, so
/// every arm that calls into the heap copies its operands out first and
/// takes the window again afterwards (`set!`, `write!`, and the common tail).
pub(super) fn run_at(cx: &mut Cx<'_>, start: usize) -> Result<Value, Throw> {
    let store: &CodeStore = cx.code;
    let mut frame = *cx.heap.vm.frames.last().expect("an entry frame");
    let mut unit: &CodeUnit = store.get(frame.unit);
    let mut code: &[Insn] = &unit.code;
    let mut base = frame.base as usize;
    let mut strict = unit.has(flags::STRICT);
    let mut ic_base = unit.ic_base as usize;
    let mut pc = start;

    macro_rules! window {
        () => {
            window(&mut cx.heap.vm.registers, base)
        };
    }
    let mut regs: &mut Window = window!();
    macro_rules! reg {
        ($register:expr) => {
            regs[$register as usize]
        };
    }
    // Reloads the frame-derived locals from the top frame.
    macro_rules! load_frame {
        () => {{
            frame = *cx.heap.vm.frames.last().expect("a frame");
            unit = store.get(frame.unit);
            code = &unit.code;
            base = frame.base as usize;
            strict = unit.has(flags::STRICT);
            ic_base = unit.ic_base as usize;
        }};
    }
    // Writes a value computed from the heap, then continues.
    macro_rules! set {
        ($label:lifetime, $register:expr, $value:expr) => {{
            let value: Value = $value;
            regs = window!();
            regs[$register as usize] = value;
            continue $label;
        }};
    }
    // Writes a slow path's result and continues, or evaluates to its throw.
    macro_rules! write {
        ($label:lifetime, $register:expr, $result:expr) => {
            match $result {
                Ok(value) => set!($label, $register, value),
                Err(throw) => Err(throw),
            }
        };
    }

    'dispatch: loop {
        let insn = code[pc];
        pc += 1;
        let outcome: Result<(), Throw> = 'op: {
            match insn.op {
                Opcode::Nop => continue 'dispatch,

                // ---- moves and constants ----
                Opcode::Mov => {
                    reg!(insn.a) = reg!(insn.b);
                    continue 'dispatch;
                }
                Opcode::Swap => {
                    regs.swap(insn.a as usize, insn.b as usize);
                    continue 'dispatch;
                }
                Opcode::LoadWide => {
                    let value = cx.heap.vm.registers[base + insn.imm as usize];
                    set!('dispatch, insn.a, value)
                }
                Opcode::StoreWide => {
                    let value = reg!(insn.a);
                    cx.heap.vm.registers[base + insn.imm as usize] = value;
                    regs = window!();
                    continue 'dispatch;
                }
                Opcode::LoadInt => {
                    reg!(insn.a) = Value::int32(insn.int());
                    continue 'dispatch;
                }
                Opcode::LoadConst => {
                    set!('dispatch, insn.a, cx.heap.bytecode_constant(frame.bytecode, insn.imm))
                }
                Opcode::LoadUndefined => {
                    reg!(insn.a) = Value::UNDEFINED;
                    continue 'dispatch;
                }
                Opcode::LoadNull => {
                    reg!(insn.a) = Value::NULL;
                    continue 'dispatch;
                }
                Opcode::LoadTrue => {
                    reg!(insn.a) = Value::TRUE;
                    continue 'dispatch;
                }
                Opcode::LoadFalse => {
                    reg!(insn.a) = Value::FALSE;
                    continue 'dispatch;
                }
                Opcode::LoadEmpty => {
                    reg!(insn.a) = Value::EMPTY;
                    continue 'dispatch;
                }
                Opcode::LoadFunction => {
                    reg!(insn.a) = Value::object(frame.function);
                    continue 'dispatch;
                }
                Opcode::CheckInit => {
                    if !reg!(insn.a).is_empty() {
                        continue 'dispatch;
                    }
                    Err(key_error(
                        cx,
                        ErrorKind::Reference,
                        insn,
                        "",
                        " is not initialized",
                    ))
                }
                Opcode::ThrowConstAssign => {
                    Err(key_error(cx, ErrorKind::Type, insn, "'", "' is read-only"))
                }

                // ---- unary operators ----
                Opcode::Inc => {
                    let x = reg!(insn.b);
                    if let Some(i) = x.as_int32()
                        && let Some(value) = i.checked_add(1)
                    {
                        reg!(insn.a) = Value::int32(value);
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, ops::unary(cx, Unary::Inc, x))
                }
                Opcode::Dec => {
                    let x = reg!(insn.b);
                    if let Some(i) = x.as_int32()
                        && let Some(value) = i.checked_sub(1)
                    {
                        reg!(insn.a) = Value::int32(value);
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, ops::unary(cx, Unary::Dec, x))
                }
                Opcode::Neg | Opcode::Plus | Opcode::ToNumeric | Opcode::BitNot => {
                    let x = reg!(insn.b);
                    if let Some(value) = unary_fast(insn.op, x) {
                        reg!(insn.a) = value;
                        continue 'dispatch;
                    }
                    let op = match insn.op {
                        Opcode::Neg => Unary::Neg,
                        Opcode::Plus => Unary::Plus,
                        Opcode::ToNumeric => Unary::ToNumeric,
                        _ => Unary::BitNot,
                    };
                    write!('dispatch, insn.a, ops::unary(cx, op, x))
                }
                Opcode::Not => {
                    let x = reg!(insn.b);
                    if let Some(value) = x.as_bool() {
                        reg!(insn.a) = Value::bool(!value);
                        continue 'dispatch;
                    }
                    set!('dispatch, insn.a, Value::bool(!ops::to_boolean(cx.heap, x)))
                }
                Opcode::TypeOf => {
                    let x = reg!(insn.b);
                    set!('dispatch, insn.a, ops::type_of(cx, x))
                }
                // A primitive key converts without side effects: the base
                // check of `ToPropertyKeyOf` is left to the read.
                Opcode::ToPropertyKey | Opcode::ToPropertyKeyOf => {
                    let x = reg!(insn.b);
                    if x.is_int32() || x.as_string().is_some() || x.as_symbol().is_some() {
                        reg!(insn.a) = x;
                        continue 'dispatch;
                    }
                    let result = if insn.op == Opcode::ToPropertyKey {
                        ops::to_property_key(cx, x)
                    } else {
                        let base = reg!(insn.c);
                        to_property_key_of(cx, x, base)
                    };
                    write!('dispatch, insn.a, result.map(PropertyKey::to_value))
                }
                Opcode::IsUndefinedOrNull => {
                    let x = reg!(insn.b);
                    reg!(insn.a) = Value::bool(x.is_undefined() || x.is_null());
                    continue 'dispatch;
                }
                Opcode::IsUndefined | Opcode::TypeOfIsUndefined => {
                    reg!(insn.a) = Value::bool(reg!(insn.b).is_undefined());
                    continue 'dispatch;
                }
                Opcode::IsNull => {
                    reg!(insn.a) = Value::bool(reg!(insn.b).is_null());
                    continue 'dispatch;
                }
                Opcode::TypeOfIsFunction => {
                    let x = reg!(insn.b);
                    set!('dispatch, insn.a, Value::bool(ops::is_callable(cx.heap, x)))
                }

                // ---- arithmetic ----
                Opcode::Add | Opcode::AddI => {
                    let x = reg!(insn.b);
                    let y = if insn.op == Opcode::Add {
                        reg!(insn.c)
                    } else {
                        Value::int32(insn.int())
                    };
                    if let (Some(i), Some(j)) = (x.as_int32(), y.as_int32())
                        && let Some(sum) = i.checked_add(j)
                    {
                        reg!(insn.a) = Value::int32(sum);
                        continue 'dispatch;
                    }
                    if let (Some(f), Some(g)) = (x.as_f64(), y.as_f64()) {
                        reg!(insn.a) = Value::number(f + g);
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, ops::add(cx, x, y))
                }
                Opcode::Sub | Opcode::SubI => {
                    let x = reg!(insn.b);
                    let y = if insn.op == Opcode::Sub {
                        reg!(insn.c)
                    } else {
                        Value::int32(insn.int())
                    };
                    if let (Some(i), Some(j)) = (x.as_int32(), y.as_int32())
                        && let Some(difference) = i.checked_sub(j)
                    {
                        reg!(insn.a) = Value::int32(difference);
                        continue 'dispatch;
                    }
                    if let (Some(f), Some(g)) = (x.as_f64(), y.as_f64()) {
                        reg!(insn.a) = Value::number(f - g);
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, ops::arith(cx, Arith::Sub, x, y))
                }
                Opcode::Mul
                | Opcode::Div
                | Opcode::Mod
                | Opcode::Pow
                | Opcode::Shl
                | Opcode::Sar
                | Opcode::Shr
                | Opcode::BitAnd
                | Opcode::BitOr
                | Opcode::BitXor
                | Opcode::MulI
                | Opcode::BitAndI
                | Opcode::BitOrI
                | Opcode::BitXorI
                | Opcode::ShlI
                | Opcode::SarI
                | Opcode::ShrI => {
                    let (op, immediate) = arith_of(insn.op);
                    let x = reg!(insn.b);
                    let y = if immediate {
                        Value::int32(insn.int())
                    } else {
                        reg!(insn.c)
                    };
                    if let Some(value) = arith_fast(op, x, y) {
                        reg!(insn.a) = value;
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, ops::arith(cx, op, x, y))
                }

                // ---- comparisons as values ----
                Opcode::Lt
                | Opcode::Lte
                | Opcode::Gt
                | Opcode::Gte
                | Opcode::Eq
                | Opcode::Neq
                | Opcode::StrictEq
                | Opcode::StrictNeq => {
                    let (x, y) = (reg!(insn.b), reg!(insn.c));
                    if let Some(order) = order(x, y) {
                        reg!(insn.a) = Value::bool(JUMP_MASKS[insn.op as usize] & order != 0);
                        continue 'dispatch;
                    }
                    let (test, when) = test_of(insn.op);
                    write!('dispatch,
                        insn.a,
                        test.slow(cx, x, y).map(|holds| Value::bool(holds == when))
                    )
                }
                Opcode::InstanceOf => {
                    let (x, y) = (reg!(insn.b), reg!(insn.c));
                    write!('dispatch, insn.a, ops::instance_of(cx, x, y).map(Value::bool))
                }
                Opcode::In => {
                    let (x, y) = (reg!(insn.b), reg!(insn.c));
                    write!('dispatch, insn.a, ops::has_property(cx, x, y).map(Value::bool))
                }

                // ---- control ----
                Opcode::Jmp => {
                    pc = insn.imm as usize;
                    continue 'dispatch;
                }
                Opcode::JTrue | Opcode::JFalse => {
                    let x = reg!(insn.a);
                    let truthy = match x.as_bool() {
                        Some(value) => value,
                        None => {
                            let truthy = ops::to_boolean(cx.heap, x);
                            regs = window!();
                            truthy
                        }
                    };
                    if truthy == (insn.op == Opcode::JTrue) {
                        pc = insn.imm as usize;
                    }
                    continue 'dispatch;
                }
                Opcode::JUndefinedOrNull => {
                    let x = reg!(insn.a);
                    if x.is_undefined() || x.is_null() {
                        pc = insn.imm as usize;
                    }
                    continue 'dispatch;
                }
                Opcode::Gosub => {
                    reg!(insn.a) = Value::int32(pc as i32);
                    pc = insn.imm as usize;
                    continue 'dispatch;
                }
                Opcode::Ret => {
                    pc = reg!(insn.a).as_int32().expect("a gosub return index") as usize;
                    continue 'dispatch;
                }
                Opcode::JLt
                | Opcode::JLte
                | Opcode::JGt
                | Opcode::JGte
                | Opcode::JNlt
                | Opcode::JNlte
                | Opcode::JNgt
                | Opcode::JNgte
                | Opcode::JEq
                | Opcode::JNeq
                | Opcode::JStrictEq
                | Opcode::JStrictNeq
                | Opcode::JLtI
                | Opcode::JLteI
                | Opcode::JGtI
                | Opcode::JGteI
                | Opcode::JNltI
                | Opcode::JNlteI
                | Opcode::JNgtI
                | Opcode::JNgteI
                | Opcode::JEqI
                | Opcode::JNeqI
                | Opcode::JStrictEqI
                | Opcode::JStrictNeqI => {
                    let mask = JUMP_MASKS[insn.op as usize];
                    let x = reg!(insn.a);
                    let y = if mask & IMMEDIATE != 0 {
                        Value::int32(insn.short().into())
                    } else {
                        reg!(insn.b)
                    };
                    let taken = match order(x, y) {
                        Some(order) => mask & order != 0,
                        None => {
                            let (test, when) = jump_of(insn.op);
                            match test.slow(cx, x, y) {
                                Ok(holds) => {
                                    regs = window!();
                                    holds == when
                                }
                                Err(throw) => break 'op Err(throw),
                            }
                        }
                    };
                    if taken {
                        pc = insn.imm as usize;
                    }
                    continue 'dispatch;
                }

                // ---- properties ----
                Opcode::GetField => {
                    // Section 2.2. The hit reads the heap's arenas as fields,
                    // disjoint from the register file, so the window stays.
                    let object = reg!(insn.b);
                    let site = ic_base + insn.c as usize;
                    let entry = cx.heap.ics.entries[site];
                    let heap = &cx.heap.objects;
                    if let Some(value) = ic::get_hit(
                        heap,
                        &cx.heap.cells,
                        &cx.heap.values,
                        &cx.heap.strings,
                        entry,
                        object,
                    ) {
                        reg!(insn.a) = value;
                        cx.heap.ics.count_hit(IcKind::Get);
                        continue 'dispatch;
                    }
                    let fill = insn.c != FIELD_NO_SITE;
                    write!('dispatch, insn.a, property::get_cached(cx, object, insn.key(), site, fill))
                }
                Opcode::SetField => {
                    // Section 2.3, with the arenas borrowed as in `GetField`.
                    let (object, value) = (reg!(insn.a), reg!(insn.b));
                    let site = ic_base + insn.c as usize;
                    let entry = cx.heap.ics.entries[site];
                    if ic::set_hit(
                        &mut cx.heap.objects,
                        &cx.heap.cells,
                        &mut cx.heap.values,
                        entry,
                        object,
                        value,
                    ) {
                        cx.heap.ics.count_hit(IcKind::Set);
                        continue 'dispatch;
                    }
                    let fill = insn.c != FIELD_NO_SITE;
                    property::set_cached(cx, object, insn.key(), value, strict, site, fill)
                }
                Opcode::DefineField => {
                    let (object, value) = (reg!(insn.a), reg!(insn.b));
                    let object = object.as_object().expect("an object");
                    property::define(cx, object, insn.key(), value)
                }
                Opcode::DeleteField => {
                    let object = reg!(insn.b);
                    write!('dispatch, insn.a,
                        property::delete(cx, object, insn.key(), strict).map(Value::bool)
                    )
                }
                Opcode::GetElem => {
                    let (object, key) = (reg!(insn.b), reg!(insn.c));
                    if let (Some(target), Some(index)) = (object.as_object(), key.as_int32())
                        && index >= 0
                    {
                        let element = cx.heap.get_element(target, index as u32);
                        if !element.is_empty() {
                            set!('dispatch, insn.a, element)
                        }
                        // Section 4.4: a typed array's element.
                        if cx.heap.object_kind(target) == ObjectKind::TypedArray
                            && let Some(element) = typed::fast_get(cx.heap, target, index as u32)
                        {
                            set!('dispatch, insn.a, element)
                        }
                    }
                    let result = property::get_computed(cx, object, key);
                    write!('dispatch, insn.a, result)
                }
                Opcode::SetElem => {
                    let (object, key, value) = (reg!(insn.a), reg!(insn.b), reg!(insn.c));
                    if let (Some(target), Some(index)) = (object.as_object(), key.as_int32())
                        && index >= 0
                    {
                        let header = cx.heap.header(target);
                        let elements = header.elements();
                        // An `arguments` object's mapped indices go through
                        // its cells: only arrays and ordinary objects take the
                        // dense write.
                        let length = match header.kind {
                            ObjectKind::Array => header.payload,
                            ObjectKind::Ordinary => elements.len,
                            ObjectKind::TypedArray => {
                                if typed::fast_set(cx.heap, target, index as u32, value) {
                                    regs = window!();
                                    continue 'dispatch;
                                }
                                0
                            }
                            _ => 0,
                        };
                        // A hole defers to the chain unless no prototype
                        // can intercept it.
                        if (index as u32) < elements.len.min(length)
                            && (cx.heap.holes_writable()
                                || !cx.heap.values.get(elements, index as u32).is_empty())
                        {
                            cx.heap.values.set(elements, index as u32, value);
                            regs = window!();
                            continue 'dispatch;
                        }
                    }
                    ops::to_property_key(cx, key)
                        .and_then(|key| property::set(cx, object, key, value, strict))
                }
                Opcode::DefineElem => {
                    let (object, key, value) = (reg!(insn.a), reg!(insn.b), reg!(insn.c));
                    let object = object.as_object().expect("an object");
                    ops::to_property_key(cx, key)
                        .and_then(|key| property::define(cx, object, key, value))
                }
                Opcode::DeleteElem => {
                    let (object, key) = (reg!(insn.b), reg!(insn.c));
                    let result = ops::to_property_key(cx, key)
                        .and_then(|key| property::delete(cx, object, key, strict));
                    write!('dispatch, insn.a, result.map(Value::bool))
                }

                // ---- globals ----
                Opcode::GetGlobal | Opcode::GetGlobalOrUndefined => {
                    // Section 2.4: one generation compare and a load.
                    let site = ic_base + global_site(insn);
                    let entry = cx.heap.ics.entries[site];
                    if entry.w0 == cx.heap.global_generation {
                        let start = cx.heap.objects.get(entry.w3).slots_start;
                        reg!(insn.a) = cx.heap.values.get_at(start + entry.w1);
                        cx.heap.ics.count_hit(IcKind::Global);
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, get_global(cx, insn, site))
                }
                Opcode::SetGlobal => {
                    let value = reg!(insn.a);
                    let site = ic_base + global_site(insn);
                    let entry = cx.heap.ics.entries[site];
                    if entry.w0 == cx.heap.global_generation
                        && entry.w2 & u32::from(PropertyFlags::WRITABLE) != 0
                    {
                        let start = cx.heap.objects.get(entry.w3).slots_start;
                        cx.heap.values.set_at(start + entry.w1, value);
                        cx.heap.ics.count_hit(IcKind::Global);
                        continue 'dispatch;
                    }
                    set_global(cx, insn, value, strict, site)
                }
                Opcode::DefineGlobal => {
                    let value = reg!(insn.a);
                    let configurable = insn.b & global_flags::EVAL != 0;
                    define_global(cx, insn.key(), value, true, configurable)
                }
                Opcode::DeclareGlobal
                | Opcode::DeclareGlobalLet
                | Opcode::DeclareGlobalConst
                | Opcode::CheckGlobalVar
                | Opcode::CheckGlobalLexical => declare_global(cx, insn),
                Opcode::DeleteGlobal => write!('dispatch, insn.a, delete_global(cx, insn.key())),

                // ---- captured bindings ----
                Opcode::NewCell => {
                    let value = reg!(insn.b);
                    write!('dispatch, insn.a, new_cell(cx, value))
                }
                Opcode::NewCellEmpty => write!('dispatch, insn.a, new_cell(cx, Value::EMPTY)),
                Opcode::CloseCell => {
                    let old = cell(reg!(insn.a));
                    let value = cx.heap.scope_get(old, 0, 0);
                    write!('dispatch, insn.a, new_cell(cx, value))
                }
                Opcode::LoadCell => {
                    let cell = cell(reg!(insn.b));
                    set!('dispatch, insn.a, cx.heap.scope_get(cell, 0, 0))
                }
                Opcode::StoreCell => {
                    let (cell, value) = (cell(reg!(insn.a)), reg!(insn.b));
                    cx.heap.scope_set(cell, 0, 0, value);
                    regs = window!();
                    continue 'dispatch;
                }
                Opcode::CheckCellInit => {
                    let cell = cell(reg!(insn.a));
                    if !cx.heap.scope_get(cell, 0, 0).is_empty() {
                        regs = window!();
                        continue 'dispatch;
                    }
                    Err(key_error(
                        cx,
                        ErrorKind::Reference,
                        insn,
                        "",
                        " is not initialized",
                    ))
                }
                Opcode::LoadCaptured => {
                    let cell = captured_cell(cx.heap, frame.function, insn.imm);
                    set!('dispatch, insn.a, cx.heap.scope_get(cell, 0, 0))
                }
                Opcode::StoreCaptured => {
                    let value = reg!(insn.a);
                    let cell = captured_cell(cx.heap, frame.function, insn.imm);
                    cx.heap.scope_set(cell, 0, 0, value);
                    regs = window!();
                    continue 'dispatch;
                }

                // ---- the phase 3 surface: rare forms, out of line ----
                Opcode::DefineGetter
                | Opcode::DefineSetter
                | Opcode::NewSpill
                | Opcode::ClearCell
                | Opcode::LoadSlot
                | Opcode::StoreSlot
                | Opcode::NewArguments
                | Opcode::Rest
                | Opcode::LoadNewTarget
                | Opcode::NewRegExp
                | Opcode::ForInStart
                | Opcode::AppendSpread
                | Opcode::ThrowInternal
                | Opcode::CheckCtor
                | Opcode::LoadHomeObject
                | Opcode::ToObject
                | Opcode::SetProto
                | Opcode::DefineMethod
                | Opcode::DefineMethodComputed
                | Opcode::SetFunctionName
                | Opcode::ThrowType
                | Opcode::ThrowReference
                | Opcode::ImportCall => rare_op(cx, &frame, unit, insn),
                Opcode::IterStart
                | Opcode::IterNext
                | Opcode::IterClose
                | Opcode::IterCloseThrow
                | Opcode::IterCallNext
                | Opcode::IterCall
                | Opcode::IterCheckObject
                | Opcode::IterStartAsync
                | Opcode::IterNextRaw
                | Opcode::IterResult => super::iter::iter_op(cx, &frame, insn),
                Opcode::MakeClass
                | Opcode::InstallFields
                | Opcode::InitFields
                | Opcode::RunStaticInit
                | Opcode::CallStaticBlock
                | Opcode::GetSuper
                | Opcode::GetSuperValue
                | Opcode::PutSuperValue
                | Opcode::InitDerived
                | Opcode::InitThis
                | Opcode::InitThisCell
                | Opcode::InitThisCaptured
                | Opcode::CheckDerivedReturn
                | Opcode::NewPrivateName
                | Opcode::SetHomeObject
                | Opcode::NamePrivateMethod
                | Opcode::GetPrivate
                | Opcode::PutPrivate
                | Opcode::DefinePrivate
                | Opcode::PrivateIn
                | Opcode::GetPrivateMethod
                | Opcode::CallPrivateGetter
                | Opcode::CallPrivateSetter
                | Opcode::PrivateBrandIn
                | Opcode::CopyData
                | Opcode::CopyDataExcluded
                | Opcode::GlobalRef
                | Opcode::GetRef
                | Opcode::PutRef => super::class::class_op(cx, &frame, unit, insn),
                Opcode::ForInNext => {
                    let iterator = reg!(insn.b);
                    match property::for_in_next(cx, iterator) {
                        Ok(Some(key)) => {
                            pc = insn.imm as usize;
                            set!('dispatch, insn.a, key)
                        }
                        Ok(None) => Ok(()),
                        Err(throw) => Err(throw),
                    }
                }

                // ---- allocation ----
                Opcode::NewObject => {
                    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
                    let slots = if insn.imm == 0 {
                        MIN_OPEN_SLOTS
                    } else {
                        insn.imm
                    };
                    let object = cx
                        .heap
                        .alloc_object(ObjectKind::Ordinary, Some(prototype), slots);
                    write!('dispatch, insn.a,
                        object
                            .map(Value::object)
                            .map_err(|error| error::heap_error(cx, error))
                    )
                }
                Opcode::NewArray => write!('dispatch, insn.a, new_array(cx, base, insn)),
                Opcode::MakeClosure => {
                    write!('dispatch, insn.a, make_closure(cx, &frame, unit, insn.imm))
                }

                // ---- calls ----
                Opcode::Call
                | Opcode::CallMethod
                | Opcode::TailCall
                | Opcode::TailCallMethod
                | Opcode::New
                | Opcode::CallSpread
                | Opcode::NewSpread => {
                    let b = insn.b as usize;
                    let (callee_register, mut window) = match insn.op {
                        Opcode::Call | Opcode::TailCall => (b, b),
                        Opcode::New | Opcode::CallSpread | Opcode::NewSpread => (b, b + 1),
                        _ => (b + 1, b + 1),
                    };
                    let mut function = reg!(callee_register);
                    let mut this = match insn.op {
                        Opcode::CallMethod | Opcode::TailCallMethod => reg!(b),
                        Opcode::CallSpread => reg!(b + 1),
                        _ => Value::UNDEFINED,
                    };
                    let mut argc = insn.c as usize;
                    let tail = matches!(insn.op, Opcode::TailCall | Opcode::TailCallMethod);
                    let construct = matches!(insn.op, Opcode::New | Opcode::NewSpread);
                    if matches!(insn.op, Opcode::CallSpread | Opcode::NewSpread) {
                        match arguments::spread(cx, base + window) {
                            Ok(count) => argc = count,
                            Err(throw) => break 'op Err(throw),
                        }
                    }
                    // Section 5.2: `f.call(…)` and `f.apply(…)` move the window
                    // onto their `thisArg` and dispatch on `f`.
                    let mut target = callee(cx.heap, function);
                    if let Callee::Native(Native::FunctionCall | Native::FunctionApply) = target
                        && !construct
                    {
                        match rewrite_call(cx, base, window, function, this, argc) {
                            Ok(rewritten) => {
                                (target, function, window, this, argc) = rewritten;
                            }
                            Err(throw) => break 'op Err(throw),
                        }
                    }
                    match target {
                        Callee::Bytecode(object, bytecode) => {
                            let unit_id: CodeUnitId = cx.heap.bytecode_unit(bytecode);
                            let callee_unit = store.get(unit_id);
                            let mut new_target = Value::UNDEFINED;
                            if construct {
                                let site =
                                    (insn.op == Opcode::New).then(|| ic_base + insn.imm as usize);
                                match construct_this(
                                    cx,
                                    object,
                                    function,
                                    callee_unit,
                                    base + window,
                                    site,
                                ) {
                                    Ok(pair) => (this, new_target) = pair,
                                    Err(throw) => break 'op Err(throw),
                                }
                            }
                            cx.heap.vm.registers[base + window] = this;
                            let mut frame_flags = 0;
                            if construct {
                                frame_flags |= frame_flags::CONSTRUCT;
                            }
                            if tail {
                                frame_flags |= frame_flags::TAIL;
                            }
                            let callee_frame = Frame {
                                unit: unit_id,
                                function: object,
                                bytecode,
                                base: (base + window) as u32,
                                return_pc: pc as u32,
                                return_dst: (base + insn.a as usize) as u32,
                                argc: argc as u32,
                                flags: frame_flags,
                            };
                            if let Err(throw) = enter_frame(cx, callee_unit, callee_frame) {
                                break 'op Err(throw);
                            }
                            if construct {
                                write_new_target(cx, base + window, callee_unit, new_target);
                            }
                            load_frame!();
                            pc = 0;
                            regs = window!();
                            continue 'dispatch;
                        }
                        other => {
                            let result = call_other(
                                cx,
                                other,
                                function,
                                base + window,
                                this,
                                argc,
                                construct,
                                pc - 1,
                            );
                            cx.heap.vm.top = frame_end(&frame, unit);
                            match result {
                                Ok(value) if tail => match finish_return(cx, value) {
                                    Returned::Exit(value) => return Ok(value),
                                    Returned::Resume(resume) => {
                                        load_frame!();
                                        pc = resume as usize;
                                        regs = window!();
                                        continue 'dispatch;
                                    }
                                },
                                Ok(value) => set!('dispatch, insn.a, value),
                                Err(throw) => Err(throw),
                            }
                        }
                    }
                }
                Opcode::Return | Opcode::ReturnUndefined => {
                    let value = if insn.op == Opcode::Return {
                        reg!(insn.a)
                    } else {
                        Value::UNDEFINED
                    };
                    // A plain call's frame (not an entry, a construction or
                    // a tail call) returns to the frame below, loaded once.
                    if frame.flags == 0 {
                        cx.heap.vm.frames.pop();
                        let caller = *cx.heap.vm.frames.last().expect("a caller");
                        let caller_unit = store.get(caller.unit);
                        raise_top(cx, frame_end(&caller, caller_unit));
                        cx.heap.vm.registers[frame.return_dst as usize] = value;
                        pc = frame.return_pc as usize;
                        frame = caller;
                        unit = caller_unit;
                        code = &unit.code;
                        base = frame.base as usize;
                        strict = unit.has(flags::STRICT);
                        ic_base = unit.ic_base as usize;
                        regs = window!();
                        continue 'dispatch;
                    }
                    match finish_return(cx, value) {
                        Returned::Exit(value) => return Ok(value),
                        Returned::Resume(resume) => {
                            load_frame!();
                            pc = resume as usize;
                            regs = window!();
                            continue 'dispatch;
                        }
                    }
                }
                Opcode::Throw => Err(Throw(reg!(insn.a))),

                // ---- suspension (section 4.3), out of line ----
                Opcode::InitialYield
                | Opcode::AsyncStart
                | Opcode::Yield
                | Opcode::YieldStar
                | Opcode::Await => match suspend_op(cx, &frame, unit, insn, pc) {
                    Ok(Returned::Exit(value)) => return Ok(value),
                    Ok(Returned::Resume(resume)) => {
                        load_frame!();
                        pc = resume as usize;
                        regs = window!();
                        continue 'dispatch;
                    }
                    Err(throw) => Err(throw),
                },
            }
        };

        // A slow path finished: continue 'dispatch, or unwind its throw.
        let Err(Throw(thrown)) = outcome else {
            regs = window!();
            continue 'dispatch;
        };
        let mut at = pc - 1;
        loop {
            if let Some(handler) = unit
                .handlers
                .iter()
                .find(|handler| handler.start as usize <= at && at < handler.end as usize)
            {
                cx.heap.vm.registers[base + handler.register as usize] = thrown;
                pc = handler.target as usize;
                break;
            }
            let done = cx.heap.vm.frames.pop().expect("a frame");
            if done.flags & frame_flags::ENTRY != 0 {
                return Err(Throw(thrown));
            }
            load_frame!();
            raise_top(cx, frame_end(&frame, unit));
            at = done.return_pc as usize - 1;
        }
        regs = window!();
    }
}

/// `NewArray a, b, imm`: an array of the `imm` registers from `b`.
#[inline(never)]
fn new_array(cx: &mut Cx<'_>, base: usize, insn: Insn) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), insn.imm)
        .map_err(|error| error::heap_error(cx, error))?;
    let first = base + insn.b as usize;
    let heap = &mut *cx.heap;
    let elements = heap.header(array).elements();
    for index in 0..insn.imm {
        let value = heap.vm.registers[first + index as usize];
        heap.values.set(elements, index, value);
    }
    heap.set_array_length(array, insn.imm);
    Ok(Value::object(array))
}

/// How two numbers compare, as one of the [`LESS`], [`EQUAL`], [`GREATER`]
/// and [`UNORDERED`] bits; `None` unless both are numbers.
#[inline(always)]
fn order(x: Value, y: Value) -> Option<u8> {
    if let (Some(i), Some(j)) = (x.as_int32(), y.as_int32()) {
        return Some(match i.cmp(&j) {
            Ordering::Less => LESS,
            Ordering::Equal => EQUAL,
            Ordering::Greater => GREATER,
        });
    }
    let (f, g) = (x.as_f64()?, y.as_f64()?);
    Some(match f.partial_cmp(&g) {
        Some(Ordering::Less) => LESS,
        Some(Ordering::Equal) => EQUAL,
        Some(Ordering::Greater) => GREATER,
        None => UNORDERED,
    })
}

const LESS: u8 = 1 << 0;
const EQUAL: u8 = 1 << 1;
const GREATER: u8 = 1 << 2;
const UNORDERED: u8 = 1 << 3;
/// A compare-jump's right operand is the `i16` literal.
const IMMEDIATE: u8 = 1 << 4;

/// Per opcode, the orders of two numbers for which a compare-jump is taken or
/// a comparison is true (`==` and `===` agree on numbers), and
/// [`IMMEDIATE`]. Zero for every other opcode.
static JUMP_MASKS: [u8; 256] = {
    let mut masks = [0u8; 256];
    let mut index = 0;
    while index < Opcode::ALL.len() {
        let op = Opcode::ALL[index];
        let unordered_not = LESS | EQUAL | GREATER | UNORDERED;
        let mask = match op {
            Opcode::JLt | Opcode::JLtI | Opcode::Lt => LESS,
            Opcode::JLte | Opcode::JLteI | Opcode::Lte => LESS | EQUAL,
            Opcode::JGt | Opcode::JGtI | Opcode::Gt => GREATER,
            Opcode::JGte | Opcode::JGteI | Opcode::Gte => GREATER | EQUAL,
            Opcode::JNlt | Opcode::JNltI => unordered_not & !LESS,
            Opcode::JNlte | Opcode::JNlteI => unordered_not & !(LESS | EQUAL),
            Opcode::JNgt | Opcode::JNgtI => unordered_not & !GREATER,
            Opcode::JNgte | Opcode::JNgteI => unordered_not & !(GREATER | EQUAL),
            Opcode::JEq
            | Opcode::JEqI
            | Opcode::JStrictEq
            | Opcode::JStrictEqI
            | Opcode::Eq
            | Opcode::StrictEq => EQUAL,
            Opcode::JNeq
            | Opcode::JNeqI
            | Opcode::JStrictNeq
            | Opcode::JStrictNeqI
            | Opcode::Neq
            | Opcode::StrictNeq => unordered_not & !EQUAL,
            _ => 0,
        };
        let immediate = match op.format() {
            crate::bytecode::Format::AShortJump => IMMEDIATE,
            _ => 0,
        };
        masks[op as usize] = mask | immediate;
        index += 1;
    }
    masks
};

/// A comparison the loop evaluates: a relation, `==` or `===`.
#[derive(Clone, Copy)]
enum Test {
    Relation(Relation),
    Loose,
    Strict,
}

impl Test {
    /// `==` and `===` decided from the bits, before the general algorithms:
    /// the same
    /// value that is not a double (`NaN` is not equal to itself), and
    /// `null` or `undefined` against anything (phase 3 M5, on Richards'
    /// `x == null` tests). `None` for the rest.
    #[inline(always)]
    fn quick(self, heap: &Heap, x: Value, y: Value) -> Option<bool> {
        // Kept out of the comparison arms (it is called from `slow`), whose
        // size the hot numeric loops are sensitive to.
        if matches!(self, Test::Relation(_)) {
            return None;
        }
        if x == y && !x.is_double() {
            return Some(true);
        }
        let nullish = |value: Value| value.is_undefined() || value.is_null();
        match self {
            // Against `null` or `undefined`, an object is equal only if it
            // is `IsHTMLDDA`.
            Test::Loose if nullish(x) || nullish(y) => Some(
                (nullish(x) || ops::is_html_dda(heap, x))
                    && (nullish(y) || ops::is_html_dda(heap, y)),
            ),
            Test::Loose if nullish(x) && nullish(y) => Some(true),
            // Different bits: equal only between numbers, strings or
            // BigInts of equal value.
            Test::Strict
                if nullish(x)
                    || nullish(y)
                    || x.as_bool().is_some()
                    || y.as_bool().is_some()
                    || x.as_object().is_some()
                    || y.as_object().is_some() =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    #[inline(never)]
    fn slow(self, cx: &mut Cx<'_>, x: Value, y: Value) -> Result<bool, Throw> {
        if let Some(holds) = self.quick(cx.heap, x, y) {
            return Ok(holds);
        }
        match self {
            Test::Relation(relation) => ops::relation(cx, relation, x, y),
            Test::Loose => ops::loose_eq(cx, x, y),
            Test::Strict => Ok(ops::strict_eq(cx.heap, x, y)),
        }
    }
}

/// A value comparison's test and the result that makes it true, for the
/// slow path.
fn test_of(op: Opcode) -> (Test, bool) {
    match op {
        Opcode::Lt => (Test::Relation(Relation::Lt), true),
        Opcode::Lte => (Test::Relation(Relation::Lte), true),
        Opcode::Gt => (Test::Relation(Relation::Gt), true),
        Opcode::Gte => (Test::Relation(Relation::Gte), true),
        Opcode::Eq => (Test::Loose, true),
        Opcode::Neq => (Test::Loose, false),
        Opcode::StrictEq => (Test::Strict, true),
        _ => (Test::Strict, false),
    }
}

/// A compare-jump's test and the result that takes it, for the slow path.
fn jump_of(op: Opcode) -> (Test, bool) {
    use Relation as R;
    use Test as T;
    match op {
        Opcode::JLt => (T::Relation(R::Lt), true),
        Opcode::JLte => (T::Relation(R::Lte), true),
        Opcode::JGt => (T::Relation(R::Gt), true),
        Opcode::JGte => (T::Relation(R::Gte), true),
        Opcode::JNlt => (T::Relation(R::Lt), false),
        Opcode::JNlte => (T::Relation(R::Lte), false),
        Opcode::JNgt => (T::Relation(R::Gt), false),
        Opcode::JNgte => (T::Relation(R::Gte), false),
        Opcode::JEq => (T::Loose, true),
        Opcode::JNeq => (T::Loose, false),
        Opcode::JStrictEq => (T::Strict, true),
        Opcode::JStrictNeq => (T::Strict, false),
        Opcode::JLtI => (T::Relation(R::Lt), true),
        Opcode::JLteI => (T::Relation(R::Lte), true),
        Opcode::JGtI => (T::Relation(R::Gt), true),
        Opcode::JGteI => (T::Relation(R::Gte), true),
        Opcode::JNltI => (T::Relation(R::Lt), false),
        Opcode::JNlteI => (T::Relation(R::Lte), false),
        Opcode::JNgtI => (T::Relation(R::Gt), false),
        Opcode::JNgteI => (T::Relation(R::Gte), false),
        Opcode::JEqI => (T::Loose, true),
        Opcode::JNeqI => (T::Loose, false),
        Opcode::JStrictEqI => (T::Strict, true),
        _ => (T::Strict, false),
    }
}

/// The operator of an arithmetic arm and whether its right operand is the
/// `i32` literal.
#[inline(always)]
fn arith_of(op: Opcode) -> (Arith, bool) {
    match op {
        Opcode::Mul => (Arith::Mul, false),
        Opcode::Div => (Arith::Div, false),
        Opcode::Mod => (Arith::Mod, false),
        Opcode::Pow => (Arith::Pow, false),
        Opcode::Shl => (Arith::Shl, false),
        Opcode::Sar => (Arith::Sar, false),
        Opcode::Shr => (Arith::Shr, false),
        Opcode::BitAnd => (Arith::BitAnd, false),
        Opcode::BitOr => (Arith::BitOr, false),
        Opcode::BitXor => (Arith::BitXor, false),
        Opcode::MulI => (Arith::Mul, true),
        Opcode::BitAndI => (Arith::BitAnd, true),
        Opcode::BitOrI => (Arith::BitOr, true),
        Opcode::BitXorI => (Arith::BitXor, true),
        Opcode::ShlI => (Arith::Shl, true),
        Opcode::SarI => (Arith::Sar, true),
        _ => (Arith::Shr, true),
    }
}

/// `x op y` for two numbers; `None` sends the operation to the slow path.
#[inline(always)]
fn arith_fast(op: Arith, x: Value, y: Value) -> Option<Value> {
    if let (Some(i), Some(j)) = (x.as_int32(), y.as_int32()) {
        let shift = j as u32 & 31;
        let value = match op {
            Arith::Mul => mul_int(i, j),
            Arith::Mod => mod_int(i, j),
            Arith::Shl => Some(Value::int32(i.wrapping_shl(shift))),
            Arith::Sar => Some(Value::int32(i >> shift)),
            Arith::Shr => i32::try_from((i as u32) >> shift).ok().map(Value::int32),
            Arith::BitAnd => Some(Value::int32(i & j)),
            Arith::BitOr => Some(Value::int32(i | j)),
            Arith::BitXor => Some(Value::int32(i ^ j)),
            Arith::Add | Arith::Sub | Arith::Div | Arith::Pow => None,
        };
        if value.is_some() {
            return value;
        }
    }
    if x.is_number() && y.is_number() && op != Arith::Pow {
        let (f, g) = (x.as_f64()?, y.as_f64()?);
        // The double forms of the hottest operators stay in the arm. A
        // product stays a double, as a sum does; a quotient becomes an
        // `Int32` when it is one, since it is often an index.
        return Some(match op {
            Arith::Mul => Value::number(f * g),
            Arith::Div => number::number(f / g),
            _ => ops::number_op(op, f, g),
        });
    }
    None
}

/// `Neg`, `Plus`, `ToNumeric` and `BitNot` on numbers.
#[inline(always)]
fn unary_fast(op: Opcode, x: Value) -> Option<Value> {
    if let Some(i) = x.as_int32() {
        return match op {
            Opcode::Neg if i != 0 && i != i32::MIN => Some(Value::int32(-i)),
            Opcode::Neg => None,
            Opcode::BitNot => Some(Value::int32(!i)),
            _ => Some(x),
        };
    }
    match op {
        Opcode::Plus | Opcode::ToNumeric if x.is_number() => Some(x),
        Opcode::Neg if x.is_double() => Some(number::number(-x.as_f64()?)),
        _ => None,
    }
}

enum Returned {
    /// The entry frame returned this value.
    Exit(Value),
    /// The caller is on top again and resumes at this index.
    Resume(u32),
}

/// The suspension opcodes (section 4.3). Each saves the running frame into
/// its activation record and leaves it: `InitialYield` returns the new
/// generator to the caller, `AsyncStart` runs the body and returns the
/// promise, `Yield`, `YieldStar` and `Await` leave the nested loop of the
/// resumption (their frame is its entry frame).
#[inline(never)]
fn suspend_op(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    insn: Insn,
    pc: usize,
) -> Result<Returned, Throw> {
    use super::suspend;
    use crate::heap::object::InternalTag;
    match insn.op {
        Opcode::InitialYield => {
            let asynchronous = unit.has(flags::ASYNC);
            let prototype = suspend::generator_prototype(cx, frame.function, asynchronous)?;
            let (tag, aux) = if asynchronous {
                (InternalTag::AsyncGenerator, Value::UNDEFINED)
            } else {
                (InternalTag::Generator, Value::UNDEFINED)
            };
            let record = suspend::new_record(cx, tag, prototype, frame.function, aux)?;
            suspend::save_frame(cx, frame, unit, pc as u32, record, suspend::START);
            Ok(finish_return(cx, Value::object(record)))
        }
        Opcode::AsyncStart => {
            let prototype = cx.realm.intrinsic(Intrinsic::PromisePrototype);
            let promise = super::jobs::new_promise(cx, prototype)?;
            cx.heap.roots().push(Value::object(promise));
            let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
            let record = suspend::new_record(
                cx,
                InternalTag::AsyncFunction,
                object_prototype,
                frame.function,
                Value::object(promise),
            );
            cx.heap.roots().pop_one();
            let record = record?;
            suspend::save_frame(cx, frame, unit, pc as u32, record, suspend::START);
            let done = cx.heap.vm.frames.pop().expect("the async frame");
            cx.heap.vm.top = done.base;
            let promise = suspend::start_async(cx, record)?;
            if done.flags & frame_flags::ENTRY != 0 {
                return Ok(Returned::Exit(promise));
            }
            let caller = *cx.heap.vm.frames.last().expect("a caller");
            let caller_unit = cx.code.get(caller.unit);
            raise_top(cx, frame_end(&caller, caller_unit));
            cx.heap.vm.registers[done.return_dst as usize] = promise;
            Ok(Returned::Resume(done.return_pc))
        }
        Opcode::Yield | Opcode::YieldStar => {
            let record = *cx.heap.vm.active.last().expect("a running generator");
            let value = cx.heap.vm.registers[frame.base as usize + insn.a as usize];
            suspend::save_frame(cx, frame, unit, pc as u32, record, suspend::YIELD);
            Ok(finish_return(cx, value))
        }
        _ => {
            let value = cx.heap.vm.registers[frame.base as usize + insn.a as usize];
            suspend::await_value(cx, value)?;
            let record = *cx
                .heap
                .vm
                .active
                .last()
                .expect("a running async activation");
            suspend::save_frame(cx, frame, unit, pc as u32, record, suspend::AWAIT);
            Ok(finish_return(cx, Value::UNDEFINED))
        }
    }
}

/// Pops the returning frame (and the frames a tail call returns through),
/// applies a constructor's receiver rule and writes `value` into the
/// caller's destination register.
#[inline]
fn finish_return(cx: &mut Cx<'_>, mut value: Value) -> Returned {
    loop {
        let done = cx.heap.vm.frames.pop().expect("a frame");
        if done.flags & frame_flags::CONSTRUCT != 0 {
            learn_receiver_size(cx, &done);
            if value.as_object().is_none() {
                value = cx.heap.vm.registers[done.base as usize];
            }
        }
        if done.flags & frame_flags::ENTRY != 0 {
            return Returned::Exit(value);
        }
        if done.flags & frame_flags::TAIL != 0 {
            continue;
        }
        let caller = *cx.heap.vm.frames.last().expect("a caller");
        let unit = cx.code.get(caller.unit);
        raise_top(cx, frame_end(&caller, unit));
        cx.heap.vm.registers[done.return_dst as usize] = value;
        return Returned::Resume(done.return_pc);
    }
}

/// `i * j` unless it overflows or is `-0`.
#[inline]
fn mul_int(i: i32, j: i32) -> Option<Value> {
    let product = i.checked_mul(j)?;
    if product == 0 && (i < 0 || j < 0) {
        return None;
    }
    Some(Value::int32(product))
}

/// `i % j` for a non-negative dividend and positive divisor.
#[inline]
fn mod_int(i: i32, j: i32) -> Option<Value> {
    if i >= 0 && j > 0 {
        Some(Value::int32(i % j))
    } else {
        None
    }
}
