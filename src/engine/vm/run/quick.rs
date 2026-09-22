//! Small guarded dispatch over authenticated words; hot success never reads
//! canonical Instruction. Guards preserve the inputs; numeric fallback carries
//! its decoded kind, while other declines re-enter the same canonical PC.
use super::{Error, JsValue, RunSlots, StoreMode, cold, hot};
use crate::engine::api::Runtime;
use crate::engine::code::function::metadata::ClosureVariableKind;
use crate::engine::code::quick::DecodedOp;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::vm::numeric::operation::NumericKind;

pub(super) enum Outcome {
    Completed,
    Canonical,
    Numeric(NumericKind),
}

#[inline]
pub(super) fn execute(
    operation: DecodedOp,
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    slots: &mut RunSlots<'_>,
    pc: usize,
    next_pc: &mut usize,
) -> Result<Outcome, Error> {
    use DecodedOp as Q;
    use NumericKind as N;
    match operation {
        Q::GenericCanonical => Ok(Outcome::Canonical),
        Q::Nop => Ok(Outcome::Completed),
        Q::PushI32(number) => {
            slots.push(JsValue::Int(number))?;
            Ok(Outcome::Completed)
        }
        Q::Undefined => {
            slots.push(JsValue::Undefined)?;
            Ok(Outcome::Completed)
        }
        Q::Null => {
            slots.push(JsValue::Null)?;
            Ok(Outcome::Completed)
        }
        Q::Bool(boolean) => {
            slots.push(JsValue::Bool(boolean))?;
            Ok(Outcome::Completed)
        }
        Q::Goto(target) => {
            *next_pc = usize::try_from(target)
                .map_err(|_| cold::internal("jump target does not fit PC"))?;
            Ok(Outcome::Completed)
        }
        Q::Add => binary(slots, N::Add),
        Q::Sub => binary(slots, N::Sub),
        Q::Mul => binary(slots, N::Mul),
        Q::Div => binary(slots, N::Div),
        Q::Mod => binary(slots, N::Mod),
        Q::Pow => binary(slots, N::Pow),
        Q::Shl => binary(slots, N::Shl),
        Q::Sar => binary(slots, N::Sar),
        Q::Shr => binary(slots, N::Shr),
        Q::BitAnd => binary(slots, N::BitAnd),
        Q::BitOr => binary(slots, N::BitOr),
        Q::BitXor => binary(slots, N::BitXor),
        Q::Eq => comparison(executable, slots, pc, N::Eq),
        Q::Neq => comparison(executable, slots, pc, N::Neq),
        Q::Lt => comparison(executable, slots, pc, N::Lt),
        Q::Lte => comparison(executable, slots, pc, N::Lte),
        Q::Gt => comparison(executable, slots, pc, N::Gt),
        Q::Gte => comparison(executable, slots, pc, N::Gte),
        Q::IfTrue(target) => branch(slots, target, true, next_pc),
        Q::IfFalse(target) => branch(slots, target, false, next_pc),
        Q::GetLocal(index) => {
            if executable.fusion.update(pc).is_some()
                || executable.fusion.local_add_span(pc).is_some()
            {
                return Ok(Outcome::Canonical);
            }
            // Object/captured cases decline inside read_scalar, preserving
            // borrowed-base field and other canonical read protocols.
            canonical_or_completed(hot::read_scalar(runtime, slots, index, false))
        }
        Q::GetArg(index) => canonical_or_completed(hot::read_scalar(runtime, slots, index, true)),
        Q::PutLocal(index) => store(
            runtime,
            executable,
            slots,
            pc,
            index,
            false,
            StoreMode::Consume,
        ),
        Q::SetLocal(index) => store(
            runtime,
            executable,
            slots,
            pc,
            index,
            false,
            StoreMode::Keep,
        ),
        Q::PutArg(index) => store(
            runtime,
            executable,
            slots,
            pc,
            index,
            true,
            StoreMode::Consume,
        ),
        Q::SetArg(index) => store(runtime, executable, slots, pc, index, true, StoreMode::Keep),
    }
}

#[inline]
fn canonical_or_completed(result: Result<bool, Error>) -> Result<Outcome, Error> {
    result.map(|completed| {
        if completed {
            Outcome::Completed
        } else {
            Outcome::Canonical
        }
    })
}

#[inline]
fn binary(slots: &mut RunSlots<'_>, kind: NumericKind) -> Result<Outcome, Error> {
    Ok(if hot::binary(slots, kind)? {
        Outcome::Completed
    } else {
        Outcome::Numeric(kind)
    })
}

#[inline]
fn comparison(
    executable: &PublishedFunctionSnapshot,
    slots: &mut RunSlots<'_>,
    pc: usize,
    kind: NumericKind,
) -> Result<Outcome, Error> {
    if executable.fusion.compare_branch(pc) {
        return Ok(Outcome::Canonical);
    }
    binary(slots, kind)
}

#[inline]
fn branch(
    slots: &mut RunSlots<'_>,
    target: u32,
    when: bool,
    next_pc: &mut usize,
) -> Result<Outcome, Error> {
    let Some(target) = hot::branch(slots, target, when)? else {
        return Ok(Outcome::Canonical);
    };
    if target != usize::MAX {
        *next_pc = target;
    }
    Ok(Outcome::Completed)
}

#[inline]
fn store(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    slots: &mut RunSlots<'_>,
    pc: usize,
    index: u16,
    argument: bool,
    mode: StoreMode,
) -> Result<Outcome, Error> {
    #[cfg(any(test, oxide_store_drop_fusion))]
    if executable.fusion.store_drop(pc).is_some() {
        return Ok(Outcome::Canonical);
    }
    #[cfg(not(any(test, oxide_store_drop_fusion)))]
    let _ = pc;
    if !argument
        && !executable
            .local_definitions
            .get(usize::from(index))
            .is_some_and(|definition| {
                definition.kind == ClosureVariableKind::Normal && !definition.is_const
            })
    {
        return Ok(Outcome::Canonical);
    }
    canonical_or_completed(hot::store_scalar(runtime, slots, index, argument, mode))
}
