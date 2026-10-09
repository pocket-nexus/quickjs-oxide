//! Specialized dispatch loop for the closed conditional-branch loop shape.
//!
//! This loop mirrors the fast paths of the corresponding generic arms in
//! `execute.rs` operation for operation, with the slot slice, operand depth,
//! and region boundaries held in locals instead of re-derived through
//! `FrameSlots` helpers on every instruction. Every opcode outside the fast
//! set, every guard miss, and every step that can fail or observe state
//! synchronizes the hot locals back into the frame (`window.depth`, cursor
//! `fault`/`resume`) and returns `Fallback`; the generic dispatch loop then
//! re-executes the same instruction from `pc`, preserving its exact error,
//! action, and profiling behavior. See
//! docs/performance/runtime-item5-5e2-design.md.
//!
//! Profiling parity: dispatch/outcome records fire only on completed fast
//! hits, positioned after the operation. A guard miss records nothing here
//! and falls back, where the generic arm records its own dispatch/outcome —
//! so per-site visit and outcome totals match the all-generic build. The
//! only divergent case is an invariant-level error mid-hit (e.g. operand
//! capacity), where the generic arm may have recorded a dispatch first;
//! such errors are engine-internal and off every benchmark path.

use super::{
    FrameCursor, binary_number_result, compare_direct_numbers, is_immediate, number_value,
    published_u16,
};
use crate::engine::api::Runtime;
use crate::engine::code::exec::{OPCODE_MASK, WIDE_FIRST, WIDTH_SHIFT};
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{BytecodeConstant, RawValue};
use crate::engine::value::JsValue;
use crate::engine::value::number::operations::Number;
use crate::engine::vm::bindings::FrameBinding;
use crate::engine::vm::stack::SlotStore;
use crate::engine::api::error::Error;

const NOP: u16 = Opcode::Nop as u16;
const PUSH_I32: u16 = Opcode::PushI32 as u16;
const PUSH_CONST: u16 = Opcode::PushConst as u16;
const UNDEFINED: u16 = Opcode::Undefined as u16;
const NULL: u16 = Opcode::Null as u16;
const PUSH_FALSE: u16 = Opcode::PushFalse as u16;
const PUSH_TRUE: u16 = Opcode::PushTrue as u16;
const GET_LOCAL: u16 = Opcode::GetLocal as u16;
const PUT_LOCAL: u16 = Opcode::PutLocal as u16;
const SET_LOCAL: u16 = Opcode::SetLocal as u16;
const GET_LOCAL_CHECK: u16 = Opcode::GetLocalCheck as u16;
const PUT_LOCAL_CHECK: u16 = Opcode::PutLocalCheck as u16;
const SET_LOCAL_CHECK: u16 = Opcode::SetLocalCheck as u16;
const GET_ARG: u16 = Opcode::GetArg as u16;
const DROP: u16 = Opcode::Drop as u16;
const NIP: u16 = Opcode::Nip as u16;
const NEG: u16 = Opcode::Neg as u16;
const PLUS: u16 = Opcode::Plus as u16;
const DEC: u16 = Opcode::Dec as u16;
const INC: u16 = Opcode::Inc as u16;
const POST_DEC: u16 = Opcode::PostDec as u16;
const POST_INC: u16 = Opcode::PostInc as u16;
const BIT_NOT: u16 = Opcode::BitNot as u16;
const NOT: u16 = Opcode::Not as u16;
const ADD: u16 = Opcode::Add as u16;
const SUB: u16 = Opcode::Sub as u16;
const MUL: u16 = Opcode::Mul as u16;
const DIV: u16 = Opcode::Div as u16;
const MOD: u16 = Opcode::Mod as u16;
const POW: u16 = Opcode::Pow as u16;
const SHL: u16 = Opcode::Shl as u16;
const SAR: u16 = Opcode::Sar as u16;
const SHR: u16 = Opcode::Shr as u16;
const BIT_AND: u16 = Opcode::BitAnd as u16;
const BIT_XOR: u16 = Opcode::BitXor as u16;
const BIT_OR: u16 = Opcode::BitOr as u16;
const EQ: u16 = Opcode::Eq as u16;
const NEQ: u16 = Opcode::Neq as u16;
const LT: u16 = Opcode::Lt as u16;
const LTE: u16 = Opcode::Lte as u16;
const GT: u16 = Opcode::Gt as u16;
const GTE: u16 = Opcode::Gte as u16;
const STRICT_EQ: u16 = Opcode::StrictEq as u16;
const STRICT_NEQ: u16 = Opcode::StrictNeq as u16;
const IF_FALSE: u16 = Opcode::IfFalse as u16;
const IF_TRUE: u16 = Opcode::IfTrue as u16;
const GOTO: u16 = Opcode::Goto as u16;
const MARK_SUPER_CALL: u16 = Opcode::MarkSuperCall as u16;
const NUMBER_LOCAL_INC: u16 = Opcode::NumberLocalInc as u16;
const NUMBER_ARG_INC: u16 = Opcode::NumberArgInc as u16;
const COMPARE_BRANCH_LOCAL: u16 = Opcode::CompareBranchLocal as u16;
const COMPARE_BRANCH_ARG: u16 = Opcode::CompareBranchArg as u16;
const COMPARE_BRANCH_STACK: u16 = Opcode::CompareBranchStack as u16;
const UPDATE_LOCAL_DISCARD: u16 = Opcode::UpdateLocalDiscard as u16;
const UPDATE_LOCAL_DISCARD_CHECK: u16 = Opcode::UpdateLocalDiscardCheck as u16;
const COMPARE_BRANCH_LOCAL_LT: u16 = Opcode::CompareBranchLocalLt as u16;
const COMPARE_BRANCH_ARG_LT: u16 = Opcode::CompareBranchArgLt as u16;

#[inline(always)]
fn is_fast_raw(raw: u16) -> bool {
    matches!(
        raw,
        NOP | PUSH_I32
            | PUSH_CONST
            | UNDEFINED
            | NULL
            | PUSH_FALSE
            | PUSH_TRUE
            | GET_LOCAL
            | PUT_LOCAL
            | SET_LOCAL
            | GET_LOCAL_CHECK
            | PUT_LOCAL_CHECK
            | SET_LOCAL_CHECK
            | GET_ARG
            | DROP
            | NIP
            | NEG
            | PLUS
            | DEC
            | INC
            | POST_DEC
            | POST_INC
            | BIT_NOT
            | NOT
            | ADD
            | SUB
            | MUL
            | DIV
            | MOD
            | POW
            | SHL
            | SAR
            | SHR
            | BIT_AND
            | BIT_XOR
            | BIT_OR
            | EQ
            | NEQ
            | LT
            | LTE
            | GT
            | GTE
            | STRICT_EQ
            | STRICT_NEQ
            | IF_FALSE
            | IF_TRUE
            | GOTO
            | MARK_SUPER_CALL
            | NUMBER_LOCAL_INC
            | NUMBER_ARG_INC
            | COMPARE_BRANCH_LOCAL
            | COMPARE_BRANCH_ARG
            | COMPARE_BRANCH_STACK
            | UPDATE_LOCAL_DISCARD
            | UPDATE_LOCAL_DISCARD_CHECK
            | COMPARE_BRANCH_LOCAL_LT
            | COMPARE_BRANCH_ARG_LT
    )
}

/// Cheap entry filter: one word load and the fast-set bit test, run at the
/// top of the generic dispatch loop before paying the call.
#[inline(always)]
pub(super) fn is_fast_word(executable: &PublishedFunctionSnapshot, pc: usize) -> bool {
    executable
        .exec
        .published_words()
        .get(pc)
        .is_some_and(|word: &std::cell::Cell<u32>| {
            is_fast_raw((word.get() >> 16) as u16 & OPCODE_MASK)
        })
}

/// Why the specialized loop handed control back to the generic dispatch loop.
pub(super) enum SpecializedExit {
    /// The instruction at the returned state needs the generic loop. Cursor
    /// and window are synchronized: `fault == resume == pc` of that
    /// instruction and `window.depth` is published, so the generic loop's
    /// `begin()` reproduces the exact pre-instruction state.
    Fallback,
}

#[cold]
#[inline(never)]
fn operand_stack_underflow() -> Error {
    Error::internal("owned operand stack underflow")
}

#[cold]
#[inline(never)]
fn operand_slot_not_a_value() -> Error {
    Error::internal("owned operand slot is not a value")
}

#[cold]
#[inline(never)]
fn operand_stack_capacity_exceeded() -> Error {
    Error::internal("owned operand stack exceeds verified capacity")
}

#[cold]
#[inline(never)]
fn operand_push_replaces_live_value() -> Error {
    Error::internal("owned operand push would replace a live value")
}

#[cold]
#[inline(never)]
fn local_index_out_of_bounds() -> Error {
    Error::internal("owned local index is out of bounds")
}


#[cold]
#[inline(never)]
fn invalid_word() -> Error {
    Error::internal("published execution word is invalid")
}

/// Install one operand, mirroring `SlotStore::push_current`: the same two
/// invariant checks in the same order, then the slot write and depth bump.
#[inline(always)]
fn hot_push(
    store: &mut SlotStore,
    operands_start: usize,
    depth: &mut usize,
    operand_room: usize,
    value: JsValue,
) -> Result<(), Error> {
    if *depth >= operand_room {
        return Err(operand_stack_capacity_exceeded());
    }
    let index = operands_start + *depth;
    if store.hot_slots()[index].is_some() {
        return Err(operand_push_replaces_live_value());
    }
    store.hot_slots()[index] = Some(FrameBinding::Direct(value));
    *depth += 1;
    #[cfg(feature = "profiling")]
    store.record_hot_install();
    Ok(())
}

/// A scalar copy from a direct binding, like `FrameSlots::push_direct_immediate`.
#[inline(always)]
fn scalar_copy(value: &JsValue) -> Option<JsValue> {
    match value {
        JsValue::Undefined => Some(JsValue::Undefined),
        JsValue::Null => Some(JsValue::Null),
        JsValue::Bool(value) => Some(JsValue::Bool(*value)),
        JsValue::Int(value) => Some(JsValue::Int(*value)),
        JsValue::Float(value) => Some(JsValue::Float(*value)),
        JsValue::ShortBigInt(value) => Some(JsValue::ShortBigInt(*value)),
        _ => None,
    }
}

/// Run the conditional-branch loop shape until an instruction needs the
/// generic dispatch loop. Hot state stays in locals; every exit publishes
/// it back (see the module docs).
#[inline(never)]
pub(super) fn conditional_shape_loop(
    cursor: &mut FrameCursor<'_>,
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
) -> Result<SpecializedExit, Error> {
    #[cfg(not(feature = "profiling"))]
    let _ = runtime;
    let words = executable.exec.published_words();
    let window = &mut *cursor.transaction.window;
    let store = &mut *cursor.transaction.store;
    let (params_start, locals_start, operands_start, operand_room) = window.hot_geometry();
    let locals_len = operands_start - locals_start;
    let params_len = locals_start - params_start;
    let mut depth = window.hot_depth();
    let mut pc = cursor.resume;

    macro_rules! sync {
        () => {{
            window.set_hot_depth(depth);
            cursor.fault = pc;
            cursor.resume = pc;
        }};
    }
    macro_rules! fallback {
        () => {{
            sync!();
            return Ok(SpecializedExit::Fallback);
        }};
    }
    macro_rules! fail {
        ($error:expr) => {{
            sync!();
            return Err($error);
        }};
    }
    macro_rules! push_operand {
        ($value:expr) => {
            if let Err(error) = hot_push(store, operands_start, &mut depth, operand_room, $value) {
                fail!(error);
            }
        };
    }

    loop {
        // Direct indexing mirrors `PublishedDecoded`: publish-time
        // verification guarantees the word at every visited pc.
        let Some(word) = words.get(pc) else {
            fail!(invalid_word());
        };
        let word: u32 = word.get();
        let header = (word >> 16) as u16;
        let raw = header & OPCODE_MASK;
        if !is_fast_raw(raw) {
            fallback!();
        }
        let first_wide = header & WIDE_FIRST != 0;
        let next_pc = pc + usize::from((header >> WIDTH_SHIFT) & 3) + 1;
        let operand = if first_wide { words[pc + 1].get() } else { word & 0xffff };
        // Extension operands follow the same convention as
        // `PublishedDecoded::operand` for index >= 1.
        let ext = |index: usize| words[pc + index + usize::from(first_wide)].get();

        match raw {
            NOP | MARK_SUPER_CALL => {}
            GOTO => {
                pc = operand as usize;
                continue;
            }
            PUSH_I32 => {
                let value = if next_pc == pc + 1 {
                    i32::from(operand as u16 as i16)
                } else {
                    operand as i32
                };
                push_operand!(JsValue::Int(value));
            }
            UNDEFINED => push_operand!(JsValue::Undefined),
            NULL => push_operand!(JsValue::Null),
            PUSH_FALSE => push_operand!(JsValue::Bool(false)),
            PUSH_TRUE => push_operand!(JsValue::Bool(true)),
            PUSH_CONST => {
                let value = match executable.constant(operand) {
                    Some(BytecodeConstant::Value(RawValue::Int(value))) => JsValue::Int(*value),
                    Some(BytecodeConstant::Value(RawValue::Float(value))) => JsValue::Float(*value),
                    Some(BytecodeConstant::Value(RawValue::Undefined)) => JsValue::Undefined,
                    Some(BytecodeConstant::Value(RawValue::Null)) => JsValue::Null,
                    Some(BytecodeConstant::Value(RawValue::Bool(value))) => JsValue::Bool(*value),
                    Some(BytecodeConstant::Value(RawValue::ShortBigInt(value))) => {
                        JsValue::ShortBigInt(*value)
                    }
                    // String/BigInt constants and missing entries take the
                    // generic loop's Pure(Constant) action path.
                    _ => fallback!(),
                };
                push_operand!(value);
            }
            GET_LOCAL | GET_LOCAL_CHECK | GET_ARG => {
                let index = usize::from(published_u16(operand));
                let (base, len) = if raw == GET_ARG {
                    (params_start, params_len)
                } else {
                    (locals_start, locals_len)
                };
                let value = if index < len {
                    match store.hot_slots().get(base + index) {
                        Some(Some(FrameBinding::Direct(value))) => match scalar_copy(value) {
                            Some(value) => value,
                            None => fallback!(),
                        },
                        _ => fallback!(),
                    }
                } else {
                    fallback!()
                };
                push_operand!(value);
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_execution_dispatch(
                    runtime, executable, pc, false,
                );
            }
            PUT_LOCAL | SET_LOCAL | PUT_LOCAL_CHECK | SET_LOCAL_CHECK => {
                let keep = raw == SET_LOCAL || raw == SET_LOCAL_CHECK;
                let index = usize::from(published_u16(operand));
                if index >= locals_len {
                    fail!(local_index_out_of_bounds());
                }
                let slot = &store.hot_slots()[locals_start + index];
                let binding_ok = matches!(
                    slot,
                    Some(FrameBinding::Direct(value)) if value.as_number_repr().is_some()
                );
                if !binding_ok {
                    // Captured, uninitialized, and private bindings take the
                    // generic loop's action paths; non-number direct values
                    // take its general store path.
                    fallback!();
                }
                // store_proven_number_operand: both the destination and the
                // top operand are verified numbers; any deviation hands the
                // whole instruction back to the generic path.
                let Some(top) = depth.checked_sub(1) else {
                    fallback!()
                };
                let top_index = operands_start + top;
                let value = match &store.hot_slots()[top_index] {
                    Some(FrameBinding::Direct(value)) => match value.as_number_repr() {
                        Some(value) => value,
                        None => fallback!(),
                    },
                    _ => fallback!(),
                };
                store.hot_slots()[locals_start + index] =
                    Some(FrameBinding::Direct(value.into()));
                if !keep {
                    store.hot_slots()[top_index] = None;
                    depth -= 1;
                    #[cfg(feature = "profiling")]
                    {
                        store.adjust_hot_live(-1);
                        crate::engine::api::profiling::record_owned_storage(
                            crate::engine::api::profiling::OwnedStorageEvent::Clear(1),
                        );
                    }
                }
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_owned_execution_event(
                        "ordinary_store.complete_scalar",
                    );
                    crate::engine::api::profiling::record_owned_execution_event(
                        "local_completion.store",
                    );
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(1),
                    );
                }
            }
            UPDATE_LOCAL_DISCARD | UPDATE_LOCAL_DISCARD_CHECK => {
                let index = usize::from(published_u16(operand & 0x1fff));
                let descriptor = operand >> 13;
                let needed = if descriptor & 2 != 0 { 2 } else { 1 };
                if !depth.checked_add(needed).is_some_and(|d| d <= operand_room) {
                    fallback!();
                }
                let slot_index = locals_start + index;
                let old = match store.hot_slots().get(slot_index) {
                    Some(Some(FrameBinding::Direct(value))) => match value.as_number_repr() {
                        Some(value) => value,
                        None => fallback!(),
                    },
                    _ => fallback!(),
                };
                store.hot_slots()[slot_index] =
                    Some(FrameBinding::Direct(number_value(old.update(descriptor & 1 != 0))));
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_execution_dispatch(
                        runtime, executable, pc, true,
                    );
                    crate::engine::api::profiling::record_execution_outcome(
                        runtime,
                        executable,
                        pc,
                        "update_local_discard",
                        None,
                    );
                }
                pc = next_pc + if descriptor & 4 != 0 { 3 } else { 2 };
                continue;
            }
            NUMBER_LOCAL_INC | NUMBER_ARG_INC => {
                let index = usize::from(published_u16(operand));
                let (base, len) = if raw == NUMBER_ARG_INC {
                    (params_start, params_len)
                } else {
                    (locals_start, locals_len)
                };
                if index >= len {
                    fallback!();
                }
                let value = match store.hot_slots().get(base + index) {
                    Some(Some(FrameBinding::Direct(value))) => match value.as_number_repr() {
                        Some(value) => value,
                        None => fallback!(),
                    },
                    _ => fallback!(),
                };
                if !depth.checked_add(2).is_some_and(|d| d <= operand_room) {
                    fallback!();
                }
                let value = number_value(value.add(Number::Int(1)));
                push_operand!(value);
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_execution_dispatch(
                        runtime, executable, pc, true,
                    );
                    crate::engine::api::profiling::record_execution_outcome(
                        runtime,
                        executable,
                        pc,
                        if raw == NUMBER_ARG_INC {
                            "number_arg_inc"
                        } else {
                            "number_local_inc"
                        },
                        None,
                    );
                }
                pc = next_pc + 2;
                continue;
            }
            ADD | SUB | MUL | DIV | MOD | POW | SHL | SAR | SHR | BIT_AND | BIT_XOR | BIT_OR
            | EQ | NEQ | LT | LTE | GT | GTE | STRICT_EQ | STRICT_NEQ => {
                // binary_number_current: authenticate the pair, operate on
                // two numbers in place, drop the upper slot.
                let Some(offset) = depth.checked_sub(2) else {
                    fail!(operand_stack_underflow());
                };
                let index = operands_start + offset;
                let (left, right) = match &store.hot_slots()[index..index + 2] {
                    [Some(FrameBinding::Direct(left)), Some(FrameBinding::Direct(right))] => {
                        match (left.as_number_repr(), right.as_number_repr()) {
                            (Some(left), Some(right)) => (left, right),
                            // Nullish equality, strict comparison, and the
                            // numeric driver all live in the generic arm.
                            _ => fallback!(),
                        }
                    }
                    _ => fail!(operand_slot_not_a_value()),
                };
                let opcode = Opcode::from_raw(raw).expect("fast set is published");
                let result = binary_number_result(opcode, left, right);
                let slots = store.hot_slots();
                slots[index] = Some(FrameBinding::Direct(result));
                slots[index + 1] = None;
                depth -= 1;
                #[cfg(feature = "profiling")]
                {
                    store.adjust_hot_live(-1);
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(1),
                    );
                    crate::engine::api::profiling::record_owned_execution_event(
                        "binary_number_in_place",
                    );
                }
            }
            NEG | PLUS | DEC | INC | POST_DEC | POST_INC | BIT_NOT => {
                let Some(top) = depth.checked_sub(1) else {
                    fail!(operand_stack_underflow());
                };
                let top_index = operands_start + top;
                let old = match &store.hot_slots()[top_index] {
                    Some(FrameBinding::Direct(value)) => match value.as_number_repr() {
                        Some(value) => value,
                        None => fallback!(),
                    },
                    _ => fail!(operand_slot_not_a_value()),
                };
                let is_post = raw == POST_INC || raw == POST_DEC;
                if is_post && !depth.checked_add(1).is_some_and(|d| d <= operand_room) {
                    fallback!();
                }
                let value = match raw {
                    NEG => old.negate(),
                    PLUS => old,
                    BIT_NOT => Number::Int(!old.int32()),
                    _ => old.update(raw == INC || raw == POST_INC),
                };
                if is_post {
                    push_operand!(number_value(value));
                } else {
                    // The generic arm pops and repushes; both effects are
                    // net-zero on depth but keep the storage records.
                    store.hot_slots()[top_index] = None;
                    depth -= 1;
                    #[cfg(feature = "profiling")]
                    {
                        store.adjust_hot_live(-1);
                        crate::engine::api::profiling::record_owned_storage(
                            crate::engine::api::profiling::OwnedStorageEvent::Move(1),
                        );
                    }
                    push_operand!(number_value(value));
                }
            }
            NOT => {
                let Some(top) = depth.checked_sub(1) else {
                    fail!(operand_stack_underflow());
                };
                let top_index = operands_start + top;
                let truthy = match &store.hot_slots()[top_index] {
                    Some(FrameBinding::Direct(value)) if is_immediate(value) => {
                        value.to_boolean_primitive()
                    }
                    Some(FrameBinding::Direct(_)) => fallback!(),
                    _ => fail!(operand_slot_not_a_value()),
                };
                store.hot_slots()[top_index] = Some(FrameBinding::Direct(JsValue::Bool(!truthy)));
                #[cfg(feature = "profiling")]
                {
                    // move_owned + commit_push: one removal record, one
                    // install record, net-zero live count.
                    store.adjust_hot_live(-1);
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(1),
                    );
                    store.record_hot_install();
                }
            }
            DROP => {
                let Some(top) = depth.checked_sub(1) else {
                    fail!(operand_stack_underflow());
                };
                let top_index = operands_start + top;
                match &store.hot_slots()[top_index] {
                    Some(FrameBinding::Direct(value)) if is_immediate(value) => {}
                    Some(FrameBinding::Direct(_)) => fallback!(),
                    _ => fail!(operand_slot_not_a_value()),
                }
                store.hot_slots()[top_index] = None;
                depth -= 1;
                #[cfg(feature = "profiling")]
                {
                    store.adjust_hot_live(-1);
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(1),
                    );
                }
            }
            NIP => {
                let Some(below) = depth.checked_sub(2) else {
                    fail!(operand_stack_underflow());
                };
                let removed_index = operands_start + below;
                let kept_index = removed_index + 1;
                match &store.hot_slots()[removed_index] {
                    Some(FrameBinding::Direct(value)) if is_immediate(value) => {}
                    Some(FrameBinding::Direct(_)) => fallback!(),
                    _ => fail!(operand_slot_not_a_value()),
                }
                let kept = match store.hot_slots()[kept_index].take() {
                    Some(FrameBinding::Direct(value)) => value,
                    _ => fail!(operand_slot_not_a_value()),
                };
                store.hot_slots()[removed_index] = Some(FrameBinding::Direct(kept));
                depth -= 1;
                #[cfg(feature = "profiling")]
                {
                    // Two pops and one push in the generic arm.
                    store.adjust_hot_live(-1);
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(2),
                    );
                    store.record_hot_install();
                }
            }
            IF_TRUE | IF_FALSE => {
                let Some(top) = depth.checked_sub(1) else {
                    fail!(operand_stack_underflow());
                };
                let top_index = operands_start + top;
                let truthy = match &store.hot_slots()[top_index] {
                    Some(FrameBinding::Direct(value)) if is_immediate(value) => {
                        value.to_boolean_primitive()
                    }
                    // The general conversion publishes the fault PC and may
                    // release an owner; keep it in the generic arm.
                    Some(FrameBinding::Direct(_)) => fallback!(),
                    _ => fail!(operand_slot_not_a_value()),
                };
                store.hot_slots()[top_index] = None;
                depth -= 1;
                #[cfg(feature = "profiling")]
                {
                    store.adjust_hot_live(-1);
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(1),
                    );
                }
                pc = if truthy == (raw == IF_TRUE) {
                    operand as usize
                } else {
                    next_pc
                };
                continue;
            }
            COMPARE_BRANCH_STACK => {
                let Some(offset) = depth.checked_sub(2) else {
                    fail!(operand_stack_underflow());
                };
                let index = operands_start + offset;
                let descriptor = operand;
                let (left, right) = match &store.hot_slots()[index..index + 2] {
                    [Some(FrameBinding::Direct(left)), Some(FrameBinding::Direct(right))] => {
                        match (left.as_number_repr(), right.as_number_repr()) {
                            (Some(left), Some(right)) => (left, right),
                            _ => fallback!(),
                        }
                    }
                    _ => fail!(operand_slot_not_a_value()),
                };
                let decision =
                    compare_direct_numbers((descriptor & 0x3ff) as u16, left, right);
                // number_pair_branch: consume both operands without a result.
                let slots = store.hot_slots();
                slots[index] = None;
                slots[index + 1] = None;
                depth -= 2;
                #[cfg(feature = "profiling")]
                {
                    store.adjust_hot_live(-2);
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Move(2),
                    );
                    crate::engine::api::profiling::record_owned_execution_event(
                        "number_pair_branch",
                    );
                    crate::engine::api::profiling::record_execution_dispatch(
                        runtime, executable, pc, true,
                    );
                    crate::engine::api::profiling::record_execution_outcome(
                        runtime,
                        executable,
                        pc,
                        "compare_branch_stack",
                        None,
                    );
                }
                pc = if decision == (descriptor & 0x400 != 0) {
                    ext(1) as usize
                } else {
                    next_pc + 2
                };
                continue;
            }
            COMPARE_BRANCH_LOCAL | COMPARE_BRANCH_ARG | COMPARE_BRANCH_LOCAL_LT
            | COMPARE_BRANCH_ARG_LT => {
                let left_index = usize::from(published_u16(operand));
                let descriptor = ext(1);
                let right_index = usize::from(published_u16(descriptor & 0xffff));
                let left_base = if raw == COMPARE_BRANCH_ARG || raw == COMPARE_BRANCH_ARG_LT {
                    params_start
                } else {
                    locals_start
                };
                let right_base = if descriptor & 0x1_0000 != 0 {
                    params_start
                } else {
                    locals_start
                };
                if !depth.checked_add(2).is_some_and(|d| d <= operand_room) {
                    fallback!();
                }
                let direct_number = |store: &mut SlotStore, index: usize| -> Option<Number> {
                    match store.hot_slots().get(index) {
                        Some(Some(FrameBinding::Direct(value))) => value.as_number_repr(),
                        _ => None,
                    }
                };
                let left = match direct_number(store, left_base + left_index) {
                    Some(value) => value,
                    None => fallback!(),
                };
                let right = match direct_number(store, right_base + right_index) {
                    Some(value) => value,
                    None => fallback!(),
                };
                let decision = if raw == COMPARE_BRANCH_LOCAL_LT || raw == COMPARE_BRANCH_ARG_LT {
                    left.float() < right.float()
                } else {
                    compare_direct_numbers(((descriptor >> 17) & 0x3ff) as u16, left, right)
                };
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_execution_dispatch(
                        runtime, executable, pc, true,
                    );
                    crate::engine::api::profiling::record_execution_outcome(
                        runtime,
                        executable,
                        pc,
                        if raw == COMPARE_BRANCH_LOCAL_LT || raw == COMPARE_BRANCH_ARG_LT {
                            "compare_branch_lt"
                        } else {
                            "compare_branch"
                        },
                        None,
                    );
                }
                pc = if decision == (descriptor & 0x800_0000 != 0) {
                    ext(2) as usize
                } else {
                    next_pc + 4
                };
                continue;
            }
            _ => fallback!(),
        }
        pc = next_pc;
    }
}
