//! The opcode table: one line per opcode with its operand format.

/// How an instruction's `a`, `b`, `c` and `imm` fields are read. The
/// validator, the disassembler and the builder's checks all go through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// No operand.
    None,
    /// `a` register.
    A,
    /// `a`, `b` registers.
    AB,
    /// `a`, `b`, `c` registers.
    ABC,
    /// `a` register, `imm` an `i32` literal.
    AInt,
    /// `a`, `b` registers, `imm` an `i32` literal.
    ABInt,
    /// `a` register, `imm` a constant-pool index.
    AConst,
    /// `imm` a `PropertyKey`.
    Key,
    /// `a` register, `imm` a `PropertyKey`.
    AKey,
    /// `a`, `b` registers, `imm` a `PropertyKey`.
    ABKey,
    /// `a` register, `imm` a scope slot: depth in the high 16 bits, index in
    /// the low 16.
    AScope,
    /// `imm` a count (scope slots).
    Count,
    /// `a` register, `imm` a count (object slots).
    ACount,
    /// `a` register, `b` the first of `imm` consecutive registers.
    ABRange,
    /// `a` register, `imm` an index into the unit's children.
    AChild,
    /// `a` destination, `b` callee, `c` argument count: the callee is in `b`,
    /// the receiver in `b + 1`, the arguments in `b + 2 ..= b + 1 + c`.
    Call,
    /// `imm` a jump target.
    Jump,
    /// `a` register, `imm` a jump target.
    AJump,
    /// `a`, `b` registers, `imm` a jump target.
    ABJump,
    /// `a` register, `b | c << 8` an `i16` literal, `imm` a jump target.
    AShortJump,
    /// `a` register, `imm` a register above the 8-bit window.
    AWide,
}

macro_rules! opcodes {
    ($($(#[$doc:meta])* $name:ident: $format:ident,)*) => {
        /// A register instruction's operation. The lowering builds instructions
        /// as Rust values, so the main loop never decodes or validates an
        /// opcode byte.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum Opcode {
            $($(#[$doc])* $name,)*
        }

        impl Opcode {
            /// Every opcode, in discriminant order.
            pub const ALL: &'static [Opcode] = &[$(Opcode::$name,)*];

            #[inline]
            pub const fn format(self) -> Format {
                match self {
                    $(Opcode::$name => Format::$format,)*
                }
            }

            pub const fn name(self) -> &'static str {
                match self {
                    $(Opcode::$name => stringify!($name),)*
                }
            }
        }
    };
}

opcodes! {
    Nop: None,

    // ---- moves and constants ----
    /// `a = b`.
    Mov: AB,
    /// `a = register imm`, for a register above the window.
    LoadWide: AWide,
    /// `register imm = a`.
    StoreWide: AWide,
    LoadInt: AInt,
    LoadConst: AConst,
    LoadUndefined: A,
    LoadNull: A,
    LoadTrue: A,
    LoadFalse: A,
    /// The TDZ marker, for a lexical binding entering scope.
    LoadEmpty: A,
    /// The running function object, for a named function expression.
    LoadFunction: A,
    /// Throws `ReferenceError` naming key `imm` when `a` is the TDZ marker.
    CheckInit: AKey,
    /// Throws `TypeError` for an assignment to the constant named `imm`.
    ThrowConstAssign: Key,

    // ---- unary operators: a = op b ----
    Neg: AB,
    /// `ToNumber`, the unary `+`.
    Plus: AB,
    /// `ToNumeric`, the old value of a postfix update.
    ToNumeric: AB,
    Inc: AB,
    Dec: AB,
    BitNot: AB,
    Not: AB,
    TypeOf: AB,
    ToPropertyKey: AB,
    IsUndefinedOrNull: AB,
    IsUndefined: AB,
    IsNull: AB,
    TypeOfIsUndefined: AB,
    TypeOfIsFunction: AB,

    // ---- binary operators: a = b op c ----
    Add: ABC,
    Sub: ABC,
    Mul: ABC,
    Div: ABC,
    Mod: ABC,
    Pow: ABC,
    Shl: ABC,
    Sar: ABC,
    Shr: ABC,
    BitAnd: ABC,
    BitOr: ABC,
    BitXor: ABC,
    Eq: ABC,
    Neq: ABC,
    StrictEq: ABC,
    StrictNeq: ABC,
    Lt: ABC,
    Lte: ABC,
    Gt: ABC,
    Gte: ABC,
    InstanceOf: ABC,
    In: ABC,

    // ---- immediate variants: a = b op imm ----
    AddI: ABInt,
    SubI: ABInt,
    MulI: ABInt,
    BitAndI: ABInt,
    BitOrI: ABInt,
    BitXorI: ABInt,
    ShlI: ABInt,
    SarI: ABInt,
    ShrI: ABInt,

    // ---- control ----
    Jmp: Jump,
    JTrue: AJump,
    JFalse: AJump,
    JUndefinedOrNull: AJump,
    /// Stores the index of the next instruction in `a` as an `Int32` and
    /// jumps to `imm` (a `finally` block).
    Gosub: AJump,
    /// Jumps to the instruction index held in `a`.
    Ret: A,

    // ---- compare-jumps: jump to imm when a op b holds ----
    JLt: ABJump,
    JLte: ABJump,
    JGt: ABJump,
    JGte: ABJump,
    /// `!(a < b)`, not `a >= b`: the negated forms differ on NaN.
    JNlt: ABJump,
    JNlte: ABJump,
    JNgt: ABJump,
    JNgte: ABJump,
    JEq: ABJump,
    JNeq: ABJump,
    JStrictEq: ABJump,
    JStrictNeq: ABJump,

    // ---- compare-jumps against an i16 literal ----
    JLtI: AShortJump,
    JLteI: AShortJump,
    JGtI: AShortJump,
    JGteI: AShortJump,
    JNltI: AShortJump,
    JNlteI: AShortJump,
    JNgtI: AShortJump,
    JNgteI: AShortJump,
    JEqI: AShortJump,
    JNeqI: AShortJump,
    JStrictEqI: AShortJump,
    JStrictNeqI: AShortJump,

    // ---- properties ----
    /// `a = b.imm`.
    GetField: ABKey,
    /// `a.imm = b`.
    SetField: ABKey,
    /// Defines own data property `imm` of `a` as `b` (literals).
    DefineField: ABKey,
    /// `a = delete b.imm`.
    DeleteField: ABKey,
    /// `a = b[c]`.
    GetElem: ABC,
    /// `a[b] = c`.
    SetElem: ABC,
    /// Defines own data property `b` of `a` as `c` (computed literal keys).
    DefineElem: ABC,
    /// `a = delete b[c]`.
    DeleteElem: ABC,

    // ---- globals ----
    GetGlobal: AKey,
    /// `typeof name` on a name that may be undeclared.
    GetGlobalOrUndefined: AKey,
    SetGlobal: AKey,
    /// A `var` or function declaration at script level.
    DefineGlobal: AKey,
    DeleteGlobal: AKey,

    // ---- scopes ----
    LoadScope: AScope,
    StoreScope: AScope,
    /// Pushes a scope of `imm` slots whose parent is the frame's scope.
    EnterScope: Count,
    LeaveScope: None,
    /// Replaces the frame's scope with a copy of it (a `for (let …)`
    /// iteration).
    CopyScope: None,

    // ---- allocation ----
    /// `a = {}` with room for `imm` properties.
    NewObject: ACount,
    /// `a = [b, b + 1, …]` of `imm` elements.
    NewArray: ABRange,
    /// `a` = a closure over the frame's scope for child unit `imm`.
    MakeClosure: AChild,

    // ---- calls ----
    Call: Call,
    /// As `Call`, with the receiver left in `b + 1` by the property read.
    CallMethod: Call,
    /// Reuses the frame's window; `a` is ignored.
    TailCall: Call,
    /// As `Call`; `b + 1` receives the constructed object.
    New: Call,
    Return: A,
    ReturnUndefined: None,
    Throw: A,
}

impl Opcode {
    /// Whether control never falls through to the next instruction.
    pub const fn is_terminator(self) -> bool {
        matches!(
            self,
            Opcode::Jmp
                | Opcode::Ret
                | Opcode::TailCall
                | Opcode::Return
                | Opcode::ReturnUndefined
                | Opcode::Throw
                | Opcode::ThrowConstAssign
        )
    }
}
