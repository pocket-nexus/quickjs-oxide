//! The main loop (section 4.2): one `match` on the typed opcode, registers
//! read as `registers[base + r]`, inline `Int32` and double fast paths, and
//! `#[inline(never)]` slow paths in [`ops`] and [`property`]. Calls, returns
//! and `new` on bytecode functions stay in the loop (section 4.3); a throw
//! unwinds through the handler tables (section 4.4).

use std::cmp::Ordering;

use super::arguments;
use super::error::{self, ErrorKind};
use super::ic::{self, FIELD_NO_SITE, GLOBAL_NO_SITE, IcEntry, IcKind};
use super::native::{self, CallSite, Native};
use super::number;
use super::ops::{self, Arith, Relation, Unary};
use super::property;
use super::regexp;
use super::{Cx, Frame, SPAN, Throw, frame_flags};
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
        }
    }
    cx.heap.vm.frames.push(frame);
    cx.heap.vm.top = end as u32;
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
        _ => unreachable!("{} is not a rare form", insn.op.name()),
    };
    cx.heap.vm.registers[base + register as usize] = value;
    Ok(())
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
) -> Result<(Callee, usize, Value, usize), Throw> {
    loop {
        let target = callee(cx.heap, function);
        let Callee::Native(native @ (Native::FunctionCall | Native::FunctionApply)) = target else {
            return Ok((target, window, this, argc));
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
#[inline(never)]
fn call_native(
    cx: &mut Cx<'_>,
    native: Native,
    at: usize,
    this: Value,
    argc: usize,
    construct: bool,
) -> Result<Value, Throw> {
    if !construct {
        cx.heap.vm.registers[at] = this;
    }
    let end = (at + 1 + argc) as u32;
    cx.heap.vm.top = cx.heap.vm.top.max(end);
    native::call(
        cx,
        native,
        CallSite {
            this: at,
            args: at + 1,
            argc,
        },
    )
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
            },
        ),
        Callee::NotCallable => Err(error::type_error(cx, "not a function")),
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
#[inline]
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
        let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
        let function = cx
            .heap
            .alloc_closure(Some(prototype), scope, child)
            .map_err(|error| error::heap_error(cx, error))?;
        if child_unit.has(flags::CONSTRUCTOR) {
            cx.heap.roots().push(Value::object(function));
            let names = cx.realm.names;
            let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
            let heap = &mut *cx.heap;
            let instance = heap
                .alloc_object(ObjectKind::Ordinary, Some(object_prototype), 1)
                .and_then(|instance| {
                    heap.add_property(
                        instance,
                        Names::key(names.constructor),
                        PropertyFlags::data(true, false, true),
                        Value::object(function),
                    )?;
                    heap.add_property(
                        function,
                        Names::key(names.prototype),
                        PropertyFlags::data(true, false, false),
                        Value::object(instance),
                    )
                });
            instance.map_err(|error| error::heap_error(cx, error))?;
        }
        Ok(Value::object(function))
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `New`'s receiver (phase 3, section 2.5): an ordinary object inheriting
/// from the constructor's `prototype`, sized by the site's learned slot count.
/// A hit is a shape compare on the constructor and a load of its `prototype`
/// slot, read live; a miss looks it up and refills the entry with the
/// constructor's static `this.x =` count.
#[inline(never)]
fn construct_receiver(
    cx: &mut Cx<'_>,
    constructor: ObjectId,
    this_slots: u16,
    site: Option<usize>,
) -> Result<Value, Throw> {
    let Some(site) = site else {
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
        .alloc_object(ObjectKind::Ordinary, Some(prototype), slots)
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

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
    let caller = cx.heap.vm.frames.last().expect("a caller");
    let unit = cx.code.get(caller.unit);
    let insn = unit.code[done.return_pc as usize - 1];
    if insn.op != Opcode::New {
        return;
    }
    let site = unit.ic_base as usize + insn.imm as usize;
    let mut entry = cx.heap.ics.entries[site];
    let Some(receiver) = cx.heap.vm.registers[done.base as usize].as_object() else {
        return;
    };
    if !entry.is_filled() || cx.heap.is_dictionary(receiver) {
        return;
    }
    let width = cx.heap.shape(cx.heap.shape_of(receiver)).width;
    if width > entry.w2 {
        entry.w2 = width.min(MAX_LEARNED_SLOTS);
        cx.heap.ics.entries[site] = entry;
    }
}

/// `GetGlobal` and `GetGlobalOrUndefined` past the cache (section 2.4): an
/// own data property of the global object fills the entry with the global
/// generation; anything else (an accessor, a global found on
/// `%Object.prototype%`) is read uncached.
#[inline(never)]
fn get_global(cx: &mut Cx<'_>, insn: Insn, site: usize) -> Result<Value, Throw> {
    let key = insn.key();
    let global = cx.realm.global();
    let heap = &*cx.heap;
    if let Some((slot, property)) = heap.lookup_own(global, key)
        && !property.has(PropertyFlags::ACCESSOR)
    {
        let value = heap.get_slot(global, slot);
        let entry = global_entry(heap, slot, property);
        let filled = insn.site() != Some(GLOBAL_NO_SITE) && cx.heap.ics.record(site, entry);
        cx.heap.ics.count_miss(site, filled);
        return Ok(value);
    }
    cx.heap.ics.count_miss(site, false);
    match property::find(cx.heap, &cx.realm.names, global, key) {
        Some(found) => property::value_of(cx, Some(found), Value::object(global)),
        None if insn.op == Opcode::GetGlobalOrUndefined => Ok(Value::UNDEFINED),
        None => Err(key_error(
            cx,
            ErrorKind::Reference,
            insn,
            "'",
            "' is not defined",
        )),
    }
}

/// `SetGlobal` past the cache: the uncached write, then a fill when the
/// global is an own writable data property of the global object.
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
    if strict && !property::has(cx.heap, &cx.realm.names, global, key) {
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
            let entry = global_entry(heap, slot, property);
            heap.ics.record(site, entry)
        }
        _ => false,
    };
    heap.ics.count_miss(site, filled);
    Ok(())
}

fn global_entry(heap: &Heap, slot: u32, property: PropertyFlags) -> IcEntry {
    IcEntry {
        w0: heap.global_generation,
        w1: slot,
        w2: u32::from(property.bits()),
        w3: NONE,
    }
}

/// Defines a script-level declaration on the global object.
#[inline(never)]
fn define_global(
    cx: &mut Cx<'_>,
    key: PropertyKey,
    value: Value,
    overwrite: bool,
) -> Result<(), Throw> {
    let global = cx.realm.global();
    let heap = &mut *cx.heap;
    if let Some((slot, _)) = heap.lookup_own(global, key) {
        if overwrite {
            heap.set_slot(global, slot, value);
        }
        return Ok(());
    }
    heap.add_property(global, key, PropertyFlags::data(true, true, false), value)
        .map_err(|error| error::heap_error(cx, error))
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

/// Runs the top frame until the entry frame returns.
///
/// `regs` is the running frame's window. It borrows the register file, so
/// every arm that calls into the heap copies its operands out first and
/// takes the window again afterwards (`set!`, `write!`, and the common tail).
fn run(cx: &mut Cx<'_>) -> Result<Value, Throw> {
    let store: &CodeStore = cx.code;
    let mut frame = *cx.heap.vm.frames.last().expect("an entry frame");
    let mut unit: &CodeUnit = store.get(frame.unit);
    let mut code: &[Insn] = &unit.code;
    let mut base = frame.base as usize;
    let mut strict = unit.has(flags::STRICT);
    let mut ic_base = unit.ic_base as usize;
    let mut pc = 0usize;

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
                Opcode::ToPropertyKey => {
                    let x = reg!(insn.b);
                    if x.is_int32() || x.as_string().is_some() || x.as_symbol().is_some() {
                        reg!(insn.a) = x;
                        continue 'dispatch;
                    }
                    write!('dispatch, insn.a, ops::to_property_key(cx, x).map(PropertyKey::to_value))
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
                    if let Some(value) =
                        ic::get_hit(heap, &cx.heap.cells, &cx.heap.values, entry, object)
                    {
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
                    let object = object.as_object().expect("a literal object");
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
                    }
                    let result = ops::to_property_key(cx, key)
                        .and_then(|key| property::get(cx, object, key));
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
                            _ => 0,
                        };
                        if (index as u32) < elements.len.min(length) {
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
                    let object = object.as_object().expect("a literal object");
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
                        let global = cx.realm.global();
                        let start = cx.heap.objects.get(global.index()).slots_start;
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
                        let global = cx.realm.global();
                        let start = cx.heap.objects.get(global.index()).slots_start;
                        cx.heap.values.set_at(start + entry.w1, value);
                        cx.heap.ics.count_hit(IcKind::Global);
                        continue 'dispatch;
                    }
                    set_global(cx, insn, value, strict, site)
                }
                Opcode::DefineGlobal => {
                    let value = reg!(insn.a);
                    define_global(cx, insn.key(), value, true)
                }
                Opcode::DeclareGlobal => define_global(cx, insn.key(), Value::UNDEFINED, false),
                Opcode::DeleteGlobal => {
                    let global = Value::object(cx.realm.global());
                    write!('dispatch, insn.a,
                        property::delete(cx, global, insn.key(), false).map(Value::bool)
                    )
                }

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
                | Opcode::LoadSlot
                | Opcode::StoreSlot
                | Opcode::NewArguments
                | Opcode::Rest
                | Opcode::LoadNewTarget
                | Opcode::NewRegExp
                | Opcode::ForInStart
                | Opcode::AppendSpread
                | Opcode::ThrowInternal => rare_op(cx, &frame, unit, insn),
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
                    let object =
                        cx.heap
                            .alloc_object(ObjectKind::Ordinary, Some(prototype), insn.imm);
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
                    let function = reg!(callee_register);
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
                                (target, window, this, argc) = rewritten;
                            }
                            Err(throw) => break 'op Err(throw),
                        }
                    }
                    match target {
                        Callee::Bytecode(object, bytecode) => {
                            let unit_id: CodeUnitId = cx.heap.bytecode_unit(bytecode);
                            let callee_unit = store.get(unit_id);
                            if construct {
                                if !callee_unit.has(flags::CONSTRUCTOR) {
                                    break 'op Err(error::type_error(cx, "not a constructor"));
                                }
                                let site =
                                    (insn.op == Opcode::New).then(|| ic_base + insn.imm as usize);
                                let slots = callee_unit.this_slots;
                                match construct_receiver(cx, object, slots, site) {
                                    Ok(receiver) => this = receiver,
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
                            if let Err(throw) = enter(cx, callee_unit, callee_frame) {
                                break 'op Err(throw);
                            }
                            load_frame!();
                            pc = 0;
                            regs = window!();
                            continue 'dispatch;
                        }
                        Callee::Native(native) if !construct || native.is_constructor() => {
                            let result =
                                call_native(cx, native, base + window, this, argc, construct);
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
                        Callee::Native(_) => Err(error::type_error(cx, "not a constructor")),
                        Callee::NotCallable => Err(error::type_error(
                            cx,
                            if construct {
                                "not a constructor"
                            } else {
                                "not a function"
                            },
                        )),
                    }
                }
                Opcode::Return | Opcode::ReturnUndefined => {
                    let value = if insn.op == Opcode::Return {
                        reg!(insn.a)
                    } else {
                        Value::UNDEFINED
                    };
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
    #[inline(never)]
    fn slow(self, cx: &mut Cx<'_>, x: Value, y: Value) -> Result<bool, Throw> {
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
        return Some(ops::number_op(op, x.as_f64()?, y.as_f64()?));
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
