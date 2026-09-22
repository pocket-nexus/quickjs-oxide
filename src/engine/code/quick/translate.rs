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
        // No wildcard: future opcodes must explicitly choose a reviewed hot
        // encoding or the unchanged canonical handler. Fusion entries and all
        // observable/owner-bearing operations stay canonical in this batch.
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
        | Instruction::GetLocal(..)
        | Instruction::PutLocal(..)
        | Instruction::SetLocal(..)
        | Instruction::SetLocalUninitialized(..)
        | Instruction::GetLocalCheck(..)
        | Instruction::InitializeLocal(..)
        | Instruction::InitializeDerivedLocal(..)
        | Instruction::PutLocalCheck(..)
        | Instruction::SetLocalCheck(..)
        | Instruction::GetArg(..)
        | Instruction::PutArg(..)
        | Instruction::SetArg(..)
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
        | Instruction::Add
        | Instruction::Sub
        | Instruction::Mul
        | Instruction::Div
        | Instruction::Mod
        | Instruction::Pow
        | Instruction::Shl
        | Instruction::Sar
        | Instruction::Shr
        | Instruction::BitAnd
        | Instruction::BitXor
        | Instruction::BitOr
        | Instruction::Eq
        | Instruction::StrictEq
        | Instruction::Neq
        | Instruction::StrictNeq
        | Instruction::Lt
        | Instruction::Lte
        | Instruction::Gt
        | Instruction::Gte
        | Instruction::InstanceOf
        | Instruction::In
        | Instruction::IfFalse(..)
        | Instruction::IfTrue(..)
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
