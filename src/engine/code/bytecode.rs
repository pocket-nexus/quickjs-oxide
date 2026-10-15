use crate::engine::api::Error;
use std::collections::VecDeque;

/// QuickJS `JS_MAX_LOCAL_VARS` for one bytecode function.
pub const MAX_LOCAL_SLOTS: u16 = 65_534;

/// Ordinary-function arguments object selected by QuickJS's
/// `OP_special_object` lowering.
///
/// Simple sloppy parameter lists share their argument cells with indexed
/// properties; strict (and, once supported, non-simple) functions receive an
/// independent snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArgumentsKind {
    Mapped,
    Unmapped,
}

/// Storage which owns one hidden sloppy-eval variable object.
///
/// The object itself lives in a compiler-authenticated local or closure
/// `VarRef`. Keeping that source typed prevents the dynamic-name opcodes from
/// accepting an arbitrary JavaScript-visible object from the operand stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EvalVariableSource {
    Local(u16),
    Closure(u16),
}

/// Authenticated lexical storage for one class private name.
///
/// The private identity is deliberately never placed on the public operand
/// stack.  Dedicated class-element opcodes resolve it from a compiler-authored
/// local or closure cell after publication has verified the binding kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivateNameSource {
    Local(u16),
    Closure(u16),
}

/// Storage which owns one hidden sloppy-`with` object.
///
/// Like [`EvalVariableSource`], this is compiler-authenticated frame state and
/// cannot be supplied by JavaScript through the operand stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WithObjectSource {
    Local(u16),
    Closure(u16),
}

/// One ordered object-environment record consulted by dynamic name lookup.
///
/// Keeping eval-variable and `with` sources distinct lets the host apply the
/// latter's `Symbol.unscopables` rules without weakening either source type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DynamicEnvironmentSource {
    Eval(EvalVariableSource),
    With(WithObjectSource),
}

/// Object-literal function role carried by QuickJS's `OP_define_method`
/// family.
///
/// The role is deliberately part of the verified instruction rather than a
/// JavaScript stack value: it selects both the function-name prefix and the
/// property descriptor shape used at publication time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DefineMethodKind {
    Method,
    Getter,
    Setter,
}

/// Typed flags for QuickJS `OP_iterator_call` in a `yield*` delegation loop.
/// The method lookup always targets the retained iterator object. Missing or
/// nullish methods preserve the input value and return a true missing flag;
/// present methods replace that value with their result and return false.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[expect(clippy::enum_variant_names)]
pub enum IteratorCallKind {
    ReturnWithValue,
    ThrowWithValue,
    ReturnWithoutValue,
}

/// Typed form of QuickJS's `OP_apply` magic operand.
///
/// Both forms consume `function, this-or-new-target, argument-array`; the
/// distinction is authenticated bytecode metadata rather than a
/// JavaScript-visible stack value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApplyKind {
    Call,
    Construct,
}

/// Stack-machine operations deliberately use the names and stack behavior of
/// their `QuickJS` counterparts. This typed form is compiler IR. Publication
/// consumes it into `ExecCode`; the VM never dispatches this enum.
#[derive(Clone, Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "The verifier and VM accept instruction forms that current compiler paths do not emit."
    )
)]
pub enum Instruction {
    Nop,
    PushI32(i32),
    /// QuickJS `OP_push_atom_value` for a tagged non-negative integer atom.
    /// Each execution materializes a fresh decimal String; it must not reuse
    /// an atom-backed or ordinary constant-pool String representation.
    PushAtomValueIndex(u32),
    PushConst(u32),
    FClosure(u32),
    /// QuickJS `OP_regexp`, represented as one typed compile-time constant
    /// instead of two generic stack values. Each execution allocates a fresh
    /// branded RegExp in the bytecode realm without consulting user code.
    RegExp(u32),
    /// QuickJS `OP_set_name`: conditionally define the name of the object at
    /// the top of the stack from a string constant, without consuming it.
    SetName(u32),
    /// QuickJS `OP_throw_error atom JS_THROW_VAR_RO` for a strict immutable
    /// binding. It consumes no stack value and terminates, abandoning the
    /// complete frame stack on the exception path.
    ThrowReadOnly(u32),
    /// QuickJS `OP_throw_error atom JS_THROW_VAR_REDECL`: throw the eval-time
    /// SyntaxError used for a `var`/function declaration which crosses an
    /// imported lexical scope. The verified String constant preserves the
    /// conflicting binding identity even though QuickJS's message is generic.
    ThrowRedeclaration(u32),
    /// QuickJS `JS_THROW_ERROR_DELETE_SUPER`. The three authenticated
    /// SuperProperty Reference operands are consumed conceptually and the
    /// terminal throw occupies the expression's one-value stack shape.
    ThrowDeleteSuper,
    Undefined,
    Null,
    PushFalse,
    PushTrue,
    PushThis,
    /// Push the exact function object of the active bytecode frame. Derived
    /// constructors use its live `[[Prototype]]` as the target of `super()`, so
    /// this must not be reconstructed from HomeObject or the original heritage
    /// expression.
    PushActiveFunction,
    /// QuickJS's authenticated `<home_object>` pseudo binding. Only bytecode
    /// whose immutable metadata declares `needs_home_object` may execute this
    /// operation; the runtime reads it from the active function object rather
    /// than from a JavaScript-visible operand.
    PushHomeObject,
    PushNewTarget,
    /// QuickJS `OP_special_object` for an ordinary function's lazily selected
    /// arguments binding. Runtime creation still occurs in the entry prologue,
    /// before body function hoists.
    Arguments(ArgumentsKind),
    /// QuickJS `OP_rest`: collect the actual arguments beginning at this
    /// formal-parameter index into a fresh Array in the callee realm. The
    /// operand may equal `argument_count` for a rest BindingPattern, which
    /// owns no physical argument slot.
    Rest(u16),
    /// QuickJS `OP_special_object` with `OP_SPECIAL_OBJECT_VAR_OBJECT`: create
    /// the null-prototype object which stores names introduced by sloppy
    /// direct eval in this activation.
    VariableEnvironment,
    /// Test whether the hidden variable object has an own property named by a
    /// verified String constant.
    HasEvalVariable {
        source: EvalVariableSource,
        name: u32,
    },
    /// Read one property from the hidden variable object.
    GetEvalVariable {
        source: EvalVariableSource,
        name: u32,
    },
    /// Consume and assign one property on the hidden variable object.
    PutEvalVariable {
        source: EvalVariableSource,
        name: u32,
    },
    /// Delete one configurable property from the hidden variable object and
    /// push the Boolean result.
    DeleteEvalVariable {
        source: EvalVariableSource,
        name: u32,
    },
    /// Consume a value and define an own configurable, writable, enumerable
    /// data property on the hidden variable object.
    DefineEvalVariable {
        source: EvalVariableSource,
        name: u32,
    },
    /// ECMAScript `ToObject`: preserve Objects, reject nullish values, and box
    /// all other primitives in the executing realm.
    ToObject,
    /// Test whether an ordered hidden environment exposes one dynamic name.
    HasDynamicBinding {
        source: DynamicEnvironmentSource,
        name: u32,
    },
    /// Read one dynamic binding using the current frame's strictness.
    GetDynamicBinding {
        source: DynamicEnvironmentSource,
        name: u32,
    },
    /// Consume and write one dynamic binding using the current frame's
    /// strictness.
    PutDynamicBinding {
        source: DynamicEnvironmentSource,
        name: u32,
    },
    /// Delete one dynamic binding and push the Boolean result.
    DeleteDynamicBinding {
        source: DynamicEnvironmentSource,
        name: u32,
    },
    /// Push the authenticated object behind one dynamic environment source.
    DynamicEnvironmentObject(DynamicEnvironmentSource),
    /// QuickJS `OP_make_var_ref`: resolve one authenticated global closure
    /// name to the current realm's lexical storage object, global object, or
    /// the unresolved-reference `undefined` sentinel. Lexical TDZ and
    /// read-only checks happen while this reference is made, before a later
    /// right-hand side can run.
    GlobalReference(u16),
    /// Preserve an environment Object and append its named reference value.
    /// A missing property observes the current frame's strictness.
    GetRefValue(u32),
    /// Preserve an environment Object and append its named reference value,
    /// producing `undefined` when the property is missing.
    GetRefValueUndef(u32),
    /// Consume an environment Object and value and write the named reference
    /// using the current frame's strictness.
    PutRefValue(u32),
    GetLocal(u16),
    PutLocal(u16),
    SetLocal(u16),
    /// QuickJS `OP_set_loc_uninitialized`: enter one lexical scope with its
    /// local slot in the temporal dead zone.
    SetLocalUninitialized(u16),
    /// QuickJS `OP_get_loc_check`: read a lexical local after its TDZ check.
    GetLocalCheck(u16),
    /// Typed-format distinction for QuickJS's lexical-initializer `put_loc`.
    /// The published vardef must be lexical, but execution deliberately keeps
    /// ordinary `put_loc` overwrite semantics rather than conflating this with
    /// derived-`this`'s upstream `put_loc_check_init` opcode.
    InitializeLocal(u16),
    /// QuickJS `OP_put_loc_check_init`: consume the result of `super()` and
    /// initialize the authenticated derived-constructor `this` local exactly
    /// once. A second execution throws after the super constructor has run.
    InitializeDerivedLocal(u16),
    /// QuickJS `OP_put_loc_check`: consume and assign a mutable lexical local.
    PutLocalCheck(u16),
    /// QuickJS `OP_set_loc_check`: assign a mutable lexical local while
    /// preserving the assigned value on the operand stack.
    SetLocalCheck(u16),
    GetArg(u16),
    PutArg(u16),
    SetArg(u16),
    GetVarRef(u16),
    PutVarRef(u16),
    SetVarRef(u16),
    /// QuickJS `OP_get_var_ref_check`: read a captured lexical binding after
    /// its TDZ check.
    GetVarRefCheck(u16),
    /// QuickJS `OP_put_var_ref_check`: consume and assign a captured mutable
    /// lexical binding. Value-preserving writes use `Dup; PutVarRefCheck`.
    PutVarRefCheck(u16),
    /// QuickJS's declaration-time raw VarRef write for an ordinary lexical
    /// module binding. Publication proves this occurs once in the authored
    /// evaluation path; the cell may already have been changed indirectly by
    /// QuickJS's import/declaration collision quirk.
    InitializeVarRef(u16),
    /// Pinned QuickJS declaration initialization when an import and a module
    /// declaration share the import's first closure slot. Publication accepts
    /// this raw write only for a matching module collision ledger entry;
    /// ordinary writes to the same binding remain read-only.
    InitializeModuleImportCollision(u16),
    /// Captured-binding counterpart of [`Instruction::InitializeDerivedLocal`].
    /// Arrow functions and direct eval use this to initialize their enclosing
    /// derived constructor's single `this` cell.
    InitializeDerivedVarRef(u16),
    /// QuickJS `OP_close_loc`: detach a captured local when its lexical scope
    /// exits so a later scope entry receives a fresh cell.
    CloseLocal(u16),
    /// QuickJS `OP_get_var`: read a global-environment VarRef closure slot.
    GetVar(u16),
    /// QuickJS `OP_get_var_undef`: as `GetVar`, but suppress a genuinely
    /// missing global binding for a direct `typeof IdentifierReference`.
    GetVarUndef(u16),
    /// QuickJS `OP_delete_var`: delete one sloppy direct global binding and
    /// push the Boolean result. Lexical and resolved non-global bindings are
    /// folded to `PushFalse` during scope resolution.
    DeleteVar(u16),
    /// QuickJS `OP_put_var`: assign and consume a global binding value.
    PutVar(u16),
    /// QuickJS `OP_put_var_init`: initialize a lexical global binding.
    PutVarInit(u16),
    /// Allocate one fresh QuickJS private symbol and initialize the named
    /// class-private lexical local without exposing that symbol as a Value.
    InitializePrivateName(u16),
    /// QuickJS private-method declaration bootstrap. Consume the class-side
    /// HomeObject and freshly instantiated method closure, install the hidden
    /// HomeObject edge, initialize the authenticated private-method lexical
    /// cell, and retain only the HomeObject on the operand stack.
    InitializePrivateMethod(u16),
    /// QuickJS private-accessor declaration bootstrap. Consume the class-side
    /// HomeObject and freshly instantiated getter or setter closure, install
    /// the hidden HomeObject edge, initialize the authenticated accessor cell,
    /// and retain only the HomeObject. Unlike a private method, an accessor's
    /// public function name remains the empty string.
    InitializePrivateAccessor(u16),
    /// QuickJS `OP_get_private_field`: own-only private field read.
    GetPrivateField(PrivateNameSource),
    /// Call/compound counterpart which retains the original receiver.
    GetPrivateField2(PrivateNameSource),
    /// QuickJS `OP_put_private_field`: own-only private field write.
    PutPrivateField(PrivateNameSource),
    /// QuickJS `OP_define_private_field`: define a fresh private own field
    /// even when the receiver is non-extensible.
    DefinePrivateField(PrivateNameSource),
    /// QuickJS `OP_private_in` for a private data-field binding.
    PrivateIn(PrivateNameSource),
    /// QuickJS `OP_get_field`: replace an object-like base with the value of
    /// one constant string-keyed property.
    GetField(u32),
    /// QuickJS `OP_get_field2`: keep the base below the fetched value so a
    /// following `CallMethod` observes the original reference receiver.
    GetField2(u32),
    /// QuickJS `OP_get_array_el`: `base key -> value`, including observable
    /// `ToPropertyKey` conversion in the runtime host.
    GetArrayEl,
    /// QuickJS `OP_get_array_el2`: `base key -> base value` for method calls.
    GetArrayEl2,
    /// QuickJS `OP_get_array_el3`: `base raw-key -> base converted-key value`
    /// for compound/logical assignment without repeated key conversion.
    GetArrayEl3,
    /// QuickJS `OP_get_super`: replace a method HomeObject with its current
    /// prototype. A null prototype is represented by JavaScript `null` and is
    /// diagnosed only by the following property operation.
    GetSuper,
    /// QuickJS `OP_get_super_value`: `receiver base key -> value`. The base is
    /// the prototype captured before a computed key is evaluated, while
    /// accessors observe the method's actual `this` as their receiver.
    GetSuperValue,
    /// QuickJS's call-site rewrite of `OP_get_super_value` to
    /// `OP_get_array_el`: `receiver base key -> receiver value`. This
    /// deliberately gives a getter `base` as its receiver before the returned
    /// callable is invoked with the preserved method receiver.
    GetSuperValueForCall,
    /// QuickJS `OP_array_from`: consume `element_count` dense literal values
    /// and replace them with a fresh Array in the bytecode's realm.
    ArrayFrom(u16),
    /// QuickJS `OP_object`: push a fresh ordinary Object rooted in the
    /// executing bytecode's realm.
    Object,
    /// QuickJS `OP_to_propkey`: observable `ToPropertyKey` while retaining a
    /// canonical Int/String/Symbol value on the VM stack.
    ToPropKey,
    /// QuickJS `OP_insert2`: `base value -> value base value`.
    Insert2,
    /// QuickJS `OP_insert3`: `base key value -> value base key value`.
    Insert3,
    /// QuickJS `OP_dup3`: `a b c -> a b c a b c`.
    Dup3,
    /// QuickJS `OP_insert4`: `this base key value -> value this base key value`.
    Insert4,
    /// QuickJS `OP_perm3`: `base old new -> old base new`.
    Perm3,
    /// QuickJS `OP_perm4`: `base key old new -> old base key new`.
    Perm4,
    /// QuickJS `OP_perm5`: `this base key old new -> old this base key new`.
    Perm5,
    /// QuickJS `OP_rot4l`: `value this base key -> this base key value`.
    Rot4Left,
    /// QuickJS `OP_put_field`: assign one constant string-keyed property.
    PutField(u32),
    /// QuickJS `OP_put_array_el`: assign a computed property, converting the
    /// still-raw key only after the right-hand side has been evaluated.
    PutArrayEl,
    /// QuickJS `OP_put_super_value`: assign through `base` with `receiver` as
    /// the actual receiver. `JS_PROP_THROW_STRICT` rejects failed writes only
    /// when the containing method is strict.
    PutSuperValue,
    /// QuickJS `OP_define_field`: define one C_W_E data property while
    /// preserving the object below it (`object value -> object`). Array
    /// literals use this after their initial dense prefix.
    DefineField(u32),
    /// Computed-key class-field form of QuickJS `OP_define_array_el`. The key
    /// is the authenticated output of `ToPropKey`, so execution must not run
    /// observable property-key conversion again. Defines one C_W_E own data
    /// property while preserving only the object
    /// (`object canonical-key value -> object`).
    DefineFieldComputed,
    /// QuickJS `OP_define_method`: name and publish an object-literal method
    /// under one verified String constant while preserving the fresh literal
    /// (`object closure -> object`).
    DefineMethod {
        key: u32,
        kind: DefineMethodKind,
        enumerable: bool,
    },
    /// QuickJS `OP_define_method_computed`: publish an object-literal method
    /// under an already-canonical property key while preserving the fresh
    /// literal (`object key closure -> object`).
    DefineMethodComputed {
        kind: DefineMethodKind,
        enumerable: bool,
    },
    /// QuickJS `OP_define_class`: replace `parent, constructor` with the
    /// published class `constructor, prototype` pair. The heritage flag is
    /// authenticated bytecode metadata rather than a JavaScript stack value.
    DefineClass {
        name: u32,
        has_heritage: bool,
    },
    /// Attach one authenticated hidden instance-fields program to a freshly
    /// defined class constructor while installing the class prototype as its
    /// HomeObject (`constructor prototype initializer -> constructor prototype`).
    InstallClassInstanceInitializer,
    /// Run the hidden instance-fields program attached to the active class
    /// constructor with the retained receiver as `this`
    /// (`receiver active-constructor -> receiver`).
    /// Absence of an initializer is the ordinary no-fields fast path.
    CallClassInstanceInitializer,
    /// Install the constructor as HomeObject and immediately execute the
    /// aggregate static-elements program (`constructor initializer -> constructor`).
    RunClassStaticInitializer,
    /// Execute a non-escaping static-block child with the aggregate static
    /// initializer's receiver and HomeObject (`block ->`).
    CallClassStaticBlock,
    /// QuickJS `OP_define_array_el`: define a computed C_W_E data property
    /// while preserving the Array and its dynamic index
    /// (`array index value -> array index`).
    DefineArrayEl,
    /// QuickJS `OP_set_name_computed`: infer an anonymous function's name
    /// from an already-canonical computed property key while preserving both
    /// operands (`key value -> key value`).
    SetNameComputed,
    /// QuickJS `OP_set_proto`: consume one object-literal prototype candidate
    /// and preserve the fresh literal (`object value -> object`). Object and
    /// null candidates replace `[[Prototype]]`; primitive candidates are ignored.
    SetProto,
    /// Object-literal specialization of QuickJS `OP_copy_data_properties`.
    /// It snapshots enumerable own String/Symbol keys from an Object source,
    /// defines C_W_E data properties, and preserves the fresh literal
    /// (`target source -> target`).
    CopyDataProperties,
    /// Exclusion-aware QuickJS `OP_copy_data_properties` used by an Object
    /// binding rest element. The three operands are addressed by depth from
    /// the top of the stack (depth zero is the top) and are only borrowed:
    /// successful execution leaves the complete operand stack unchanged.
    ///
    /// This is intentionally depth-addressed instead of consuming three
    /// adjacent values. A sloppy `var` binding may need to keep its prepared
    /// Reference between the retained source and the fresh target while the
    /// copy executes observable getters.
    CopyDataPropertiesExcluded {
        target_depth: u8,
        source_depth: u8,
        excluded_depth: u8,
    },
    /// QuickJS `OP_append`: append every value from one iterable and replace
    /// the dynamic index with the first unused index
    /// (`array index iterable -> array index`).
    Append,
    /// QuickJS `OP_delete`: `base key -> bool` with strictness supplied by the
    /// active call frame.
    Delete,
    Drop,
    /// QuickJS `OP_nip`: discard the value immediately below the stack top,
    /// preserving the top value (`a b -> b`).
    Nip,
    /// QuickJS `OP_swap`: exchange the top two operands (`a b -> b a`).
    Swap,
    Dup,
    /// QuickJS `OP_dup1`: duplicate the value immediately below the stack top
    /// in place (`a b -> a a b`).
    Dup1,
    Neg,
    Plus,
    Inc,
    Dec,
    /// QuickJS `OP_post_inc`: convert the operand to numeric, then leave the
    /// old numeric value below its incremented replacement.
    PostInc,
    /// QuickJS `OP_post_dec`: convert the operand to numeric, then leave the
    /// old numeric value below its decremented replacement.
    PostDec,
    BitNot,
    Not,
    TypeOf,
    IsUndefinedOrNull,
    /// Exact `undefined` tag test. Unlike `typeof`, HTMLDDA objects are false.
    IsUndefined,
    /// Exact `null` tag test.
    IsNull,
    /// Fused `typeof value === "undefined"`, including HTMLDDA objects.
    TypeOfIsUndefined,
    /// Fused `typeof value === "function"`, excluding HTMLDDA objects.
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
    IfFalse(u32),
    IfTrue(u32),
    Goto(u32),
    /// QuickJS `OP_catch`, represented as private VM handler metadata rather
    /// than a forgeable operand-stack value.
    Catch(u32),
    /// Remove the innermost active catch marker on a normal path.
    DropCatch,
    /// Preserve the top value while discarding the innermost catch marker and
    /// every intermediate operand above its entry depth.
    NipCatch,
    /// QuickJS `OP_gosub`: push the fallthrough PC as a real Int value and
    /// enter a shared finally subroutine.
    Gosub(u32),
    /// QuickJS `OP_ret`: pop and validate the Int return PC from `Gosub`.
    Ret,
    /// Typed finally cleanup for an abrupt path which discards, rather than
    /// follows, the real Int return PC pushed by `Gosub`.
    DropGosub,
    /// QuickJS `OP_for_of_start`: replace one iterable with the two public
    /// iterator-record values (`iterator`, `next`) plus a private unwind
    /// marker represented only in the verifier and VM region stack.
    ForOfStart,
    /// QuickJS `OP_for_await_of_start`: acquire an async iterator (including
    /// Async-from-Sync fallback) and install the same three-slot conceptual
    /// unwind record as [`Instruction::ForOfStart`]. Unlike
    /// [`Instruction::AsyncIteratorStart`], this record participates in
    /// automatic loop cleanup.
    ForAwaitOfStart,
    /// QuickJS `OP_for_of_next`: retain the three-slot iterator record below
    /// `offset` intermediate operands and append `value`, `done`.
    ForOfNext(u8),
    /// QuickJS `OP_for_await_of_next`: temporarily disable the async iterator
    /// unwind marker, call its cached `next` method without arguments, and
    /// append the raw result which the following [`Instruction::Await`] must
    /// settle (`iterator, next, marker -> ... result`).
    ForAwaitOfNext,
    /// QuickJS `OP_iterator_get_value_done`: consume the settled async iterator
    /// result together with the conceptual private marker, read `done` and then
    /// `value`, restore the stable unwind marker, and append `value`, `done`.
    /// The private marker is represented only by verifier/VM region state.
    IteratorGetValueDone,
    /// QuickJS `OP_for_in_start`: replace one source value with a hidden
    /// enumeration object.
    ForInStart,
    /// QuickJS `OP_for_in_next`: retain the enumeration object and append the
    /// next string key plus its `done` flag.
    ForInNext,
    /// QuickJS `OP_iterator_close`: remove the innermost iterator record and
    /// close it for a normal abrupt completion.
    IteratorClose,
    /// Preserve the top operand while removing the innermost iterator record
    /// and every intermediate operand, then close the iterator. This is the
    /// typed equivalent of QuickJS's `nip_catch; rot3r; undefined;
    /// iterator_close` return-unwind sequence.
    IteratorClosePreserve,
    /// Preserve the top operand while removing the innermost iterator record
    /// and every intermediate operand, without invoking the iterator's
    /// `return` method. QuickJS uses this shape when an injected generator
    /// return leaves the precompiled assignment fragment of a for-of head:
    /// the loop iterator record is abandoned rather than normally closed.
    IteratorDropPreserve,
    /// Preserve the top completion value while dynamically removing the
    /// innermost stable iterator record and all intermediate operands, but keep
    /// the iterator itself above that completion value
    /// (`... iterator, next, marker, ..., completion -> ... completion, iterator`).
    /// Async-generator return lowering uses the exposed iterator to perform its
    /// explicit call/check/await close sequence.
    IteratorDetachPreserve,
    /// `yield*`-specific GetIterator. Unlike [`Instruction::ForOfStart`], its
    /// three outputs are ordinary retained operands (`iterator`, cached
    /// `next`, placeholder) and do not install an automatic unwind region.
    IteratorStart,
    /// Async-generator `yield*` GetAsyncIterator. A present
    /// `Symbol.asyncIterator` is used directly; a nullish method falls back to
    /// a branded Async-from-Sync iterator. The retained three-slot record has
    /// the same compiler-private shape as [`Instruction::IteratorStart`].
    AsyncIteratorStart,
    /// QuickJS `OP_iterator_next`: call the cached `next` method with the top
    /// resume value and replace that value with the materialized result object
    /// (`iterator, next, placeholder, value -> ... result`).
    IteratorNext,
    /// QuickJS `OP_iterator_call`: look up `return` or `throw` on the retained
    /// iterator and append a Boolean indicating that the method was missing.
    IteratorCall(IteratorCallKind),
    /// Reject a non-Object iterator-result while preserving it at TOS.
    IteratorCheckObject,
    /// QuickJS `OP_initial_yield`: suspend a freshly invoked generator after
    /// parameter initialization and before entering its authored body. It has
    /// no JavaScript-visible operand; resumption continues at the next PC.
    InitialYield,
    /// QuickJS `OP_yield`: consume the value exposed to the caller. Resumption
    /// replaces it with the injected resume value and appends the authenticated
    /// next/return discriminator (`value -> resume-value, resume-kind`). A
    /// throw resumption enters through the VM exception path instead.
    Yield,
    /// QuickJS `OP_yield_star`: the suspension half of delegated yield. The
    /// operand is an already-materialized iterator-result object, which the
    /// generator driver must return without re-wrapping (`pdone == 2`). The
    /// compiler-owned delegation loop consumes the same two resume operands as
    /// ordinary `Yield`.
    YieldStar,
    /// QuickJS `OP_async_yield_star`: suspend an async generator with the
    /// delegate result's already-extracted `value`. The async-generator driver
    /// wraps that value in the current request's `{ value, done: false }`
    /// result, while resumption uses the same next/return/throw magic ABI as
    /// synchronous delegation.
    AsyncYieldStar,
    /// QuickJS `OP_await`: consume the value which the async-function driver
    /// resolves through the realm's intrinsic Promise constructor. Fulfilment
    /// resumes by replacing the retained slot with the resolution value;
    /// rejection is raised through the activation's ordinary unwind path.
    Await,
    /// Terminal TypeError used by `yield*` when the delegate has no `throw`
    /// method. A present `return` method has already been called for close.
    ThrowIteratorMissingThrow,
    /// QuickJS `OP_import`: consume the evaluated module specifier and import
    /// options in source order and replace them with the host-created Promise
    /// (`specifier, options -> promise`). The compiler keeps this opcode behind
    /// its dynamic-import frontier until the runtime host implements the full
    /// Promise/job/module-cache protocol.
    Import,
    Call(u16),
    /// QuickJS `OP_tail_call`: call with an undefined receiver and use the
    /// resulting completion as the completion of the current frame.
    TailCall(u16),
    /// QuickJS `OP_eval`: a syntactic direct-eval call site. The runtime first
    /// compares the resolved callee with the executing realm's original
    /// `%eval%`; a replacement callee falls back to an ordinary call with an
    /// undefined receiver. `environment` selects the immutable linked lexical
    /// environment published beside this function's bytecode; String-source
    /// execution retains it for authenticated nested direct-eval relays.
    Eval {
        argument_count: u16,
        environment: u16,
    },
    CallMethod(u16),
    /// Receiver-preserving counterpart of [`Instruction::TailCall`].
    TailCallMethod(u16),
    /// QuickJS `OP_call_constructor`: `func new.target args -> result`.
    Construct(u16),
    /// Authenticate the two stack operands prepared for a derived-constructor
    /// `super()` call. Runtime execution is a no-op; the bytecode verifier
    /// protects the marked `super_constructor, new.target` pair until exactly
    /// one matching typed construction consumes it.
    MarkSuperCall,
    /// Derived-constructor counterpart of [`Instruction::Construct`]. The
    /// verifier accepts it only when it exactly consumes the most recent
    /// protected super-call pair together with `argument_count` values.
    ConstructSuper(u16),
    /// QuickJS `OP_check_ctor`: reject an ordinary call to a class
    /// constructor by checking the current frame's `new.target`.
    CheckCtor,
    /// QuickJS `OP_init_ctor`: run the implicit derived constructor by reading
    /// the active function object's live `[[Prototype]]`, forwarding the frame's
    /// raw actual arguments, and preserving the original `new.target`.
    InitDerivedConstructor,
    /// QuickJS `OP_apply`: consume a compiler-created dense argument Array
    /// after all spread iterables have been evaluated.
    Apply(ApplyKind),
    /// Spread-argument counterpart of [`Instruction::ConstructSuper`].
    ApplySuper,
    /// QuickJS `OP_apply_eval`: the spread-argument counterpart of `Eval`.
    /// The original-eval identity check happens only after the dense argument
    /// Array has passed through QuickJS-compatible `build_arg_list` semantics.
    ApplyEval {
        environment: u16,
    },
    Return,
    /// Return `undefined` without consuming an operand-stack value.
    ReturnUndefined,
    /// Complete a derived constructor. Object results are returned directly;
    /// `undefined` resolves the authenticated derived-`this` local; every other
    /// primitive throws a TypeError. Return-protocol errors belong to the
    /// caller realm rather than the constructor's defining realm.
    ReturnDerived(u16),
    Throw,
}

impl Instruction {
    /// String constant used as a static name by the VM's property-key bridge.
    /// Publication links these operands once, including unreachable code.
    #[must_use]
    pub(crate) const fn constant_property_key_index(&self) -> Option<u32> {
        self.operand_contract().static_name()
    }

    #[must_use]
    pub const fn stack_effect(&self) -> (usize, usize) {
        self.nominal_stack_effect()
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub struct DetachedBytecode<T> {
    pub code: Vec<Instruction>,
    pub constants: Vec<T>,
    /// Declared local frame width. Local operands are verified against this
    /// boundary even when their instructions are unreachable.
    pub local_count: u16,
    pub max_stack: u16,
}

#[cfg(test)]
impl<T> DetachedBytecode<T> {
    /// Recompute the reachable stack depth against the declared maximum.
    ///
    /// # Errors
    /// Returns an internal error when the declared maximum is too small.
    pub fn verify(&self) -> Result<VerifiedBytecode, Error> {
        compute_stack_size(&self.code, self.max_stack)
    }
}

/// QuickJS `compute_stack_size`: the maximum operand-stack depth over every
/// reachable path, including exception and `gosub` edges. Fails once a
/// reachable depth exceeds `declared_max_stack`, which lowering reports as
/// QuickJS's "stack overflow".
///
/// Unwind regions are tracked only because `NipCatch` and the iterator
/// `*Preserve` cleanups truncate the stack to their region's base. The input
/// is compiler output; malformed bytecode is not diagnosed beyond the errors
/// needed to keep the walk bounds-safe.
pub fn compute_stack_size(
    code: &[Instruction],
    declared_max_stack: u16,
) -> Result<VerifiedBytecode, Error> {
    // Each region records the depth its abrupt cleanup restores: the catch
    // marker depth for `Catch`, or the iterator record base for for-of.
    let mut worklist = VecDeque::from([(0_usize, 0_usize, Vec::<usize>::new())]);
    // usize::MAX marks unvisited PCs; depths are bounded by MAX_STACK_SIZE.
    let mut visited = vec![usize::MAX; code.len()];
    let mut maximum = 0_usize;

    while let Some((pc, depth, mut regions)) = worklist.pop_front() {
        record_maximum_depth(&mut maximum, depth, declared_max_stack)?;
        let seen = visited
            .get_mut(pc)
            .ok_or_else(|| Error::internal("control flow target is out of bounds"))?;
        if *seen != usize::MAX {
            continue;
        }
        *seen = depth;

        let instruction = &code[pc];
        let effect = instruction.stack_contract();
        let remaining_depth = depth
            .checked_sub(effect.popped)
            .ok_or_else(|| Error::internal("bytecode stack underflow"))?;
        let mut next_depth = remaining_depth + effect.pushed;
        let missing_region = || Error::internal("bytecode unwind region underflow");
        match instruction {
            Instruction::DropCatch | Instruction::IteratorClose => {
                regions.pop().ok_or_else(missing_region)?;
            }
            Instruction::NipCatch => next_depth = regions.pop().ok_or_else(missing_region)?,
            Instruction::ForOfStart | Instruction::ForAwaitOfStart => {
                regions.push(remaining_depth);
            }
            Instruction::IteratorClosePreserve | Instruction::IteratorDropPreserve => {
                next_depth = regions.pop().ok_or_else(missing_region)? + 1;
            }
            Instruction::IteratorDetachPreserve => {
                next_depth = regions.pop().ok_or_else(missing_region)? + 2;
            }
            _ => {}
        }
        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;
        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses
        // JS_STACK_SIZE_MAX, before any later instruction is considered.
        match *instruction {
            // QuickJS terminal completion opcodes consume their completion
            // value and abandon the rest of the frame stack. In particular,
            // `return` and `throw` inside a switch leave its discriminant
            // below that value rather than emitting synthetic cleanup.
            // `OP_throw_error` is likewise terminal: a postfix update may
            // retain its old value below the replaced immutable write.
            Instruction::TailCall(_)
            | Instruction::TailCallMethod(_)
            | Instruction::Return
            | Instruction::ReturnUndefined
            | Instruction::ReturnDerived(_)
            | Instruction::Throw
            | Instruction::Ret
            | Instruction::ThrowReadOnly(_)
            | Instruction::ThrowRedeclaration(_)
            | Instruction::ThrowDeleteSuper
            | Instruction::ThrowIteratorMissingThrow => {}
            Instruction::Goto(target) => worklist.push_back((target as usize, next_depth, regions)),
            Instruction::IfFalse(target) | Instruction::IfTrue(target) => {
                worklist.push_back((target as usize, next_depth, regions.clone()));
                worklist.push_back((pc + 1, next_depth, regions));
            }
            Instruction::Catch(target) => {
                worklist.push_back((target as usize, next_depth, regions.clone()));
                regions.push(next_depth);
                worklist.push_back((pc + 1, next_depth, regions));
            }
            Instruction::Gosub(target) => {
                let subroutine_depth = depth + 1;
                record_maximum_depth(&mut maximum, subroutine_depth, declared_max_stack)?;
                worklist.push_back((target as usize, subroutine_depth, regions.clone()));
                worklist.push_back((pc + 1, next_depth, regions));
            }
            _ => worklist.push_back((pc + 1, next_depth, regions)),
        }
    }

    let maximum =
        u16::try_from(maximum).map_err(|_| Error::internal("bytecode stack exceeds u16::MAX"))?;
    Ok(VerifiedBytecode { max_stack: maximum })
}

fn record_maximum_depth(
    maximum: &mut usize,
    depth: usize,
    declared_max_stack: u16,
) -> Result<(), Error> {
    *maximum = (*maximum).max(depth);
    if *maximum > usize::from(declared_max_stack) {
        return Err(Error::internal(
            "declared maximum stack is smaller than required",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedBytecode {
    pub max_stack: u16,
}

#[cfg(test)]
impl<T> Default for DetachedBytecode<T> {
    fn default() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            local_count: 0,
            max_stack: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyKind, ArgumentsKind, DefineMethodKind, DetachedBytecode, DynamicEnvironmentSource,
        EvalVariableSource, Instruction, WithObjectSource,
    };
    use crate::engine::api::JsString;
    use crate::engine::value::PrimitiveValue as Value;

    #[test]
    fn verifier_computes_reachable_stack_depth() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushTrue,
                Instruction::IfFalse(4),
                Instruction::PushI32(1),
                Instruction::Goto(5),
                Instruction::PushI32(2),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(function.verify().unwrap().max_stack, 1);
    }

    #[test]
    fn verifier_models_generator_suspension_resume_stack_shapes() {
        for suspension in [
            Instruction::Yield,
            Instruction::YieldStar,
            Instruction::AsyncYieldStar,
        ] {
            let function = DetachedBytecode::<Value> {
                code: vec![
                    Instruction::InitialYield,
                    Instruction::PushI32(42),
                    suspension,
                    Instruction::IfFalse(5),
                    Instruction::Return,
                    Instruction::Return,
                ],
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(function.verify().unwrap().max_stack, 2);
        }

        let underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::Yield, Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );
    }

    #[test]
    fn verifier_models_await_as_a_value_preserving_suspension() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(42),
                Instruction::Await,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(function.verify().unwrap().max_stack, 1);

        let underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::Await, Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );
    }

    #[test]
    fn verifier_models_dynamic_import_as_two_inputs_to_one_promise() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(20),
                Instruction::PushI32(22),
                Instruction::Import,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(function.verify().unwrap().max_stack, 2);

        let underflow = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(20),
                Instruction::Import,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );

        let undersized = DetachedBytecode::<Value> {
            max_stack: 1,
            ..function
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_typed_class_definition_operations() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::CheckCtor,
                Instruction::Undefined,
                Instruction::PushI32(7),
                Instruction::Swap,
                Instruction::Swap,
                Instruction::DefineClass {
                    name: 0,
                    has_heritage: false,
                },
                Instruction::Nip,
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::api::JsString::from_static(
                "C",
            ))],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(function.verify().unwrap().max_stack, 2);

        let underflow = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::DefineClass {
                    name: 0,
                    has_heritage: false,
                },
                Instruction::Return,
            ],
            max_stack: 2,
            ..function.clone()
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );

        let heritage = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::PushI32(7),
                Instruction::DefineClass {
                    name: 0,
                    has_heritage: true,
                },
                Instruction::Drop,
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::api::JsString::from_static(
                "Derived",
            ))],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(heritage.verify().unwrap().max_stack, 2);
    }

    #[test]
    fn verifier_models_arguments_creation_as_one_fresh_value() {
        for kind in [ArgumentsKind::Mapped, ArgumentsKind::Unmapped] {
            let function = DetachedBytecode::<Value> {
                code: vec![Instruction::Arguments(kind), Instruction::Return],
                constants: vec![],
                local_count: 0,
                max_stack: 1,
            };
            assert_eq!(function.verify().unwrap().max_stack, 1);
        }

        let undersized = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Arguments(ArgumentsKind::Mapped),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );

        let rest = DetachedBytecode::<Value> {
            code: vec![Instruction::Rest(3), Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(rest.verify().unwrap().max_stack, 1);

        let undersized_rest = DetachedBytecode::<Value> {
            max_stack: 0,
            ..rest
        };
        assert_eq!(
            undersized_rest.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_tail_invocations_as_terminal_zero_result_operations() {
        for (code, maximum) in [
            (
                vec![
                    Instruction::Undefined,
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    Instruction::TailCall(2),
                    // Terminal tail invocation makes this underflowing
                    // instruction unreachable.
                    Instruction::Drop,
                ],
                3,
            ),
            (
                vec![
                    Instruction::PushI32(9),
                    Instruction::Undefined,
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    Instruction::TailCallMethod(2),
                    Instruction::Drop,
                ],
                4,
            ),
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: maximum,
            };
            assert_eq!(function.verify().unwrap().max_stack, maximum);
        }

        for instruction in [Instruction::TailCall(0), Instruction::TailCallMethod(0)] {
            let function = DetachedBytecode::<Value> {
                code: vec![instruction],
                constants: vec![],
                local_count: 0,
                max_stack: 0,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }

        let undersized = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::TailCall(2),
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );

        for instruction in [
            Instruction::TailCall(u16::MAX),
            Instruction::TailCallMethod(u16::MAX),
        ] {
            let function = DetachedBytecode::<Value> {
                code: vec![instruction],
                constants: vec![],
                local_count: 0,
                max_stack: 0,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }
    }

    #[test]
    fn verifier_models_eval_with_the_quickjs_call_stack_shape() {
        for (argument_count, code, maximum) in [
            (
                0,
                vec![
                    Instruction::Undefined,
                    Instruction::Eval {
                        argument_count: 0,
                        environment: 0,
                    },
                    Instruction::Return,
                ],
                1,
            ),
            (
                2,
                vec![
                    Instruction::Undefined,
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    Instruction::Eval {
                        argument_count: 2,
                        environment: 0,
                    },
                    Instruction::Return,
                ],
                3,
            ),
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: maximum,
            };
            assert_eq!(
                function.verify().unwrap().max_stack,
                maximum,
                "argument count {argument_count}"
            );
        }

        for code in [
            vec![
                Instruction::Eval {
                    argument_count: 0,
                    environment: 0,
                },
                Instruction::Return,
            ],
            vec![
                Instruction::Undefined,
                Instruction::PushI32(1),
                Instruction::Eval {
                    argument_count: 2,
                    environment: 0,
                },
                Instruction::Return,
            ],
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }

        let undersized = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::Eval {
                    argument_count: 2,
                    environment: 0,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_apply_and_apply_eval_stack_shapes() {
        for (code, maximum) in [
            (
                vec![
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Apply(ApplyKind::Call),
                    Instruction::Return,
                ],
                3,
            ),
            (
                vec![
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Apply(ApplyKind::Construct),
                    Instruction::Return,
                ],
                3,
            ),
            (
                vec![
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::ApplyEval { environment: 7 },
                    Instruction::Return,
                ],
                2,
            ),
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: maximum,
            };
            assert_eq!(function.verify().unwrap().max_stack, maximum);
        }

        for instruction in [
            Instruction::Apply(ApplyKind::Call),
            Instruction::Apply(ApplyKind::Construct),
            Instruction::ApplyEval { environment: 0 },
        ] {
            let function = DetachedBytecode::<Value> {
                code: vec![Instruction::Undefined, instruction, Instruction::Return],
                constants: vec![],
                local_count: 0,
                max_stack: 1,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }
    }

    #[test]
    fn verifier_models_membership_operators_as_binary_booleans() {
        for operator in [Instruction::InstanceOf, Instruction::In] {
            let valid = DetachedBytecode::<Value> {
                code: vec![
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    operator.clone(),
                    Instruction::Return,
                ],
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(valid.verify().unwrap().max_stack, 2);

            let underflow = DetachedBytecode::<Value> {
                code: vec![Instruction::PushI32(1), operator, Instruction::Return],
                constants: vec![],
                local_count: 0,
                max_stack: 1,
            };
            assert!(underflow.verify().is_err());
        }
    }

    #[test]
    fn verifier_models_eval_variable_object_stack_and_operands() {
        let source = EvalVariableSource::Local(0);
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::VariableEnvironment,
                Instruction::PutLocal(0),
                Instruction::HasEvalVariable { source, name: 0 },
                Instruction::Drop,
                Instruction::GetEvalVariable { source, name: 0 },
                Instruction::Drop,
                Instruction::PushI32(1),
                Instruction::PutEvalVariable { source, name: 0 },
                Instruction::DeleteEvalVariable { source, name: 0 },
                Instruction::Drop,
                Instruction::PushI32(2),
                Instruction::DefineEvalVariable { source, name: 0 },
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![Value::String(JsString::from_static("added"))],
            local_count: 1,
            max_stack: 1,
        };
        assert_eq!(function.verify().unwrap().max_stack, 1);
    }

    #[test]
    fn verifier_models_typed_dynamic_environment_stack_and_operands() {
        let eval = DynamicEnvironmentSource::Eval(EvalVariableSource::Local(0));
        let with = DynamicEnvironmentSource::With(WithObjectSource::Local(1));
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::VariableEnvironment,
                Instruction::PutLocal(0),
                Instruction::PushI32(1),
                Instruction::ToObject,
                Instruction::PutLocal(1),
                Instruction::HasDynamicBinding {
                    source: eval,
                    name: 0,
                },
                Instruction::Drop,
                Instruction::GetDynamicBinding {
                    source: with,
                    name: 0,
                },
                Instruction::Drop,
                Instruction::PushI32(2),
                Instruction::PutDynamicBinding {
                    source: eval,
                    name: 0,
                },
                Instruction::DeleteDynamicBinding {
                    source: with,
                    name: 0,
                },
                Instruction::Drop,
                Instruction::DynamicEnvironmentObject(eval),
                Instruction::GetRefValue(0),
                Instruction::PutRefValue(0),
                Instruction::DynamicEnvironmentObject(with),
                Instruction::GetRefValueUndef(0),
                Instruction::PutRefValue(0),
                Instruction::GlobalReference(0),
                Instruction::Drop,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![Value::String(JsString::from_static("binding"))],
            local_count: 2,
            max_stack: 2,
        };
        assert_eq!(function.verify().unwrap().max_stack, 2);

        let undersized_global_reference = DetachedBytecode::<Value> {
            code: vec![Instruction::GlobalReference(0), Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(
            undersized_global_reference.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_eval_redeclaration_as_a_zero_stack_terminal() {
        let function = DetachedBytecode::<Value> {
            code: vec![Instruction::ThrowRedeclaration(0)],
            constants: vec![Value::String(JsString::from_static("conflict"))],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(function.verify().unwrap().max_stack, 0);
    }

    #[test]
    fn verifier_accepts_closed_non_terminating_control_flow() {
        let cycle = DetachedBytecode::<Value> {
            code: vec![Instruction::Goto(0)],
            constants: vec![],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(cycle.verify().unwrap().max_stack, 0);
    }

    #[test]
    fn verifier_allows_terminal_completion_to_abandon_switch_values() {
        for completion in [Instruction::Return, Instruction::Throw] {
            let function = DetachedBytecode::<Value> {
                code: vec![Instruction::PushI32(1), Instruction::PushI32(2), completion],
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(function.verify().unwrap().max_stack, 2);
        }
    }

    #[test]
    fn verifier_tracks_catch_markers_and_exceptional_edges() {
        let normal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(4),
                Instruction::DropCatch,
                Instruction::PushI32(3),
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(normal.verify().unwrap().max_stack, 1);

        let thrown = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(3),
                Instruction::PushI32(7),
                Instruction::Throw,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(thrown.verify().unwrap().max_stack, 2);

        let nip = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(10),
                Instruction::Catch(6),
                Instruction::PushI32(20),
                Instruction::PushI32(30),
                Instruction::NipCatch,
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(nip.verify().unwrap().max_stack, 4);

        let marker_exceeds_declared_stack = DetachedBytecode::<Value> {
            max_stack: 0,
            ..normal
        };
        assert!(marker_exceeds_declared_stack.verify().is_err());
    }

    #[test]
    fn verifier_tracks_typed_gosub_return_addresses() {
        let returning = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(9),
                Instruction::Gosub(4),
                Instruction::Return,
                Instruction::Nop,
                Instruction::Ret,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(returning.verify().unwrap().max_stack, 2);

        let abrupt_cleanup = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(9),
                Instruction::Gosub(4),
                Instruction::Return,
                Instruction::Nop,
                Instruction::DropGosub,
                Instruction::Drop,
                Instruction::PushI32(4),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(abrupt_cleanup.verify().unwrap().max_stack, 2);

        let return_address_exceeds_declared_stack = DetachedBytecode::<Value> {
            max_stack: 1,
            ..returning
        };
        assert!(return_address_exceeds_declared_stack.verify().is_err());
    }

    #[test]
    fn verifier_dynamic_cleanup_discards_intermediate_gosub_return_addresses() {
        // A return from a finally subroutine can override the pending
        // completion protected by this catch. NipCatch preserves the new
        // return value and intentionally truncates the now-obsolete Gosub PC.
        let nip_catch = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(10),
                Instruction::Undefined,
                Instruction::Gosub(7),
                Instruction::Drop,
                Instruction::DropCatch,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::PushI32(7),
                Instruction::NipCatch,
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(nip_catch.verify().unwrap().max_stack, 4);

        // The same dynamic truncation is required when a return crosses an
        // iterator region while a nested finally Gosub is active.
        let iterator_close = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::Gosub(6),
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::PushI32(9),
                Instruction::IteratorClosePreserve,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(iterator_close.verify().unwrap().max_stack, 5);
    }

    #[test]
    fn verifier_tracks_for_of_records_offsets_and_dynamic_close() {
        let normal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::ForOfNext(0),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(normal.verify().unwrap().max_stack, 5);

        let offset = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(2),
                Instruction::ForOfNext(1),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 6,
        };
        assert_eq!(offset.verify().unwrap().max_stack, 6);

        let preserve = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(42),
                Instruction::IteratorClosePreserve,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(preserve.verify().unwrap().max_stack, 4);

        let drop_preserve = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(42),
                Instruction::IteratorDropPreserve,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(drop_preserve.verify().unwrap().max_stack, 4);
    }

    #[test]
    fn verifier_tracks_for_await_records_phases_and_dynamic_detach() {
        let normal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForAwaitOfStart,
                Instruction::ForAwaitOfNext,
                Instruction::Await,
                Instruction::IteratorGetValueDone,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(normal.verify().unwrap().max_stack, 5);

        for start in [Instruction::ForOfStart, Instruction::ForAwaitOfStart] {
            let detach = DetachedBytecode::<Value> {
                code: vec![
                    Instruction::PushI32(1),
                    start,
                    Instruction::PushI32(42),
                    Instruction::IteratorDetachPreserve,
                    // Detach leaves the iterator above the preserved
                    // completion, ready for explicit close lowering.
                    Instruction::Drop,
                    Instruction::Return,
                ],
                constants: vec![],
                local_count: 0,
                max_stack: 4,
            };
            assert_eq!(detach.verify().unwrap().max_stack, 4);
        }
    }

    #[test]
    fn verifier_tracks_for_in_as_one_ordinary_retained_operand() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForInStart,
                Instruction::ForInNext,
                Instruction::IfFalse(8),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::Drop,
                Instruction::Goto(2),
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(function.verify().unwrap().max_stack, 3);

        let underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::ForInStart, Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert!(underflow.verify().is_err());
    }

    #[test]
    fn verifier_orders_iterator_catch_and_gosub_regions() {
        // The iterator is inside a catch region. Preserving the return value
        // closes it first, then NipCatch removes the surrounding catch marker.
        let iterator_inside_catch = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(7),
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(9),
                Instruction::IteratorClosePreserve,
                Instruction::NipCatch,
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(iterator_inside_catch.verify().unwrap().max_stack, 5);

        // The catch is inside an iterator. Its exceptional edge retains the
        // outer iterator region, which is then explicitly closed.
        let catch_inside_iterator = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::Catch(7),
                Instruction::DropCatch,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(catch_inside_iterator.verify().unwrap().max_stack, 4);

        // A finally return PC is below the iterator record. Closing the record
        // must leave that typed address at TOS for Ret.
        let iterator_above_outer_gosub = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::Gosub(5),
                Instruction::Return,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::IteratorClose,
                Instruction::Ret,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(iterator_above_outer_gosub.verify().unwrap().max_stack, 5);

        // Conversely a gosub may temporarily put its address above an outer
        // iterator marker, provided Ret consumes it before iterator cleanup.
        let gosub_above_iterator = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::Gosub(6),
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::Ret,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(gosub_above_iterator.verify().unwrap().max_stack, 4);
    }

    #[test]
    fn verifier_models_quickjs_lexical_frame_operations() {
        let initialized_then_updated = DetachedBytecode::<Value> {
            code: vec![
                Instruction::SetLocalUninitialized(0),
                Instruction::PushI32(1),
                Instruction::InitializeLocal(0),
                Instruction::GetLocalCheck(0),
                Instruction::PushI32(1),
                Instruction::Add,
                Instruction::SetLocalCheck(0),
                Instruction::CloseLocal(0),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 1,
            max_stack: 2,
        };
        assert_eq!(initialized_then_updated.verify().unwrap().max_stack, 2);

        let consuming_write = DetachedBytecode::<Value> {
            code: vec![
                Instruction::SetLocalUninitialized(0),
                Instruction::PushI32(1),
                Instruction::InitializeLocal(0),
                Instruction::PushI32(2),
                Instruction::PutLocalCheck(0),
                Instruction::GetLocalCheck(0),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 1,
            max_stack: 1,
        };
        assert_eq!(consuming_write.verify().unwrap().max_stack, 1);

        let checked_var_ref = DetachedBytecode::<Value> {
            code: vec![
                Instruction::GetVarRefCheck(0),
                Instruction::PutVarRefCheck(0),
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(checked_var_ref.verify().unwrap().max_stack, 1);

        for instruction in [
            Instruction::InitializeLocal(0),
            Instruction::InitializeDerivedLocal(0),
            Instruction::PutLocalCheck(0),
            Instruction::SetLocalCheck(0),
            Instruction::PutVarRefCheck(0),
            Instruction::InitializeDerivedVarRef(0),
        ] {
            let underflow = DetachedBytecode::<Value> {
                code: vec![instruction, Instruction::Undefined, Instruction::Return],
                constants: vec![],
                local_count: 1,
                max_stack: 1,
            };
            assert!(underflow.verify().is_err());
        }
    }

    #[test]
    fn verifier_models_typed_derived_constructor_operations() {
        let default_derived = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushActiveFunction,
                Instruction::PutLocal(1),
                Instruction::InitDerivedConstructor,
                Instruction::Dup,
                Instruction::InitializeDerivedLocal(0),
                Instruction::ReturnDerived(0),
            ],
            constants: vec![],
            local_count: 2,
            max_stack: 2,
        };
        assert_eq!(default_derived.verify().unwrap().max_stack, 2);

        let captured_initializer = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::InitializeDerivedVarRef(0),
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(captured_initializer.verify().unwrap().max_stack, 1);

        let return_underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::ReturnDerived(0)],
            local_count: 1,
            max_stack: 0,
            ..DetachedBytecode::<Value>::default()
        };
        assert_eq!(
            return_underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );

        // ReturnDerived is a terminal completion and must not make its
        // unreachable successor part of the control-flow graph.
        let terminal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::ReturnDerived(0),
                Instruction::PushI32(1),
            ],
            local_count: 1,
            max_stack: 1,
            ..DetachedBytecode::<Value>::default()
        };
        assert_eq!(terminal.verify().unwrap().max_stack, 1);
    }

    #[test]
    fn verifier_models_array_literal_stack_contracts() {
        let fixed = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::ArrayFrom(2),
                Instruction::PushI32(3),
                Instruction::DefineField(0),
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::value::JsString::from_static(
                "2",
            ))],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(fixed.verify().unwrap().max_stack, 2);

        let dynamic = DetachedBytecode::<Value> {
            code: vec![
                Instruction::ArrayFrom(0),
                Instruction::PushI32(0),
                Instruction::Undefined,
                Instruction::DefineArrayEl,
                Instruction::Inc,
                Instruction::Undefined,
                Instruction::Append,
                Instruction::Drop,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(dynamic.verify().unwrap().max_stack, 3);

        let trailing_hole = DetachedBytecode::<Value> {
            code: vec![
                Instruction::ArrayFrom(0),
                Instruction::PushI32(2),
                Instruction::Dup1,
                Instruction::PutField(0),
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::value::JsString::from_static(
                "length",
            ))],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(trailing_hole.verify().unwrap().max_stack, 3);
    }

    #[test]
    fn verifier_models_object_literal_stack_contracts() {
        let fixed_proto_spread = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Object,
                Instruction::PushI32(1),
                Instruction::DefineField(0),
                Instruction::Undefined,
                Instruction::DefineMethod {
                    key: 1,
                    kind: DefineMethodKind::Method,
                    enumerable: true,
                },
                Instruction::Null,
                Instruction::SetProto,
                Instruction::Undefined,
                Instruction::CopyDataProperties,
                Instruction::Return,
            ],
            constants: vec![
                Value::String(crate::engine::value::JsString::from_static("field")),
                Value::String(crate::engine::value::JsString::from_static("method")),
            ],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(fixed_proto_spread.verify().unwrap().max_stack, 2);

        let computed = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Object,
                Instruction::PushI32(1),
                Instruction::ToPropKey,
                Instruction::Undefined,
                Instruction::DefineMethodComputed {
                    kind: DefineMethodKind::Getter,
                    enumerable: true,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(computed.verify().unwrap().max_stack, 3);
    }

    #[test]
    fn verifier_models_depth_addressed_object_rest_copy_without_stack_mutation() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1), // excluded
                Instruction::PushI32(2), // source
                Instruction::PushI32(3), // prepared Reference
                Instruction::PushI32(4), // target
                Instruction::CopyDataPropertiesExcluded {
                    target_depth: 0,
                    source_depth: 2,
                    excluded_depth: 3,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(function.verify().unwrap().max_stack, 4);

        let underflow = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::PushI32(3),
                Instruction::CopyDataPropertiesExcluded {
                    target_depth: 0,
                    source_depth: 2,
                    excluded_depth: 3,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );
    }

    #[test]
    fn verifier_rejects_object_literal_opcode_underflow() {
        for instruction in [
            Instruction::SetNameComputed,
            Instruction::DefineMethod {
                key: 0,
                kind: DefineMethodKind::Method,
                enumerable: true,
            },
            Instruction::DefineMethodComputed {
                kind: DefineMethodKind::Setter,
                enumerable: true,
            },
            Instruction::SetProto,
            Instruction::CopyDataProperties,
        ] {
            let constants = if matches!(&instruction, Instruction::DefineMethod { .. }) {
                vec![Value::String(crate::engine::value::JsString::from_static(
                    "method",
                ))]
            } else {
                vec![]
            };
            let function = DetachedBytecode::<Value> {
                code: vec![instruction, Instruction::Return],
                constants,
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }
    }
}
