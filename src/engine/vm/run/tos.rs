//! Select the cache-aware facade by opcode, not by repeating the handler's
//! binding/value proofs. The listed handlers use checked stack operations;
//! heap retains cannot observe the activation, and all releases/publications
//! still canonicalize at their actual boundary. Ordinary push itself admits
//! only scalars. Unlisted handlers keep their whole borrow canonical.
use super::Instruction;

#[cfg(test)]
mod tests;

#[inline]
pub(super) fn resident(instruction: &Instruction) -> bool {
    use Instruction as I;
    matches!(
        instruction,
        I::Nop
            | I::Drop
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
            | I::PostDec
            | I::IfTrue(_)
            | I::IfFalse(_)
            | I::PushConst(_)
            | I::GetLocal(_)
            | I::GetLocalCheck(_)
            | I::GetArg(_)
            | I::PutLocal(_)
            | I::SetLocal(_)
            | I::PutLocalCheck(_)
            | I::SetLocalCheck(_)
            | I::PutArg(_)
            | I::SetArg(_)
    )
}
