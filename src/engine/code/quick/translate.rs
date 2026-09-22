//! Exhaustive classification keeps new canonical variants a compile-time review.
use super::{Instruction, QuickOp, QuickTag};

pub(super) fn translate_instruction(instruction: &Instruction) -> QuickOp {
    match instruction {
        Instruction::Nop => QuickOp::pack(QuickTag::Nop, 0),
        Instruction::PushI32(value) => {
            QuickOp::pack(QuickTag::PushI32, u32::from_le_bytes(value.to_le_bytes()))
        }
        Instruction::Undefined => QuickOp::pack(QuickTag::Undefined, 0),
        Instruction::Null => QuickOp::pack(QuickTag::Null, 0),
        Instruction::PushFalse => QuickOp::pack(QuickTag::Bool, 0),
        Instruction::PushTrue => QuickOp::pack(QuickTag::Bool, 1),
        Instruction::Goto(target) => QuickOp::pack(QuickTag::Goto, *target),
        Instruction::Add => QuickOp::pack(QuickTag::Add, 0),
        Instruction::Sub => QuickOp::pack(QuickTag::Sub, 0),
        Instruction::Mul => QuickOp::pack(QuickTag::Mul, 0),
        Instruction::Div => QuickOp::pack(QuickTag::Div, 0),
        Instruction::Mod => QuickOp::pack(QuickTag::Mod, 0),
        Instruction::Pow => QuickOp::pack(QuickTag::Pow, 0),
        Instruction::Shl => QuickOp::pack(QuickTag::Shl, 0),
        Instruction::Sar => QuickOp::pack(QuickTag::Sar, 0),
        Instruction::Shr => QuickOp::pack(QuickTag::Shr, 0),
        Instruction::BitAnd => QuickOp::pack(QuickTag::BitAnd, 0),
        Instruction::BitOr => QuickOp::pack(QuickTag::BitOr, 0),
        Instruction::BitXor => QuickOp::pack(QuickTag::BitXor, 0),
        Instruction::Eq => QuickOp::pack(QuickTag::Eq, 0),
        Instruction::Neq => QuickOp::pack(QuickTag::Neq, 0),
        Instruction::Lt => QuickOp::pack(QuickTag::Lt, 0),
        Instruction::Lte => QuickOp::pack(QuickTag::Lte, 0),
        Instruction::Gt => QuickOp::pack(QuickTag::Gt, 0),
        Instruction::Gte => QuickOp::pack(QuickTag::Gte, 0),
        Instruction::IfTrue(target) => QuickOp::pack(QuickTag::IfTrue, *target),
        Instruction::IfFalse(target) => QuickOp::pack(QuickTag::IfFalse, *target),
        Instruction::GetLocal(index) => QuickOp::pack(QuickTag::GetLocal, u32::from(*index)),
        Instruction::PutLocal(index) => QuickOp::pack(QuickTag::PutLocal, u32::from(*index)),
        Instruction::SetLocal(index) => QuickOp::pack(QuickTag::SetLocal, u32::from(*index)),
        Instruction::GetArg(index) => QuickOp::pack(QuickTag::GetArg, u32::from(*index)),
        Instruction::PutArg(index) => QuickOp::pack(QuickTag::PutArg, u32::from(*index)),
        Instruction::SetArg(index) => QuickOp::pack(QuickTag::SetArg, u32::from(*index)),
        // No wildcard: future opcodes must explicitly choose a reviewed hot
        // encoding or the unchanged canonical handler. Guarded hot operations
        // retain canonical fallback and never erase fusion interior PCs.
        Instruction::PushAtomValueIndex(..)
        | Instruction::PushConst(..)
        | Instruction::FClosure(..)
        | Instruction::RegExp(..)
        | Instruction::SetName(..)
        | Instruction::ThrowReadOnly(..)
        | Instruction::ThrowRedeclaration(..)
        | Instruction::ThrowDeleteSuper
        | Instruction::PushThis
        | Instruction::PushActiveFunction
        | Instruction::PushHomeObject
        | Instruction::PushNewTarget
        | Instruction::Arguments(..)
        | Instruction::Rest(..)
        | Instruction::VariableEnvironment
        | Instruction::HasEvalVariable { .. }
        | Instruction::GetEvalVariable { .. }
        | Instruction::PutEvalVariable { .. }
        | Instruction::DeleteEvalVariable { .. }
        | Instruction::DefineEvalVariable { .. }
        | Instruction::ToObject
        | Instruction::HasDynamicBinding { .. }
        | Instruction::GetDynamicBinding { .. }
        | Instruction::PutDynamicBinding { .. }
        | Instruction::DeleteDynamicBinding { .. }
        | Instruction::DynamicEnvironmentObject(..)
        | Instruction::GlobalReference(..)
        | Instruction::GetRefValue(..)
        | Instruction::GetRefValueUndef(..)
        | Instruction::PutRefValue(..)
        | Instruction::SetLocalUninitialized(..)
        | Instruction::GetLocalCheck(..)
        | Instruction::InitializeLocal(..)
        | Instruction::InitializeDerivedLocal(..)
        | Instruction::PutLocalCheck(..)
        | Instruction::SetLocalCheck(..)
        | Instruction::GetVarRef(..)
        | Instruction::PutVarRef(..)
        | Instruction::SetVarRef(..)
        | Instruction::GetVarRefCheck(..)
        | Instruction::PutVarRefCheck(..)
        | Instruction::InitializeVarRef(..)
        | Instruction::InitializeModuleImportCollision(..)
        | Instruction::InitializeDerivedVarRef(..)
        | Instruction::CloseLocal(..)
        | Instruction::GetVar(..)
        | Instruction::GetVarUndef(..)
        | Instruction::DeleteVar(..)
        | Instruction::PutVar(..)
        | Instruction::PutVarInit(..)
        | Instruction::InitializePrivateName(..)
        | Instruction::InitializePrivateMethod(..)
        | Instruction::InitializePrivateAccessor(..)
        | Instruction::GetPrivateField(..)
        | Instruction::GetPrivateField2(..)
        | Instruction::PutPrivateField(..)
        | Instruction::DefinePrivateField(..)
        | Instruction::PrivateIn(..)
        | Instruction::GetField(..)
        | Instruction::GetField2(..)
        | Instruction::GetArrayEl
        | Instruction::GetArrayEl2
        | Instruction::GetArrayEl3
        | Instruction::GetSuper
        | Instruction::GetSuperValue
        | Instruction::GetSuperValueForCall
        | Instruction::ArrayFrom(..)
        | Instruction::Object
        | Instruction::ToPropKey
        | Instruction::Insert2
        | Instruction::Insert3
        | Instruction::Dup3
        | Instruction::Insert4
        | Instruction::Perm3
        | Instruction::Perm4
        | Instruction::Perm5
        | Instruction::Rot4Left
        | Instruction::PutField(..)
        | Instruction::PutArrayEl
        | Instruction::PutSuperValue
        | Instruction::DefineField(..)
        | Instruction::DefineFieldComputed
        | Instruction::DefineMethod { .. }
        | Instruction::DefineMethodComputed { .. }
        | Instruction::DefineClass { .. }
        | Instruction::InstallClassInstanceInitializer
        | Instruction::CallClassInstanceInitializer
        | Instruction::RunClassStaticInitializer
        | Instruction::CallClassStaticBlock
        | Instruction::DefineArrayEl
        | Instruction::SetNameComputed
        | Instruction::SetProto
        | Instruction::CopyDataProperties
        | Instruction::CopyDataPropertiesExcluded { .. }
        | Instruction::Append
        | Instruction::Delete
        | Instruction::Drop
        | Instruction::Nip
        | Instruction::Swap
        | Instruction::Dup
        | Instruction::Dup1
        | Instruction::Neg
        | Instruction::Plus
        | Instruction::Inc
        | Instruction::Dec
        | Instruction::PostInc
        | Instruction::PostDec
        | Instruction::BitNot
        | Instruction::Not
        | Instruction::TypeOf
        | Instruction::IsUndefinedOrNull
        | Instruction::IsUndefined
        | Instruction::IsNull
        | Instruction::TypeOfIsUndefined
        | Instruction::TypeOfIsFunction
        | Instruction::StrictEq
        | Instruction::StrictNeq
        | Instruction::InstanceOf
        | Instruction::In
        | Instruction::Catch(..)
        | Instruction::DropCatch
        | Instruction::NipCatch
        | Instruction::Gosub(..)
        | Instruction::Ret
        | Instruction::DropGosub
        | Instruction::ForOfStart
        | Instruction::ForAwaitOfStart
        | Instruction::ForOfNext(..)
        | Instruction::ForAwaitOfNext
        | Instruction::IteratorGetValueDone
        | Instruction::ForInStart
        | Instruction::ForInNext
        | Instruction::IteratorClose
        | Instruction::IteratorClosePreserve
        | Instruction::IteratorDropPreserve
        | Instruction::IteratorDetachPreserve
        | Instruction::IteratorStart
        | Instruction::AsyncIteratorStart
        | Instruction::IteratorNext
        | Instruction::IteratorCall(..)
        | Instruction::IteratorCheckObject
        | Instruction::InitialYield
        | Instruction::Yield
        | Instruction::YieldStar
        | Instruction::AsyncYieldStar
        | Instruction::Await
        | Instruction::ThrowIteratorMissingThrow
        | Instruction::Import
        | Instruction::Call(..)
        | Instruction::TailCall(..)
        | Instruction::Eval { .. }
        | Instruction::CallMethod(..)
        | Instruction::TailCallMethod(..)
        | Instruction::Construct(..)
        | Instruction::MarkSuperCall
        | Instruction::ConstructSuper(..)
        | Instruction::CheckCtor
        | Instruction::InitDerivedConstructor
        | Instruction::Apply(..)
        | Instruction::ApplySuper
        | Instruction::ApplyEval { .. }
        | Instruction::Return
        | Instruction::ReturnUndefined
        | Instruction::ReturnDerived(..)
        | Instruction::Throw => QuickOp::pack(QuickTag::GenericCanonical, 0),
    }
}
