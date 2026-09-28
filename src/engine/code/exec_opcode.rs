//! Numeric execution opcodes shared by the publisher, verifier, disassembler and VM.
//! The compiler Instruction is consumed during publication.
use super::bytecode::Instruction;

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Opcode {
    Nop,
    PushI32,
    PushAtomValueIndex,
    PushConst,
    FClosure,
    RegExp,
    SetName,
    ThrowReadOnly,
    ThrowRedeclaration,
    ThrowDeleteSuper,
    Undefined,
    Null,
    PushFalse,
    PushTrue,
    PushThis,
    PushActiveFunction,
    PushHomeObject,
    PushNewTarget,
    Arguments,
    Rest,
    VariableEnvironment,
    HasEvalVariable,
    GetEvalVariable,
    PutEvalVariable,
    DeleteEvalVariable,
    DefineEvalVariable,
    ToObject,
    HasDynamicBinding,
    GetDynamicBinding,
    PutDynamicBinding,
    DeleteDynamicBinding,
    DynamicEnvironmentObject,
    GlobalReference,
    GetRefValue,
    GetRefValueUndef,
    PutRefValue,
    GetLocal,
    PutLocal,
    SetLocal,
    SetLocalUninitialized,
    GetLocalCheck,
    InitializeLocal,
    InitializeDerivedLocal,
    PutLocalCheck,
    SetLocalCheck,
    GetArg,
    PutArg,
    SetArg,
    GetVarRef,
    PutVarRef,
    SetVarRef,
    GetVarRefCheck,
    PutVarRefCheck,
    InitializeVarRef,
    InitializeModuleImportCollision,
    InitializeDerivedVarRef,
    CloseLocal,
    GetVar,
    GetVarUndef,
    DeleteVar,
    PutVar,
    PutVarInit,
    InitializePrivateName,
    InitializePrivateMethod,
    InitializePrivateAccessor,
    GetPrivateField,
    GetPrivateField2,
    PutPrivateField,
    DefinePrivateField,
    PrivateIn,
    GetField,
    GetField2,
    GetArrayEl,
    GetArrayEl2,
    GetArrayEl3,
    GetSuper,
    GetSuperValue,
    GetSuperValueForCall,
    ArrayFrom,
    Object,
    ToPropKey,
    Insert2,
    Insert3,
    Dup3,
    Insert4,
    Perm3,
    Perm4,
    Perm5,
    Rot4Left,
    PutField,
    PutArrayEl,
    PutSuperValue,
    DefineField,
    DefineFieldComputed,
    DefineMethod,
    DefineMethodComputed,
    DefineClass,
    InstallClassInstanceInitializer,
    CallClassInstanceInitializer,
    RunClassStaticInitializer,
    CallClassStaticBlock,
    DefineArrayEl,
    SetNameComputed,
    SetProto,
    CopyDataProperties,
    CopyDataPropertiesExcluded,
    Append,
    Delete,
    Drop,
    Nip,
    Swap,
    Dup,
    Dup1,
    Neg,
    Plus,
    Inc,
    Dec,
    PostInc,
    PostDec,
    BitNot,
    Not,
    TypeOf,
    IsUndefinedOrNull,
    IsUndefined,
    IsNull,
    TypeOfIsUndefined,
    TypeOfIsFunction,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Shl,
    Sar,
    Shr,
    BitAnd,
    BitXor,
    BitOr,
    Eq,
    StrictEq,
    Neq,
    StrictNeq,
    Lt,
    Lte,
    Gt,
    Gte,
    InstanceOf,
    In,
    IfFalse,
    IfTrue,
    Goto,
    Catch,
    DropCatch,
    NipCatch,
    Gosub,
    Ret,
    DropGosub,
    ForOfStart,
    ForAwaitOfStart,
    ForOfNext,
    ForAwaitOfNext,
    IteratorGetValueDone,
    ForInStart,
    ForInNext,
    IteratorClose,
    IteratorClosePreserve,
    IteratorDropPreserve,
    IteratorDetachPreserve,
    IteratorStart,
    AsyncIteratorStart,
    IteratorNext,
    IteratorCall,
    IteratorCheckObject,
    InitialYield,
    Yield,
    YieldStar,
    AsyncYieldStar,
    Await,
    ThrowIteratorMissingThrow,
    Import,
    Call,
    TailCall,
    Eval,
    CallMethod,
    TailCallMethod,
    Construct,
    MarkSuperCall,
    ConstructSuper,
    CheckCtor,
    InitDerivedConstructor,
    Apply,
    ApplySuper,
    ApplyEval,
    Return,
    ReturnUndefined,
    ReturnDerived,
    Throw,
    /// A published GetLocal + PushI32(1) + Add span. On a guard miss the
    /// same word acts as GetLocal and the following generic words run.
    NumberLocalInc,
    /// Planned numeric Array read, multiplication, addition and direct local
    /// commit. The original words remain the generic continuation.
    NumericArrayAccumulate,
    NumberArgInc,
    /// Field reads with a direct location-cache probe in the execution loop.
    GetFieldCached,
    GetField2Cached,
    GetArrayElDense,
    GetArrayEl2Dense,
    GetArrayEl3Dense,
    /// Published five-operation base/index update and dense read. Operand 1
    /// carries the updated slot and operation; operand 2 is the verified end PC.
    DensePreUpdateLocal,
    DensePreUpdateArg,
    DenseReadLocal,
    DenseReadArg,
    BorrowedFieldLocal,
    BorrowedFieldArg,
    CompareBranchLocal,
    CompareBranchArg,
    CompareBranchStack,
    UpdateLocalDiscard,
    UpdateLocalDiscardCheck,
    DensePostUpdateLocal,
    DensePostUpdateLocalCheck,
    DensePostUpdateArg,
    DenseReadBinaryLocal,
    DenseReadBinaryArg,
    DenseIndexBinaryLocal,
    DenseIndexBinaryArg,
    DenseAccIndexSetDrop,
    FieldAccSetDrop,
    CompareBranchLocalLt,
    CompareBranchArgLt,
    NumericArrayStoreProduct,
    NumericArrayCopyElement,
    NumericArrayAddPreInc,
    NumericArrayStoreAndLocal,
    NumericArrayUpdateElement,
    NumericArrayCompareBranch,
}

const OPCODES: &[Opcode] = &[
    Opcode::Nop,
    Opcode::PushI32,
    Opcode::PushAtomValueIndex,
    Opcode::PushConst,
    Opcode::FClosure,
    Opcode::RegExp,
    Opcode::SetName,
    Opcode::ThrowReadOnly,
    Opcode::ThrowRedeclaration,
    Opcode::ThrowDeleteSuper,
    Opcode::Undefined,
    Opcode::Null,
    Opcode::PushFalse,
    Opcode::PushTrue,
    Opcode::PushThis,
    Opcode::PushActiveFunction,
    Opcode::PushHomeObject,
    Opcode::PushNewTarget,
    Opcode::Arguments,
    Opcode::Rest,
    Opcode::VariableEnvironment,
    Opcode::HasEvalVariable,
    Opcode::GetEvalVariable,
    Opcode::PutEvalVariable,
    Opcode::DeleteEvalVariable,
    Opcode::DefineEvalVariable,
    Opcode::ToObject,
    Opcode::HasDynamicBinding,
    Opcode::GetDynamicBinding,
    Opcode::PutDynamicBinding,
    Opcode::DeleteDynamicBinding,
    Opcode::DynamicEnvironmentObject,
    Opcode::GlobalReference,
    Opcode::GetRefValue,
    Opcode::GetRefValueUndef,
    Opcode::PutRefValue,
    Opcode::GetLocal,
    Opcode::PutLocal,
    Opcode::SetLocal,
    Opcode::SetLocalUninitialized,
    Opcode::GetLocalCheck,
    Opcode::InitializeLocal,
    Opcode::InitializeDerivedLocal,
    Opcode::PutLocalCheck,
    Opcode::SetLocalCheck,
    Opcode::GetArg,
    Opcode::PutArg,
    Opcode::SetArg,
    Opcode::GetVarRef,
    Opcode::PutVarRef,
    Opcode::SetVarRef,
    Opcode::GetVarRefCheck,
    Opcode::PutVarRefCheck,
    Opcode::InitializeVarRef,
    Opcode::InitializeModuleImportCollision,
    Opcode::InitializeDerivedVarRef,
    Opcode::CloseLocal,
    Opcode::GetVar,
    Opcode::GetVarUndef,
    Opcode::DeleteVar,
    Opcode::PutVar,
    Opcode::PutVarInit,
    Opcode::InitializePrivateName,
    Opcode::InitializePrivateMethod,
    Opcode::InitializePrivateAccessor,
    Opcode::GetPrivateField,
    Opcode::GetPrivateField2,
    Opcode::PutPrivateField,
    Opcode::DefinePrivateField,
    Opcode::PrivateIn,
    Opcode::GetField,
    Opcode::GetField2,
    Opcode::GetArrayEl,
    Opcode::GetArrayEl2,
    Opcode::GetArrayEl3,
    Opcode::GetSuper,
    Opcode::GetSuperValue,
    Opcode::GetSuperValueForCall,
    Opcode::ArrayFrom,
    Opcode::Object,
    Opcode::ToPropKey,
    Opcode::Insert2,
    Opcode::Insert3,
    Opcode::Dup3,
    Opcode::Insert4,
    Opcode::Perm3,
    Opcode::Perm4,
    Opcode::Perm5,
    Opcode::Rot4Left,
    Opcode::PutField,
    Opcode::PutArrayEl,
    Opcode::PutSuperValue,
    Opcode::DefineField,
    Opcode::DefineFieldComputed,
    Opcode::DefineMethod,
    Opcode::DefineMethodComputed,
    Opcode::DefineClass,
    Opcode::InstallClassInstanceInitializer,
    Opcode::CallClassInstanceInitializer,
    Opcode::RunClassStaticInitializer,
    Opcode::CallClassStaticBlock,
    Opcode::DefineArrayEl,
    Opcode::SetNameComputed,
    Opcode::SetProto,
    Opcode::CopyDataProperties,
    Opcode::CopyDataPropertiesExcluded,
    Opcode::Append,
    Opcode::Delete,
    Opcode::Drop,
    Opcode::Nip,
    Opcode::Swap,
    Opcode::Dup,
    Opcode::Dup1,
    Opcode::Neg,
    Opcode::Plus,
    Opcode::Inc,
    Opcode::Dec,
    Opcode::PostInc,
    Opcode::PostDec,
    Opcode::BitNot,
    Opcode::Not,
    Opcode::TypeOf,
    Opcode::IsUndefinedOrNull,
    Opcode::IsUndefined,
    Opcode::IsNull,
    Opcode::TypeOfIsUndefined,
    Opcode::TypeOfIsFunction,
    Opcode::Add,
    Opcode::Sub,
    Opcode::Mul,
    Opcode::Div,
    Opcode::Mod,
    Opcode::Pow,
    Opcode::Shl,
    Opcode::Sar,
    Opcode::Shr,
    Opcode::BitAnd,
    Opcode::BitXor,
    Opcode::BitOr,
    Opcode::Eq,
    Opcode::StrictEq,
    Opcode::Neq,
    Opcode::StrictNeq,
    Opcode::Lt,
    Opcode::Lte,
    Opcode::Gt,
    Opcode::Gte,
    Opcode::InstanceOf,
    Opcode::In,
    Opcode::IfFalse,
    Opcode::IfTrue,
    Opcode::Goto,
    Opcode::Catch,
    Opcode::DropCatch,
    Opcode::NipCatch,
    Opcode::Gosub,
    Opcode::Ret,
    Opcode::DropGosub,
    Opcode::ForOfStart,
    Opcode::ForAwaitOfStart,
    Opcode::ForOfNext,
    Opcode::ForAwaitOfNext,
    Opcode::IteratorGetValueDone,
    Opcode::ForInStart,
    Opcode::ForInNext,
    Opcode::IteratorClose,
    Opcode::IteratorClosePreserve,
    Opcode::IteratorDropPreserve,
    Opcode::IteratorDetachPreserve,
    Opcode::IteratorStart,
    Opcode::AsyncIteratorStart,
    Opcode::IteratorNext,
    Opcode::IteratorCall,
    Opcode::IteratorCheckObject,
    Opcode::InitialYield,
    Opcode::Yield,
    Opcode::YieldStar,
    Opcode::AsyncYieldStar,
    Opcode::Await,
    Opcode::ThrowIteratorMissingThrow,
    Opcode::Import,
    Opcode::Call,
    Opcode::TailCall,
    Opcode::Eval,
    Opcode::CallMethod,
    Opcode::TailCallMethod,
    Opcode::Construct,
    Opcode::MarkSuperCall,
    Opcode::ConstructSuper,
    Opcode::CheckCtor,
    Opcode::InitDerivedConstructor,
    Opcode::Apply,
    Opcode::ApplySuper,
    Opcode::ApplyEval,
    Opcode::Return,
    Opcode::ReturnUndefined,
    Opcode::ReturnDerived,
    Opcode::Throw,
    Opcode::NumberLocalInc,
    Opcode::NumericArrayAccumulate,
    Opcode::NumberArgInc,
    Opcode::GetFieldCached,
    Opcode::GetField2Cached,
    Opcode::GetArrayElDense,
    Opcode::GetArrayEl2Dense,
    Opcode::GetArrayEl3Dense,
    Opcode::DensePreUpdateLocal,
    Opcode::DensePreUpdateArg,
    Opcode::DenseReadLocal,
    Opcode::DenseReadArg,
    Opcode::BorrowedFieldLocal,
    Opcode::BorrowedFieldArg,
    Opcode::CompareBranchLocal,
    Opcode::CompareBranchArg,
    Opcode::CompareBranchStack,
    Opcode::UpdateLocalDiscard,
    Opcode::UpdateLocalDiscardCheck,
    Opcode::DensePostUpdateLocal,
    Opcode::DensePostUpdateLocalCheck,
    Opcode::DensePostUpdateArg,
    Opcode::DenseReadBinaryLocal,
    Opcode::DenseReadBinaryArg,
    Opcode::DenseIndexBinaryLocal,
    Opcode::DenseIndexBinaryArg,
    Opcode::DenseAccIndexSetDrop,
    Opcode::FieldAccSetDrop,
    Opcode::CompareBranchLocalLt,
    Opcode::CompareBranchArgLt,
    Opcode::NumericArrayStoreProduct,
    Opcode::NumericArrayCopyElement,
    Opcode::NumericArrayAddPreInc,
    Opcode::NumericArrayStoreAndLocal,
    Opcode::NumericArrayUpdateElement,
    Opcode::NumericArrayCompareBranch,
];

impl Opcode {
    pub(crate) fn from_raw(raw: u16) -> Option<Self> {
        OPCODES.get(usize::from(raw)).copied()
    }

    pub(crate) const fn from_instruction(instruction: &Instruction) -> Self {
        match instruction {
            Instruction::Nop => Self::Nop,
            Instruction::PushI32(..) => Self::PushI32,
            Instruction::PushAtomValueIndex(..) => Self::PushAtomValueIndex,
            Instruction::PushConst(..) => Self::PushConst,
            Instruction::FClosure(..) => Self::FClosure,
            Instruction::RegExp(..) => Self::RegExp,
            Instruction::SetName(..) => Self::SetName,
            Instruction::ThrowReadOnly(..) => Self::ThrowReadOnly,
            Instruction::ThrowRedeclaration(..) => Self::ThrowRedeclaration,
            Instruction::ThrowDeleteSuper => Self::ThrowDeleteSuper,
            Instruction::Undefined => Self::Undefined,
            Instruction::Null => Self::Null,
            Instruction::PushFalse => Self::PushFalse,
            Instruction::PushTrue => Self::PushTrue,
            Instruction::PushThis => Self::PushThis,
            Instruction::PushActiveFunction => Self::PushActiveFunction,
            Instruction::PushHomeObject => Self::PushHomeObject,
            Instruction::PushNewTarget => Self::PushNewTarget,
            Instruction::Arguments(..) => Self::Arguments,
            Instruction::Rest(..) => Self::Rest,
            Instruction::VariableEnvironment => Self::VariableEnvironment,
            Instruction::HasEvalVariable { .. } => Self::HasEvalVariable,
            Instruction::GetEvalVariable { .. } => Self::GetEvalVariable,
            Instruction::PutEvalVariable { .. } => Self::PutEvalVariable,
            Instruction::DeleteEvalVariable { .. } => Self::DeleteEvalVariable,
            Instruction::DefineEvalVariable { .. } => Self::DefineEvalVariable,
            Instruction::ToObject => Self::ToObject,
            Instruction::HasDynamicBinding { .. } => Self::HasDynamicBinding,
            Instruction::GetDynamicBinding { .. } => Self::GetDynamicBinding,
            Instruction::PutDynamicBinding { .. } => Self::PutDynamicBinding,
            Instruction::DeleteDynamicBinding { .. } => Self::DeleteDynamicBinding,
            Instruction::DynamicEnvironmentObject(..) => Self::DynamicEnvironmentObject,
            Instruction::GlobalReference(..) => Self::GlobalReference,
            Instruction::GetRefValue(..) => Self::GetRefValue,
            Instruction::GetRefValueUndef(..) => Self::GetRefValueUndef,
            Instruction::PutRefValue(..) => Self::PutRefValue,
            Instruction::GetLocal(..) => Self::GetLocal,
            Instruction::PutLocal(..) => Self::PutLocal,
            Instruction::SetLocal(..) => Self::SetLocal,
            Instruction::SetLocalUninitialized(..) => Self::SetLocalUninitialized,
            Instruction::GetLocalCheck(..) => Self::GetLocalCheck,
            Instruction::InitializeLocal(..) => Self::InitializeLocal,
            Instruction::InitializeDerivedLocal(..) => Self::InitializeDerivedLocal,
            Instruction::PutLocalCheck(..) => Self::PutLocalCheck,
            Instruction::SetLocalCheck(..) => Self::SetLocalCheck,
            Instruction::GetArg(..) => Self::GetArg,
            Instruction::PutArg(..) => Self::PutArg,
            Instruction::SetArg(..) => Self::SetArg,
            Instruction::GetVarRef(..) => Self::GetVarRef,
            Instruction::PutVarRef(..) => Self::PutVarRef,
            Instruction::SetVarRef(..) => Self::SetVarRef,
            Instruction::GetVarRefCheck(..) => Self::GetVarRefCheck,
            Instruction::PutVarRefCheck(..) => Self::PutVarRefCheck,
            Instruction::InitializeVarRef(..) => Self::InitializeVarRef,
            Instruction::InitializeModuleImportCollision(..) => {
                Self::InitializeModuleImportCollision
            }
            Instruction::InitializeDerivedVarRef(..) => Self::InitializeDerivedVarRef,
            Instruction::CloseLocal(..) => Self::CloseLocal,
            Instruction::GetVar(..) => Self::GetVar,
            Instruction::GetVarUndef(..) => Self::GetVarUndef,
            Instruction::DeleteVar(..) => Self::DeleteVar,
            Instruction::PutVar(..) => Self::PutVar,
            Instruction::PutVarInit(..) => Self::PutVarInit,
            Instruction::InitializePrivateName(..) => Self::InitializePrivateName,
            Instruction::InitializePrivateMethod(..) => Self::InitializePrivateMethod,
            Instruction::InitializePrivateAccessor(..) => Self::InitializePrivateAccessor,
            Instruction::GetPrivateField(..) => Self::GetPrivateField,
            Instruction::GetPrivateField2(..) => Self::GetPrivateField2,
            Instruction::PutPrivateField(..) => Self::PutPrivateField,
            Instruction::DefinePrivateField(..) => Self::DefinePrivateField,
            Instruction::PrivateIn(..) => Self::PrivateIn,
            Instruction::GetField(..) => Self::GetField,
            Instruction::GetField2(..) => Self::GetField2,
            Instruction::GetArrayEl => Self::GetArrayEl,
            Instruction::GetArrayEl2 => Self::GetArrayEl2,
            Instruction::GetArrayEl3 => Self::GetArrayEl3,
            Instruction::GetSuper => Self::GetSuper,
            Instruction::GetSuperValue => Self::GetSuperValue,
            Instruction::GetSuperValueForCall => Self::GetSuperValueForCall,
            Instruction::ArrayFrom(..) => Self::ArrayFrom,
            Instruction::Object => Self::Object,
            Instruction::ToPropKey => Self::ToPropKey,
            Instruction::Insert2 => Self::Insert2,
            Instruction::Insert3 => Self::Insert3,
            Instruction::Dup3 => Self::Dup3,
            Instruction::Insert4 => Self::Insert4,
            Instruction::Perm3 => Self::Perm3,
            Instruction::Perm4 => Self::Perm4,
            Instruction::Perm5 => Self::Perm5,
            Instruction::Rot4Left => Self::Rot4Left,
            Instruction::PutField(..) => Self::PutField,
            Instruction::PutArrayEl => Self::PutArrayEl,
            Instruction::PutSuperValue => Self::PutSuperValue,
            Instruction::DefineField(..) => Self::DefineField,
            Instruction::DefineFieldComputed => Self::DefineFieldComputed,
            Instruction::DefineMethod { .. } => Self::DefineMethod,
            Instruction::DefineMethodComputed { .. } => Self::DefineMethodComputed,
            Instruction::DefineClass { .. } => Self::DefineClass,
            Instruction::InstallClassInstanceInitializer => Self::InstallClassInstanceInitializer,
            Instruction::CallClassInstanceInitializer => Self::CallClassInstanceInitializer,
            Instruction::RunClassStaticInitializer => Self::RunClassStaticInitializer,
            Instruction::CallClassStaticBlock => Self::CallClassStaticBlock,
            Instruction::DefineArrayEl => Self::DefineArrayEl,
            Instruction::SetNameComputed => Self::SetNameComputed,
            Instruction::SetProto => Self::SetProto,
            Instruction::CopyDataProperties => Self::CopyDataProperties,
            Instruction::CopyDataPropertiesExcluded { .. } => Self::CopyDataPropertiesExcluded,
            Instruction::Append => Self::Append,
            Instruction::Delete => Self::Delete,
            Instruction::Drop => Self::Drop,
            Instruction::Nip => Self::Nip,
            Instruction::Swap => Self::Swap,
            Instruction::Dup => Self::Dup,
            Instruction::Dup1 => Self::Dup1,
            Instruction::Neg => Self::Neg,
            Instruction::Plus => Self::Plus,
            Instruction::Inc => Self::Inc,
            Instruction::Dec => Self::Dec,
            Instruction::PostInc => Self::PostInc,
            Instruction::PostDec => Self::PostDec,
            Instruction::BitNot => Self::BitNot,
            Instruction::Not => Self::Not,
            Instruction::TypeOf => Self::TypeOf,
            Instruction::IsUndefinedOrNull => Self::IsUndefinedOrNull,
            Instruction::IsUndefined => Self::IsUndefined,
            Instruction::IsNull => Self::IsNull,
            Instruction::TypeOfIsUndefined => Self::TypeOfIsUndefined,
            Instruction::TypeOfIsFunction => Self::TypeOfIsFunction,
            Instruction::Add => Self::Add,
            Instruction::Sub => Self::Sub,
            Instruction::Mul => Self::Mul,
            Instruction::Div => Self::Div,
            Instruction::Mod => Self::Mod,
            Instruction::Pow => Self::Pow,
            Instruction::Shl => Self::Shl,
            Instruction::Sar => Self::Sar,
            Instruction::Shr => Self::Shr,
            Instruction::BitAnd => Self::BitAnd,
            Instruction::BitXor => Self::BitXor,
            Instruction::BitOr => Self::BitOr,
            Instruction::Eq => Self::Eq,
            Instruction::StrictEq => Self::StrictEq,
            Instruction::Neq => Self::Neq,
            Instruction::StrictNeq => Self::StrictNeq,
            Instruction::Lt => Self::Lt,
            Instruction::Lte => Self::Lte,
            Instruction::Gt => Self::Gt,
            Instruction::Gte => Self::Gte,
            Instruction::InstanceOf => Self::InstanceOf,
            Instruction::In => Self::In,
            Instruction::IfFalse(..) => Self::IfFalse,
            Instruction::IfTrue(..) => Self::IfTrue,
            Instruction::Goto(..) => Self::Goto,
            Instruction::Catch(..) => Self::Catch,
            Instruction::DropCatch => Self::DropCatch,
            Instruction::NipCatch => Self::NipCatch,
            Instruction::Gosub(..) => Self::Gosub,
            Instruction::Ret => Self::Ret,
            Instruction::DropGosub => Self::DropGosub,
            Instruction::ForOfStart => Self::ForOfStart,
            Instruction::ForAwaitOfStart => Self::ForAwaitOfStart,
            Instruction::ForOfNext(..) => Self::ForOfNext,
            Instruction::ForAwaitOfNext => Self::ForAwaitOfNext,
            Instruction::IteratorGetValueDone => Self::IteratorGetValueDone,
            Instruction::ForInStart => Self::ForInStart,
            Instruction::ForInNext => Self::ForInNext,
            Instruction::IteratorClose => Self::IteratorClose,
            Instruction::IteratorClosePreserve => Self::IteratorClosePreserve,
            Instruction::IteratorDropPreserve => Self::IteratorDropPreserve,
            Instruction::IteratorDetachPreserve => Self::IteratorDetachPreserve,
            Instruction::IteratorStart => Self::IteratorStart,
            Instruction::AsyncIteratorStart => Self::AsyncIteratorStart,
            Instruction::IteratorNext => Self::IteratorNext,
            Instruction::IteratorCall(..) => Self::IteratorCall,
            Instruction::IteratorCheckObject => Self::IteratorCheckObject,
            Instruction::InitialYield => Self::InitialYield,
            Instruction::Yield => Self::Yield,
            Instruction::YieldStar => Self::YieldStar,
            Instruction::AsyncYieldStar => Self::AsyncYieldStar,
            Instruction::Await => Self::Await,
            Instruction::ThrowIteratorMissingThrow => Self::ThrowIteratorMissingThrow,
            Instruction::Import => Self::Import,
            Instruction::Call(..) => Self::Call,
            Instruction::TailCall(..) => Self::TailCall,
            Instruction::Eval { .. } => Self::Eval,
            Instruction::CallMethod(..) => Self::CallMethod,
            Instruction::TailCallMethod(..) => Self::TailCallMethod,
            Instruction::Construct(..) => Self::Construct,
            Instruction::MarkSuperCall => Self::MarkSuperCall,
            Instruction::ConstructSuper(..) => Self::ConstructSuper,
            Instruction::CheckCtor => Self::CheckCtor,
            Instruction::InitDerivedConstructor => Self::InitDerivedConstructor,
            Instruction::Apply(..) => Self::Apply,
            Instruction::ApplySuper => Self::ApplySuper,
            Instruction::ApplyEval { .. } => Self::ApplyEval,
            Instruction::Return => Self::Return,
            Instruction::ReturnUndefined => Self::ReturnUndefined,
            Instruction::ReturnDerived(..) => Self::ReturnDerived,
            Instruction::Throw => Self::Throw,
        }
    }

    pub(crate) const fn operand_count(self) -> u8 {
        match self {
            Self::Nop
            | Self::ThrowDeleteSuper
            | Self::Undefined
            | Self::Null
            | Self::PushFalse
            | Self::PushTrue
            | Self::PushThis
            | Self::PushActiveFunction
            | Self::PushHomeObject
            | Self::PushNewTarget
            | Self::VariableEnvironment
            | Self::ToObject
            | Self::GetArrayEl
            | Self::GetArrayEl2
            | Self::GetArrayEl3
            | Self::GetArrayElDense
            | Self::GetArrayEl2Dense
            | Self::GetArrayEl3Dense
            | Self::GetSuper
            | Self::GetSuperValue
            | Self::GetSuperValueForCall
            | Self::Object
            | Self::ToPropKey
            | Self::Insert2
            | Self::Insert3
            | Self::Dup3
            | Self::Insert4
            | Self::Perm3
            | Self::Perm4
            | Self::Perm5
            | Self::Rot4Left
            | Self::PutArrayEl
            | Self::PutSuperValue
            | Self::DefineFieldComputed
            | Self::InstallClassInstanceInitializer
            | Self::CallClassInstanceInitializer
            | Self::RunClassStaticInitializer
            | Self::CallClassStaticBlock
            | Self::DefineArrayEl
            | Self::SetNameComputed
            | Self::SetProto
            | Self::CopyDataProperties
            | Self::Append
            | Self::Delete
            | Self::Drop
            | Self::Nip
            | Self::Swap
            | Self::Dup
            | Self::Dup1
            | Self::Neg
            | Self::Plus
            | Self::Inc
            | Self::Dec
            | Self::PostInc
            | Self::PostDec
            | Self::BitNot
            | Self::Not
            | Self::TypeOf
            | Self::IsUndefinedOrNull
            | Self::IsUndefined
            | Self::IsNull
            | Self::TypeOfIsUndefined
            | Self::TypeOfIsFunction
            | Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Mod
            | Self::Pow
            | Self::Shl
            | Self::Sar
            | Self::Shr
            | Self::BitAnd
            | Self::BitXor
            | Self::BitOr
            | Self::Eq
            | Self::StrictEq
            | Self::Neq
            | Self::StrictNeq
            | Self::Lt
            | Self::Lte
            | Self::Gt
            | Self::Gte
            | Self::InstanceOf
            | Self::In
            | Self::DropCatch
            | Self::NipCatch
            | Self::Ret
            | Self::DropGosub
            | Self::ForOfStart
            | Self::ForAwaitOfStart
            | Self::ForAwaitOfNext
            | Self::IteratorGetValueDone
            | Self::ForInStart
            | Self::ForInNext
            | Self::IteratorClose
            | Self::IteratorClosePreserve
            | Self::IteratorDropPreserve
            | Self::IteratorDetachPreserve
            | Self::IteratorStart
            | Self::AsyncIteratorStart
            | Self::IteratorNext
            | Self::IteratorCheckObject
            | Self::InitialYield
            | Self::Yield
            | Self::YieldStar
            | Self::AsyncYieldStar
            | Self::Await
            | Self::ThrowIteratorMissingThrow
            | Self::Import
            | Self::MarkSuperCall
            | Self::CheckCtor
            | Self::InitDerivedConstructor
            | Self::ApplySuper
            | Self::Return
            | Self::ReturnUndefined
            | Self::Throw => 0,
            Self::PushI32
            | Self::PushAtomValueIndex
            | Self::PushConst
            | Self::FClosure
            | Self::RegExp
            | Self::SetName
            | Self::ThrowReadOnly
            | Self::ThrowRedeclaration
            | Self::Arguments
            | Self::Rest
            | Self::DynamicEnvironmentObject
            | Self::GlobalReference
            | Self::GetRefValue
            | Self::GetRefValueUndef
            | Self::PutRefValue
            | Self::GetLocal
            | Self::NumberLocalInc
            | Self::NumberArgInc
            | Self::PutLocal
            | Self::SetLocal
            | Self::SetLocalUninitialized
            | Self::GetLocalCheck
            | Self::InitializeLocal
            | Self::InitializeDerivedLocal
            | Self::PutLocalCheck
            | Self::SetLocalCheck
            | Self::GetArg
            | Self::PutArg
            | Self::SetArg
            | Self::GetVarRef
            | Self::PutVarRef
            | Self::SetVarRef
            | Self::GetVarRefCheck
            | Self::PutVarRefCheck
            | Self::InitializeVarRef
            | Self::InitializeModuleImportCollision
            | Self::InitializeDerivedVarRef
            | Self::CloseLocal
            | Self::GetVar
            | Self::GetVarUndef
            | Self::DeleteVar
            | Self::PutVar
            | Self::PutVarInit
            | Self::InitializePrivateName
            | Self::InitializePrivateMethod
            | Self::InitializePrivateAccessor
            | Self::GetPrivateField
            | Self::GetPrivateField2
            | Self::PutPrivateField
            | Self::DefinePrivateField
            | Self::PrivateIn
            | Self::GetField
            | Self::GetField2
            | Self::GetFieldCached
            | Self::GetField2Cached
            | Self::ArrayFrom
            | Self::PutField
            | Self::DefineField
            | Self::IfFalse
            | Self::IfTrue
            | Self::Goto
            | Self::Catch
            | Self::Gosub
            | Self::ForOfNext
            | Self::IteratorCall
            | Self::Call
            | Self::TailCall
            | Self::CallMethod
            | Self::TailCallMethod
            | Self::Construct
            | Self::ConstructSuper
            | Self::Apply
            | Self::ApplyEval
            | Self::ReturnDerived => 1,
            Self::HasEvalVariable
            | Self::GetEvalVariable
            | Self::PutEvalVariable
            | Self::DeleteEvalVariable
            | Self::DefineEvalVariable
            | Self::HasDynamicBinding
            | Self::GetDynamicBinding
            | Self::PutDynamicBinding
            | Self::DeleteDynamicBinding
            | Self::DefineMethodComputed
            | Self::DefineClass
            | Self::Eval
            | Self::FieldAccSetDrop => 2,
            Self::DefineMethod
            | Self::CopyDataPropertiesExcluded
            | Self::DensePreUpdateLocal
            | Self::DensePreUpdateArg
            | Self::DenseReadLocal
            | Self::DenseReadArg
            | Self::DenseReadBinaryLocal
            | Self::DenseReadBinaryArg
            | Self::DenseIndexBinaryLocal
            | Self::DenseIndexBinaryArg
            | Self::DenseAccIndexSetDrop
            | Self::BorrowedFieldLocal
            | Self::BorrowedFieldArg
            | Self::CompareBranchLocal
            | Self::CompareBranchArg
            | Self::CompareBranchLocalLt
            | Self::CompareBranchArgLt => 3,
            Self::CompareBranchStack => 2,
            Self::UpdateLocalDiscard | Self::UpdateLocalDiscardCheck => 1,
            Self::DensePostUpdateLocal
            | Self::DensePostUpdateLocalCheck
            | Self::DensePostUpdateArg => 3,
            Self::NumericArrayAccumulate
            | Self::NumericArrayStoreProduct
            | Self::NumericArrayCopyElement
            | Self::NumericArrayAddPreInc
            | Self::NumericArrayStoreAndLocal
            | Self::NumericArrayUpdateElement
            | Self::NumericArrayCompareBranch => 3,
        }
    }

    pub(crate) const fn target_operand(self) -> Option<u8> {
        match self {
            Self::IfFalse | Self::IfTrue | Self::Goto | Self::Catch | Self::Gosub => Some(0),
            Self::CompareBranchStack => Some(1),
            _ => None,
        }
    }
}
