//! Conservative scalar-cache admission. An unlisted opcode receives a
//! canonical-only facade for its entire handler, including scalar outputs.
use super::{BytecodeConstant, FrameBinding, Instruction, RawValue, RunSlots, immediate};
use crate::engine::code::runtime::PublishedFunctionSnapshot;

#[cfg(test)]
mod tests;

#[inline]
pub(super) fn resident(
    instruction: &Instruction,
    executable: &PublishedFunctionSnapshot,
    slots: &RunSlots<'_>,
) -> bool {
    use Instruction as I;
    match instruction {
        I::Nop
        | I::PushI32(_)
        | I::Undefined
        | I::Null
        | I::PushTrue
        | I::PushFalse
        | I::Goto(_)
        | I::Gosub(_)
        | I::Ret
        | I::DropGosub
        | I::Add
        | I::Sub
        | I::Mul
        | I::Div
        | I::Mod
        | I::Pow
        | I::Shl
        | I::Sar
        | I::Shr
        | I::BitAnd
        | I::BitOr
        | I::BitXor
        | I::Lt
        | I::Lte
        | I::Gt
        | I::Gte
        | I::Eq
        | I::Neq
        | I::Neg
        | I::Plus
        | I::BitNot
        | I::Inc
        | I::Dec
        | I::PostInc
        | I::PostDec => true,
        I::IfTrue(_) | I::IfFalse(_) => slots.peek(0).is_ok_and(immediate),
        I::PushConst(index) => matches!(
            executable.constant(*index),
            Some(BytecodeConstant::Value(
                RawValue::Undefined
                    | RawValue::Null
                    | RawValue::Bool(_)
                    | RawValue::Int(_)
                    | RawValue::Float(_)
                    | RawValue::ShortBigInt(_)
            ))
        ),
        I::GetLocal(index) | I::GetLocalCheck(index) => {
            matches!(slots.local(*index), Ok(FrameBinding::Direct(value)) if immediate(value))
        }
        I::GetArg(index) => {
            matches!(slots.parameter(*index), Ok(FrameBinding::Direct(value)) if immediate(value))
        }
        I::PutLocal(index)
        | I::SetLocal(index)
        | I::PutLocalCheck(index)
        | I::SetLocalCheck(index) => {
            slots.peek(0).is_ok_and(immediate)
                && matches!(slots.local(*index), Ok(FrameBinding::Direct(value)) if immediate(value))
        }
        I::PutArg(index) | I::SetArg(index) => {
            slots.peek(0).is_ok_and(immediate)
                && matches!(slots.parameter(*index), Ok(FrameBinding::Direct(value)) if immediate(value))
        }
        _ => false,
    }
}
