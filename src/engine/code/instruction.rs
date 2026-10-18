//! The opcode table: every compiler [`Instruction`] and the per-opcode facts
//! read by lowering and stack-size computation are declared once, in the
//! style of QuickJS `quickjs-opcode.h`.
use super::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, DynamicEnvironmentSource, EvalVariableSource,
    IteratorCallKind, PrivateNameSource,
};

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
            Self::Next | Self::Ret | Self::Return | Self::Throw | Self::Suspend => None,
        }
    }
    pub(crate) const fn ends_block(self) -> bool {
        !matches!(self, Self::Next)
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

impl Instruction {
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

/// Declares [`Instruction`] and every per-opcode fact from one table, in the
/// style of QuickJS `quickjs-opcode.h`. Each row reads
///
/// `Variant(binder: Type) = (popped, pushed), EFFECTS[, ctrl Control(target)][, state State];`
///
/// `(popped, pushed)` is the nominal stack effect; `EFFECTS` a
/// [`PotentialEffects`] preset; `ctrl` the [`ControlEffect`] (default `Next`);
/// and `state` the [`StackStateEffect`] (default `Ordinary`). Expressions read
/// the row's binders by reference.
macro_rules! opcode_table {
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
                    = $stack:expr, $effects:ident
                    $(, ctrl $control:ident $(($target:expr))?)?
                    $(, state $state:ident)?;
            )*
        }
    ) => {
        $(#[$enum_meta])*
        pub enum Instruction {
            $($(#[$doc])* $name $(($binder_type))? $({ $($field: $field_type),+ })?,)*
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
    /// their `QuickJS` counterparts. This typed form is the front end's IR,
    /// which the register lowering consumes.
    #[derive(Clone, Debug)]
    pub enum Instruction {
        Nop = (0, 0), PURE;
        PushI32(value: i32) = (0, 1), PURE;
        PushConst(index: u32) = (0, 1), PURE;
        FClosure(index: u32) = (0, 1), ALLOC;
        /// QuickJS `OP_regexp`, represented as one typed compile-time constant
        /// instead of two generic stack values. Each execution allocates a fresh
        /// branded RegExp in the bytecode realm without consulting user code.
        RegExp(index: u32) = (0, 1), ALLOC;
        /// QuickJS `OP_set_name`: conditionally define the name of the object at
        /// the top of the stack from a string constant, without consuming it.
        SetName(name: u32) = (1, 1), CALL;
        /// QuickJS `OP_throw_error atom JS_THROW_VAR_RO` for a strict immutable
        /// binding. It consumes no stack value and terminates, abandoning the
        /// complete frame stack on the exception path.
        ThrowReadOnly(name: u32) = (0, 0), CALL, ctrl Throw;
        /// QuickJS `OP_throw_error atom JS_THROW_VAR_REDECL`: throw the eval-time
        /// SyntaxError used for a `var`/function declaration which crosses an
        /// imported lexical scope. The verified String constant preserves the
        /// conflicting binding identity even though QuickJS's message is generic.
        ThrowRedeclaration(name: u32) = (0, 0), CALL, ctrl Throw;
        /// QuickJS `JS_THROW_ERROR_DELETE_SUPER`. The three authenticated
        /// SuperProperty Reference operands are consumed conceptually and the
        /// terminal throw occupies the expression's one-value stack shape.
        ThrowDeleteSuper = (3, 1), CALL, ctrl Throw;
        Undefined = (0, 1), PURE;
        Null = (0, 1), PURE;
        PushFalse = (0, 1), PURE;
        PushTrue = (0, 1), PURE;
        PushThis = (0, 1), ALLOC;
        /// Push the exact function object of the active bytecode frame. Derived
        /// constructors use its live `[[Prototype]]` as the target of `super()`, so
        /// this must not be reconstructed from HomeObject or the original heritage
        /// expression.
        PushActiveFunction = (0, 1), PURE;
        /// QuickJS's authenticated `<home_object>` pseudo binding. Only bytecode
        /// whose immutable metadata declares `needs_home_object` may execute this
        /// operation; the runtime reads it from the active function object rather
        /// than from a JavaScript-visible operand.
        PushHomeObject = (0, 1), PURE;
        PushNewTarget = (0, 1), PURE;
        /// QuickJS `OP_special_object` for an ordinary function's lazily selected
        /// arguments binding. Runtime creation still occurs in the entry prologue,
        /// before body function hoists.
        Arguments(kind: ArgumentsKind) = (0, 1), ALLOC;
        /// QuickJS `OP_rest`: collect the actual arguments beginning at this
        /// formal-parameter index into a fresh Array in the callee realm. The
        /// operand may equal `argument_count` for a rest BindingPattern, which
        /// owns no physical argument slot.
        Rest(start: u16) = (0, 1), ALLOC;
        /// QuickJS `OP_special_object` with `OP_SPECIAL_OBJECT_VAR_OBJECT`: create
        /// the null-prototype object which stores names introduced by sloppy
        /// direct eval in this activation.
        VariableEnvironment = (0, 1), ALLOC;
        /// Consume a value and define an own configurable, writable, enumerable
        /// data property on the hidden variable object.
        DefineEvalVariable { source: EvalVariableSource, name: u32 } = (1, 0), CALL;
        /// ECMAScript `ToObject`: preserve Objects, reject nullish values, and box
        /// all other primitives in the executing realm.
        ToObject = (1, 1), CALL;
        /// Test whether an ordered hidden environment exposes one dynamic name.
        HasDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = (0, 1), CALL;
        /// Read one dynamic binding using the current frame's strictness.
        GetDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = (0, 1), CALL;
        /// Consume and write one dynamic binding using the current frame's
        /// strictness.
        PutDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = (1, 0), CALL;
        /// Delete one dynamic binding and push the Boolean result.
        DeleteDynamicBinding { source: DynamicEnvironmentSource, name: u32 } = (0, 1), CALL;
        /// Push the authenticated object behind one dynamic environment source.
        DynamicEnvironmentObject(source: DynamicEnvironmentSource) = (0, 1), THROW_ALLOC;
        /// QuickJS `OP_make_var_ref`: resolve one authenticated global closure
        /// name to the current realm's lexical storage object, global object, or
        /// the unresolved-reference `undefined` sentinel. Lexical TDZ and
        /// read-only checks happen while this reference is made, before a later
        /// right-hand side can run.
        GlobalReference(closure: u16) = (0, 1), CALL;
        /// Preserve an environment Object and append its named reference value.
        /// A missing property observes the current frame's strictness.
        GetRefValue(name: u32) = (1, 2), CALL;
        /// Preserve an environment Object and append its named reference value,
        /// producing `undefined` when the property is missing.
        GetRefValueUndef(name: u32) = (1, 2), CALL;
        /// Consume an environment Object and value and write the named reference
        /// using the current frame's strictness.
        PutRefValue(name: u32) = (2, 0), CALL;
        GetLocal(local: u16) = (0, 1), PURE;
        PutLocal(local: u16) = (1, 0), PURE;
        SetLocal(local: u16) = (1, 1), PURE;
        /// QuickJS `OP_set_loc_uninitialized`: enter one lexical scope with its
        /// local slot in the temporal dead zone.
        SetLocalUninitialized(local: u16) = (0, 0), PURE;
        /// QuickJS `OP_get_loc_check`: read a lexical local after its TDZ check.
        GetLocalCheck(local: u16) = (0, 1), THROW_ALLOC;
        /// Typed-format distinction for QuickJS's lexical-initializer `put_loc`.
        /// The published vardef must be lexical, but execution deliberately keeps
        /// ordinary `put_loc` overwrite semantics rather than conflating this with
        /// derived-`this`'s upstream `put_loc_check_init` opcode.
        InitializeLocal(local: u16) = (1, 0), ALLOC;
        /// QuickJS `OP_put_loc_check_init`: consume the result of `super()` and
        /// initialize the authenticated derived-constructor `this` local exactly
        /// once. A second execution throws after the super constructor has run.
        InitializeDerivedLocal(local: u16) = (1, 0), THROW_ALLOC;
        /// QuickJS `OP_put_loc_check`: consume and assign a mutable lexical local.
        PutLocalCheck(local: u16) = (1, 0), THROW_ALLOC;
        /// QuickJS `OP_set_loc_check`: assign a mutable lexical local while
        /// preserving the assigned value on the operand stack.
        SetLocalCheck(local: u16) = (1, 1), THROW_ALLOC;
        GetArg(arg: u16) = (0, 1), PURE;
        PutArg(arg: u16) = (1, 0), PURE;
        SetArg(arg: u16) = (1, 1), PURE;
        GetVarRef(closure: u16) = (0, 1), PURE;
        PutVarRef(closure: u16) = (1, 0), PURE;
        SetVarRef(closure: u16) = (1, 1), PURE;
        /// QuickJS `OP_get_var_ref_check`: read a captured lexical binding after
        /// its TDZ check.
        GetVarRefCheck(closure: u16) = (0, 1), THROW_ALLOC;
        /// QuickJS `OP_put_var_ref_check`: consume and assign a captured mutable
        /// lexical binding. Value-preserving writes use `Dup; PutVarRefCheck`.
        PutVarRefCheck(closure: u16) = (1, 0), THROW_ALLOC;
        /// QuickJS's declaration-time raw VarRef write for an ordinary lexical
        /// module binding. Publication proves this occurs once in the authored
        /// evaluation path; the cell may already have been changed indirectly by
        /// QuickJS's import/declaration collision quirk.
        InitializeVarRef(closure: u16) = (1, 0), ALLOC;
        /// Pinned QuickJS declaration initialization when an import and a module
        /// declaration share the import's first closure slot. Publication accepts
        /// this raw write only for a matching module collision ledger entry;
        /// ordinary writes to the same binding remain read-only.
        InitializeModuleImportCollision(closure: u16) = (1, 0), ALLOC;
        /// Captured-binding counterpart of [`Instruction::InitializeDerivedLocal`].
        /// Arrow functions and direct eval use this to initialize their enclosing
        /// derived constructor's single `this` cell.
        InitializeDerivedVarRef(closure: u16) = (1, 0), THROW_ALLOC;
        /// QuickJS `OP_close_loc`: detach a captured local when its lexical scope
        /// exits so a later scope entry receives a fresh cell.
        CloseLocal(local: u16) = (0, 0), ALLOC;
        /// QuickJS `OP_get_var`: read a global-environment VarRef closure slot.
        GetVar(closure: u16) = (0, 1), CALL;
        /// QuickJS `OP_get_var_undef`: as `GetVar`, but suppress a genuinely
        /// missing global binding for a direct `typeof IdentifierReference`.
        GetVarUndef(closure: u16) = (0, 1), CALL;
        /// QuickJS `OP_delete_var`: delete one sloppy direct global binding and
        /// push the Boolean result. Lexical and resolved non-global bindings are
        /// folded to `PushFalse` during scope resolution.
        DeleteVar(closure: u16) = (0, 1), CALL;
        /// QuickJS `OP_put_var`: assign and consume a global binding value.
        PutVar(closure: u16) = (1, 0), CALL;
        /// QuickJS `OP_put_var_init`: initialize a lexical global binding.
        PutVarInit(closure: u16) = (1, 0), CALL;
        /// Allocate one fresh QuickJS private symbol and initialize the named
        /// class-private lexical local without exposing that symbol as a Value.
        InitializePrivateName(local: u16) = (0, 0), ALLOC;
        /// QuickJS private-method declaration bootstrap. Consume the class-side
        /// HomeObject and freshly instantiated method closure, install the hidden
        /// HomeObject edge, initialize the authenticated private-method lexical
        /// cell, and retain only the HomeObject on the operand stack.
        InitializePrivateMethod(local: u16) = (2, 1), CALL;
        /// QuickJS private-accessor declaration bootstrap. Consume the class-side
        /// HomeObject and freshly instantiated getter or setter closure, install
        /// the hidden HomeObject edge, initialize the authenticated accessor cell,
        /// and retain only the HomeObject. Unlike a private method, an accessor's
        /// public function name remains the empty string.
        InitializePrivateAccessor(local: u16) = (2, 1), CALL;
        /// QuickJS `OP_get_private_field`: own-only private field read.
        GetPrivateField(source: PrivateNameSource) = (1, 1), CALL;
        /// Call/compound counterpart which retains the original receiver.
        GetPrivateField2(source: PrivateNameSource) = (1, 2), CALL;
        /// QuickJS `OP_put_private_field`: own-only private field write.
        PutPrivateField(source: PrivateNameSource) = (2, 0), CALL;
        /// QuickJS `OP_define_private_field`: define a fresh private own field
        /// even when the receiver is non-extensible.
        DefinePrivateField(source: PrivateNameSource) = (2, 1), CALL;
        /// QuickJS `OP_private_in` for a private data-field binding.
        PrivateIn(source: PrivateNameSource) = (1, 1), CALL;
        /// QuickJS `OP_get_field`: replace an object-like base with the value of
        /// one constant string-keyed property.
        GetField(name: u32) = (1, 1), CALL;
        /// QuickJS `OP_get_field2`: keep the base below the fetched value so a
        /// following `CallMethod` observes the original reference receiver.
        GetField2(name: u32) = (1, 2), CALL;
        /// QuickJS `OP_get_array_el`: `base key -> value`, including observable
        /// `ToPropertyKey` conversion in the runtime host.
        GetArrayEl = (2, 1), CALL;
        /// QuickJS `OP_get_array_el2`: `base key -> base value` for method calls.
        GetArrayEl2 = (2, 2), CALL;
        /// QuickJS `OP_get_array_el3`: `base raw-key -> base converted-key value`
        /// for compound/logical assignment without repeated key conversion.
        GetArrayEl3 = (2, 3), CALL;
        /// QuickJS `OP_get_super`: replace a method HomeObject with its current
        /// prototype. A null prototype is represented by JavaScript `null` and is
        /// diagnosed only by the following property operation.
        GetSuper = (1, 1), CALL;
        /// QuickJS `OP_get_super_value`: `receiver base key -> value`. The base is
        /// the prototype captured before a computed key is evaluated, while
        /// accessors observe the method's actual `this` as their receiver.
        GetSuperValue = (3, 1), CALL;
        /// QuickJS's call-site rewrite of `OP_get_super_value` to
        /// `OP_get_array_el`: `receiver base key -> receiver value`. This
        /// deliberately gives a getter `base` as its receiver before the returned
        /// callable is invoked with the preserved method receiver.
        GetSuperValueForCall = (3, 2), CALL;
        /// QuickJS `OP_array_from`: consume `element_count` dense literal values
        /// and replace them with a fresh Array in the bytecode's realm.
        ArrayFrom(count: u16) = (*count as usize, 1), ALLOC;
        /// QuickJS `OP_object`: push a fresh ordinary Object rooted in the
        /// executing bytecode's realm.
        Object = (0, 1), ALLOC;
        /// QuickJS `OP_to_propkey`: observable `ToPropertyKey` while retaining a
        /// canonical Int/String/Symbol value on the VM stack.
        ToPropKey = (1, 1), CALL;
        /// QuickJS `OP_insert2`: `base value -> value base value`.
        Insert2 = (2, 3), PURE;
        /// QuickJS `OP_insert3`: `base key value -> value base key value`.
        Insert3 = (3, 4), PURE;
        /// QuickJS `OP_dup3`: `a b c -> a b c a b c`.
        Dup3 = (3, 6), PURE;
        /// QuickJS `OP_insert4`: `this base key value -> value this base key value`.
        Insert4 = (4, 5), PURE;
        /// QuickJS `OP_perm3`: `base old new -> old base new`.
        Perm3 = (3, 3), PURE;
        /// QuickJS `OP_perm4`: `base key old new -> old base key new`.
        Perm4 = (4, 4), PURE;
        /// QuickJS `OP_perm5`: `this base key old new -> old this base key new`.
        Perm5 = (5, 5), PURE;
        /// QuickJS `OP_rot4l`: `value this base key -> this base key value`.
        Rot4Left = (4, 4), PURE;
        /// QuickJS `OP_put_field`: assign one constant string-keyed property.
        PutField(name: u32) = (2, 0), CALL;
        /// QuickJS `OP_put_array_el`: assign a computed property, converting the
        /// still-raw key only after the right-hand side has been evaluated.
        PutArrayEl = (3, 0), CALL;
        /// QuickJS `OP_put_super_value`: assign through `base` with `receiver` as
        /// the actual receiver. `JS_PROP_THROW_STRICT` rejects failed writes only
        /// when the containing method is strict.
        PutSuperValue = (4, 0), CALL;
        /// QuickJS `OP_define_field`: define one C_W_E data property while
        /// preserving the object below it (`object value -> object`). Array
        /// literals use this after their initial dense prefix.
        DefineField(name: u32) = (2, 1), CALL;
        /// Computed-key class-field form of QuickJS `OP_define_array_el`. The key
        /// is the authenticated output of `ToPropKey`, so execution must not run
        /// observable property-key conversion again. Defines one C_W_E own data
        /// property while preserving only the object
        /// (`object canonical-key value -> object`).
        DefineFieldComputed = (3, 1), CALL;
        /// QuickJS `OP_define_method`: name and publish an object-literal method
        /// under one verified String constant while preserving the fresh literal
        /// (`object closure -> object`).
        DefineMethod { key: u32, kind: DefineMethodKind, enumerable: bool } = (2, 1), CALL;
        /// QuickJS `OP_define_method_computed`: publish an object-literal method
        /// under an already-canonical property key while preserving the fresh
        /// literal (`object key closure -> object`).
        DefineMethodComputed { kind: DefineMethodKind, enumerable: bool } = (3, 1), CALL;
        /// QuickJS `OP_define_class`: replace `parent, constructor` with the
        /// published class `constructor, prototype` pair. The heritage flag is
        /// authenticated bytecode metadata rather than a JavaScript stack value.
        DefineClass { name: u32, has_heritage: bool } = (2, 2), CALL;
        /// Attach one authenticated hidden instance-fields program to a freshly
        /// defined class constructor while installing the class prototype as its
        /// HomeObject (`constructor prototype initializer -> constructor prototype`).
        InstallClassInstanceInitializer = (3, 2), CALL;
        /// Run the hidden instance-fields program attached to the active class
        /// constructor with the retained receiver as `this`
        /// (`receiver active-constructor -> receiver`).
        /// Absence of an initializer is the ordinary no-fields fast path.
        CallClassInstanceInitializer = (2, 1), CALL;
        /// Install the constructor as HomeObject and immediately execute the
        /// aggregate static-elements program (`constructor initializer -> constructor`).
        RunClassStaticInitializer = (2, 1), CALL;
        /// Execute a non-escaping static-block child with the aggregate static
        /// initializer's receiver and HomeObject (`block ->`).
        CallClassStaticBlock = (1, 0), CALL;
        /// QuickJS `OP_define_array_el`: define a computed C_W_E data property
        /// while preserving the Array and its dynamic index
        /// (`array index value -> array index`).
        DefineArrayEl = (3, 2), CALL;
        /// QuickJS `OP_set_name_computed`: infer an anonymous function's name
        /// from an already-canonical computed property key while preserving both
        /// operands (`key value -> key value`).
        SetNameComputed = (2, 2), CALL;
        /// QuickJS `OP_set_proto`: consume one object-literal prototype candidate
        /// and preserve the fresh literal (`object value -> object`). Object and
        /// null candidates replace `[[Prototype]]`; primitive candidates are ignored.
        SetProto = (2, 1), CALL;
        /// Object-literal specialization of QuickJS `OP_copy_data_properties`.
        /// It snapshots enumerable own String/Symbol keys from an Object source,
        /// defines C_W_E data properties, and preserves the fresh literal
        /// (`target source -> target`).
        CopyDataProperties = (2, 1), CALL;
        /// Exclusion-aware QuickJS `OP_copy_data_properties` used by an Object
        /// binding rest element. The three operands are addressed by depth from
        /// the top of the stack (depth zero is the top) and are only borrowed:
        /// successful execution leaves the complete operand stack unchanged.
        ///
        /// This is intentionally depth-addressed instead of consuming three
        /// adjacent values. A sloppy `var` binding may need to keep its prepared
        /// Reference between the retained source and the fresh target while the
        /// copy executes observable getters.
        CopyDataPropertiesExcluded { target_depth: u8, source_depth: u8, excluded_depth: u8 } =
            excluded_copy_window([*target_depth, *source_depth, *excluded_depth]), CALL;
        /// QuickJS `OP_append`: append every value from one iterable and replace
        /// the dynamic index with the first unused index
        /// (`array index iterable -> array index`).
        Append = (3, 2), CALL;
        /// QuickJS `OP_delete`: `base key -> bool` with strictness supplied by the
        /// active call frame.
        Delete = (2, 1), CALL;
        Drop = (1, 0), PURE;
        /// QuickJS `OP_nip`: discard the value immediately below the stack top,
        /// preserving the top value (`a b -> b`).
        Nip = (2, 1), PURE;
        /// QuickJS `OP_swap`: exchange the top two operands (`a b -> b a`).
        Swap = (2, 2), PURE;
        Dup = (1, 2), PURE;
        /// QuickJS `OP_dup1`: duplicate the value immediately below the stack top
        /// in place (`a b -> a a b`).
        Dup1 = (2, 3), PURE;
        Neg = (1, 1), CALL;
        Plus = (1, 1), CALL;
        Inc = (1, 1), CALL;
        Dec = (1, 1), CALL;
        /// QuickJS `OP_post_inc`: convert the operand to numeric, then leave the
        /// old numeric value below its incremented replacement.
        PostInc = (1, 2), CALL;
        /// QuickJS `OP_post_dec`: convert the operand to numeric, then leave the
        /// old numeric value below its decremented replacement.
        PostDec = (1, 2), CALL;
        BitNot = (1, 1), CALL;
        Not = (1, 1), PURE;
        TypeOf = (1, 1), ALLOC;
        IsUndefinedOrNull = (1, 1), PURE;
        Add = (2, 1), CALL;
        Sub = (2, 1), CALL;
        Mul = (2, 1), CALL;
        Div = (2, 1), CALL;
        Mod = (2, 1), CALL;
        Pow = (2, 1), CALL;
        Shl = (2, 1), CALL;
        Sar = (2, 1), CALL;
        Shr = (2, 1), CALL;
        BitAnd = (2, 1), CALL;
        BitXor = (2, 1), CALL;
        BitOr = (2, 1), CALL;
        Eq = (2, 1), CALL;
        StrictEq = (2, 1), PURE;
        Neq = (2, 1), CALL;
        StrictNeq = (2, 1), PURE;
        Lt = (2, 1), CALL;
        Lte = (2, 1), CALL;
        Gt = (2, 1), CALL;
        Gte = (2, 1), CALL;
        InstanceOf = (2, 1), CALL;
        In = (2, 1), CALL;
        IfFalse(target: u32) = (1, 0), THROW, ctrl Branch(*target);
        IfTrue(target: u32) = (1, 0), THROW, ctrl Branch(*target);
        Goto(target: u32) = (0, 0), THROW, ctrl Jump(*target);
        /// QuickJS `OP_catch`, represented as private VM handler metadata rather
        /// than a forgeable operand-stack value.
        // The nominal marker push models the handler entry; at run time the
        // marker lives in private handler metadata.
        Catch(target: u32) = (0, 1), THROW, ctrl Catch(*target), state EnterCatch;
        /// Remove the innermost active catch marker on a normal path.
        DropCatch = (1, 0), THROW, state DropCatch;
        /// Preserve the top value while discarding the innermost catch marker and
        /// every intermediate operand above its entry depth.
        // `compute_stack_size` replaces the nominal effects of NipCatch and the
        // iterator *Preserve cleanups with the depth recorded by their region.
        NipCatch = (1, 1), THROW, state PreserveCatch;
        /// QuickJS `OP_gosub`: push the fallthrough PC as a real Int value and
        /// enter a shared finally subroutine.
        Gosub(target: u32) = (0, 0), THROW, ctrl Gosub(*target), state EnterGosub;
        /// QuickJS `OP_ret`: pop and validate the Int return PC from `Gosub`.
        Ret = (1, 0), THROW, ctrl Ret, state ConsumeGosub;
        /// Typed finally cleanup for an abrupt path which discards, rather than
        /// follows, the real Int return PC pushed by `Gosub`.
        DropGosub = (1, 0), THROW, state ConsumeGosub;
        /// QuickJS `OP_for_of_start`: replace one iterable with the two public
        /// iterator-record values (`iterator`, `next`) plus a private unwind
        /// marker represented only in the verifier and VM region stack.
        ForOfStart = (1, 3), CALL, state EnterIterator;
        /// QuickJS `OP_for_await_of_start`: acquire an async iterator (including
        /// Async-from-Sync fallback) and install the same three-slot conceptual
        /// unwind record as [`Instruction::ForOfStart`]. Unlike
        /// [`Instruction::AsyncIteratorStart`], this record participates in
        /// automatic loop cleanup.
        ForAwaitOfStart = (1, 3), CALL, state EnterIterator;
        /// QuickJS `OP_for_of_next`: retain the three-slot iterator record below
        /// `offset` intermediate operands and append `value`, `done`.
        ForOfNext(offset: u8) = (3, 5), CALL, state AdvanceIterator;
        /// QuickJS `OP_for_await_of_next`: temporarily disable the async iterator
        /// unwind marker, call its cached `next` method without arguments, and
        /// append the raw result which the following [`Instruction::Await`] must
        /// settle (`iterator, next, marker -> ... result`).
        ForAwaitOfNext = (3, 4), CALL, state AdvanceIterator;
        /// QuickJS `OP_iterator_get_value_done`: consume the settled async iterator
        /// result together with the conceptual private marker, read `done` and then
        /// `value`, restore the stable unwind marker, and append `value`, `done`.
        /// The private marker is represented only by verifier/VM region state.
        IteratorGetValueDone = (2, 3), CALL, state FinishIteratorValue;
        /// QuickJS `OP_for_in_start`: replace one source value with a hidden
        /// enumeration object.
        ForInStart = (1, 1), CALL;
        /// QuickJS `OP_for_in_next`: retain the enumeration object and append the
        /// next string key plus its `done` flag.
        ForInNext = (1, 3), CALL;
        /// QuickJS `OP_iterator_close`: remove the innermost iterator record and
        /// close it for a normal abrupt completion.
        IteratorClose = (3, 0), CALL, state CloseIterator;
        /// Preserve the top operand while removing the innermost iterator record
        /// and every intermediate operand, then close the iterator. This is the
        /// typed equivalent of QuickJS's `nip_catch; rot3r; undefined;
        /// iterator_close` return-unwind sequence.
        IteratorClosePreserve = (1, 1), CALL, state PreserveIterator;
        /// Preserve the top operand while removing the innermost iterator record
        /// and every intermediate operand, without invoking the iterator's
        /// `return` method. QuickJS uses this shape when an injected generator
        /// return leaves the precompiled assignment fragment of a for-of head:
        /// the loop iterator record is abandoned rather than normally closed.
        IteratorDropPreserve = (1, 1), CALL, state PreserveIterator;
        /// Preserve the top completion value while dynamically removing the
        /// innermost stable iterator record and all intermediate operands, but keep
        /// the iterator itself above that completion value
        /// (`... iterator, next, marker, ..., completion -> ... completion, iterator`).
        /// Async-generator return lowering uses the exposed iterator to perform its
        /// explicit call/check/await close sequence.
        IteratorDetachPreserve = (1, 2), CALL, state DetachIterator;
        /// `yield*`-specific GetIterator. Unlike [`Instruction::ForOfStart`], its
        /// three outputs are ordinary retained operands (`iterator`, cached
        /// `next`, placeholder) and do not install an automatic unwind region.
        IteratorStart = (1, 3), CALL;
        /// Async-generator `yield*` GetAsyncIterator. A present
        /// `Symbol.asyncIterator` is used directly; a nullish method falls back to
        /// a branded Async-from-Sync iterator. The retained three-slot record has
        /// the same compiler-private shape as [`Instruction::IteratorStart`].
        AsyncIteratorStart = (1, 3), CALL;
        /// QuickJS `OP_iterator_next`: call the cached `next` method with the top
        /// resume value and replace that value with the materialized result object
        /// (`iterator, next, placeholder, value -> ... result`).
        IteratorNext = (4, 4), CALL;
        /// QuickJS `OP_iterator_call`: look up `return` or `throw` on the retained
        /// iterator and append a Boolean indicating that the method was missing.
        IteratorCall(kind: IteratorCallKind) = (4, 5), CALL;
        /// Reject a non-Object iterator-result while preserving it at TOS.
        IteratorCheckObject = (1, 1), CALL;
        /// QuickJS `OP_initial_yield`: suspend a freshly invoked generator after
        /// parameter initialization and before entering its authored body. It has
        /// no JavaScript-visible operand; resumption continues at the next PC.
        InitialYield = (0, 0), PURE, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_yield`: consume the value exposed to the caller. Resumption
        /// replaces it with the injected resume value and appends the authenticated
        /// next/return discriminator (`value -> resume-value, resume-kind`). A
        /// throw resumption enters through the VM exception path instead.
        Yield = (1, 2), CALL, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_yield_star`: the suspension half of delegated yield. The
        /// operand is an already-materialized iterator-result object, which the
        /// generator driver must return without re-wrapping (`pdone == 2`). The
        /// compiler-owned delegation loop consumes the same two resume operands as
        /// ordinary `Yield`.
        YieldStar = (1, 2), CALL, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_async_yield_star`: suspend an async generator with the
        /// delegate result's already-extracted `value`. The async-generator driver
        /// wraps that value in the current request's `{ value, done: false }`
        /// result, while resumption uses the same next/return/throw magic ABI as
        /// synchronous delegation.
        AsyncYieldStar = (1, 2), CALL, ctrl Suspend, state ResumeYield;
        /// QuickJS `OP_await`: consume the value which the async-function driver
        /// resolves through the realm's intrinsic Promise constructor. Fulfilment
        /// resumes by replacing the retained slot with the resolution value;
        /// rejection is raised through the activation's ordinary unwind path.
        Await = (1, 1), CALL, ctrl Suspend, state ResumeAwait;
        /// Terminal TypeError used by `yield*` when the delegate has no `throw`
        /// method. A present `return` method has already been called for close.
        ThrowIteratorMissingThrow = (0, 0), CALL, ctrl Throw;
        /// QuickJS `OP_import`: consume the evaluated module specifier and import
        /// options in source order and replace them with the host-created Promise
        /// (`specifier, options -> promise`). The compiler keeps this opcode behind
        /// its dynamic-import frontier until the runtime host implements the full
        /// Promise/job/module-cache protocol.
        Import = (2, 1), CALL;
        Call(argc: u16) = (*argc as usize + 1, 1), CALL;
        /// QuickJS `OP_eval`: a syntactic direct-eval call site. The runtime first
        /// compares the resolved callee with the executing realm's original
        /// `%eval%`; a replacement callee falls back to an ordinary call with an
        /// undefined receiver. `environment` selects the immutable linked lexical
        /// environment published beside this function's bytecode; String-source
        /// execution retains it for authenticated nested direct-eval relays.
        Eval { argument_count: u16, environment: u16 } = (*argument_count as usize + 1, 1), CALL;
        CallMethod(argc: u16) = (*argc as usize + 2, 1), CALL;
        /// QuickJS `OP_call_constructor`: `func new.target args -> result`.
        Construct(argc: u16) = (*argc as usize + 2, 1), CALL;
        /// Authenticate the two stack operands prepared for a derived-constructor
        /// `super()` call. Runtime execution is a no-op; the bytecode verifier
        /// protects the marked `super_constructor, new.target` pair until exactly
        /// one matching typed construction consumes it.
        MarkSuperCall = (2, 2), PURE, state MarkSuperCall;
        /// Derived-constructor counterpart of [`Instruction::Construct`]. The
        /// verifier accepts it only when it exactly consumes the most recent
        /// protected super-call pair together with `argument_count` values.
        ConstructSuper(argc: u16) = (*argc as usize + 2, 1), CALL, state ConsumeSuperCall;
        /// QuickJS `OP_check_ctor`: reject an ordinary call to a class
        /// constructor by checking the current frame's `new.target`.
        CheckCtor = (0, 0), CALL;
        /// QuickJS `OP_init_ctor`: run the implicit derived constructor by reading
        /// the active function object's live `[[Prototype]]`, forwarding the frame's
        /// raw actual arguments, and preserving the original `new.target`.
        InitDerivedConstructor = (0, 1), CALL;
        /// QuickJS `OP_apply`: consume a compiler-created dense argument Array
        /// after all spread iterables have been evaluated.
        Apply(kind: ApplyKind) = (3, 1), CALL;
        /// Spread-argument counterpart of [`Instruction::ConstructSuper`].
        ApplySuper = (3, 1), CALL, state ConsumeSuperCall;
        /// QuickJS `OP_apply_eval`: the spread-argument counterpart of `Eval`.
        /// The original-eval identity check happens only after the dense argument
        /// Array has passed through QuickJS-compatible `build_arg_list` semantics.
        ApplyEval { environment: u16 } = (2, 1), CALL;
        Return = (1, 0), THROW, ctrl Return;
        /// Complete a derived constructor. Object results are returned directly;
        /// `undefined` resolves the authenticated derived-`this` local; every other
        /// primitive throws a TypeError. Return-protocol errors belong to the
        /// caller realm rather than the constructor's defining realm.
        ReturnDerived(local: u16) = (1, 0), CALL, ctrl Return;
        Throw = (1, 0), CALL, ctrl Throw;
    }
}
