//! Numeric execution opcodes shared by the publisher, verifier, disassembler and VM.
//! The compiler Instruction is consumed during publication.
use super::bytecode::Instruction;

// Keep the published discriminants and checked conversion in one list. A
// checked match exposes the identity mapping to LLVM without a table load.
macro_rules! execution_opcodes {
    ($($(#[$attr:meta])* $name:ident = $raw:literal,)*) => {
        #[repr(u16)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum Opcode {
            $($(#[$attr])* $name = $raw,)*
        }

        // The published VM dispatches these verified tags directly. Keep the
        // wire tags and the checked external decoder in the same definition.
        #[allow(dead_code, non_upper_case_globals)]
        pub(crate) mod tags {
            $($(#[$attr])* pub(crate) const $name: u16 = $raw;)*
        }

        impl Opcode {
            #[cfg(feature = "profiling")]
            pub(crate) fn profiling_label(self) -> &'static str {
                match self {
                    $(Self::$name => concat!("opcode.", stringify!($name)),)*
                }
            }

            #[inline(always)]
            pub(crate) fn from_raw(raw: u16) -> Option<Self> {
                match raw {
                    $($raw => Some(Self::$name),)*
                    _ => None,
                }
            }
        }
    };
}

execution_opcodes! {
    Nop = 0,
    PushI32 = 1,
    PushAtomValueIndex = 2,
    PushConst = 3,
    FClosure = 4,
    RegExp = 5,
    SetName = 6,
    ThrowReadOnly = 7,
    ThrowRedeclaration = 8,
    ThrowDeleteSuper = 9,
    Undefined = 10,
    Null = 11,
    PushFalse = 12,
    PushTrue = 13,
    PushThis = 14,
    PushActiveFunction = 15,
    PushHomeObject = 16,
    PushNewTarget = 17,
    Arguments = 18,
    Rest = 19,
    VariableEnvironment = 20,
    HasEvalVariable = 21,
    GetEvalVariable = 22,
    PutEvalVariable = 23,
    DeleteEvalVariable = 24,
    DefineEvalVariable = 25,
    ToObject = 26,
    HasDynamicBinding = 27,
    GetDynamicBinding = 28,
    PutDynamicBinding = 29,
    DeleteDynamicBinding = 30,
    DynamicEnvironmentObject = 31,
    GlobalReference = 32,
    GetRefValue = 33,
    GetRefValueUndef = 34,
    PutRefValue = 35,
    GetLocal = 36,
    PutLocal = 37,
    SetLocal = 38,
    SetLocalUninitialized = 39,
    GetLocalCheck = 40,
    InitializeLocal = 41,
    InitializeDerivedLocal = 42,
    PutLocalCheck = 43,
    SetLocalCheck = 44,
    GetArg = 45,
    PutArg = 46,
    SetArg = 47,
    GetVarRef = 48,
    PutVarRef = 49,
    SetVarRef = 50,
    GetVarRefCheck = 51,
    PutVarRefCheck = 52,
    InitializeVarRef = 53,
    InitializeModuleImportCollision = 54,
    InitializeDerivedVarRef = 55,
    CloseLocal = 56,
    GetVar = 57,
    GetVarUndef = 58,
    DeleteVar = 59,
    PutVar = 60,
    PutVarInit = 61,
    InitializePrivateName = 62,
    InitializePrivateMethod = 63,
    InitializePrivateAccessor = 64,
    GetPrivateField = 65,
    GetPrivateField2 = 66,
    PutPrivateField = 67,
    DefinePrivateField = 68,
    PrivateIn = 69,
    GetField = 70,
    GetField2 = 71,
    GetArrayEl = 72,
    GetArrayEl2 = 73,
    GetArrayEl3 = 74,
    GetSuper = 75,
    GetSuperValue = 76,
    GetSuperValueForCall = 77,
    ArrayFrom = 78,
    Object = 79,
    ToPropKey = 80,
    Insert2 = 81,
    Insert3 = 82,
    Dup3 = 83,
    Insert4 = 84,
    Perm3 = 85,
    Perm4 = 86,
    Perm5 = 87,
    Rot4Left = 88,
    PutField = 89,
    PutArrayEl = 90,
    PutSuperValue = 91,
    DefineField = 92,
    DefineFieldComputed = 93,
    DefineMethod = 94,
    DefineMethodComputed = 95,
    DefineClass = 96,
    InstallClassInstanceInitializer = 97,
    CallClassInstanceInitializer = 98,
    RunClassStaticInitializer = 99,
    CallClassStaticBlock = 100,
    DefineArrayEl = 101,
    SetNameComputed = 102,
    SetProto = 103,
    CopyDataProperties = 104,
    CopyDataPropertiesExcluded = 105,
    Append = 106,
    Delete = 107,
    Drop = 108,
    Nip = 109,
    Swap = 110,
    Dup = 111,
    Dup1 = 112,
    Neg = 113,
    Plus = 114,
    Inc = 115,
    Dec = 116,
    PostInc = 117,
    PostDec = 118,
    BitNot = 119,
    Not = 120,
    TypeOf = 121,
    IsUndefinedOrNull = 122,
    IsUndefined = 123,
    IsNull = 124,
    TypeOfIsUndefined = 125,
    TypeOfIsFunction = 126,
    Add = 127,
    Sub = 128,
    Mul = 129,
    Div = 130,
    Mod = 131,
    Pow = 132,
    Shl = 133,
    Sar = 134,
    Shr = 135,
    BitAnd = 136,
    BitXor = 137,
    BitOr = 138,
    Eq = 139,
    StrictEq = 140,
    Neq = 141,
    StrictNeq = 142,
    Lt = 143,
    Lte = 144,
    Gt = 145,
    Gte = 146,
    InstanceOf = 147,
    In = 148,
    IfFalse = 149,
    IfTrue = 150,
    Goto = 151,
    Catch = 152,
    DropCatch = 153,
    NipCatch = 154,
    Gosub = 155,
    Ret = 156,
    DropGosub = 157,
    ForOfStart = 158,
    ForAwaitOfStart = 159,
    ForOfNext = 160,
    ForAwaitOfNext = 161,
    IteratorGetValueDone = 162,
    ForInStart = 163,
    ForInNext = 164,
    IteratorClose = 165,
    IteratorClosePreserve = 166,
    IteratorDropPreserve = 167,
    IteratorDetachPreserve = 168,
    IteratorStart = 169,
    AsyncIteratorStart = 170,
    IteratorNext = 171,
    IteratorCall = 172,
    IteratorCheckObject = 173,
    InitialYield = 174,
    Yield = 175,
    YieldStar = 176,
    AsyncYieldStar = 177,
    Await = 178,
    ThrowIteratorMissingThrow = 179,
    Import = 180,
    Call = 181,
    TailCall = 182,
    Eval = 183,
    CallMethod = 184,
    TailCallMethod = 185,
    Construct = 186,
    MarkSuperCall = 187,
    ConstructSuper = 188,
    CheckCtor = 189,
    InitDerivedConstructor = 190,
    Apply = 191,
    ApplySuper = 192,
    ApplyEval = 193,
    Return = 194,
    ReturnUndefined = 195,
    ReturnDerived = 196,
    Throw = 197,
    /// A published GetLocal + PushI32(1) + Add span. On a guard miss the
    /// same word acts as GetLocal and the following generic words run.
    NumberLocalInc = 198,
    /// Planned numeric Array read, multiplication, addition and direct local
    /// commit. The original words remain the generic continuation.
    NumericArrayAccumulate = 199,
    NumberArgInc = 200,
    /// Field reads with a direct location-cache probe in the execution loop.
    GetFieldCached = 201,
    GetField2Cached = 202,
    GetArrayElDense = 203,
    GetArrayEl2Dense = 204,
    GetArrayEl3Dense = 205,
    /// Published five-operation base/index update and dense read. Operand 1
    /// carries the updated slot and operation; operand 2 is the verified end PC.
    DensePreUpdateLocal = 206,
    DensePreUpdateArg = 207,
    DenseReadLocal = 208,
    DenseReadArg = 209,
    BorrowedFieldLocal = 210,
    BorrowedFieldArg = 211,
    CompareBranchLocal = 212,
    CompareBranchArg = 213,
    CompareBranchStack = 214,
    UpdateLocalDiscard = 215,
    UpdateLocalDiscardCheck = 216,
    DensePostUpdateLocal = 217,
    DensePostUpdateLocalCheck = 218,
    DensePostUpdateArg = 219,
    DenseReadBinaryLocal = 220,
    DenseReadBinaryArg = 221,
    DenseIndexBinaryLocal = 222,
    DenseIndexBinaryArg = 223,
    DenseAccIndexSetDrop = 224,
    FieldAccSetDrop = 225,
    CompareBranchLocalLt = 226,
    CompareBranchArgLt = 227,
    NumericArrayStoreProduct = 228,
    NumericArrayCopyElement = 229,
    NumericArrayAddPreInc = 230,
    NumericArrayStoreAndLocal = 231,
    NumericArrayUpdateElement = 232,
    NumericArrayCompareBranch = 233,
    BorrowedFieldThis = 234,
}

impl Opcode {
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
            | Self::BorrowedFieldThis
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

#[cfg(test)]
mod tests {
    use super::Opcode;

    #[test]
    fn numeric_opcode_conversion_rejects_unassigned_discriminants() {
        // Keep the existing execution-word assignments stable while changing
        // how a numeric opcode is authenticated.
        assert_eq!(Opcode::BorrowedFieldThis as u16, 234);
        for raw in 0..=u16::MAX {
            match Opcode::from_raw(raw) {
                Some(opcode) => {
                    assert_eq!(opcode as u16, raw);
                    assert!(raw <= 234);
                }
                None => assert!(raw > 234),
            }
        }
    }
}
