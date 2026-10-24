//! The MIR operations: the core's opcodes without register moves, fused
//! compare-jumps, immediate forms and control transfers, which the lowering
//! chooses. Each operation names the core opcodes it lowers to and carries
//! its effects; `tests` checks that every core opcode is either named here or
//! deliberately left to the lowering.

use oxide_core::bytecode::Opcode;

/// What an operation may do besides computing its results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effects(u8);

impl Effects {
    pub const NONE: Self = Self(0);
    /// It may throw: its block's handler edge is live.
    pub const THROW: Self = Self(1);
    /// It reads the heap (properties, cells, globals).
    pub const READ: Self = Self(1 << 1);
    /// It writes the heap.
    pub const WRITE: Self = Self(1 << 2);
    /// It allocates: a collection safepoint.
    pub const ALLOC: Self = Self(1 << 3);
    /// It may run arbitrary JavaScript (a call, a getter, a coercion).
    pub const CALL: Self = Self(1 << 4);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn is_pure(self) -> bool {
        self.0 == 0
    }
}

// Running arbitrary JavaScript does everything.
const ANY: Effects = Effects(0b1_1111);
const THROW: Effects = Effects::THROW;
const READ: Effects = Effects::READ;
const WRITE: Effects = Effects::WRITE;
const ALLOC: Effects = Effects::ALLOC;
const PURE: Effects = Effects::NONE;
const fn e(a: Effects, b: Effects) -> Effects {
    a.union(b)
}

/// How many value operands an operation takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arity {
    Fixed(u8),
    /// Any number, at least the given one.
    AtLeast(u8),
}

/// What an instruction's immediate holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImmKind {
    None,
    /// An `i32` literal.
    Int,
    /// A constant-pool index.
    Const,
    /// A property key.
    Key,
    /// A property key and a flags byte.
    KeyFlags,
    /// A flags or count word.
    Flags,
    /// An index into the running function's captures.
    Capture,
    /// An index into the unit's children.
    Child,
    /// A variable of the function.
    Var,
}

macro_rules! ops {
    ($($(#[$doc:meta])* $name:ident($arity:expr, $results:literal, $imm:ident, $effects:expr) => [$($core:ident),*];)*) => {
        /// A MIR operation.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Op {
            $($(#[$doc])* $name,)*
        }

        impl Op {
            /// Every operation, in declaration order.
            pub const ALL: &'static [Op] = &[$(Op::$name,)*];

            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Op::$name => stringify!($name),)*
                }
            }

            #[must_use]
            pub const fn arity(self) -> Arity {
                match self {
                    $(Op::$name => $arity,)*
                }
            }

            /// The number of SSA values it defines.
            #[must_use]
            pub const fn results(self) -> u8 {
                match self {
                    $(Op::$name => $results,)*
                }
            }

            #[must_use]
            pub const fn imm(self) -> ImmKind {
                match self {
                    $(Op::$name => ImmKind::$imm,)*
                }
            }

            #[must_use]
            pub const fn effects(self) -> Effects {
                match self {
                    $(Op::$name => $effects,)*
                }
            }

            /// The core opcodes the lowering may emit for it, the plain form
            /// first; the lowering picks immediate and fused forms itself.
            #[must_use]
            pub const fn core(self) -> &'static [Opcode] {
                match self {
                    $(Op::$name => &[$(Opcode::$core),*],)*
                }
            }
        }
    };
}

use Arity::{AtLeast, Fixed};

ops! {
    // ---- constants ----
    Const(Fixed(0), 1, Const, PURE) => [LoadConst];
    ConstInt(Fixed(0), 1, Int, PURE) => [LoadInt];
    Undefined(Fixed(0), 1, None, PURE) => [LoadUndefined];
    Null(Fixed(0), 1, None, PURE) => [LoadNull];
    True(Fixed(0), 1, None, PURE) => [LoadTrue];
    False(Fixed(0), 1, None, PURE) => [LoadFalse];
    /// The TDZ marker.
    Empty(Fixed(0), 1, None, PURE) => [LoadEmpty];
    /// The running function object.
    Function(Fixed(0), 1, None, PURE) => [LoadFunction];
    NewTarget(Fixed(0), 1, None, PURE) => [LoadNewTarget];
    HomeObject(Fixed(0), 1, None, PURE) => [LoadHomeObject];

    // ---- variables: one register each for the whole function ----
    GetVar(Fixed(0), 1, Var, PURE) => [];
    SetVar(Fixed(1), 0, Var, WRITE) => [];
    /// A copy, where the builder needs a fresh value.
    Copy(Fixed(1), 1, None, PURE) => [];
    /// An iterator record's iterator, its first register.
    IterIterator(Fixed(1), 1, None, PURE) => [];

    // ---- unary ----
    Neg(Fixed(1), 1, None, ANY) => [Neg];
    Plus(Fixed(1), 1, None, ANY) => [Plus];
    ToNumeric(Fixed(1), 1, None, ANY) => [ToNumeric];
    Inc(Fixed(1), 1, None, ANY) => [Inc];
    Dec(Fixed(1), 1, None, ANY) => [Dec];
    BitNot(Fixed(1), 1, None, ANY) => [BitNot];
    Not(Fixed(1), 1, None, PURE) => [Not];
    TypeOf(Fixed(1), 1, None, PURE) => [TypeOf];
    ToPropertyKey(Fixed(1), 1, None, ANY) => [ToPropertyKey];
    IsUndefinedOrNull(Fixed(1), 1, None, PURE) => [IsUndefinedOrNull];
    IsUndefined(Fixed(1), 1, None, PURE) => [IsUndefined];
    IsNull(Fixed(1), 1, None, PURE) => [IsNull];
    TypeOfIsUndefined(Fixed(1), 1, None, PURE) => [TypeOfIsUndefined];
    TypeOfIsFunction(Fixed(1), 1, None, PURE) => [TypeOfIsFunction];
    ToObject(Fixed(1), 1, None, e(THROW, ALLOC)) => [ToObject];

    // ---- binary: the immediate forms are the lowering's choice ----
    Add(Fixed(2), 1, None, ANY) => [Add, AddI];
    Sub(Fixed(2), 1, None, ANY) => [Sub, SubI];
    Mul(Fixed(2), 1, None, ANY) => [Mul, MulI];
    Div(Fixed(2), 1, None, ANY) => [Div];
    Mod(Fixed(2), 1, None, ANY) => [Mod];
    Pow(Fixed(2), 1, None, ANY) => [Pow];
    Shl(Fixed(2), 1, None, ANY) => [Shl, ShlI];
    Sar(Fixed(2), 1, None, ANY) => [Sar, SarI];
    Shr(Fixed(2), 1, None, ANY) => [Shr, ShrI];
    BitAnd(Fixed(2), 1, None, ANY) => [BitAnd, BitAndI];
    BitOr(Fixed(2), 1, None, ANY) => [BitOr, BitOrI];
    BitXor(Fixed(2), 1, None, ANY) => [BitXor, BitXorI];
    Eq(Fixed(2), 1, None, ANY) => [Eq];
    Neq(Fixed(2), 1, None, ANY) => [Neq];
    StrictEq(Fixed(2), 1, None, PURE) => [StrictEq];
    StrictNeq(Fixed(2), 1, None, PURE) => [StrictNeq];
    Lt(Fixed(2), 1, None, ANY) => [Lt];
    Lte(Fixed(2), 1, None, ANY) => [Lte];
    Gt(Fixed(2), 1, None, ANY) => [Gt];
    Gte(Fixed(2), 1, None, ANY) => [Gte];
    InstanceOf(Fixed(2), 1, None, ANY) => [InstanceOf];
    In(Fixed(2), 1, None, ANY) => [In];
    /// `ToPropertyKey(key)` for base `base` of a compound assignment: key,
    /// base.
    ToPropertyKeyOf(Fixed(2), 1, None, ANY) => [ToPropertyKeyOf];
    /// A `+` chain joined once: the operands in order.
    ConcatN(AtLeast(2), 1, None, ANY) => [ConcatN];

    // ---- TDZ ----
    /// Throws `ReferenceError` naming the key when the operand is the TDZ
    /// marker.
    CheckInit(Fixed(1), 0, Key, THROW) => [CheckInit];

    // ---- properties ----
    /// obj → value.
    GetField(Fixed(1), 1, Key, ANY) => [GetField];
    /// obj, value.
    SetField(Fixed(2), 0, Key, ANY) => [SetField];
    /// obj, value: an own data property of a fresh literal.
    DefineField(Fixed(2), 0, Key, e(e(THROW, WRITE), ALLOC)) => [DefineField];
    DeleteField(Fixed(1), 1, Key, ANY) => [DeleteField];
    /// obj, function.
    DefineGetter(Fixed(2), 0, Key, e(e(THROW, WRITE), ALLOC)) => [DefineGetter];
    DefineSetter(Fixed(2), 0, Key, e(e(THROW, WRITE), ALLOC)) => [DefineSetter];
    /// obj, key → value.
    GetElem(Fixed(2), 1, None, ANY) => [GetElem];
    /// obj, key, value.
    SetElem(Fixed(3), 0, None, ANY) => [SetElem];
    DefineElem(Fixed(3), 0, None, ANY) => [DefineElem];
    DeleteElem(Fixed(2), 1, None, ANY) => [DeleteElem];

    // ---- globals ----
    GetGlobal(Fixed(0), 1, Key, ANY) => [GetGlobal];
    GetGlobalOrUndefined(Fixed(0), 1, Key, ANY) => [GetGlobalOrUndefined];
    SetGlobal(Fixed(1), 0, Key, ANY) => [SetGlobal];
    DefineGlobal(Fixed(1), 0, KeyFlags, ANY) => [DefineGlobal];
    DeleteGlobal(Fixed(0), 1, Key, ANY) => [DeleteGlobal];
    DeclareGlobal(Fixed(0), 0, KeyFlags, ANY) => [DeclareGlobal];
    DeclareGlobalLet(Fixed(0), 0, Key, ANY) => [DeclareGlobalLet];
    DeclareGlobalConst(Fixed(0), 0, Key, ANY) => [DeclareGlobalConst];
    CheckGlobalVar(Fixed(0), 0, KeyFlags, ANY) => [CheckGlobalVar];
    CheckGlobalLexical(Fixed(0), 0, Key, ANY) => [CheckGlobalLexical];
    /// The environment holding global `key`.
    GlobalRef(Fixed(0), 1, Key, ANY) => [GlobalRef];
    /// env → binding `key` of it; the flag 1 reads a missing one as
    /// `undefined`.
    GetRef(Fixed(1), 1, KeyFlags, ANY) => [GetRef];
    /// env, value.
    PutRef(Fixed(2), 0, Key, ANY) => [PutRef];

    // ---- cells: a captured binding's cell is an ordinary value ----
    NewCell(Fixed(1), 1, None, ALLOC) => [NewCell];
    NewCellEmpty(Fixed(0), 1, None, ALLOC) => [NewCellEmpty];
    /// cell → a fresh cell with its value (a `for (let …)` iteration); the
    /// result takes the operand's register.
    CloseCell(Fixed(1), 1, None, ALLOC) => [CloseCell];
    LoadCell(Fixed(1), 1, None, READ) => [LoadCell];
    /// cell, value.
    StoreCell(Fixed(2), 0, None, WRITE) => [StoreCell];
    CheckCellInit(Fixed(1), 0, Key, e(THROW, READ)) => [CheckCellInit];
    /// Puts the cell back in the TDZ.
    ClearCell(Fixed(1), 0, None, WRITE) => [ClearCell];
    LoadCaptured(Fixed(0), 1, Capture, READ) => [LoadCaptured];
    StoreCaptured(Fixed(1), 0, Capture, WRITE) => [StoreCaptured];

    // ---- arguments and rest ----
    /// The flag 1 for the mapped form.
    NewArguments(Fixed(0), 1, Int, ALLOC) => [NewArguments];
    /// The arguments from the given index.
    Rest(Fixed(0), 1, Int, ALLOC) => [Rest];

    // ---- objects and functions ----
    NewObject(Fixed(0), 1, Flags, ALLOC) => [NewObject];
    /// The elements in order.
    NewArray(AtLeast(0), 1, None, ALLOC) => [NewArray];
    /// The cells and captures the child's capture list names, in its order.
    MakeClosure(AtLeast(0), 1, Child, ALLOC) => [MakeClosure];
    /// The source and flags are the constant and the next one.
    NewRegExp(Fixed(0), 1, Const, e(THROW, ALLOC)) => [NewRegExp];
    /// array, index, source → the next index (in the index's register).
    AppendSpread(Fixed(3), 1, None, ANY) => [AppendSpread];
    /// object, prototype.
    SetProto(Fixed(2), 0, None, WRITE) => [SetProto];
    /// object, function; the method flags in the immediate.
    DefineMethod(Fixed(2), 0, KeyFlags, e(e(THROW, WRITE), ALLOC)) => [DefineMethod];
    /// object, key, function.
    DefineMethodComputed(Fixed(3), 0, Flags, e(e(THROW, WRITE), ALLOC)) => [DefineMethodComputed];
    /// function, key.
    SetFunctionName(Fixed(2), 0, None, e(THROW, WRITE)) => [SetFunctionName];
    /// target, source.
    CopyData(Fixed(2), 0, None, ANY) => [CopyData];
    /// target, source, excluded keys.
    CopyDataExcluded(Fixed(3), 0, None, ANY) => [CopyDataExcluded];
    /// specifier, options.
    ImportCall(Fixed(2), 1, None, ANY) => [ImportCall];

    // ---- calls: the lowering lays the operands out in the callee window ----
    /// callee, arguments…
    Call(AtLeast(1), 1, None, ANY) => [Call, TailCall];
    /// A call written as a direct `eval`, which runs as indirect eval: the
    /// backtrace lists no native frame for it. It lowers to `Call`.
    CallEval(AtLeast(1), 1, None, ANY) => [];
    /// receiver, callee, arguments…
    CallMethod(AtLeast(2), 1, None, ANY) => [CallMethod, TailCallMethod];
    /// callee, new.target, arguments…
    New(AtLeast(2), 1, None, ANY) => [New];
    /// callee, receiver, arguments array.
    CallSpread(Fixed(3), 1, None, ANY) => [CallSpread];
    /// callee, new.target, arguments array.
    NewSpread(Fixed(3), 1, None, ANY) => [NewSpread];

    // ---- iteration: a record is a value three registers wide ----
    /// iterable → record; the immediate 1 forces the generic protocol.
    IterStart(Fixed(1), 1, Int, ANY) => [IterStart];
    /// iterable → async record.
    IterStartAsync(Fixed(1), 1, None, ANY) => [IterStartAsync];
    /// record → value, done.
    IterNext(Fixed(1), 2, None, ANY) => [IterNext];
    IterClose(Fixed(1), 0, None, ANY) => [IterClose];
    /// record, thrown value.
    IterCloseThrow(Fixed(2), 0, None, ANY) => [IterCloseThrow];
    /// record, value → result.
    IterCallNext(Fixed(2), 1, None, ANY) => [IterCallNext];
    /// record, value → result, whether the method was missing; the
    /// immediate 0 calls `return`, 1 `throw`.
    IterCall(Fixed(2), 2, Int, ANY) => [IterCall];
    IterCheckObject(Fixed(1), 0, None, THROW) => [IterCheckObject];
    /// record → the raw `next()` result.
    IterNextRaw(Fixed(1), 1, None, ANY) => [IterNextRaw];
    /// result, record → value, done.
    IterResult(Fixed(2), 2, None, ANY) => [IterResult];
    /// object → enumeration.
    ForInStart(Fixed(1), 1, None, ANY) => [ForInStart];

    // ---- classes ----
    /// parent, constructor closure → class, prototype; the flag 1 when it
    /// has a heritage. The results take the operands' registers.
    MakeClass(Fixed(2), 2, KeyFlags, ANY) => [MakeClass];
    /// constructor, home object, initializer.
    InstallFields(Fixed(3), 0, None, WRITE) => [InstallFields];
    /// this, constructor.
    InitFields(Fixed(2), 0, None, ANY) => [InitFields];
    /// home object, initializer.
    RunStaticInit(Fixed(2), 0, None, ANY) => [RunStaticInit];
    CallStaticBlock(Fixed(1), 0, None, ANY) => [CallStaticBlock];
    /// home object → its prototype.
    GetSuper(Fixed(1), 1, None, e(THROW, READ)) => [GetSuper];
    /// receiver, base, key → value; the immediate 1 for the call form.
    GetSuperValue(Fixed(3), 1, Int, ANY) => [GetSuperValue];
    /// receiver, base, key, value.
    PutSuperValue(Fixed(4), 0, None, ANY) => [PutSuperValue];
    /// new.target → the implicit derived constructor's result.
    InitDerived(Fixed(1), 1, None, ANY) => [InitDerived];
    /// this, value → this bound (in this's register).
    InitThis(Fixed(2), 1, None, THROW) => [InitThis];
    /// cell, value.
    InitThisCell(Fixed(2), 0, None, e(THROW, WRITE)) => [InitThisCell];
    InitThisCaptured(Fixed(1), 0, Capture, e(THROW, WRITE)) => [InitThisCaptured];
    /// value, this → the result (in the value's register).
    CheckDerivedReturn(Fixed(2), 1, None, THROW) => [CheckDerivedReturn];
    /// new.target.
    CheckCtor(Fixed(1), 0, None, THROW) => [CheckCtor];

    // ---- private names ----
    NewPrivateName(Fixed(0), 1, Key, ALLOC) => [NewPrivateName];
    /// method, home object.
    SetHomeObject(Fixed(2), 0, None, WRITE) => [SetHomeObject];
    /// method; the key and the prefix (0 none, 1 `get `, 2 `set `).
    NamePrivateMethod(Fixed(1), 0, KeyFlags, WRITE) => [NamePrivateMethod];
    /// object, name → value.
    GetPrivate(Fixed(2), 1, None, e(THROW, READ)) => [GetPrivate];
    /// object, value, name.
    PutPrivate(Fixed(3), 0, None, e(THROW, WRITE)) => [PutPrivate];
    DefinePrivate(Fixed(3), 0, None, e(e(THROW, WRITE), ALLOC)) => [DefinePrivate];
    /// object, name.
    PrivateIn(Fixed(2), 1, None, THROW) => [PrivateIn];
    GetPrivateMethod(Fixed(2), 1, None, THROW) => [GetPrivateMethod];
    CallPrivateGetter(Fixed(2), 1, None, ANY) => [CallPrivateGetter];
    /// object, value, setter.
    CallPrivateSetter(Fixed(3), 0, None, ANY) => [CallPrivateSetter];
    PrivateBrandIn(Fixed(2), 1, None, THROW) => [PrivateBrandIn];

    // ---- function entry ----
    /// An async function's start: its promise becomes the call's result.
    AsyncStart(Fixed(0), 0, None, ANY) => [AsyncStart];
}

impl Op {
    /// The operand whose register each result must take, for operations the
    /// core runs in place.
    #[must_use]
    pub const fn tied(self) -> [Option<u8>; 2] {
        match self {
            Op::CloseCell | Op::InitThis | Op::CheckDerivedReturn => [Some(0), None],
            Op::AppendSpread => [Some(1), None],
            Op::MakeClass => [Some(0), Some(1)],
            _ => [None, None],
        }
    }

    /// Whether removing an unused instance changes nothing observable.
    #[must_use]
    pub const fn removable(self) -> bool {
        !self.effects().contains(Effects::THROW)
            && !self.effects().contains(Effects::WRITE)
            && !self.effects().contains(Effects::CALL)
    }
}

/// Core opcodes no MIR operation names: the lowering makes them from
/// register allocation, terminators and fusion.
pub const LOWERING_ONLY: &[Opcode] = &[
    Opcode::Nop,
    Opcode::Mov,
    Opcode::Swap,
    Opcode::LoadWide,
    Opcode::StoreWide,
    Opcode::NewSpill,
    Opcode::LoadSlot,
    Opcode::StoreSlot,
    Opcode::Jmp,
    Opcode::JmpBack,
    Opcode::JTrue,
    Opcode::JFalse,
    Opcode::JUndefinedOrNull,
    Opcode::Gosub,
    Opcode::Ret,
    Opcode::JLt,
    Opcode::JLte,
    Opcode::JGt,
    Opcode::JGte,
    Opcode::JNlt,
    Opcode::JNlte,
    Opcode::JNgt,
    Opcode::JNgte,
    Opcode::JEq,
    Opcode::JNeq,
    Opcode::JStrictEq,
    Opcode::JStrictNeq,
    Opcode::JLtI,
    Opcode::JLteI,
    Opcode::JGtI,
    Opcode::JGteI,
    Opcode::JNltI,
    Opcode::JNlteI,
    Opcode::JNgtI,
    Opcode::JNgteI,
    Opcode::JEqI,
    Opcode::JNeqI,
    Opcode::JStrictEqI,
    Opcode::JStrictNeqI,
    Opcode::ForInNext,
    Opcode::Return,
    Opcode::ReturnUndefined,
    Opcode::Throw,
    Opcode::ThrowConstAssign,
    Opcode::ThrowType,
    Opcode::ThrowReference,
    Opcode::ThrowInternal,
    Opcode::InitialYield,
    Opcode::Yield,
    Opcode::YieldStar,
    Opcode::Await,
];
