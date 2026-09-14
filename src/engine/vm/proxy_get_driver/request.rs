//! Closed reply routing between domain states and the owned scheduler.
mod array;
mod buffer;
mod conversion;
mod function;
mod iterator;
mod module;
mod native;
mod object;
mod object_builtins;
mod scalar;
mod string;
mod vm;

use super::{
    BytecodeCallRequest, CompleteOrdinaryPropertyDescriptor, Completion, DescriptorResume,
    DescriptorStep, DirectCallTarget, NativeConversion, ObjectRef, OrdinaryPropertyDescriptor,
    OrdinaryRead, PropertyKey, ProxyBooleanResume, ProxyBooleanStep, ProxyGetResume, ProxyGetStep,
    ProxyOwnResume, ProxyOwnStep, Runtime, Value,
};
use crate::engine::object::operations::{
    InternalDefineResult, InternalSetResult, PropertySetAction,
};
use crate::engine::object::{
    ProxyDefineResume, ProxyDefineStep, ProxySetResume, ProxySetStep, SetResume, SetStep,
    set_completion,
};
use crate::engine::vm::ToPrimitiveHint;

use crate::engine::object::operations::ArrayLengthConversion;
use crate::engine::object::{ArrayLengthResume, ArrayLengthStep};
use crate::engine::value::conversion::number::{NumberResume, NumberStep};

use crate::engine::builtins::native::TypedArrayElementKind;
use crate::engine::builtins::{ElementResume, ElementStep, TypedWriteResume, TypedWriteStep};

use crate::engine::object::{ProxyPrototypeResume, ProxyPrototypeStep};

pub(super) enum Resume {
    RootDescriptor,
    RootDefine,
    RootSet,
    ModuleCallback(Box<crate::engine::modules::callback::CallbackResume>),
    ModuleEvaluation(Box<crate::engine::modules::evaluation::EvaluationResume>),
    ModuleBody(Box<crate::engine::modules::body::BodyResume>),
    ModuleLink(Box<crate::engine::modules::link::LinkResume>),
    Import(Box<crate::engine::modules::import::ImportResume>),
    #[cfg(feature = "test262-host")]
    Test262Agent(crate::engine::api::test262_agent::operation::AgentResume),
    #[cfg(feature = "test262-host")]
    EvalScript(crate::engine::api::test262_host::operation::EvalScriptResume),
    FromSync(Box<crate::engine::vm::async_from_sync_iterator::FromSyncResume>),
    AsyncGenerator(Box<crate::engine::vm::async_generator::AsyncGeneratorResume>),
    Async(Box<crate::engine::vm::async_function::AsyncResume>),
    Promise(Box<crate::engine::builtins::promise::operation::PromiseResume>),
    GeneratorCreate(crate::engine::vm::suspend::creation::GeneratorCreation),
    GeneratorPrototype(Box<crate::engine::vm::suspend::creation::GeneratorPrototype>),
    Generator(Box<crate::engine::vm::generator::GeneratorResume>),
    ForIn(crate::engine::vm::for_in::operation::ForInResume),
    Atomics(crate::engine::builtins::AtomicsResume),
    PublicField,
    LiteralDefinition(crate::engine::object::object_literal::element::LiteralDefinitionResume),
    TypedCreate(crate::engine::builtins::TypedCreateResume),
    TypedCollect(crate::engine::builtins::TypedCollectResume),
    TypedIteratorMethod(crate::engine::builtins::TypedIteratorMethodResume),

    BufferSlice(crate::engine::builtins::BufferSliceResume),
    TypedWith(crate::engine::builtins::TypedWithResume),
    Uint8Codec(crate::engine::builtins::Uint8CodecResume),

    VmNumeric(crate::engine::vm::numeric::operation::NumericResume),
    TypedSearch(crate::engine::builtins::TypedSearchResume),
    TypedString(crate::engine::builtins::TypedStringResume),
    TypedSlice(crate::engine::builtins::TypedSliceResume),
    TypedMutation(crate::engine::builtins::TypedMutationResume),
    StringFactory(crate::engine::builtins::StringFactoryResume),

    WeakConstructor(crate::engine::builtins::WeakConstructorResume),
    Environment(crate::engine::vm::environment_bindings::operation::EnvironmentResume),
    RegExpIteratorSet {
        key: PropertyKey,
        resume: crate::engine::builtins::RegExpIteratorResume,
    },
    RegExpMatchAll(crate::engine::builtins::RegExpMatchAllResume),
    RegExpSplit(crate::engine::builtins::RegExpSplitResume),
    RegExpIterator(crate::engine::builtins::RegExpIteratorResume),
    RegExpSpecies(crate::engine::builtins::RegExpSpeciesResume),

    JsonRaw(crate::engine::builtins::JsonRawResume),
    ObjectConstructor(crate::engine::builtins::ObjectConstructorResume),
    Bind(crate::engine::builtins::BindResume),
    FunctionText(crate::engine::builtins::FunctionTextResume),
    DynamicFunction(crate::engine::builtins::DynamicFunctionResume),
    JsonParse(crate::engine::builtins::JsonParseResume),
    JsonStringify(crate::engine::builtins::JsonStringifyResume),
    BufferConstructor(crate::engine::builtins::BufferConstructorResume),
    DataViewConstructor(crate::engine::builtins::DataViewConstructorResume),
    TypedSet(crate::engine::builtins::TypedSetResume),
    RegExpConstructor(crate::engine::builtins::RegExpConstructorResume),
    RegExpSearch(crate::engine::builtins::RegExpSearchResume),
    RegExpMatch(crate::engine::builtins::RegExpMatchResume),
    RegExpCompile(crate::engine::builtins::RegExpCompileResume),
    StringProtocol(crate::engine::builtins::StringProtocolResume),

    TypedSort(crate::engine::builtins::TypedSortResume),
    Math(crate::engine::builtins::MathResume),
    Sum(crate::engine::builtins::SumResume),
    PrimitiveConstructor(crate::engine::builtins::PrimitiveConstructorResume),
    Global(crate::engine::builtins::GlobalResume),
    Numeric(crate::engine::builtins::NumericResume),
    ScalarText(crate::engine::builtins::ScalarTextResume),
    DateConstructor(crate::engine::builtins::DateConstructorResume),
    DatePrototype(crate::engine::builtins::DatePrototypeResume),
    Error(crate::engine::builtins::ErrorResume),
    Aggregate(crate::engine::builtins::AggregateResume),
    PrimitiveConstructorValue(crate::engine::builtins::PrimitiveConstructorResume),
    NumericPrimitive(crate::engine::builtins::NumericResume),
    DateConstructorPrimitive(crate::engine::builtins::DateConstructorResume),

    MapCallback(crate::engine::builtins::MapCallbackResume),
    SetEach(crate::engine::builtins::SetEachResume),
    SetOperation(crate::engine::builtins::SetOperationResume),
    Collection(crate::engine::builtins::CollectionResume),
    WeakComputed(crate::engine::builtins::WeakComputedResume),
    IteratorInvalidCount(crate::engine::builtins::IteratorCreateResume),
    ArrayConstructor(crate::engine::builtins::ArrayConstructorResume),
    ArraySlice(crate::engine::builtins::ArraySliceResume),
    IteratorConstructor(crate::engine::builtins::IteratorConstructorResume),
    IteratorTag(crate::engine::builtins::IteratorTagResume),
    TypedTraversal(crate::engine::builtins::TypedTraversalResume),
    TypedSpecies(crate::engine::builtins::TypedSpeciesResume),
    TypedIteration(crate::engine::builtins::TypedIterationResume),

    ConstructorSource(crate::engine::vm::call::prototype::ProtoSourceResume),
    ArrayConstructorSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayConstructorResume,
    },
    ArraySliceSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArraySliceResume,
    },

    ArrayCopy(crate::engine::builtins::ArrayCopyResume),
    ArrayConcat(crate::engine::builtins::ArrayConcatResume),
    ArrayFlatten(crate::engine::builtins::ArrayFlattenResume),
    ObjectCopy(crate::engine::builtins::ObjectCopyResume),
    StringText(crate::engine::builtins::StringTextResume),
    StringSearch(crate::engine::builtins::StringSearchResume),
    StringSplit(crate::engine::builtins::StringSplitResume),
    ArrayCopySet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayCopyResume,
    },
    ArrayConcatSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayConcatResume,
    },

    Instance(crate::engine::builtins::InstanceResume),
    IteratorFrom(crate::engine::builtins::IteratorFromResume),
    IteratorWrap(crate::engine::builtins::IteratorWrapResume),
    IteratorConcat(crate::engine::builtins::IteratorConcatResume),
    ArrayBuild(crate::engine::builtins::ArrayBuildResume),
    ArrayBuildSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayBuildResume,
    },

    ArraySort(crate::engine::builtins::ArraySortResume),
    ArrayIndexed(crate::engine::builtins::ArrayIndexedResume),
    ArrayReverse(crate::engine::builtins::ArrayReverseResume),
    ArrayString(crate::engine::builtins::ArrayStringResume),
    RegExpExec(crate::engine::builtins::RegExpExecResume),
    RegExpPresentation(crate::engine::builtins::RegExpPresentationResume),
    RegExpReplace(crate::engine::builtins::RegExpReplaceResume),
    IteratorConsume(crate::engine::builtins::IteratorConsumeResume),
    IteratorHelper(crate::engine::builtins::IteratorHelperResume),
    IteratorCreate(crate::engine::builtins::IteratorCreateResume),

    StringValue {
        realm: crate::engine::heap::ContextId,
        resume: Box<Resume>,
    },
    ArraySortSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArraySortResume,
    },
    ArrayIndexedSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayIndexedResume,
    },
    ArrayReverseSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayReverseResume,
    },

    ArrayNext(crate::engine::builtins::ArrayNextResume),
    ArrayMutation(crate::engine::builtins::ArrayMutationResume),
    ArrayMutationSet {
        key: PropertyKey,
        resume: crate::engine::builtins::ArrayMutationResume,
    },
    ArrayCallback(crate::engine::builtins::ArrayCallbackResume),
    ArraySpecies(crate::engine::builtins::ArraySpeciesResume),
    StringReplace(crate::engine::builtins::StringReplaceResume),
    DataView(crate::engine::builtins::DataViewAccessResume),
    BufferMutation(crate::engine::builtins::BufferMutationResume),
    ObjectIteration(crate::engine::builtins::ObjectIterationResume),
    ObjectIterationKey(crate::engine::builtins::ObjectIterationResume),
    IteratorNext(crate::engine::builtins::IteratorNextResume),
    IteratorClose(crate::engine::builtins::IteratorCloseResume),

    ProxyConstruct(crate::engine::object::ProxyConstructResume),
    ConstructorPrototype {
        request: Box<BytecodeCallRequest>,
        resume: Box<Resume>,
    },
    Arguments(crate::engine::builtins::ArgumentsResume),
    Invoke(crate::engine::builtins::InvokeResume),
    Identity,
    ObjectString(crate::engine::builtins::ObjectStringResume),
    Definitions(crate::engine::builtins::DefinitionsResume),
    PredicateKey(crate::engine::builtins::PredicateResume),
    Predicate(crate::engine::builtins::PredicateResume),
    OwnFlagReply {
        enumerable: bool,
        resume: Box<Resume>,
    },
    Keys(crate::engine::object::KeysResume),
    Property(crate::engine::builtins::PropertyResume),
    PropertyKey(crate::engine::builtins::PropertyResume),
    Primitive(crate::engine::value::conversion::primitive::PrimitiveResume),
    BuiltinPrototype(crate::engine::builtins::BuiltinPrototypeResume),
    Prototype(ProxyPrototypeResume),
    PrototypeGetReply(Box<Resume>),
    PrototypeSetReply(Box<Resume>),
    BooleanResult {
        _object: ObjectRef,
        _key: Option<PropertyKey>,
        strict_delete: bool,
    },
    ReadOwner(ObjectRef),
    Element(ElementResume),
    TypedElement(TypedWriteResume),
    SetTyped(SetResume),
    DefineTyped {
        object: ObjectRef,
        _descriptor: OrdinaryPropertyDescriptor,
        resume: Box<Resume>,
    },
    Number(NumberResume),
    LengthNumber(ArrayLengthResume),
    SetLength(SetResume),
    DefineLength {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: Box<Resume>,
    },
    OrdinarySet(SetResume),
    ProxySet(ProxySetResume),
    Define(ProxyDefineResume),
    Setter,
    Get(ProxyGetResume),
    Call(crate::engine::object::ProxyCallResume),
    Own(ProxyOwnResume),
    Conversion(DescriptorResume),
    Boolean(ProxyBooleanResume),
}

pub(super) enum Step {
    RootDescriptor(crate::engine::vm::entry::DescriptorReply),
    ModuleCallbackOperation {
        step: Box<crate::engine::modules::callback::CallbackStep>,
        resume: Resume,
    },
    ModuleBodyOperation {
        step: Box<crate::engine::modules::body::BodyStep>,
        resume: Resume,
    },
    ModuleLink {
        realm: crate::engine::heap::ContextId,
        callable: crate::engine::object::CallableRef,
        resume: Resume,
    },
    PromiseOperation {
        step: Box<crate::engine::builtins::promise::operation::PromiseStep>,
        resume: Resume,
    },
    IntrinsicPromiseResolve {
        value: Value,
        realm: crate::engine::heap::ContextId,
        resume: Resume,
    },
    ResumeFrame {
        activation: Box<crate::engine::vm::suspend::RootedVmActivation>,
        input: crate::engine::vm::suspend::VmActivationResume,
        resume: Resume,
    },
    ForInComplete {
        value: Value,
        done: Option<bool>,
    },
    TypedIteratorMethod {
        source: Value,
        resume: Resume,
    },
    TypedIteratorMethodComplete(NativeConversion<Option<crate::engine::object::CallableRef>>),
    TypedCollect {
        source: Value,
        method: crate::engine::object::CallableRef,
        element: TypedArrayElementKind,
        resume: Resume,
    },
    TypedCollectComplete(NativeConversion<Vec<Value>>),
    TypedCreate {
        constructor: Value,
        length: u64,
        resume: Resume,
    },

    NumericComplete {
        value: Value,
        previous: Option<Value>,
    },
    NumericHtmlDda {
        value: Value,
        resume: crate::engine::vm::numeric::operation::NumericResume,
    },
    TypedSpeciesView {
        source: ObjectRef,
        element: TypedArrayElementKind,
        buffer: ObjectRef,
        offset: u64,
        length: Option<u64>,
        resume: Resume,
    },
    RegExpSpecies {
        regexp: ObjectRef,
        resume: Resume,
    },
    RegExpSpeciesComplete(NativeConversion<crate::engine::vm::call::ConstructorRef>),
    IndirectEval {
        source: crate::engine::value::JsString,
        resume: Resume,
    },
    Aggregate {
        iterable: Value,
        resume: Resume,
    },
    OrdinaryPrimitive {
        object: ObjectRef,
        hint: ToPrimitiveHint,
    },

    ConstructorSource {
        new_target: Value,
        resume: Resume,
    },
    ConstructorSourceComplete(
        NativeConversion<crate::engine::vm::call::ConstructorPrototypeSource>,
    ),
    TypedSpecies {
        source: ObjectRef,
        element: TypedArrayElementKind,
        length: u64,
        resume: Resume,
    },
    TypedSpeciesComplete(NativeConversion<ObjectRef>),
    ArrayCopy {
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: Resume,
    },
    OrdinaryInstance {
        constructor: crate::engine::object::CallableRef,
        value: Value,
        resume: Resume,
    },
    ParseIterator {
        result: Completion,
        resume: Resume,
    },
    String {
        value: Value,
        resume: Resume,
    },
    ObjectTag {
        receiver: Value,
    },
    RegExpExec {
        regexp: Value,
        input: Value,
        resume: Resume,
    },
    IteratorCloseWithResume {
        iterator: ObjectRef,
        completion: Completion,
        resume: Resume,
    },
    NativeRawComplete(crate::engine::vm::call::NativeInvokeOutcome),
    ArraySpecies {
        source: ObjectRef,
        length: u64,
        resume: Resume,
    },
    ArrayPush {
        object: ObjectRef,
        value: Value,
        resume: Resume,
    },
    IteratorNext {
        iterator: ObjectRef,
        method: Value,
        resume: Resume,
    },
    IteratorNextComplete(crate::engine::builtins::ObjectIteratorStep),
    IteratorCall {
        callable: crate::engine::object::CallableRef,
        iterator: ObjectRef,
        resume: crate::engine::builtins::IteratorNextResume,
    },
    IteratorClose {
        iterator: ObjectRef,
        completion: Completion,
    },

    Native {
        callable: crate::engine::object::CallableRef,
        target: crate::engine::builtins::native::NativeFunctionId,
        defining_realm: crate::engine::heap::ContextId,
        min_readable_args: u8,
        mode: crate::engine::vm::call::NativeInvokeMode,
        invocation: crate::engine::vm::call::NativeInvocation,
        arguments: Vec<Value>,
        resume: Resume,
    },
    Construct {
        target: crate::engine::vm::call::ConstructorRef,
        new_target: crate::engine::vm::call::ConstructNewTarget,
        arguments: Vec<Value>,
        resume: Resume,
    },
    ConstructProxy {
        target: crate::engine::vm::call::ConstructorRef,
        new_target: crate::engine::vm::call::ConstructNewTarget,
        arguments: Vec<Value>,
        resume: Resume,
    },
    ConstructorReady {
        request: Box<BytecodeCallRequest>,
        receiver: Completion,
        derived: bool,
        resume: Resume,
    },
    Arguments {
        value: Value,
        resume: Resume,
    },
    ArgumentsComplete(NativeConversion<Vec<Value>>),
    SnapshotEnumerable {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    OwnFlag {
        object: ObjectRef,
        key: PropertyKey,
        enumerable: bool,
        resume: Resume,
    },
    Keys {
        object: ObjectRef,
        resume: Resume,
    },
    KeysComplete(NativeConversion<Vec<PropertyKey>>),
    ReadValue {
        receiver: Value,
        key: PropertyKey,
        resume: Resume,
    },
    PreparedRead {
        read: OrdinaryRead,
        key: PropertyKey,
        resume: Resume,
    },
    Primitive {
        value: Value,
        hint: crate::engine::vm::ToPrimitiveHint,
        resume: Resume,
    },
    GetPrototype {
        object: ObjectRef,
        resume: Resume,
    },
    SetPrototype {
        object: ObjectRef,
        prototype: Option<ObjectRef>,
        resume: Resume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    PreventExtensions {
        object: ObjectRef,
        resume: Resume,
    },
    Element {
        element: TypedArrayElementKind,
        value: Value,
        resume: Resume,
    },
    ElementComplete(NativeConversion<[u8; 8]>),
    TypedComplete(NativeConversion<bool>),
    Number {
        value: Value,
        resume: Resume,
    },
    NumberComplete(NativeConversion<f64>),
    LengthComplete(ArrayLengthConversion),
    SetLength {
        value: Value,
        resume: SetResume,
    },
    SetComplete(PropertySetAction),
    SetContinue(SetResume),
    SetSpecial {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: SetResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: Resume,
    },
    SetProxy {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: Resume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: Resume,
    },
    DefineOrdinary {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: Resume,
    },
    Defined(NativeConversion<InternalDefineResult>),
    Complete(Completion),
    BooleanComplete(NativeConversion<bool>),
    OwnComplete(NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>),
    Converted(NativeConversion<OrdinaryPropertyDescriptor>),
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: Resume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: Resume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    Extensible {
        object: ObjectRef,
        resume: Resume,
    },
    Convert {
        value: Value,
        resume: Resume,
    },
}

pub(super) fn set_result(
    action: PropertySetAction,
) -> Result<NativeConversion<InternalSetResult>, crate::engine::api::runtime_error::RuntimeError> {
    Ok(match action {
        PropertySetAction::Complete => NativeConversion::Value(InternalSetResult::Accepted),
        PropertySetAction::Rejected(reason) => {
            NativeConversion::Value(InternalSetResult::Rejected(reason))
        }
        PropertySetAction::RejectedProxyTrap => {
            NativeConversion::Value(InternalSetResult::RejectedProxyTrap)
        }
        PropertySetAction::Throw(value) => NativeConversion::Throw(value),
        PropertySetAction::Call { .. } => {
            return Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Set completed before its setter returned",
            ));
        }
    })
}

impl Resume {
    pub(super) fn set(
        self,
        runtime: &Runtime,
        action: PropertySetAction,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RootSet => Ok(Step::Complete(match set_result(action)? {
                NativeConversion::Throw(value) => Completion::Throw(value),
                NativeConversion::Value(result) => {
                    Completion::Return(Value::Bool(matches!(result, InternalSetResult::Accepted)))
                }
            })),

            Self::RegExpMatchAll(resume) => {
                resume.set(runtime, set_result(action)?).map(Into::into)
            }
            Self::RegExpSplit(resume) => resume.set(runtime, set_result(action)?).map(Into::into),
            Self::Environment(resume) => resume.set(runtime, set_result(action)?).map(Into::into),
            Self::RegExpIteratorSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),

            Self::RegExpSearch(resume) => resume.set(runtime, set_result(action)?).map(Into::into),
            Self::RegExpMatch(resume) => resume.set(runtime, set_result(action)?).map(Into::into),

            Self::ArrayConstructorSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::ArraySliceSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::IteratorTag(resume) => resume.set(runtime, set_result(action)?).map(Into::into),
            Self::ArrayCopySet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::ArrayConcatSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::ArrayBuildSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::RegExpReplace(resume) => resume.set(runtime, set_result(action)?).map(Into::into),
            Self::ArraySortSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::ArrayIndexedSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::ArrayReverseSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::ArrayMutationSet { key, resume } => resume
                .set(runtime, key, set_result(action)?)
                .map(Into::into),
            Self::Property(resume) => resume.set(runtime, set_result(action)?).map(Into::into),
            Self::OrdinarySet(resume) => resume.forward(action).map(Into::into),
            Self::ProxySet(resume) => resume.set(set_result(action)?).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Set result has no matching continuation",
            )),
        }
    }
    pub(super) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RootDefine => Ok(Step::Complete(match result {
                NativeConversion::Throw(value) => Completion::Throw(value),
                NativeConversion::Value(result) => {
                    Completion::Return(Value::Bool(matches!(result, InternalDefineResult::Defined)))
                }
            })),

            Self::LiteralDefinition(resume) => resume.defined(result).map(Into::into),
            Self::PublicField => match Runtime::finish_public_class_field_definition(result)? {
                crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => {
                    Ok(Step::Complete(Completion::Return(Value::Undefined)))
                }
                crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => {
                    Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                        "public field rejected without throwing",
                    ))
                }
                crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                    Ok(Step::Complete(Completion::Throw(value)))
                }
            },

            Self::JsonParse(resume) => resume
                .boolean(
                    runtime,
                    match result {
                        NativeConversion::Value(result) => {
                            NativeConversion::Value(matches!(result, InternalDefineResult::Defined))
                        }
                        NativeConversion::Throw(value) => NativeConversion::Throw(value),
                    },
                )
                .map(Into::into),

            Self::ArraySlice(resume) => resume.defined(runtime, result).map(Into::into),
            Self::IteratorTag(resume) => resume.defined(runtime, result).map(Into::into),
            Self::ArrayConcat(resume) => resume.defined(runtime, result).map(Into::into),
            Self::ArrayFlatten(resume) => resume.defined(runtime, result).map(Into::into),
            Self::ArrayBuild(resume) => resume.defined(runtime, result).map(Into::into),
            Self::ArrayCallback(resume) => resume.defined(runtime, result).map(Into::into),
            Self::ObjectIteration(resume) => resume.defined(runtime, result).map(Into::into),
            Self::Predicate(resume) => resume.defined(runtime, result).map(Into::into),
            Self::Definitions(resume) => resume.defined(runtime, result).map(Into::into),
            Self::Property(resume) => resume.defined(runtime, result).map(Into::into),
            Self::OrdinarySet(resume) => resume.defined(runtime, result).map(Into::into),
            Self::Define(resume) => resume.defined(result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Define result has no matching continuation",
            )),
        }
    }

    pub(super) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Import(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ForIn(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Environment(resume) => resume.boolean(runtime, result).map(Into::into),

            Self::Bind(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::JsonParse(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::JsonStringify(resume) => resume.boolean(runtime, result).map(Into::into),

            Self::Error(resume) => resume.boolean(runtime, result).map(Into::into),

            Self::ArraySlice(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::IteratorTag(resume) => resume.boolean(result).map(Into::into),
            Self::ArrayCopy(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArrayConcat(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArrayFlatten(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ObjectCopy(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArraySort(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArrayIndexed(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArrayReverse(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArrayMutation(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::ArrayCallback(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::BooleanResult {
                _object,
                _key,
                strict_delete,
            } => runtime
                .finish_property_delete(result, strict_delete)
                .map(Step::Complete),
            Self::Definitions(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Predicate(resume) => resume.boolean(result).map(Into::into),
            Self::Keys(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Property(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::BuiltinPrototype(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Prototype(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Own(resume) => resume.extensible(result).map(Into::into),
            Self::Boolean(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Conversion(resume) => resume.has(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Proxy Get received a boolean reply",
            )),
        }
    }

    pub(super) fn suspended(
        self,
        runtime: &Runtime,
        outcome: crate::engine::vm::suspend::VmRunOutcome,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::AsyncGenerator(resume) => resume.body(outcome).map(Into::into),
            Self::Async(resume) => resume.body(outcome).map(Into::into),
            Self::GeneratorCreate(creation) => creation.initial(runtime, outcome).map(Into::into),
            Self::Generator(resume) => resume.resume(outcome).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "ordinary callback returned a suspension",
            )),
        }
    }

    pub(super) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RootDescriptor | Self::RootDefine | Self::RootSet => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "typed root received an untyped reply",
                ))
            }
            Self::ModuleCallback(resume) => resume.resume(completion).map(Into::into),
            Self::ModuleEvaluation(resume) => resume.resume(completion).map(Into::into),
            Self::ModuleBody(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ModuleLink(resume) => {
                crate::engine::modules::link::resume_reply(runtime, resume.resume(completion))
                    .map(Into::into)
            }
            Self::Import(resume) => resume.resume(runtime, completion).map(Into::into),
            #[cfg(feature = "test262-host")]
            Self::Test262Agent(resume) => resume.resume(runtime, completion).map(Into::into),
            #[cfg(feature = "test262-host")]
            Self::EvalScript(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::FromSync(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::AsyncGenerator(resume) => resume.resume(completion).map(Into::into),
            Self::Async(resume) => resume.resume(completion).map(Into::into),
            Self::Promise(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::GeneratorCreate(creation) => creation
                .initial(
                    runtime,
                    crate::engine::vm::suspend::VmRunOutcome::Complete(completion),
                )
                .map(Into::into),
            Self::GeneratorPrototype(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Generator(resume) => resume
                .resume(crate::engine::vm::suspend::VmRunOutcome::Complete(
                    completion,
                ))
                .map(Into::into),
            Self::ForIn(_) => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "for-in requires typed reply",
            )),
            Self::Atomics(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::LiteralDefinition(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::PublicField => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "public field requires definition reply",
            )),

            Self::TypedCreate(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedCollect(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedIteratorMethod(resume) => resume.resume(runtime, completion).map(Into::into),

            Self::BufferSlice(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedWith(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Uint8Codec(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::VmNumeric(resume) => resume
                .resume(completion)
                .map(Into::into)
                .map_err(crate::engine::api::runtime_error::RuntimeError::Engine),
            Self::TypedSearch(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedString(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedSlice(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedMutation(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringFactory(resume) => resume.resume(runtime, completion).map(Into::into),

            Self::WeakConstructor(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "weak constructor requires prototype reply",
                ))
            }
            Self::RegExpMatchAll(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpSplit(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpIterator(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpSpecies(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Environment(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpIteratorSet { .. } => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "RegExp iterator requires Set reply",
                ))
            }

            Self::Bind(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::FunctionText(resume) => resume.resume(completion).map(Into::into),
            Self::DynamicFunction(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::JsonParse(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::JsonStringify(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::BufferConstructor(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::DataViewConstructor(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedSet(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpConstructor(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpSearch(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpMatch(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpCompile(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringProtocol(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ObjectConstructor(_) | Self::JsonRaw(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "native requires typed reply",
                ))
            }

            Self::Math(_) | Self::Global(_) | Self::Numeric(_) | Self::ScalarText(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "scalar conversion requires typed reply",
                ))
            }
            Self::TypedSort(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Sum(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::PrimitiveConstructor(resume) => {
                resume.resume(runtime, completion).map(Into::into)
            }
            Self::DateConstructor(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::DatePrototype(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Error(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Aggregate(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::PrimitiveConstructorValue(resume) => {
                resume.primitive(runtime, completion).map(Into::into)
            }
            Self::NumericPrimitive(resume) => resume.primitive(runtime, completion).map(Into::into),
            Self::DateConstructorPrimitive(resume) => {
                resume.primitive(runtime, completion).map(Into::into)
            }

            Self::MapCallback(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::SetEach(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::SetOperation(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Collection(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::WeakComputed(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorInvalidCount(resume) => {
                resume.invalid_count(runtime, completion).map(Into::into)
            }

            Self::ArrayConstructor(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArraySlice(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedTraversal(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedSpecies(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::TypedIteration(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ConstructorSource(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayCopy(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayConcat(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayFlatten(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ObjectCopy(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringText(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringSearch(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringSplit(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Instance(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorFrom(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorWrap(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorConcat(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayBuild(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArraySort(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayIndexed(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayReverse(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayString(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpExec(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpPresentation(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::RegExpReplace(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorConsume(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorHelper(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorCreate(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringValue { realm, resume } => {
                let result = match completion {
                    Completion::Return(value) => runtime.string_from_primitive(realm, &value)?,
                    Completion::Throw(value) => NativeConversion::Throw(value),
                };
                resume.string(runtime, result)
            }
            Self::ArrayNext(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayMutation(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArrayCallback(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ArraySpecies(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::StringReplace(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::DataView(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::BufferMutation(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ObjectIteration(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorNext(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorClose(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ObjectIterationKey(resume) => resume.key(runtime, completion).map(Into::into),
            Self::Arguments(resume) => resume.read(runtime, completion).map(Into::into),
            Self::ProxyConstruct(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::ConstructorPrototype { request, resume } => {
                super::construct::prototype(runtime, request, completion, *resume)
                    .map_err(crate::engine::api::runtime_error::RuntimeError::Engine)
            }
            Self::Identity => Ok(Step::Complete(completion)),
            Self::ObjectString(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Definitions(resume) => resume.read(completion).map(Into::into),
            Self::PredicateKey(resume) => resume.key(runtime, completion).map(Into::into),
            Self::Keys(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::PropertyKey(resume) => resume.key(runtime, completion).map(Into::into),
            Self::Property(resume) => resume.read(runtime, completion).map(Into::into),
            Self::Primitive(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::BuiltinPrototype(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "prototype builtin received an untyped reply",
                ))
            }
            Self::Prototype(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::PrototypeGetReply(resume) => {
                let result = match completion {
                    Completion::Return(Value::Object(object)) => {
                        NativeConversion::Value(Some(object))
                    }
                    Completion::Return(Value::Null) => NativeConversion::Value(None),
                    Completion::Throw(value) => NativeConversion::Throw(value),
                    _ => {
                        return Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                            "invalid GetPrototypeOf reply",
                        ));
                    }
                };
                resume.prototype(runtime, result)
            }
            Self::PrototypeSetReply(resume) => {
                let result = match completion {
                    Completion::Return(Value::Bool(value)) => NativeConversion::Value(value),
                    Completion::Throw(value) => NativeConversion::Throw(value),
                    _ => {
                        return Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                            "invalid SetPrototypeOf reply",
                        ));
                    }
                };
                resume.boolean(runtime, result)
            }
            Self::ProxySet(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Define(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Setter => Ok(Step::SetComplete(match completion {
                Completion::Return(_) => PropertySetAction::Complete,
                Completion::Throw(value) => PropertySetAction::Throw(value),
            })),
            Self::BooleanResult { .. } => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "boolean operation received an untyped reply",
                ))
            }
            Self::ReadOwner(_owner) => Ok(Step::Complete(completion)),
            Self::Element(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Number(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::IteratorConstructor(_)
            | Self::IteratorTag(_)
            | Self::ArrayConstructorSet { .. }
            | Self::ArraySliceSet { .. }
            | Self::ArrayCopySet { .. }
            | Self::ArrayConcatSet { .. }
            | Self::ArrayBuildSet { .. }
            | Self::ArraySortSet { .. }
            | Self::ArrayIndexedSet { .. }
            | Self::ArrayReverseSet { .. }
            | Self::ArrayMutationSet { .. }
            | Self::Invoke(_)
            | Self::Predicate(_)
            | Self::OwnFlagReply { .. }
            | Self::TypedElement(_)
            | Self::SetTyped(_)
            | Self::DefineTyped { .. }
            | Self::LengthNumber(_)
            | Self::SetLength(_)
            | Self::DefineLength { .. }
            | Self::OrdinarySet(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "ordinary Set received an untyped reply",
                ))
            }
            Self::Get(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Call(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Own(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Conversion(resume) => resume.read(runtime, completion).map(Into::into),
            Self::Boolean(resume) => resume.resume(runtime, completion).map(Into::into),
        }
    }
    pub(super) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RootDescriptor => Ok(Step::RootDescriptor(result)),
            Self::OwnFlagReply { enumerable, resume } => resume.boolean(
                runtime,
                match result {
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                    NativeConversion::Value(descriptor) => NativeConversion::Value(
                        descriptor.is_some_and(|descriptor| !enumerable || descriptor.enumerable()),
                    ),
                },
            ),
            Self::Predicate(resume) => resume.descriptor(result).map(Into::into),
            Self::Keys(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Get(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Property(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Own(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Boolean(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::OrdinarySet(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::ProxySet(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Define(resume) => resume.descriptor(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "descriptor conversion received an own-property reply",
            )),
        }
    }
}

impl Resume {
    pub(super) fn length(
        self,
        runtime: &Runtime,
        result: ArrayLengthConversion,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        use crate::engine::object::operations::PropertyDefineOutcome;
        match self {
            Self::SetLength(resume) => resume.array_length(runtime, result).map(Into::into),
            Self::DefineLength {
                object,
                key,
                descriptor,
                resume,
            } => {
                let result = match result {
                    ArrayLengthConversion::Throw(value) => NativeConversion::Throw(value),
                    ArrayLengthConversion::Length(length) => match runtime
                        .apply_array_length_descriptor(&object, &key, &descriptor, length)?
                    {
                        PropertyDefineOutcome::Defined(true) => {
                            NativeConversion::Value(InternalDefineResult::Defined)
                        }
                        PropertyDefineOutcome::Defined(false) => {
                            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(object))
                        }
                        PropertyDefineOutcome::Throw(value) => NativeConversion::Throw(value),
                    },
                };
                resume.defined(runtime, result)
            }
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Array length result has no matching continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn typed(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::SetTyped(resume) => {
                let result = match result {
                    NativeConversion::Value(_) => {
                        NativeConversion::Value(InternalSetResult::Accepted)
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                };
                resume.special(runtime, Some(result)).map(Into::into)
            }
            Self::DefineTyped {
                object,
                _descriptor,
                resume,
            } => {
                let result = match result {
                    NativeConversion::Value(true) => {
                        NativeConversion::Value(InternalDefineResult::Defined)
                    }
                    NativeConversion::Value(false) => {
                        NativeConversion::Value(InternalDefineResult::RejectedOrdinary(object))
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                };
                resume.defined(runtime, result)
            }
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "TypedArray result has no matching continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::ForIn(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::Instance(resume) => resume.prototype(result).map(Into::into),
            Self::Predicate(resume) => resume.prototype(result).map(Into::into),
            Self::BuiltinPrototype(resume) => resume.prototype(result).map(Into::into),
            Self::Prototype(resume) => resume.prototype(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "prototype result has no matching continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn converted(
        self,
        runtime: &Runtime,
        result: NativeConversion<OrdinaryPropertyDescriptor>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Definitions(resume) => resume.converted(result).map(Into::into),
            Self::Own(resume) => resume.converted(runtime, result).map(Into::into),
            Self::Property(resume) => resume.converted(result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "descriptor conversion has no matching operation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            #[cfg(feature = "test262-host")]
            Self::Test262Agent(resume) => resume.number(runtime, result).map(Into::into),
            Self::Atomics(resume) => resume.number(runtime, result).map(Into::into),
            Self::StringFactory(resume) => resume.number(runtime, result).map(Into::into),

            Self::JsonParse(resume) => resume.number(runtime, result).map(Into::into),
            Self::JsonStringify(resume) => resume.number(runtime, result).map(Into::into),

            Self::TypedSort(resume) => resume.number(runtime, result).map(Into::into),
            Self::Math(resume) => resume.number(result).map(Into::into),
            Self::Global(resume) => resume.number(result).map(Into::into),
            Self::Numeric(resume) => resume.number(runtime, result).map(Into::into),
            Self::ScalarText(resume) => resume.number(result).map(Into::into),
            Self::DateConstructor(resume) => resume.number(runtime, result).map(Into::into),
            Self::DatePrototype(resume) => resume.number(runtime, result).map(Into::into),

            Self::SetOperation(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArraySlice(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayConcat(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayFlatten(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayBuild(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArraySort(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayIndexed(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayReverse(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayString(resume) => resume.number(runtime, result).map(Into::into),
            Self::IteratorCreate(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayNext(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayMutation(resume) => resume.number(runtime, result).map(Into::into),
            Self::ArrayCallback(resume) => resume.number(runtime, result).map(Into::into),
            Self::Arguments(resume) => resume.number(runtime, result).map(Into::into),
            Self::LengthNumber(resume) => resume.number(runtime, result).map(Into::into),
            Self::Keys(resume) => resume.number(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "numeric reply has no matching continuation",
            )),
        }
    }
    pub(super) fn keys(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Import(resume) => resume.keys(runtime, result).map(Into::into),
            Self::ForIn(resume) => resume.keys(runtime, result).map(Into::into),
            Self::JsonParse(resume) => resume.keys(runtime, result).map(Into::into),
            Self::JsonStringify(resume) => resume.keys(runtime, result).map(Into::into),

            Self::ObjectCopy(resume) => resume.keys(runtime, result).map(Into::into),
            Self::Definitions(resume) => resume.keys(runtime, result).map(Into::into),
            Self::Keys(resume) => resume.keys(runtime, result).map(Into::into),
            Self::Property(resume) => resume.keys(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "key-list reply has no matching continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn arguments(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<Value>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Invoke(resume) => resume.arguments(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "argument list has no matching continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn iterator_next(
        self,
        runtime: &Runtime,
        result: crate::engine::builtins::ObjectIteratorStep,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Promise(resume) => resume.next(runtime, result).map(Into::into),
            Self::Sum(resume) => resume.item(runtime, result).map(Into::into),
            Self::Aggregate(resume) => resume.item(runtime, result).map(Into::into),

            Self::SetOperation(resume) => resume.parsed(runtime, result).map(Into::into),
            Self::Collection(resume) => resume.next(runtime, result).map(Into::into),
            Self::IteratorWrap(resume) => resume.next(runtime, result).map(Into::into),
            Self::IteratorConcat(resume) => resume.next(runtime, result).map(Into::into),
            Self::ArrayBuild(resume) => resume.parsed(runtime, result).map(Into::into),
            Self::IteratorConsume(resume) => resume.next(runtime, result).map(Into::into),
            Self::IteratorHelper(resume) => resume.next(runtime, result).map(Into::into),
            Self::ObjectIteration(resume) => resume.next(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "iterator result has no continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn native(
        self,
        runtime: &Runtime,
        result: crate::engine::vm::call::NativeInvokeOutcome,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::IteratorNext(resume) => resume.raw(runtime, result).map(Into::into),
            resume => resume.resume(runtime, Runtime::ordinary_native_completion(result)?),
        }
    }
}

impl Resume {
    fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::value::JsString>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Import(resume) => resume
                .resume(
                    runtime,
                    match result {
                        NativeConversion::Value(value) => Completion::Return(Value::String(value)),
                        NativeConversion::Throw(value) => Completion::Throw(value),
                    },
                )
                .map(Into::into),

            #[cfg(feature = "test262-host")]
            resume @ (Self::EvalScript(_) | Self::Test262Agent(_)) => resume.resume(
                runtime,
                match result {
                    NativeConversion::Value(value) => Completion::Return(Value::String(value)),
                    NativeConversion::Throw(value) => Completion::Throw(value),
                },
            ),

            Self::StringFactory(resume) => resume.string(runtime, result).map(Into::into),

            Self::RegExpIterator(resume) => resume.string(runtime, result).map(Into::into),

            Self::FunctionText(resume) => resume.string(result).map(Into::into),
            Self::DynamicFunction(resume) => resume.string(result).map(Into::into),
            Self::JsonParse(resume) => resume.string(runtime, result).map(Into::into),
            Self::JsonStringify(resume) => resume.string(runtime, result).map(Into::into),
            Self::JsonRaw(resume) => resume.string(runtime, result).map(Step::Complete),

            Self::PrimitiveConstructor(resume) => resume.string(runtime, result).map(Into::into),
            Self::Global(resume) => resume.string(runtime, result).map(Into::into),
            Self::ScalarText(resume) => resume.string(runtime, result).map(Into::into),
            Self::DateConstructor(resume) => resume.string(runtime, result).map(Into::into),
            Self::Error(resume) => resume.string(runtime, result).map(Into::into),

            Self::ArraySort(resume) => resume.string(runtime, result).map(Into::into),
            Self::ArrayString(resume) => resume.string(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "string result has no continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn constructor_source(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::vm::call::ConstructorPrototypeSource>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Promise(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::TypedCreate(resume) => resume.prototype(runtime, result).map(Into::into),

            Self::WeakConstructor(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::ObjectConstructor(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::BufferConstructor(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::DataViewConstructor(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::RegExpConstructor(resume) => resume.prototype(runtime, result).map(Into::into),

            Self::Collection(resume) => resume.prototype(runtime, result).map(Into::into),
            Self::IteratorConstructor(resume) => resume.prototype(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "constructor prototype source has no continuation",
            )),
        }
    }
    pub(super) fn element(
        self,
        runtime: &Runtime,
        result: NativeConversion<[u8; 8]>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.element(runtime, result).map(Into::into),

            Self::TypedMutation(resume) => resume.element(runtime, result).map(Into::into),

            Self::TypedSet(resume) => resume.element(runtime, result).map(Into::into),

            Self::TypedIteration(resume) => resume.element(runtime, result).map(Into::into),
            Self::TypedElement(resume) => resume.element(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "element result has no continuation",
            )),
        }
    }
    pub(super) fn typed_species(
        self,
        runtime: &Runtime,
        result: NativeConversion<ObjectRef>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.created(runtime, result).map(Into::into),

            Self::TypedSlice(resume) => resume.species(runtime, result).map(Into::into),

            Self::TypedIteration(resume) => resume.species(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "typed species result has no continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn regexp_species(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::vm::call::ConstructorRef>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RegExpMatchAll(resume) => resume.species(runtime, result).map(Into::into),
            Self::RegExpSplit(resume) => resume.species(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "RegExp species has no continuation",
            )),
        }
    }
}

impl Resume {
    pub(super) fn typed_iterator_method(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<crate::engine::object::CallableRef>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.method(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "typed iterator method has no continuation",
            )),
        }
    }
    pub(super) fn typed_collected(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<Value>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.collected(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "typed collected values have no continuation",
            )),
        }
    }
}
impl Step {
    /// Move the selected domain payload once. The vacant state owns no roots;
    /// a helper installs its next request here before returning Continue.
    pub(super) fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Complete(Completion::Return(Value::Undefined)))
    }
}
