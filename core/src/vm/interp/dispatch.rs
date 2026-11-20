{
    let entry = top_frame(&cx.heap.vm.frames);

    // The running unit: borrowed from the code store, or from `held`, which
    // keeps a dynamic unit alive while it runs.
    let mut held: Option<Rc<CodeUnit>> = None;
    let mut unit: &CodeUnit;
    macro_rules! set_unit {
        ($id:expr) => {{
            let id: CodeUnitId = $id;
            unit = if id.is_dynamic() | held.is_some() {
                switch_unit(cx.static_code, cx.heap, &mut held, id)
            } else {
                cx.static_code.get(id)
            };
        }};
    }
    macro_rules! stream {
        ($pc:expr) => {{
            let Some(stream) = Stream::enter(unit, cx.static_code) else {
                return Ok(DispatchExit::Switch($pc));
            };
            stream
        }};
    }
    set_unit!(entry.unit);
    let mut code = stream!(start);
    let mut base = entry.base as usize;
    let mut ic_base = unit.cache_base(cx.heap);
    let mut pc = start;

    // Keep the current PC at dispatch. Only a fallthrough needs its successor;
    // a taken jump or a frame switch supplies the next index itself.
    macro_rules! advance {
        ($label:lifetime) => {{
            pc += 1;
            continue $label;
        }};
    }
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
    // Frames remain published for GC and backtraces. Read their fields when
    // needed instead of keeping a second full activation live across dispatch.
    macro_rules! current_frame {
        () => {
            top_frame(&cx.heap.vm.frames)
        };
    }
    // Writes a value computed from the heap, then continues.
    macro_rules! set {
        ($label:lifetime, $register:expr, $value:expr) => {{
            let value: Value = $value;
            regs = window!();
            regs[$register as usize] = value;
            advance!($label);
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
    // Carry the operation from the opcode arm as a constant. A shared arm
    // first mapping Opcode to Arith leaves another hot jump-table dispatch.
    macro_rules! arith {
        ($label:lifetime, $insn:ident, $op:ident, $immediate:expr) => {{
            let $insn = *$insn;
            let x = reg!($insn.b);
            let y = if $immediate {
                Value::int32($insn.int())
            } else {
                reg!($insn.c)
            };
            if let Some(value) = arith_fast(Arith::$op, x, y) {
                reg!($insn.a) = value;
                advance!($label);
            }
            write!($label, $insn.a, arith_slow(cx, $insn.op, x, y))
        }};
    }

    // Every taken conditional back-edge polls too. Conversions finish before
    // polling, and an interrupt is attributed to the original branch PC.
    macro_rules! branch {
        ($dispatch:lifetime, $failure:lifetime, $target:expr) => {{
            let target: usize = $target;
            if target <= pc {
                let poll = &mut cx.heap.vm.poll;
                *poll -= 1;
                if *poll == 0 {
                    if let Err(throw) = error::poll_interrupts(cx) {
                        break $failure Err(throw);
                    }
                    pc = target;
                    if Stream::promote(unit, cx.static_code) {
                        return Ok(DispatchExit::Switch(pc));
                    }
                    regs = window!();
                    continue $dispatch;
                }
            }
            pc = target;
            continue $dispatch;
        }};
    }

    // Uncommon calls materialize the caller only for the helper's duration.
    macro_rules! called {
        ($dispatch:lifetime, $restore:lifetime, $insn:ident, $next_frame:ident => $result:expr) => {{
            match $result {
                Called::Entered(next) => {
                    unit = match next {
                        UnitRef::Static(next) => {
                            if held.is_some() {
                                clear_dynamic_unit(&mut held);
                            }
                            next
                        }
                        UnitRef::Dynamic(next) => hold_dynamic_unit(&mut held, next),
                    };
                    code = stream!(0);
                    base = $next_frame.base as usize;

                    ic_base = unit.cache_base(cx.heap);
                    pc = 0;
                    regs = window!();
                    continue $dispatch;
                }
                Called::Value(value) => set!($dispatch, $insn.a, value),
                Called::Threw(throw) => Err(throw),
                Called::Exit(value) => return Ok(DispatchExit::Returned(value)),
                Called::Resume(resume) => break $restore resume,
            }
        }};
    }

    'dispatch: loop {
        // Keep the dispatch descriptor borrowed; operand-heavy arms copy it
        // locally so their inputs can be decoded together.
        let fetched = code.fetch(pc);
        let insn: &Insn = core::borrow::Borrow::borrow(&fetched);
        let resume = 'restore: {
            let outcome: Result<(), Throw> = 'op: {
                match insn.op {
                    Opcode::Nop => advance!('dispatch),

                    // ---- moves and constants ----
                    Opcode::Mov => {
                        reg!(insn.a) = reg!(insn.b);
                        advance!('dispatch);
                    }
                    Opcode::Swap => {
                        regs.swap(insn.a as usize, insn.b as usize);
                        advance!('dispatch);
                    }
                    Opcode::LoadWide => {
                        let value = cx.heap.vm.registers[base + insn.imm as usize];
                        set!('dispatch, insn.a, value)
                    }
                    Opcode::StoreWide => {
                        let value = reg!(insn.a);
                        cx.heap.vm.registers[base + insn.imm as usize] = value;
                        regs = window!();
                        advance!('dispatch);
                    }
                    Opcode::LoadInt => {
                        reg!(insn.a) = Value::int32(insn.int());
                        advance!('dispatch);
                    }
                    Opcode::LoadConst => {
                        let bytecode = current_frame!().bytecode;
                        let constants = cx.heap.payloads.bytecode.get(bytecode.index());
                        debug_assert!(insn.imm < constants.constant_count);
                        reg!(insn.a) = cx.heap.values.get(constants.constants, insn.imm);
                        advance!('dispatch);
                    }
                    Opcode::LoadUndefined => {
                        reg!(insn.a) = Value::UNDEFINED;
                        advance!('dispatch);
                    }
                    Opcode::LoadNull => {
                        reg!(insn.a) = Value::NULL;
                        advance!('dispatch);
                    }
                    Opcode::LoadTrue => {
                        reg!(insn.a) = Value::TRUE;
                        advance!('dispatch);
                    }
                    Opcode::LoadFalse => {
                        reg!(insn.a) = Value::FALSE;
                        advance!('dispatch);
                    }
                    Opcode::LoadEmpty => {
                        reg!(insn.a) = Value::EMPTY;
                        advance!('dispatch);
                    }
                    Opcode::LoadFunction => {
                        reg!(insn.a) = Value::object(current_frame!().function);
                        advance!('dispatch);
                    }
                    Opcode::CheckInit => {
                        if !reg!(insn.a).is_empty() {
                            advance!('dispatch);
                        }
                        Err(local_binding_error(cx, *insn))
                    }
                    Opcode::ThrowConstAssign => Err(local_binding_error(cx, *insn)),

                    // ---- unary operators ----
                    Opcode::Inc => {
                        let insn = *insn;
                        let x = reg!(insn.b);
                        if let Some(i) = x.as_int32()
                            && let Some(value) = i.checked_add(1)
                        {
                            reg!(insn.a) = Value::int32(value);
                            advance!('dispatch);
                        }
                        write!('dispatch, insn.a, ops::unary(cx, Unary::Inc, x))
                    }
                    Opcode::IncGetElem => {
                        let insn = *insn;
                        let x = reg!(insn.b);
                        if let Some(i) = x.as_int32()
                            && let Some(value) = i.checked_add(1)
                        {
                            reg!(insn.a) = Value::int32(value);
                            if let Some(read) = code.peek(pc + 1)
                                && read.op == Opcode::GetElem
                                && let (Some(object), Some(index)) =
                                    (reg!(read.b).as_object(), reg!(read.c).as_int32())
                                && index >= 0
                            {
                                let header = cx.heap.objects.get(object.index());
                                let elements = header.elements();
                                if (index as u32) < elements.len
                                    && (header.kind != ObjectKind::Array
                                        || (index as u32) < header.payload)
                                {
                                    let element = cx.heap.values.get(elements, index as u32);
                                    if !element.is_empty() {
                                        pc += 1;
                                        reg!(read.a) = element;
                                        advance!('dispatch);
                                    }
                                }
                            }
                            advance!('dispatch);
                        }
                        write!('dispatch, insn.a, ops::unary(cx, Unary::Inc, x))
                    }
                    Opcode::Dec => {
                        let insn = *insn;
                        let x = reg!(insn.b);
                        if let Some(i) = x.as_int32()
                            && let Some(value) = i.checked_sub(1)
                        {
                            reg!(insn.a) = Value::int32(value);
                            advance!('dispatch);
                        }
                        write!('dispatch, insn.a, ops::unary(cx, Unary::Dec, x))
                    }
                    Opcode::Neg | Opcode::Plus | Opcode::ToNumeric | Opcode::BitNot => {
                        let x = reg!(insn.b);
                        if let Some(value) = unary_fast(insn.op, x) {
                            reg!(insn.a) = value;
                            advance!('dispatch);
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
                            advance!('dispatch);
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
                            advance!('dispatch);
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
                        advance!('dispatch);
                    }
                    Opcode::IsUndefined | Opcode::TypeOfIsUndefined => {
                        reg!(insn.a) = Value::bool(reg!(insn.b).is_undefined());
                        advance!('dispatch);
                    }
                    Opcode::IsNull => {
                        reg!(insn.a) = Value::bool(reg!(insn.b).is_null());
                        advance!('dispatch);
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
                            advance!('dispatch);
                        }
                        if let (Some(f), Some(g)) = (x.as_f64(), y.as_f64()) {
                            reg!(insn.a) = Value::number(f + g);
                            advance!('dispatch);
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
                            advance!('dispatch);
                        }
                        if let (Some(f), Some(g)) = (x.as_f64(), y.as_f64()) {
                            reg!(insn.a) = Value::number(f - g);
                            advance!('dispatch);
                        }
                        write!('dispatch, insn.a, ops::arith(cx, Arith::Sub, x, y))
                    }
                    Opcode::Mul | Opcode::MulI => {
                        arith!('dispatch, insn, Mul, insn.op == Opcode::MulI)
                    }
                    Opcode::Div => arith!('dispatch, insn, Div, false),
                    Opcode::Mod => arith!('dispatch, insn, Mod, false),
                    Opcode::Pow => arith!('dispatch, insn, Pow, false),
                    Opcode::Shl | Opcode::ShlI => {
                        arith!('dispatch, insn, Shl, insn.op == Opcode::ShlI)
                    }
                    Opcode::Sar | Opcode::SarI => {
                        arith!('dispatch, insn, Sar, insn.op == Opcode::SarI)
                    }
                    Opcode::Shr | Opcode::ShrI => {
                        arith!('dispatch, insn, Shr, insn.op == Opcode::ShrI)
                    }
                    Opcode::BitAnd | Opcode::BitAndI => {
                        arith!('dispatch, insn, BitAnd, insn.op == Opcode::BitAndI)
                    }
                    Opcode::BitOr | Opcode::BitOrI => {
                        arith!('dispatch, insn, BitOr, insn.op == Opcode::BitOrI)
                    }
                    Opcode::BitXor | Opcode::BitXorI => {
                        arith!('dispatch, insn, BitXor, insn.op == Opcode::BitXorI)
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
                            advance!('dispatch);
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
                    Opcode::JmpBack => {
                        let poll = &mut cx.heap.vm.poll;
                        *poll -= 1;
                        if *poll == 0 {
                            if let Err(throw) = error::poll_interrupts(cx) {
                                break 'op Err(throw);
                            }
                            pc = insn.imm as usize;
                            if Stream::promote(unit, cx.static_code) {
                                return Ok(DispatchExit::Switch(pc));
                            }
                            regs = window!();
                            continue 'dispatch;
                        }
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
                            branch!('dispatch, 'op, insn.imm as usize);
                        }
                        advance!('dispatch);
                    }
                    Opcode::JUndefinedOrNull => {
                        let x = reg!(insn.a);
                        if x.is_undefined() || x.is_null() {
                            branch!('dispatch, 'op, insn.imm as usize);
                        }
                        advance!('dispatch);
                    }
                    Opcode::Gosub => {
                        reg!(insn.a) = Value::int32((pc + 1) as i32);
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
                        let insn = *insn;
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
                            branch!('dispatch, 'op, insn.imm as usize);
                        }
                        advance!('dispatch);
                    }

                    // ---- properties ----
                    Opcode::GetField => {
                        let insn = *insn;
                        // The hit reads the heap's arenas as fields, disjoint from
                        // the register file, so the window stays.
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
                            advance!('dispatch);
                        }
                        let fill = insn.c != FIELD_NO_SITE;
                        write!('dispatch, insn.a, property::get_cached(cx, object, insn.key(), site, fill))
                    }
                    Opcode::SetField => {
                        let insn = *insn;
                        // The arenas are borrowed as in `GetField`.
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
                            advance!('dispatch);
                        }
                        let fill = insn.c != FIELD_NO_SITE;
                        property::set_cached(
                            cx,
                            object,
                            insn.key(),
                            value,
                            unit.has(flags::STRICT),
                            site,
                            fill,
                        )
                    }
                    Opcode::DefineField => {
                        let (object, value) = (reg!(insn.a), reg!(insn.b));
                        let object = object.as_object().expect("an object");
                        property::define(cx, object, insn.key(), value)
                    }
                    Opcode::DeleteField => {
                        let object = reg!(insn.b);
                        write!('dispatch, insn.a,
                            property::delete(cx, object, insn.key(), unit.has(flags::STRICT)).map(Value::bool)
                        )
                    }
                    Opcode::GetElem => {
                        let insn = *insn;
                        let (object, key) = (reg!(insn.b), reg!(insn.c));
                        if let (Some(target), Some(index)) = (object.as_object(), key.as_int32())
                            && index >= 0
                        {
                            let header = cx.heap.objects.get(target.index());
                            let elements = header.elements();
                            if (index as u32) < elements.len
                                && (header.kind != ObjectKind::Array
                                    || (index as u32) < header.payload)
                            {
                                let element = cx.heap.values.get(elements, index as u32);
                                if !element.is_empty() {
                                    reg!(insn.a) = element;
                                    advance!('dispatch);
                                }
                            }
                            // A typed array's element.
                            if header.kind == ObjectKind::TypedArray
                                && let Some(element) = typed::fast_get(
                                    &cx.heap.objects,
                                    &cx.heap.payloads.internals,
                                    target,
                                    index as u32,
                                )
                            {
                                reg!(insn.a) = element;
                                advance!('dispatch);
                            }
                        }
                        let result = property::get_computed(cx, object, key);
                        write!('dispatch, insn.a, result)
                    }
                    Opcode::SetElem => {
                        let insn = *insn;
                        let (object, key, value) = (reg!(insn.a), reg!(insn.b), reg!(insn.c));
                        if let (Some(target), Some(index)) = (object.as_object(), key.as_int32())
                            && index >= 0
                        {
                            let header = cx.heap.objects.get(target.index());
                            let elements = header.elements();
                            // An `arguments` object's mapped indices go through
                            // its cells: only arrays and ordinary objects take the
                            // dense write.
                            if header.kind == ObjectKind::TypedArray {
                                if typed::fast_set(
                                    &cx.heap.objects,
                                    &mut cx.heap.payloads.internals,
                                    target,
                                    index as u32,
                                    value,
                                ) {
                                    advance!('dispatch);
                                }
                            } else {
                                let length = match header.kind {
                                    ObjectKind::Array => header.payload,
                                    ObjectKind::Ordinary => elements.len,
                                    _ => 0,
                                };
                                // A hole defers to the chain unless no prototype
                                // can intercept it.
                                if (index as u32) < elements.len.min(length)
                                    && (cx.heap.holes_writable
                                        || !cx.heap.values.get(elements, index as u32).is_empty())
                                {
                                    cx.heap.values.set(elements, index as u32, value);
                                    advance!('dispatch);
                                }
                            }
                        }
                        ops::to_property_key(cx, key).and_then(|key| {
                            property::set(cx, object, key, value, unit.has(flags::STRICT))
                        })
                    }
                    Opcode::DefineElem => {
                        let (object, key, value) = (reg!(insn.a), reg!(insn.b), reg!(insn.c));
                        let object = object.as_object().expect("an object");
                        ops::to_property_key(cx, key)
                            .and_then(|key| property::define(cx, object, key, value))
                    }
                    Opcode::DeleteElem => {
                        let (object, key) = (reg!(insn.b), reg!(insn.c));
                        let result = ops::to_property_key(cx, key).and_then(|key| {
                            property::delete(cx, object, key, unit.has(flags::STRICT))
                        });
                        write!('dispatch, insn.a, result.map(Value::bool))
                    }

                    // ---- globals ----
                    Opcode::GetGlobal | Opcode::GetGlobalOrUndefined => {
                        // One generation compare and a load.
                        let site = ic_base + global_site(*insn);
                        let entry = cx.heap.ics.entries[site];
                        if entry.w0 == cx.heap.global_generation {
                            let start = cx.heap.objects.get(entry.w3).slots_start;
                            reg!(insn.a) = cx.heap.values.get_at(start + entry.w1);
                            cx.heap.ics.count_hit(IcKind::Global);
                            advance!('dispatch);
                        }
                        write!('dispatch, insn.a, get_global(cx, *insn, site))
                    }
                    Opcode::SetGlobal => {
                        let value = reg!(insn.a);
                        let site = ic_base + global_site(*insn);
                        let entry = cx.heap.ics.entries[site];
                        if entry.w0 == cx.heap.global_generation
                            && entry.w2 & u32::from(PropertyFlags::WRITABLE) != 0
                        {
                            let start = cx.heap.objects.get(entry.w3).slots_start;
                            cx.heap.values.set_at(start + entry.w1, value);
                            cx.heap.ics.count_hit(IcKind::Global);
                            advance!('dispatch);
                        }
                        set_global(cx, *insn, value, unit.has(flags::STRICT), site)
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
                    | Opcode::CheckGlobalLexical => declare_global(cx, *insn),
                    Opcode::DeleteGlobal => {
                        write!('dispatch, insn.a, delete_global(cx, insn.key()))
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
                        let slots = cx.heap.objects.get(cell.index()).slots();
                        reg!(insn.a) = cx.heap.values.get(slots, 0);
                        advance!('dispatch);
                    }
                    Opcode::StoreCell => {
                        let (cell, value) = (cell(reg!(insn.a)), reg!(insn.b));
                        let slots = cx.heap.objects.get(cell.index()).slots();
                        cx.heap.values.set(slots, 0, value);
                        advance!('dispatch);
                    }
                    Opcode::CheckCellInit => {
                        let cell = cell(reg!(insn.a));
                        let slots = cx.heap.objects.get(cell.index()).slots();
                        if !cx.heap.values.get(slots, 0).is_empty() {
                            advance!('dispatch);
                        }
                        Err(local_binding_error(cx, *insn))
                    }
                    Opcode::LoadCaptured => {
                        let cell = captured_cell(
                            &cx.heap.objects,
                            &cx.heap.payloads.functions,
                            &cx.heap.values,
                            current_frame!().function,
                            insn.imm,
                        );
                        let slots = cx.heap.objects.get(cell.index()).slots();
                        reg!(insn.a) = cx.heap.values.get(slots, 0);
                        advance!('dispatch);
                    }
                    Opcode::StoreCaptured => {
                        let value = reg!(insn.a);
                        let cell = captured_cell(
                            &cx.heap.objects,
                            &cx.heap.payloads.functions,
                            &cx.heap.values,
                            current_frame!().function,
                            insn.imm,
                        );
                        let slots = cx.heap.objects.get(cell.index()).slots();
                        cx.heap.values.set(slots, 0, value);
                        advance!('dispatch);
                    }

                    // ---- rare forms, out of line ----
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
                    | Opcode::NewArrayK
                    | Opcode::NewObjectLit
                    | Opcode::InitField
                    | Opcode::FinishObjectLit
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
                    | Opcode::ImportCall
                    | Opcode::ConcatN => {
                        let frame = current_frame!();
                        rare_op(cx, &frame, unit, *insn)
                    }
                    Opcode::IterStart
                    | Opcode::IterNext
                    | Opcode::IterClose
                    | Opcode::IterCloseThrow
                    | Opcode::IterCallNext
                    | Opcode::IterCall
                    | Opcode::IterCheckObject
                    | Opcode::IterStartAsync
                    | Opcode::IterNextRaw
                    | Opcode::IterResult => {
                        let frame = current_frame!();
                        super::iter::iter_op(cx, &frame, *insn)
                    }
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
                    | Opcode::PutRef => {
                        let frame = current_frame!();
                        super::class::class_op(cx, &frame, unit, *insn)
                    }
                    Opcode::ForInNext => {
                        let iterator = reg!(insn.b);
                        match property::for_in_next(cx, iterator) {
                            Ok(Some(key)) => {
                                pc = insn.imm as usize;
                                regs = window!();
                                reg!(insn.a) = key;
                                continue 'dispatch;
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
                        let object =
                            cx.heap
                                .alloc_object(ObjectKind::Ordinary, Some(prototype), slots);
                        write!('dispatch, insn.a, object.map(Value::object).map_err(|error| error::heap_error(cx, error)))
                    }
                    Opcode::NewArray => write!('dispatch, insn.a, new_array(cx, base, *insn)),
                    Opcode::MakeClosure => {
                        let frame = current_frame!();
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
                        let mut next_frame;
                        let result = if matches!(insn.op, Opcode::Call | Opcode::CallMethod) {
                            let b = insn.b as usize;
                            let method = insn.op == Opcode::CallMethod;
                            let at = b + usize::from(method);
                            let function = reg!(at);
                            let this = if method { reg!(b) } else { Value::UNDEFINED };
                            let target = if let Some(object) = function.as_object()
                                && cx.heap.object_kind(object) == ObjectKind::Function
                            {
                                let record = cx.heap.function_record(object);
                                if record.bytecode != NONE {
                                    let bytecode = BytecodeId(record.bytecode);
                                    let id = cx.heap.bytecode_unit(bytecode);
                                    if !id.is_dynamic() {
                                        let (next, plan) = cx.static_code.call_entry(id);
                                        let next_base = base + at;
                                        if !matches!(
                                            plan,
                                            crate::bytecode::call_plan::CallPlan::None
                                        ) {
                                            let value = call_plan::value(
                                                cx,
                                                next,
                                                plan,
                                                next_base,
                                                insn.c as usize,
                                                this,
                                                bytecode,
                                            );
                                            if !value.is_empty() {
                                                set!('dispatch, insn.a, value)
                                            }
                                        }
                                        match prepare_frame(
                                            cx,
                                            next,
                                            next_base,
                                            insn.c as usize,
                                            this,
                                            object,
                                        ) {
                                            Ok(end) => {
                                                let next_frame = Frame {
                                                    unit: id,
                                                    function: object,
                                                    bytecode,
                                                    base: (base + at) as u32,
                                                    return_pc: (pc + 1) as u32,
                                                    return_dst: (base + insn.a as usize) as u32,
                                                    argc: u32::from(insn.c),
                                                    flags: 0,
                                                };
                                                cx.heap.vm.frames.push(next_frame);
                                                cx.heap.vm.top = end;
                                                if held.is_some() {
                                                    clear_dynamic_unit(&mut held);
                                                }
                                                unit = next;
                                                code = stream!(0);
                                                base = next_frame.base as usize;

                                                ic_base = unit.cache_base(cx.heap);
                                                pc = 0;
                                                regs = window!();
                                                continue 'dispatch;
                                            }
                                            Err(throw) => {
                                                match entry_failed(cx, next, throw, false) {
                                                    Ok(value) => set!('dispatch, insn.a, value),
                                                    Err(throw) => break 'op Err(throw),
                                                }
                                            }
                                        }
                                    }
                                    Callee::Bytecode(object, bytecode)
                                } else {
                                    match Native::from_index(record.native) {
                                        Some(native) => Callee::Native(native),
                                        None => Callee::NotCallable,
                                    }
                                }
                            } else {
                                Callee::NotCallable
                            };
                            next_frame = current_frame!();
                            invoke::<false, false>(
                                cx,
                                &mut next_frame,
                                unit,
                                function,
                                target,
                                at,
                                this,
                                insn.c as usize,
                                pc + 1,
                                insn.a,
                                None,
                            )
                        } else {
                            next_frame = current_frame!();
                            special_call(cx, &mut next_frame, unit, *insn, pc + 1)
                        };
                        called!('dispatch, 'restore, insn, next_frame => result)
                    }
                    Opcode::Return | Opcode::ReturnUndefined => {
                        let value = if insn.op == Opcode::Return {
                            reg!(insn.a)
                        } else {
                            Value::UNDEFINED
                        };
                        let frame = cx.heap.vm.frames.pop().expect("a returning frame");
                        // A plain call's frame (not an entry, a construction or
                        // a tail call) returns to the frame below, loaded once;
                        // the backtrace's `VIA_*` marks do not matter here.
                        if frame.flags
                            & (frame_flags::ENTRY | frame_flags::CONSTRUCT | frame_flags::TAIL)
                            == 0
                        {
                            break 'restore Resume {
                                pc: frame.return_pc,
                                dst: frame.return_dst,
                                value,
                            };
                        }
                        match finish_popped_return(cx, frame, value) {
                            Returned::Exit(value) => return Ok(DispatchExit::Returned(value)),
                            Returned::Resume(resume) => break 'restore resume,
                        }
                    }
                    Opcode::Throw => Err(Throw(reg!(insn.a))),

                    // ---- suspension, out of line ----
                    Opcode::InitialYield
                    | Opcode::AsyncStart
                    | Opcode::Yield
                    | Opcode::YieldStar
                    | Opcode::Await => {
                        let frame = current_frame!();
                        match suspend_op(cx, &frame, unit, *insn, pc + 1) {
                            Ok(Returned::Exit(value)) => return Ok(DispatchExit::Returned(value)),
                            Ok(Returned::Resume(resume)) => break 'restore resume,
                            Err(throw) => Err(throw),
                        }
                    }
                }
            };

            // A slow path finished. Advance on success, or unwind its throw.
            let Err(Throw(thrown)) = outcome else {
                regs = window!();
                advance!('dispatch);
            };
            // Place the fault before unwinding removes any backtrace frames.
            error::place_frame(cx, thrown, unit, pc as u32);
            if cx.heap.vm.interrupted {
                return Err(unwind_uncatchable(cx, thrown));
            }
            break 'restore unwind_catchable(cx, thrown, pc)?;
        };
        let caller = current_frame!();
        set_unit!(caller.unit);
        base = caller.base as usize;
        raise_top(cx, frame_end(&caller, unit));
        cx.heap.vm.registers[resume.dst as usize] = resume.value;
        pc = resume.pc as usize;
        code = stream!(pc);
        ic_base = unit.cache_base(cx.heap);
        regs = window!();
    }
}
