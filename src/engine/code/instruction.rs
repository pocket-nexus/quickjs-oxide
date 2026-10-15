//! The opcode table: every compiler [`Instruction`], its published [`Opcode`]
//! number, and the per-opcode facts read by lowering, stack-size computation
//! and the encoder are declared once, in the style of QuickJS
//! `quickjs-opcode.h`.
use super::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, DynamicEnvironmentSource, EvalVariableSource,
    IteratorCallKind, PrivateNameSource,
};
use Operand as O;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StackEffect {
    pub popped: usize,
    pub pushed: usize,
    pub state: StackStateEffect,
}

/// Stack protocols whose real effect depends on control-flow state; the
/// nominal pop/push pair of such an opcode is not its whole stack behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StackStateEffect {
    Ordinary,
    MarkSuperCall,
    ConsumeSuperCall,
    EnterCatch,
    DropCatch,
    PreserveCatch,
    EnterIterator,
    AdvanceIterator,
    FinishIteratorValue,
    CloseIterator,
    PreserveIterator,
    DetachIterator,
    EnterGosub,
    ConsumeGosub,
    ResumeYield,
    ResumeAwait,
}

/// Catchable JavaScript completion, excluding allocation/invariant engine errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JsExceptionEffect {
    None,
    MayThrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlEffect {
    Next,
    Jump(u32),
    Branch(u32),
    Catch(u32),
    Gosub(u32),
    Ret,
    Return,
    TailCall,
    Throw,
    Suspend,
}

impl ControlEffect {
    pub(crate) const fn target(self) -> Option<u32> {
        match self {
            Self::Jump(target)
            | Self::Branch(target)
            | Self::Catch(target)
            | Self::Gosub(target) => Some(target),
            Self::Next
            | Self::Ret
            | Self::Return
            | Self::TailCall
            | Self::Throw
            | Self::Suspend => None,
        }
    }
    pub(crate) const fn ends_block(self) -> bool {
        !matches!(self, Self::Next)
    }
}

/// Constant-pool roles do not replace type/permission validation of the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConstantRole {
    Value,
    Function,
    RegExp,
    StaticName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StackDepthRole {
    Target,
    Source,
    Excluded,
}

/// Encoded operands retain semantic units; a slot is never confused with a PC,
/// constant-pool index, count, or authenticated hidden source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operand {
    Integer(i32),
    AtomValueIndex(u32),
    Constant { index: u32, role: ConstantRole },
    Local(u16),
    Argument(u16),
    Closure(u16),
    PrivateLocal(u16),
    ArgumentCount(u16),
    RestStart(u16),
    ElementCount(u16),
    Target(u32),
    EvalEnvironment(u16),
    EvalSource(EvalVariableSource),
    DynamicSource(DynamicEnvironmentSource),
    PrivateSource(PrivateNameSource),
    ArgumentsKind(ArgumentsKind),
    ApplyKind(ApplyKind),
    IteratorCallKind(IteratorCallKind),
    MethodKind(DefineMethodKind),
    Enumerable(bool),
    HasHeritage(bool),
    IteratorDepth(u8),
    StackDepth { role: StackDepthRole, depth: u8 },
}

impl Operand {
    const fn value(index: u32) -> Self {
        Self::Constant {
            index,
            role: ConstantRole::Value,
        }
    }
    const fn function(index: u32) -> Self {
        Self::Constant {
            index,
            role: ConstantRole::Function,
        }
    }
    const fn regexp(index: u32) -> Self {
        Self::Constant {
            index,
            role: ConstantRole::RegExp,
        }
    }
    const fn name(index: u32) -> Self {
        Self::Constant {
            index,
            role: ConstantRole::StaticName,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OperandContract(pub [Option<Operand>; 3]);

impl OperandContract {
    pub(crate) const fn static_name(self) -> Option<u32> {
        let mut index = 0;
        while index < self.0.len() {
            if let Some(Operand::Constant {
                index: constant,
                role: ConstantRole::StaticName,
            }) = self.0[index]
            {
                return Some(constant);
            }
            index += 1;
        }
        None
    }
}

/// Conservative generic-language effects. Allocation means managed values or
/// their storage, not VM window growth or reference-accounting bookkeeping.
/// A false bit is a guarantee; a true bit permits an effect and is not a claim
/// that every operand tag takes that path. Number fast paths refine these facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PotentialEffects {
    pub javascript_exception: JsExceptionEffect,
    pub may_call_js: bool,
    pub may_allocate: bool,
}

impl PotentialEffects {
    const fn new(
        javascript_exception: JsExceptionEffect,
        may_call_js: bool,
        may_allocate: bool,
    ) -> Self {
        Self {
            javascript_exception,
            may_call_js,
            may_allocate,
        }
    }
    const PURE: Self = Self::new(JsExceptionEffect::None, false, false);
    const ALLOC: Self = Self::new(JsExceptionEffect::None, false, true);
    const THROW: Self = Self::new(JsExceptionEffect::MayThrow, false, false);
    const THROW_ALLOC: Self = Self::new(JsExceptionEffect::MayThrow, false, true);
    const CALL: Self = Self::new(JsExceptionEffect::MayThrow, true, true);
}

#[cfg(feature = "profiling")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InstructionInfo {
    pub stack: StackEffect,
    pub effects: PotentialEffects,
    pub control: ControlEffect,
    pub operands: OperandContract,
}

impl Instruction {
    /// Diagnostic projection of every canonical contract, consumed only by the
    /// profiling disassembler.
    #[cfg(feature = "profiling")]
    pub(crate) const fn info(&self) -> InstructionInfo {
        InstructionInfo {
            stack: self.stack_contract(),
            effects: self.potential_effects(),
            control: self.control_effect(),
            operands: self.operand_contract(),
        }
    }

    /// Read only the needed part of the canonical contract. Stack-size
    /// computation and block discovery must not construct unrelated payloads.
    pub(crate) const fn stack_contract(&self) -> StackEffect {
        let (popped, pushed) = self.nominal_stack_effect();
        StackEffect {
            popped,
            pushed,
            state: self.stack_state_effect(),
        }
    }
}

/// `CopyDataPropertiesExcluded` addresses three operands by depth without
/// moving them, so its nominal window reaches the deepest one.
const fn excluded_copy_window(depths: [u8; 3]) -> (usize, usize) {
    let mut maximum = depths[0];
    if depths[1] > maximum {
        maximum = depths[1];
    }
    if depths[2] > maximum {
        maximum = depths[2];
    }
    let required = maximum as usize + 1;
    (required, required)
}

/// Declares [`Instruction`], [`Opcode`] and every per-opcode fact from one
/// table. Each instruction row reads
///
/// `Variant(binder: Type) = opcode, (popped, pushed), EFFECTS[, ctrl Control(target)][, state State][, [operands]];`
///
/// `opcode` is the published execution discriminant; `(popped, pushed)` the
/// nominal stack effect; `EFFECTS` a [`PotentialEffects`] preset; `ctrl` the
/// [`ControlEffect`] (default `Next`); `state` the [`StackStateEffect`]
/// (default `Ordinary`); and the operand list the [`OperandContract`], whose
/// length is also the published operand-word count. Expressions read the row's
/// binders by reference. Execution-only superinstructions follow as
/// `Name = opcode, operand_words;`.
macro_rules! opcode_table {
    (@count) => { 0 };
    (@count $a:expr) => { 1 };
    (@count $a:expr, $b:expr) => { 2 };
    (@count $a:expr, $b:expr, $c:expr) => { 3 };
    (@contract) => { OperandContract([None; 3]) };
    (@contract $a:expr) => { OperandContract([Some($a), None, None]) };
    (@contract $a:expr, $b:expr) => { OperandContract([Some($a), Some($b), None]) };
    (@contract $a:expr, $b:expr, $c:expr) => { OperandContract([Some($a), Some($b), Some($c)]) };
    (@control) => { ControlEffect::Next };
    (@control $control:ident) => { ControlEffect::$control };
    (@control $control:ident ($target:expr)) => { ControlEffect::$control($target) };
    (@state) => { StackStateEffect::Ordinary };
    (@state $state:ident) => { StackStateEffect::$state };
    (
        $(#[$enum_meta:meta])*
        pub enum Instruction {
            $(
                $(#[$doc:meta])*
                $name:ident $(($binder:ident: $binder_type:ty))? $({ $($field:ident: $field_type:ty),+ })?
                    = $opcode:literal, $stack:expr, $effects:ident
                    $(, ctrl $control:ident $(($target:expr))?)?
                    $(, state $state:ident)?
                    $(, [$($operand:expr),+ $(,)?])?;
            )*
        }
        execution_only {
            $($(#[$extra_doc:meta])* $extra:ident = $extra_opcode:literal, $extra_words:literal;)*
        }
    ) => {
        $(#[$enum_meta])*
        pub enum Instruction {
            $($(#[$doc])* $name $(($binder_type))? $({ $($field: $field_type),+ })?,)*
        }

        /// Published execution opcodes. Discriminants are the execution-word
        /// encoding and must stay stable.
        #[repr(u16)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum Opcode {
            $($name = $opcode,)*
            $($(#[$extra_doc])* $extra = $extra_opcode,)*
        }

        impl Opcode {
            // A checked match exposes the identity mapping to LLVM without a
            // table load.
            #[inline(always)]
            pub(crate) fn from_raw(raw: u16) -> Option<Self> {
                match raw {
                    $($opcode => Some(Self::$name),)*
                    $($extra_opcode => Some(Self::$extra),)*
                    _ => None,
                }
            }

            pub(crate) const fn from_instruction(instruction: &Instruction) -> Self {
                match instruction {
                    $(Instruction::$name { .. } => Self::$name,)*
                }
            }

            pub(crate) const fn operand_count(self) -> u8 {
                match self {
                    $(Self::$name => opcode_table!(@count $($($operand),+)?),)*
                    $(Self::$extra => $extra_words,)*
                }
            }
        }

        #[allow(unused_variables, reason = "rows bind every payload field")]
        impl Instruction {
            #[must_use]
            pub(crate) const fn nominal_stack_effect(&self) -> (usize, usize) {
                match self {
                    $(Self::$name $(($binder))? $({ $($field),+ })? => $stack,)*
                }
            }

            pub(crate) const fn potential_effects(&self) -> PotentialEffects {
                match self {
                    $(Self::$name { .. } => PotentialEffects::$effects,)*
                }
            }

            pub(crate) const fn control_effect(&self) -> ControlEffect {
                match self {
                    $(Self::$name $(($binder))? $({ $($field),+ })? =>
                        opcode_table!(@control $($control $(($target))?)?),)*
                }
            }

            pub(crate) const fn operand_contract(&self) -> OperandContract {
                match self {
                    $(Self::$name $(($binder))? $({ $($field),+ })? =>
                        opcode_table!(@contract $($($operand),+)?),)*
                }
            }

            const fn stack_state_effect(&self) -> StackStateEffect {
                match self {
                    $(Self::$name { .. } => opcode_table!(@state $($state)?),)*
                }
            }
        }
    };
}

opcode_table! {
    /// Stack-machine operations deliberately use the names and stack behavior of
    /// their `QuickJS` counterparts. This typed form is compiler IR. Publication
    /// consumes it into `ExecCode`; the VM never dispatches this enum.
    #[derive(Clone, Debug)]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "The encoder and VM accept instruction forms that current compiler paths do not emit."
        )
    )]
    pub enum Instruction {
        Nop = 0, (0, 0), PURE;
        PushI32(value: i32) = 1, (0, 1), PURE, [O::Integer(*value)];
        /// QuickJS `OP_push_atom_value` for a tagged non-negative integer atom.
        /// Each execution materializes a fresh decimal String; it must not reuse
        /// an atom-backed or ordinary constant-pool String representation.
        PushAtomValueIndex(index: u32) = 2, (0, 1), ALLOC, [O::AtomValueIndex(*index)];
        PushConst(index: u32) = 3, (0, 1), PURE, [O::value(*index)];
        FClosure(index: u32) = 4, (0, 1), ALLOC, [O::function(*index)];
        /// QuickJS `OP_regexp`, represented as one typed compile-time constant
        /// instead of two generic stack values. Each execution allocates a fresh
        /// branded RegExp in the bytecode realm without consulting user code.
        RegExp(index: u32) = 5, (0, 1), ALLOC, [O::regexp(*index)];
        /// QuickJS `OP_set_name`: conditionally define the name of the object at
        /// the top of the stack from a string constant, without consuming it.
        SetName(name: u32) = 6, (1, 1), CALL, [O::name(*name)];
        /// QuickJS `OP_throw_error atom JS_THROW_VAR_RO` for a strict immutable
        /// binding. It consumes no stack value and terminates, abandoning the
        /// complete frame stack on the exception path.
        ThrowReadOnly(name: u32) = 7, (0, 0), CALL, ctrl Throw, [O::name(*name)];
        /// QuickJS `OP_throw_error atom JS_THROW_VAR_REDECL`: throw the eval-time
        /// SyntaxError used for a `var`/function declaration which crosses an
        /// imported lexical scope. The verified String constant preserves the
        /// conflicting binding identity even though QuickJS's message is generic.
        ThrowRedeclaration(name: u32) = 8, (0, 0), CALL, ctrl Throw, [O::name(*name)];
        /// QuickJS `JS_THROW_ERROR_DELETE_SUPER`. The three authenticated
        /// SuperProperty Reference operands are consumed conceptually and the
        /// terminal throw occupies the expression's one-value stack shape.
        ThrowDeleteSuper = 9, (3, 1), CALL, ctrl Throw;
        Undefined = 10, (0, 1), PURE;
        Null = 11, (0, 1), PURE;
        PushFalse = 12, (0, 1), PURE;
        PushTrue = 13, (0, 1), PURE;
        PushThis = 14, (0, 1), ALLOC;
        /// Push the exact function object of the active bytecode frame. Derived
        /// constructors use its live `[[Prototype]]` as the target of `super()`, so
        /// this must not be reconstructed from HomeObject or the original heritage
        /// expression.
        PushActiveFunction = 15, (0, 1), PURE;
        /// QuickJS's authenticated `<home_object>` pseudo binding. Only bytecode
        /// whose immutable metadata declares `needs_home_object` may execute this
        /// operation; the runtime reads it from the active function object rather
        /// than from a JavaScript-visible operand.
        PushHomeObject = 16, (0, 1), PURE;
        PushNewTarget = 17, (0, 1), PURE;
        /// QuickJS `OP_special_object` for an ordinary function's lazily selected
        /// arguments binding. Runtime creation still occurs in the entry prologue,
        /// before body function hoists.
        Arguments(kind: ArgumentsKind) = 18, (0, 1), ALLOC, [O::ArgumentsKind(*kind)];
        /// QuickJS `OP_rest`: collect the actual arguments beginning at this
        /// formal-parameter index into a fresh Array in the callee realm. The
        /// operand may equal `argument_count` for a rest BindingPattern, which
        /// owns no physical argument slot.
        Rest(start: u16) = 19, (0, 1), ALLOC, [O::RestStart(*start)];
        /// QuickJS `OP_special_object` with `OP_SPECIAL_OBJECT_VAR_OBJECT`: create
        /// the null-prototype object which stores names introduced by sloppy
        /// direct eval in this activation.
        VariableEnvironment = 20, (0, 1), ALLOC;
        /// Test whether the hidden variable object has an own property named by a
        /// verified String constant.
        HasEvalVariable { source: EvalVariableSource, name: u32 } = 21, (0, 1), CALL,
            [O::EvalSource(*source), O::name(*name)];
        /// Read one property from the hidden variable object.
        GetEvalVariable { source: EvalVariableSource, name: u32 } = 22, (0, 1), CALL,
            [O::EvalSource(*source), O::name(*name)];
        /// Consume and assign one property on the hidden variable object.
        PutEvalVariable { source: EvalVariableSource, name: u32 } = 23, (1, 0), CALL,
            [O::EvalSource(*source), O::name(*name)];
        /// Delete one configurable property from the hidden variable object and
        /// push the Boolean result.
        DeleteEvalVariable { source: EvalVariableSource, name: u32 } = 24, (0, 1), CALL,
            [O::EvalSource(*source), O::name(*name)];
        /// Consume a value and define an own configurable, writable, enumerable
        /// data property on the hidden variable object.
        DefineEvalVariable { source: EvalVariableSource, name: u32 } = 25, (1, 0), CALL,
            [O::EvalSource(*source), O::name(*name)];
        /// ECMAScript `ToObject`: preserve Objects, reject nullish values, and box
        /// all other primitives in the executing realm.
        ToObject = 26, (1, 1), CALL;
        /// Test whether an ordered hidden environment exposes one dynamic name.
        HasDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = 27, (0, 1), CALL,
            [O::DynamicSource(*source), O::name(*name)];
        /// Read one dynamic binding using the current frame's strictness.
        GetDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = 28, (0, 1), CALL,
            [O::DynamicSource(*source), O::name(*name)];
        /// Consume and write one dynamic binding using the current frame's
        /// strictness.
        PutDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = 29, (1, 0), CALL,
            [O::DynamicSource(*source), O::name(*name)];
        /// Delete one dynamic binding and push the Boolean result.
        DeleteDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = 30, (0, 1), CALL,
            [O::DynamicSource(*source), O::name(*name)];
        /// Push the authenticated object behind one dynamic environment source.
        DynamicEnvironmentObject(source: DynamicEnvironmentSource) = 31, (0, 1), THROW_ALLOC,
            [O::DynamicSource(*source)];
        /// QuickJS `OP_make_var_ref`: resolve one authenticated global closure
        /// name to the current realm's lexical storage object, global object, or
        /// the unresolved-reference `undefined` sentinel. Lexical TDZ and
        /// read-only checks happen while this reference is made, before a later
        /// right-hand side can run.
        GlobalReference(closure: u16) = 32, (0, 1), CALL, [O::Closure(*closure)];
        /// Preserve an environment Object and append its named reference value.
        /// A missing property observes the current frame's strictness.
        GetRefValue(name: u32) = 33, (1, 2), CALL, [O::name(*name)];
        /// Preserve an environment Object and append its named reference value,
        /// producing `undefined` when the property is missing.
        GetRefValueUndef(name: u32) = 34, (1, 2), CALL, [O::name(*name)];
        /// Consume an environment Object and value and write the named reference
        /// using the current frame's strictness.
        PutRefValue(name: u32) = 35, (2, 0), CALL, [O::name(*name)];
        GetLocal(local: u16) = 36, (0, 1), PURE, [O::Local(*local)];
        PutLocal(local: u16) = 37, (1, 0), PURE, [O::Local(*local)];
        SetLocal(local: u16) = 38, (1, 1), PURE, [O::Local(*local)];
        /// QuickJS `OP_set_loc_uninitialized`: enter one lexical scope with its
        /// local slot in the temporal dead zone.
        SetLocalUninitialized(local: u16) = 39, (0, 0), PURE, [O::Local(*local)];
        /// QuickJS `OP_get_loc_check`: read a lexical local after its TDZ check.
        GetLocalCheck(local: u16) = 40, (0, 1), THROW_ALLOC, [O::Local(*local)];
        /// Typed-format distinction for QuickJS's lexical-initializer `put_loc`.
        /// The published vardef must be lexical, but execution deliberately keeps
        /// ordinary `put_loc` overwrite semantics rather than conflating this with
        /// derived-`this`'s upstream `put_loc_check_init` opcode.
        InitializeLocal(local: u16) = 41, (1, 0), ALLOC, [O::Local(*local)];
        /// QuickJS `OP_put_loc_check_init`: consume the result of `super()` and
        /// initialize the authenticated derived-constructor `this` local exactly
        /// once. A second execution throws after the super constructor has run.
        InitializeDerivedLocal(local: u16) = 42, (1, 0), THROW_ALLOC, [O::Local(*local)];
        /// QuickJS `OP_put_loc_check`: consume and assign a mutable lexical local.
        PutLocalCheck(local: u16) = 43, (1, 0), THROW_ALLOC, [O::Local(*local)];
        /// QuickJS `OP_set_loc_check`: assign a mutable lexical local while
        /// preserving the assigned value on the operand stack.
        SetLocalCheck(local: u16) = 44, (1, 1), THROW_ALLOC, [O::Local(*local)];
        GetArg(arg: u16) = 45, (0, 1), PURE, [O::Argument(*arg)];
        PutArg(arg: u16) = 46, (1, 0), PURE, [O::Argument(*arg)];
        SetArg(arg: u16) = 47, (1, 1), PURE, [O::Argument(*arg)];
        GetVarRef(closure: u16) = 48, (0, 1), PURE, [O::Closure(*closure)];
        PutVarRef(closure: u16) = 49, (1, 0), PURE, [O::Closure(*closure)];
        SetVarRef(closure: u16) = 50, (1, 1), PURE, [O::Closure(*closure)];
        /// QuickJS `OP_get_var_ref_check`: read a captured lexical binding after
        /// its TDZ check.
        GetVarRefCheck(closure: u16) = 51, (0, 1), THROW_ALLOC, [O::Closure(*closure)];
        /// QuickJS `OP_put_var_ref_check`: consume and assign a captured mutable
        /// lexical binding. Value-preserving writes use `Dup; PutVarRefCheck`.
        PutVarRefCheck(closure: u16) = 52, (1, 0), THROW_ALLOC, [O::Closure(*closure)];
        /// QuickJS's declaration-time raw VarRef write for an ordinary lexical
        /// module binding. Publication proves this occurs once in the authored
        /// evaluation path; the cell may already have been changed indirectly by
        /// QuickJS's import/declaration collision quirk.
        InitializeVarRef(closure: u16) = 53, (1, 0), ALLOC, [O::Closure(*closure)];
        /// Pinned QuickJS declaration initialization when an import and a module
        /// declaration share the import's first closure slot. Publication accepts
        /// this raw write only for a matching module collision ledger entry;
        /// ordinary writes to the same binding remain read-only.
        InitializeModuleImportCollision(closure: u16) = 54, (1, 0), ALLOC, [O::Closure(*closure)];
        /// Captured-binding counterpart of [`Instruction::InitializeDerivedLocal`].
        /// Arrow functions and direct eval use this to initialize their enclosing
        /// derived constructor's single `this` cell.
        InitializeDerivedVarRef(closure: u16) = 55, (1, 0), THROW_ALLOC, [O::Closure(*closure)];
        /// QuickJS `OP_close_loc`: detach a captured local when its lexical scope
        /// exits so a later scope entry receives a fresh cell.
        CloseLocal(local: u16) = 56, (0, 0), ALLOC, [O::Local(*local)];
        /// QuickJS `OP_get_var`: read a global-environment VarRef closure slot.
        GetVar(closure: u16) = 57, (0, 1), CALL, [O::Closure(*closure)];
        /// QuickJS `OP_get_var_undef`: as `GetVar`, but suppress a genuinely
        /// missing global binding for a direct `typeof IdentifierReference`.
        GetVarUndef(closure: u16) = 58, (0, 1), CALL, [O::Closure(*closure)];
        /// QuickJS `OP_delete_var`: delete one sloppy direct global binding and
        /// push the Boolean result. Lexical and resolved non-global bindings are
        /// folded to `PushFalse` during scope resolution.
        DeleteVar(closure: u16) = 59, (0, 1), CALL, [O::Closure(*closure)];
        /// QuickJS `OP_put_var`: assign and consume a global binding value.
        PutVar(closure: u16) = 60, (1, 0), CALL, [O::Closure(*closure)];
        /// QuickJS `OP_put_var_init`: initialize a lexical global binding.
        PutVarInit(closure: u16) = 61, (1, 0), CALL, [O::Closure(*closure)];
        /// Allocate one fresh QuickJS private symbol and initialize the named
        /// class-private lexical local without exposing that symbol as a Value.
        InitializePrivateName(local: u16) = 62, (0, 0), ALLOC, [O::PrivateLocal(*local)];
        /// QuickJS private-method declaration bootstrap. Consume the class-side
        /// HomeObject and freshly instantiated method closure, install the hidden
        /// HomeObject edge, initialize the authenticated private-method lexical
        /// cell, and retain only the HomeObject on the operand stack.
        InitializePrivateMethod(local: u16) = 63, (2, 1), CALL, [O::PrivateLocal(*local)];
        /// QuickJS private-accessor declaration bootstrap. Consume the class-side
        /// HomeObject and freshly instantiated getter or setter closure, install
        /// the hidden HomeObject edge, initialize the authenticated accessor cell,
        /// and retain only the HomeObject. Unlike a private method, an accessor's
        /// public function name remains the empty string.
        InitializePrivateAccessor(local: u16) = 64, (2, 1), CALL, [O::PrivateLocal(*local)];
        /// QuickJS `OP_get_private_field`: own-only private field read.
        GetPrivateField(source: PrivateNameSource) = 65, (1, 1), CALL, [O::PrivateSource(*source)];
        /// Call/compound counterpart which retains the original receiver.
        GetPrivateField2(source: PrivateNameSource) = 66, (1, 2), CALL, [O::PrivateSource(*source)];
        /// QuickJS `OP_put_private_field`: own-only private field write.
        PutPrivateField(source: PrivateNameSource) = 67, (2, 0), CALL, [O::PrivateSource(*source)];
        /// QuickJS `OP_define_private_field`: define a fresh private own field
        /// even when the receiver is non-extensible.
        DefinePrivateField(source: PrivateNameSource) = 68, (2, 1), CALL, [O::PrivateSource(*source)];
        /// QuickJS `OP_private_in` for a private data-field binding.
        PrivateIn(source: PrivateNameSource) = 69, (1, 1), CALL, [O::PrivateSource(*source)];
        /// QuickJS `OP_get_field`: replace an object-like base with the value of
        /// one constant string-keyed property.
        GetField(name: u32) = 70, (1, 1), CALL, [O::name(*name)];
        /// QuickJS `OP_get_field2`: keep the base below the fetched value so a
        /// following `CallMethod` observes the original reference receiver.
        GetField2(name: u32) = 71, (1, 2), CALL, [O::name(*name)];
        /// QuickJS `OP_get_array_el`: `base key -> value`, including observable
        /// `ToPropertyKey` conversion in the runtime host.
        GetArrayEl = 72, (2, 1), CALL;
        /// QuickJS `OP_get_array_el2`: `base key -> base value` for method calls.
        GetArrayEl2 = 73, (2, 2), CALL;
        /// QuickJS `OP_get_array_el3`: `base raw-key -> base converted-key value`
        /// for compound/logical assignment without repeated key conversion.
        GetArrayEl3 = 74, (2, 3), CALL;
        /// QuickJS `OP_get_super`: replace a method HomeObject with its current
        /// prototype. A null prototype is represented by JavaScript `null` and is
        /// diagnosed only by the following property operation.
        GetSuper = 75, (1, 1), CALL;
        /// QuickJS `OP_get_super_value`: `receiver base key -> value`. The base is
        /// the prototype captured before a computed key is evaluated, while
        /// accessors observe the method's actual `this` as their receiver.
        GetSuperValue = 76, (3, 1), CALL;
        /// QuickJS's call-site rewrite of `OP_get_super_value` to
        /// `OP_get_array_el`: `receiver base key -> receiver value`. This
        /// deliberately gives a getter `base` as its receiver before the returned
        /// callable is invoked with the preserved method receiver.
        GetSuperValueForCall = 77, (3, 2), CALL;
        /// QuickJS `OP_array_from`: consume `element_count` dense literal values
        /// and replace them with a fresh Array in the bytecode's realm.
        ArrayFrom(count: u16) = 78, (*count as usize, 1), ALLOC, [O::ElementCount(*count)];
        /// QuickJS `OP_object`: push a fresh ordinary Object rooted in the
        /// executing bytecode's realm.
        Object = 79, (0, 1), ALLOC;
        /// QuickJS `OP_to_propkey`: observable `ToPropertyKey` while retaining a
        /// canonical Int/String/Symbol value on the VM stack.
        ToPropKey = 80, (1, 1), CALL;
        /// QuickJS `OP_insert2`: `base value -> value base value`.
        Insert2 = 81, (2, 3), PURE;
        /// QuickJS `OP_insert3`: `base key value -> value base key value`.
        Insert3 = 82, (3, 4), PURE;
        /// QuickJS `OP_dup3`: `a b c -> a b c a b c`.
        Dup3 = 83, (3, 6), PURE;
        /// QuickJS `OP_insert4`: `this base key value -> value this base key value`.
        Insert4 = 84, (4, 5), PURE;
        /// QuickJS `OP_perm3`: `base old new -> old base new`.
        Perm3 = 85, (3, 3), PURE;
        /// QuickJS `OP_perm4`: `base key old new -> old base key new`.
        Perm4 = 86, (4, 4), PURE;
        /// QuickJS `OP_perm5`: `this base key old new -> old this base key new`.
        Perm5 = 87, (5, 5), PURE;
        /// QuickJS `OP_rot4l`: `value this base key -> this base key value`.
        Rot4Left = 88, (4, 4), PURE;
        /// QuickJS `OP_put_field`: assign one constant string-keyed property.
        PutField(name: u32) = 89, (2, 0), CALL, [O::name(*name)];
        /// QuickJS `OP_put_array_el`: assign a computed property, converting the
        /// still-raw key only after the right-hand side has been evaluated.
        PutArrayEl = 90, (3, 0), CALL;
        /// QuickJS `OP_put_super_value`: assign through `base` with `receiver` as
        /// the actual receiver. `JS_PROP_THROW_STRICT` rejects failed writes only
        /// when the containing method is strict.
        PutSuperValue = 91, (4, 0), CALL;
        /// QuickJS `OP_define_field`: define one C_W_E data property while
        /// preserving the object below it (`object value -> object`). Array
        /// literals use this after their initial dense prefix.
        DefineField(name: u32) = 92, (2, 1), CALL, [O::name(*name)];
        /// Computed-key class-field form of QuickJS `OP_define_array_el`. The key
        /// is the authenticated output of `ToPropKey`, so execution must not run
        /// observable property-key conversion again. Defines one C_W_E own data
        /// property while preserving only the object
        /// (`object canonical-key value -> object`).
        DefineFieldComputed = 93, (3, 1), CALL;
        /// QuickJS `OP_define_method`: name and publish an object-literal method
        /// under one verified String constant while preserving the fresh literal
        /// (`object closure -> object`).
        DefineMethod { key: u32, kind: DefineMethodKind, enumerable: bool } = 94, (2, 1), CALL,
            [O::name(*key), O::MethodKind(*kind), O::Enumerable(*enumerable)];
        /// QuickJS `OP_define_method_computed`: publish an object-literal method
        /// under an already-canonical property key while preserving the fresh
        /// literal (`object key closure -> object`).
        DefineMethodComputed { kind: DefineMethodKind, enumerable: bool } = 95, (3, 1), CALL,
            [O::MethodKind(*kind), O::Enumerable(*enumerable)];
        /// QuickJS `OP_define_class`: replace `parent, constructor` with the
        /// published class `constructor, prototype` pair. The heritage flag is
        /// authenticated bytecode metadata rather than a JavaScript stack value.
        DefineClass { name: u32, has_heritage: bool } = 96, (2, 2), CALL,
            [O::name(*name), O::HasHeritage(*has_heritage)];
        /// Attach one authenticated hidden instance-fields program to a freshly
        /// defined class constructor while installing the class prototype as its
        /// HomeObject (`constructor prototype initializer -> constructor prototype`).
        InstallClassInstanceInitializer = 97, (3, 2), CALL;
        /// Run the hidden instance-fields program attached to the active class
        /// constructor with the retained receiver as `this`
        /// (`receiver active-constructor -> receiver`).
        /// Absence of an initializer is the ordinary no-fields fast path.
        CallClassInstanceInitializer = 98, (2, 1), CALL;
        /// Install the constructor as HomeObject and immediately execute the
        /// aggregate static-elements program (`constructor initializer -> constructor`).
        RunClassStaticInitializer = 99, (2, 1), CALL;
        /// Execute a non-escaping static-block child with the aggregate static
        /// initializer's receiver and HomeObject (`block ->`).
        CallClassStaticBlock = 100, (1, 0), CALL;
        /// QuickJS `OP_define_array_el`: define a computed C_W_E data property
        /// while preserving the Array and its dynamic index
        /// (`array index value -> array index`).
        DefineArrayEl = 101, (3, 2), CALL;
        /// QuickJS `OP_set_name_computed`: infer an anonymous function's name
        /// from an already-canonical computed property key while preserving both
        /// operands (`key value -> key value`).
        SetNameComputed = 102, (2, 2), CALL;
        /// QuickJS `OP_set_proto`: consume one object-literal prototype candidate
        /// and preserve the fresh literal (`object value -> object`). Object and
        /// null candidates replace `[[Prototype]]`; primitive candidates are ignored.
        SetProto = 103, (2, 1), CALL;
        /// Object-literal specialization of QuickJS `OP_copy_data_properties`.
        /// It snapshots enumerable own String/Symbol keys from an Object source,
        /// defines C_W_E data properties, and preserves the fresh literal
        /// (`target source -> target`).
        CopyDataProperties = 104, (2, 1), CALL;
        /// Exclusion-aware QuickJS `OP_copy_data_properties` used by an Object
        /// binding rest element. The three operands are addressed by depth from
        /// the top of the stack (depth zero is the top) and are only borrowed:
        /// successful execution leaves the complete operand stack unchanged.
        ///
        /// This is intentionally depth-addressed instead of consuming three
        /// adjacent values. A sloppy `var` binding may need to keep its prepared
        /// Reference between the retained source and the fresh target while the
        /// copy executes observable getters.
        CopyDataPropertiesExcluded { target_depth: u8, source_depth: u8, excluded_depth: u8 } = 105,
            excluded_copy_window([*target_depth, *source_depth, *excluded_depth]), CALL, [
                O::StackDepth { role: StackDepthRole::Target, depth: *target_depth },
                O::StackDepth { role: StackDepthRole::Source, depth: *source_depth },
                O::StackDepth { role: StackDepthRole::Excluded, depth: *excluded_depth },
            ];
        /// QuickJS `OP_append`: append every value from one iterable and replace
        /// the dynamic index with the first unused index
        /// (`array index iterable -> array index`).
        Append = 106, (3, 2), CALL;
        /// QuickJS `OP_delete`: `base key -> bool` with strictness supplied by the
        /// active call frame.
        Delete = 107, (2, 1), CALL;
        Drop = 108, (1, 0), PURE;
        /// QuickJS `OP_nip`: discard the value immediately below the stack top,
        /// preserving the top value (`a b -> b`).
        Nip = 109, (2, 1), PURE;
        /// QuickJS `OP_swap`: exchange the top two operands (`a b -> b a`).
        Swap = 110, (2, 2), PURE;
        Dup = 111, (1, 2), PURE;
        /// QuickJS `OP_dup1`: duplicate the value immediately below the stack top
        /// in place (`a b -> a a b`).
        Dup1 = 112, (2, 3), PURE;
        Neg = 113, (1, 1), CALL;
        Plus = 114, (1, 1), CALL;
        Inc = 115, (1, 1), CALL;
        Dec = 116, (1, 1), CALL;
        /// QuickJS `OP_post_inc`: convert the operand to numeric, then leave the
        /// old numeric value below its incremented replacement.
        PostInc = 117, (1, 2), CALL;
        /// QuickJS `OP_post_dec`: convert the operand to numeric, then leave the
        /// old numeric value below its decremented replacement.
        PostDec = 118, (1, 2), CALL;
        BitNot = 119, (1, 1), CALL;
        Not = 120, (1, 1), PURE;
        TypeOf = 121, (1, 1), ALLOC;
        IsUndefinedOrNull = 122, (1, 1), PURE;
        /// Exact `undefined` tag test. Unlike `typeof`, HTMLDDA objects are false.
        IsUndefined = 123, (1, 1), PURE;
        /// Exact `null` tag test.
        IsNull = 124, (1, 1), PURE;
        /// Fused `typeof value === "undefined"`, including HTMLDDA objects.
        TypeOfIsUndefined = 125, (1, 1), PURE;
        /// Fused `typeof value === "function"`, excluding HTMLDDA objects.
        TypeOfIsFunction = 126, (1, 1), PURE;
        Add = 127, (2, 1), CALL;
        Sub = 128, (2, 1), CALL;
        Mul = 129, (2, 1), CALL;
        Div = 130, (2, 1), CALL;
        Mod = 131, (2, 1), CALL;
        Pow = 132, (2, 1), CALL;
        Shl = 133, (2, 1), CALL;
        Sar = 134, (2, 1), CALL;
        Shr = 135, (2, 1), CALL;
        BitAnd = 136, (2, 1), CALL;
        BitXor = 137, (2, 1), CALL;
        BitOr = 138, (2, 1), CALL;
        Eq = 139, (2, 1), CALL;
        StrictEq = 140, (2, 1), PURE;
        Neq = 141, (2, 1), CALL;
        StrictNeq = 142, (2, 1), PURE;
        Lt = 143, (2, 1), CALL;
        Lte = 144, (2, 1), CALL;
        Gt = 145, (2, 1), CALL;
        Gte = 146, (2, 1), CALL;
        InstanceOf = 147, (2, 1), CALL;
        In = 148, (2, 1), CALL;
        IfFalse(target: u32) = 149, (1, 0), THROW, ctrl Branch(*target), [O::Target(*target)];
        IfTrue(target: u32) = 150, (1, 0), THROW, ctrl Branch(*target), [O::Target(*target)];
        Goto(target: u32) = 151, (0, 0), THROW, ctrl Jump(*target), [O::Target(*target)];
        /// QuickJS `OP_catch`, represented as private VM handler metadata rather
        /// than a forgeable operand-stack value.
        // The nominal marker push models the handler entry; at run time the
        // marker lives in private handler metadata.
        Catch(target: u32) = 152, (0, 1), THROW, ctrl Catch(*target), state EnterCatch,
            [O::Target(*target)];
        /// Remove the innermost active catch marker on a normal path.
        DropCatch = 153, (1, 0), THROW, state DropCatch;
        /// Preserve the top value while discarding the innermost catch marker and
        /// every intermediate operand above its entry depth.
        // `compute_stack_size` replaces the nominal effects of NipCatch and the
        // iterator *Preserve cleanups with the depth recorded by their region.
        NipCatch = 154, (1, 1), THROW, state PreserveCatch;
        /// QuickJS `OP_gosub`: push the fallthrough PC as a real Int value and
        /// enter a shared finally subroutine.
        Gosub(target: u32) = 155, (0, 0), THROW, ctrl Gosub(*target), state EnterGosub,
            [O::Target(*target)];
        /// QuickJS `OP_ret`: pop and validate the Int return PC from `Gosub`.
        Ret = 156, (1, 0), THROW, ctrl Ret, state ConsumeGosub;
        /// Typed finally cleanup for an abrupt path which discards, rather than
        /// follows, the real Int return PC pushed by `Gosub`.
        DropGosub = 157, (1, 0), THROW, state ConsumeGosub;
        /// QuickJS `OP_for_of_start`: replace one iterable with the two public
        /// iterator-record values (`iterator`, `next`) plus a private unwind
        /// marker represented only in the verifier and VM region stack.
        ForOfStart = 158, (1, 3), CALL, state EnterIterator;
        /// QuickJS `OP_for_await_of_start`: acquire an async iterator (including
        /// Async-from-Sync fallback) and install the same three-slot conceptual
        /// unwind record as [`Instruction::ForOfStart`]. Unlike
        /// [`Instruction::AsyncIteratorStart`], this record participates in
        /// automatic loop cleanup.
        ForAwaitOfStart = 159, (1, 3), CALL, state EnterIterator;
        /// QuickJS `OP_for_of_next`: retain the three-slot iterator record below
        /// `offset` intermediate operands and append `value`, `done`.
        ForOfNext(offset: u8) = 160, (3, 5), CALL, state AdvanceIterator, [O::IteratorDepth(*offset)];
        /// QuickJS `OP_for_await_of_next`: temporarily disable the async iterator
        /// unwind marker, call its cached `next` method without arguments, and
        /// append the raw result which the following [`Instruction::Await`] must
        /// settle (`iterator, next, marker -> ... result`).
        ForAwaitOfNext = 161, (3, 4), CALL, state AdvanceIterator;
        /// QuickJS `OP_iterator_get_value_done`: consume the settled async iterator
        /// result together with the conceptual private marker, read `done` and then
        /// `value`, restore the stable unwind marker, and append `value`, `done`.
        /// The private marker is represented only by verifier/VM region state.
        IteratorGetValueDone = 162, (2, 3), CALL, state FinishIteratorValue;
        /// QuickJS `OP_for_in_start`: replace one source value with a hidden
        /// enumeration object.
        ForInStart = 163, (1, 1), CALL;
        /// QuickJS `OP_for_in_next`: retain the enumeration object and append the
        /// next string key plus its `done` flag.
        ForInNext = 164, (1, 3), CALL;
        /// QuickJS `OP_iterator_close`: remove the innermost iterator record and
        /// close it for a normal abrupt completion.
        IteratorClose = 165, (3, 0), CALL, state CloseIterator;
        /// Preserve the top operand while removing the innermost iterator record
        /// and every intermediate operand, then close the iterator. This is the
        /// typed equivalent of QuickJS's `nip_catch; rot3r; undefined;
        /// iterator_close` return-unwind sequence.
        IteratorClosePreserve = 166, (1, 1), CALL, state PreserveIterator;
        /// Preserve the top operand while removing the innermost iterator record
        /// and every intermediate operand, without invoking the iterator's
        /// `return` method. QuickJS uses this shape when an injected generator
        /// return leaves the precompiled assignment fragment of a for-of head:
        /// the loop iterator record is abandoned rather than normally closed.
        IteratorDropPreserve = 167, (1, 1), CALL, state PreserveIterator;
        /// Preserve the top completion value while dynamically removing the
        /// innermost stable iterator record and all intermediate operands, but keep
        /// the iterator itself above that completion value
        /// (`... iterator, next, marker, ..., completion -> ... completion, iterator`).
        /// Async-generator return lowering uses the exposed iterator to perform its
        /// explicit call/check/await close sequence.
        IteratorDetachPreserve = 168, (1, 2), CALL, state DetachIterator;
        /// `yield*`-specific GetIterator. Unlike [`Instruction::ForOfStart`], its
        /// three outputs are ordinary retained operands (`iterator`, cached
        /// `next`, placeholder) and do not install an automatic unwind region.
        IteratorStart = 169, (1, 3), CALL;
        /// Async-generator `yield*` GetAsyncIterator. A present
        /// `Symbol.asyncIterator` is used directly; a nullish method falls back to
        /// a branded Async-from-Sync iterator. The retained three-slot record has
        /// the same compiler-private shape as [`Instruction::IteratorStart`].
        AsyncIteratorStart = 170, (1, 3), CALL;
        /// QuickJS `OP_iterator_next`: call the cached `next` method with the top
        /// resume value and replace that value with the materialized result object
        /// (`iterator, next, placeholder, value -> ... result`).
        IteratorNext = 171, (4, 4), CALL;
        /// QuickJS `OP_iterator_call`: look up `return` or `throw` on the retained
        /// iterator and append a Boolean indicating that the method was missing.
        IteratorCall(kind: IteratorCallKind) = 172, (4, 5), CALL, [O::IteratorCallKind(*kind)];
        /// Reject a non-Object iterator-result while preserving it at TOS.
        IteratorCheckObject = 173, (1, 1), CALL;
        /// QuickJS `OP_initial_yield`: suspend a freshly invoked generator after
        /// parameter initialization and before entering its authored body. It has
        /// no JavaScript-visible operand; resumption continues at the next PC.
        InitialYield = 174, (0, 0), PURE, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_yield`: consume the value exposed to the caller. Resumption
        /// replaces it with the injected resume value and appends the authenticated
        /// next/return discriminator (`value -> resume-value, resume-kind`). A
        /// throw resumption enters through the VM exception path instead.
        Yield = 175, (1, 2), CALL, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_yield_star`: the suspension half of delegated yield. The
        /// operand is an already-materialized iterator-result object, which the
        /// generator driver must return without re-wrapping (`pdone == 2`). The
        /// compiler-owned delegation loop consumes the same two resume operands as
        /// ordinary `Yield`.
        YieldStar = 176, (1, 2), CALL, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_async_yield_star`: suspend an async generator with the
        /// delegate result's already-extracted `value`. The async-generator driver
        /// wraps that value in the current request's `{ value, done: false }`
        /// result, while resumption uses the same next/return/throw magic ABI as
        /// synchronous delegation.
        AsyncYieldStar = 177, (1, 2), CALL, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_await`: consume the value which the async-function driver
        /// resolves through the realm's intrinsic Promise constructor. Fulfilment
        /// resumes by replacing the retained slot with the resolution value;
        /// rejection is raised through the activation's ordinary unwind path.
        Await = 178, (1, 1), CALL, ctrl Suspend, state ResumeAwait;
        /// Terminal TypeError used by `yield*` when the delegate has no `throw`
        /// method. A present `return` method has already been called for close.
        ThrowIteratorMissingThrow = 179, (0, 0), CALL, ctrl Throw;
        /// QuickJS `OP_import`: consume the evaluated module specifier and import
        /// options in source order and replace them with the host-created Promise
        /// (`specifier, options -> promise`). The compiler keeps this opcode behind
        /// its dynamic-import frontier until the runtime host implements the full
        /// Promise/job/module-cache protocol.
        Import = 180, (2, 1), CALL;
        Call(argc: u16) = 181, (*argc as usize + 1, 1), CALL, [O::ArgumentCount(*argc)];
        /// QuickJS `OP_tail_call`: call with an undefined receiver and use the
        /// resulting completion as the completion of the current frame.
        TailCall(argc: u16) = 182, (*argc as usize + 1, 0), CALL, ctrl TailCall,
            [O::ArgumentCount(*argc)];
        /// QuickJS `OP_eval`: a syntactic direct-eval call site. The runtime first
        /// compares the resolved callee with the executing realm's original
        /// `%eval%`; a replacement callee falls back to an ordinary call with an
        /// undefined receiver. `environment` selects the immutable linked lexical
        /// environment published beside this function's bytecode; String-source
        /// execution retains it for authenticated nested direct-eval relays.
        Eval { argument_count: u16, environment: u16 } = 183, (*argument_count as usize + 1, 1), CALL,
            [O::ArgumentCount(*argument_count), O::EvalEnvironment(*environment)];
        CallMethod(argc: u16) = 184, (*argc as usize + 2, 1), CALL, [O::ArgumentCount(*argc)];
        /// Receiver-preserving counterpart of [`Instruction::TailCall`].
        TailCallMethod(argc: u16) = 185, (*argc as usize + 2, 0), CALL, ctrl TailCall,
            [O::ArgumentCount(*argc)];
        /// QuickJS `OP_call_constructor`: `func new.target args -> result`.
        Construct(argc: u16) = 186, (*argc as usize + 2, 1), CALL, [O::ArgumentCount(*argc)];
        /// Authenticate the two stack operands prepared for a derived-constructor
        /// `super()` call. Runtime execution is a no-op; the bytecode verifier
        /// protects the marked `super_constructor, new.target` pair until exactly
        /// one matching typed construction consumes it.
        MarkSuperCall = 187, (2, 2), PURE, state MarkSuperCall;
        /// Derived-constructor counterpart of [`Instruction::Construct`]. The
        /// verifier accepts it only when it exactly consumes the most recent
        /// protected super-call pair together with `argument_count` values.
        ConstructSuper(argc: u16) = 188, (*argc as usize + 2, 1), CALL, state ConsumeSuperCall,
            [O::ArgumentCount(*argc)];
        /// QuickJS `OP_check_ctor`: reject an ordinary call to a class
        /// constructor by checking the current frame's `new.target`.
        CheckCtor = 189, (0, 0), CALL;
        /// QuickJS `OP_init_ctor`: run the implicit derived constructor by reading
        /// the active function object's live `[[Prototype]]`, forwarding the frame's
        /// raw actual arguments, and preserving the original `new.target`.
        InitDerivedConstructor = 190, (0, 1), CALL;
        /// QuickJS `OP_apply`: consume a compiler-created dense argument Array
        /// after all spread iterables have been evaluated.
        Apply(kind: ApplyKind) = 191, (3, 1), CALL, [O::ApplyKind(*kind)];
        /// Spread-argument counterpart of [`Instruction::ConstructSuper`].
        ApplySuper = 192, (3, 1), CALL, state ConsumeSuperCall;
        /// QuickJS `OP_apply_eval`: the spread-argument counterpart of `Eval`.
        /// The original-eval identity check happens only after the dense argument
        /// Array has passed through QuickJS-compatible `build_arg_list` semantics.
        ApplyEval { environment: u16 } = 193, (2, 1), CALL, [O::EvalEnvironment(*environment)];
        Return = 194, (1, 0), THROW, ctrl Return;
        /// Return `undefined` without consuming an operand-stack value.
        ReturnUndefined = 195, (0, 0), THROW, ctrl Return;
        /// Complete a derived constructor. Object results are returned directly;
        /// `undefined` resolves the authenticated derived-`this` local; every other
        /// primitive throws a TypeError. Return-protocol errors belong to the
        /// caller realm rather than the constructor's defining realm.
        ReturnDerived(local: u16) = 196, (1, 0), CALL, ctrl Return, [O::Local(*local)];
        Throw = 197, (1, 0), CALL, ctrl Throw;
    }
    execution_only {
        /// A published GetLocal + PushI32(1) + Add span. On a guard miss the
        /// same word acts as GetLocal and the following generic words run.
        NumberLocalInc = 198, 1;
        /// Planned numeric Array read, multiplication, addition and direct local
        /// commit. The original words remain the generic continuation.
        NumericArrayAccumulate = 199, 3;
        NumberArgInc = 200, 1;
        /// Field reads with a direct location-cache probe in the execution loop.
        GetFieldCached = 201, 1;
        GetField2Cached = 202, 1;
        GetArrayElDense = 203, 0;
        GetArrayEl2Dense = 204, 0;
        GetArrayEl3Dense = 205, 0;
        /// Published five-operation base/index update and dense read. Operand 1
        /// carries the updated slot and operation; operand 2 is the verified end PC.
        DensePreUpdateLocal = 206, 3;
        DensePreUpdateArg = 207, 3;
        DenseReadLocal = 208, 3;
        DenseReadArg = 209, 3;
        BorrowedFieldLocal = 210, 3;
        BorrowedFieldArg = 211, 3;
        CompareBranchLocal = 212, 3;
        CompareBranchArg = 213, 3;
        CompareBranchStack = 214, 2;
        UpdateLocalDiscard = 215, 1;
        UpdateLocalDiscardCheck = 216, 1;
        DensePostUpdateLocal = 217, 3;
        DensePostUpdateLocalCheck = 218, 3;
        DensePostUpdateArg = 219, 3;
        DenseReadBinaryLocal = 220, 3;
        DenseReadBinaryArg = 221, 3;
        DenseIndexBinaryLocal = 222, 3;
        DenseIndexBinaryArg = 223, 3;
        DenseAccIndexSetDrop = 224, 3;
        FieldAccSetDrop = 225, 2;
        CompareBranchLocalLt = 226, 3;
        CompareBranchArgLt = 227, 3;
        NumericArrayStoreProduct = 228, 3;
        NumericArrayCopyElement = 229, 3;
        NumericArrayAddPreInc = 230, 3;
        NumericArrayStoreAndLocal = 231, 3;
        NumericArrayUpdateElement = 232, 3;
        NumericArrayCompareBranch = 233, 3;
        BorrowedFieldThis = 234, 3;
        /// `PushThis; GetField; GetLocal|GetArg; GetArrayEl` read without
        /// retaining `this` or the field. On a miss the word acts as PushThis.
        BorrowedFieldThisElement = 235, 3;
    }
}

/// Render the actual final instruction stream with the same facts consumed by
/// stack-size computation and optimization. Formatting is diagnostic-only.
#[cfg(feature = "profiling")]
pub(crate) fn disassemble(code: &[Instruction]) -> String {
    use std::fmt::Write;
    let mut output = String::new();
    for (pc, instruction) in code.iter().enumerate() {
        writeln!(
            &mut output,
            "{pc:05} {instruction:?} ; {:?}",
            instruction.info()
        )
        .expect("writing to a String cannot fail");
    }
    output
}
