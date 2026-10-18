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
    /// `a` register, `imm` an index into the running function's captures.
    ACapture,
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
    /// Exchanges `a` and `b` (cyclic stack shuffles).
    Swap: AB,
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
    /// Defines the getter of own accessor `imm` of `a` as function `b`
    /// (`get x() {}` in a literal), keeping an existing setter.
    DefineGetter: ABKey,
    /// Defines the setter of own accessor `imm` of `a` as function `b`.
    DefineSetter: ABKey,
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
    /// A `var` or function declaration at script level; `b` holds the
    /// [`global`] flags.
    DefineGlobal: AKey,
    DeleteGlobal: AKey,
    /// A script-level declaration: defines global `imm` as `undefined` unless
    /// it exists; `b` holds the [`global`] flags.
    DeclareGlobal: Key,
    /// A script-level `let` or `class`: a binding of the global lexical
    /// environment, in the TDZ.
    DeclareGlobalLet: Key,
    /// A script-level `const`.
    DeclareGlobalConst: Key,
    /// `GlobalDeclarationInstantiation`'s check for a `var` or function
    /// name: a `SyntaxError` if a lexical binding has it, then
    /// `CanDeclareGlobalVar` or, with [`global::FUNCTION`] in `b`,
    /// `CanDeclareGlobalFunction`, a `TypeError` when false.
    CheckGlobalVar: Key,
    /// The check for a lexical name: a `SyntaxError` if a lexical binding or
    /// a non-configurable global property has it.
    CheckGlobalLexical: Key,

    // ---- captured bindings ----
    // A binding captured by an inner function lives in a cell (a one-slot
    // scope object); its register holds the cell. A closure holds the cells it
    // captures, by index.
    /// `a` = a new cell holding the value of `b`.
    NewCell: AB,
    /// `a` = a new cell in the TDZ (a lexical binding entering scope).
    NewCellEmpty: A,
    /// `a` = a new cell holding a copy of cell `a`'s value: the binding is
    /// detached from closures created so far (a `for (let …)` iteration).
    CloseCell: A,
    /// `a` = the value in cell `b`.
    LoadCell: AB,
    /// Cell `a` = `b`.
    StoreCell: AB,
    /// Throws `ReferenceError` naming key `imm` when cell `a` is in the TDZ.
    CheckCellInit: AKey,
    /// `a` = the value of the running function's capture `imm`.
    LoadCaptured: ACapture,
    /// The running function's capture `imm` = `a`.
    StoreCaptured: ACapture,

    // ---- the spill scope of a function with more locals than the window ----
    /// `a` = a scope of `imm` slots, all `undefined`.
    NewSpill: ACount,
    /// `a` = slot `imm` of the spill scope in `b`.
    LoadSlot: ABInt,
    /// Slot `imm` of the spill scope in `b` = `a`.
    StoreSlot: ABInt,

    // ---- rare forms, out of line ----
    /// `a` = a new `arguments` object of the running frame: `imm` 1 for the
    /// mapped form (the parameters' registers hold their cells), 0 for the
    /// unmapped one.
    NewArguments: AInt,
    /// `a` = an array of the arguments from index `imm` (a rest parameter).
    Rest: AInt,
    /// `a` = new.target: the running function in a `new` call, else
    /// `undefined`.
    LoadNewTarget: A,
    /// `a` = a new regular expression over constants `imm` (the source) and
    /// `imm + 1` (the flags).
    NewRegExp: AConst,
    /// `a` = a `for-in` enumeration of `b`'s enumerable string keys.
    ForInStart: AB,
    /// The next key of enumeration `b` that still exists goes to `a` and
    /// control to `imm`; an exhausted enumeration falls through.
    ForInNext: ABJump,
    /// Appends the elements of the array or `arguments` object in `c` to the
    /// array in `a` from index `b`, leaving the next index in `b` (a spread
    /// element).
    AppendSpread: ABC,
    /// Throws `InternalError` whose message is the atom `imm` (a construct
    /// outside the surface, reached at run time).
    ThrowInternal: Key,

    // ---- classes, modules, iteration and suspension (out of line) ----
    /// Throws `TypeError` when `a` (the `new.target` register) is
    /// `undefined`: a class constructor called without `new`.
    CheckCtor: A,
    /// `a` = the running function's home object.
    LoadHomeObject: A,
    /// `a = ToObject(b)`.
    ToObject: AB,
    /// Sets the prototype of the fresh literal `a` to `b` when `b` is an
    /// object or `null` (`__proto__: v` in a literal).
    SetProto: AB,
    /// Defines method `imm` of `a` as function `b` with home object `a`;
    /// `c` holds the [`method`] flags.
    DefineMethod: ABKey,
    /// Defines method `b` (a canonical key) of `a` as function `c`; `imm`
    /// holds the [`method`] flags.
    DefineMethodComputed: ABC,
    /// Names function `a` by the key in `b` when its name is empty
    /// (`SetFunctionName` for anonymous functions in named positions).
    SetFunctionName: AB,
    /// Throws `TypeError` whose message is the atom `imm`.
    ThrowType: Key,
    /// Throws `ReferenceError` whose message is the atom `imm`.
    ThrowReference: Key,
    /// `a = import(b, c)`: the promise of module `b`'s namespace, with the
    /// options in `c`.
    ImportCall: ABC,

    // ---- iteration: a record is three registers ----
    /// The record at `a` (iterator, `next`, done flag; or array, the
    /// `Value::EMPTY` mark, index) for the iterable in `b`; with `imm` set
    /// always the generic protocol.
    IterStart: ABInt,
    /// The next value of record `b` to `a` and whether it is done to `c`.
    IterNext: ABC,
    /// Closes record `a` unless it is done (a normal completion).
    IterClose: A,
    /// Closes record `a` unless it is done for the throw completion in `b`;
    /// errors of `return` are dropped.
    IterCloseThrow: AB,
    /// `a` = record `b`'s `next` called with `c` (`yield*`), unchecked.
    IterCallNext: ABC,
    /// `a` = record `b`'s `return` (`imm` 0) or `throw` (`imm` 1) called
    /// with `c`, and `a + 1` whether the method was missing (then `a` = `c`).
    IterCall: ABC,
    /// Throws `TypeError` when `a` is not an object.
    IterCheckObject: A,
    /// The async record at `a` for `b`: its `@@asyncIterator`, or an
    /// async-from-sync iterator over its sync one.
    IterStartAsync: AB,
    /// `a` = record `b`'s `next()` result, unchecked; the record is marked
    /// done until [`Opcode::IterResult`] reads the settled result.
    IterNextRaw: AB,
    /// Reads the settled result in `a` of record `b`: its value to `a`, its
    /// done flag to `c` (the record is open again unless done).
    IterResult: ABC,

    // ---- classes ----
    /// `a` = the class constructor made from the closure in `b` with the
    /// parent in `a` (`c` 1 when it has a heritage) and `b` = its
    /// prototype; `imm` is the class name.
    MakeClass: ABKey,
    /// Installs the instance-field initializer `c` on constructor `a`, with
    /// home object `b`.
    InstallFields: ABC,
    /// Runs the instance-field initializer of constructor `b` on `a`.
    InitFields: AB,
    /// Runs the static initializer `b` with `this` and home object `a`.
    RunStaticInit: AB,
    /// Calls static block `a` with the frame's `this` and home object.
    CallStaticBlock: A,
    /// `a` = the prototype of home object `b` (`null` allowed).
    GetSuper: AB,
    /// `a` = `base[key]` with `receiver` as `this`: receiver, base, key in
    /// `b`, `b + 1`, `b + 2`; `imm` marks the call form, which reads the
    /// same way.
    GetSuperValue: ABInt,
    /// `base[key] = value` with `receiver`: receiver, base, key, value in
    /// `a` .. `a + 3`.
    PutSuperValue: A,
    /// `a` = the result of the implicit derived constructor: the parent
    /// constructed with the frame's arguments and the `new.target` in `b`.
    InitDerived: AB,
    /// Binds a derived constructor's `this` register `a` to `b` (a
    /// `ReferenceError` if already bound).
    InitThis: AB,
    /// [`Opcode::InitThis`] on the cell in `a`.
    InitThisCell: AB,
    /// [`Opcode::InitThis`] on the running function's capture `imm`.
    InitThisCaptured: ACapture,
    /// A derived constructor's return value: `a` stays if it is an object,
    /// becomes the `this` in `b` for `undefined`; anything else throws.
    CheckDerivedReturn: AB,

    // ---- private names ----
    /// `a` = a new private name described by atom `imm`.
    NewPrivateName: AKey,
    /// Sets the home object of method `a` to `b` and gives `b` a brand.
    SetHomeObject: AB,
    /// Names private method `a` by atom `imm` after the prefix `c` (0 none,
    /// 1 `get `, 2 `set `).
    NamePrivateMethod: AKey,
    /// `a` = own private field `c` of `b` (a `TypeError` if absent).
    GetPrivate: ABC,
    /// Own private field `c` of `a` = `b` (a `TypeError` if absent).
    PutPrivate: ABC,
    /// Defines own private field `c` of `a` as `b` (a `TypeError` if
    /// present).
    DefinePrivate: ABC,
    /// `a` = whether `b` has own private field `c`.
    PrivateIn: ABC,
    /// `a` = private method `c` after a brand check of `b`.
    GetPrivateMethod: ABC,
    /// `a` = the private getter `c` called on `b` after a brand check.
    CallPrivateGetter: ABC,
    /// Calls the private setter `c` on `a` with `b` after a brand check.
    CallPrivateSetter: ABC,
    /// `a` = whether `b` carries the brand of private method `c`.
    PrivateBrandIn: ABC,

    // ---- suspension ----
    /// A generator suspends before its body: the call returns the
    /// generator object.
    InitialYield: None,
    /// An async function starts: its promise is the call's result, and the
    /// body runs until it awaits or completes.
    AsyncStart: None,
    /// Suspends with the value in `a`; a resumption writes its value to `a`
    /// and its kind (0 `next`, 1 `return`) to `a + 1`, or throws here.
    Yield: A,
    /// `yield*`'s suspension: the iterator result in `a` is returned as is;
    /// a `throw` resumes with kind 2 rather than throwing.
    YieldStar: A,
    /// Awaits the value in `a`; the fulfilled value is written to `a`, a
    /// rejection throws here.
    Await: A,

    // ---- object spread and references ----
    /// Copies the enumerable own properties of `b` onto `a`.
    CopyData: AB,
    /// [`Opcode::CopyData`] except the keys own by the object in `c`.
    CopyDataExcluded: ABC,
    /// `a` = the environment holding global `imm`: the lexical one, the
    /// global object, or `undefined` when unresolvable.
    GlobalRef: AKey,
    /// `a` = binding `imm` of environment `b` (a `ReferenceError` if
    /// missing; `undefined` with `c` 1).
    GetRef: ABKey,
    /// Binding `imm` of environment `a` = `b`.
    PutRef: ABKey,

    // ---- allocation ----
    /// `a = {}` with room for `imm` properties.
    NewObject: ACount,
    /// `a = [b, b + 1, …]` of `imm` elements.
    NewArray: ABRange,
    /// `a` = a closure for child unit `imm`, capturing what the child's
    /// capture list names.
    MakeClosure: AChild,

    // ---- calls ----
    /// `a = b(b + 1, …, b + c)` with an `undefined` receiver: the callee's
    /// window starts at `b`, so `b` becomes its `this`.
    Call: Call,
    /// `a = (b + 1)(b + 2, …)` with receiver `b`: the window starts at
    /// `b + 1`, which receives the receiver.
    CallMethod: Call,
    /// `Call` that returns the callee's result from this frame; `a` is
    /// ignored.
    TailCall: Call,
    /// `TailCall` with a receiver, as `CallMethod`.
    TailCallMethod: Call,
    /// `a = new b(b + 2, …)` with new.target in `b + 1`: the window starts at
    /// `b + 1`, which receives the constructed object.
    New: Call,
    /// `a = b(...)` with receiver `b + 1` and the elements of the array in
    /// `b + 2` as the arguments (a spread call): the window starts at
    /// `b + 1`; `c` is unused.
    CallSpread: Call,
    /// `a = new b(...)` with new.target in `b + 1` and the elements of the
    /// array in `b + 2` as the arguments.
    NewSpread: Call,
    Return: A,
    ReturnUndefined: None,
    Throw: A,
    /// `a = ToPropertyKey(b)` for the base `c` of a compound assignment: a
    /// `null` or `undefined` base throws first.
    ToPropertyKeyOf: ABC,
    /// Puts the cell in `a` back in the TDZ (a lexical scope entered again):
    /// closures already holding it see the new binding state.
    ClearCell: A,
}

/// The flags of `CheckGlobalVar`, `DeclareGlobal` and `DefineGlobal`, in
/// `b`.
pub mod global {
    /// The name is a function declaration's.
    pub const FUNCTION: u8 = 1 << 0;
    /// The code is `eval` code: its bindings are configurable.
    pub const EVAL: u8 = 1 << 1;
    /// `CheckGlobalVar`: only whether a lexical declaration has the name
    /// (16.1.7 step 4).
    pub const LEXICAL_CONFLICT: u8 = 1 << 2;
    /// `CheckGlobalVar`: only whether the global object can take the
    /// declaration (CanDeclareGlobalFunction or CanDeclareGlobalVar).
    pub const DEFINABLE: u8 = 1 << 3;
}

/// The flags of `DefineMethod` and `DefineMethodComputed`.
pub mod method {
    pub const ENUMERABLE: u32 = 1 << 0;
    pub const GETTER: u32 = 1 << 1;
    pub const SETTER: u32 = 1 << 2;
}

impl Opcode {
    /// Whether control never falls through to the next instruction.
    pub const fn is_terminator(self) -> bool {
        matches!(
            self,
            Opcode::Jmp
                | Opcode::Ret
                | Opcode::TailCall
                | Opcode::TailCallMethod
                | Opcode::Return
                | Opcode::ReturnUndefined
                | Opcode::Throw
                | Opcode::ThrowConstAssign
                | Opcode::ThrowReference
                | Opcode::ThrowInternal
        )
    }
}
