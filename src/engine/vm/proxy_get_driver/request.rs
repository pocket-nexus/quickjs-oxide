//! Closed reply routing between domain states and the owned scheduler.
mod array;
pub(super) mod array_mutation;
mod buffer;
mod conversion;
mod function;
mod iterator;
mod module;
mod native;
mod object;
pub(super) mod set;
pub(in crate::engine::vm) use set::WriteKeyInputs;
mod object_builtins;
mod scalar;
mod string;
mod vm;

use super::{
    BytecodeCallRequest, Completion, DescriptorResume, DescriptorStep, DirectCallTarget, JsValue,
    NativeConversion, ObjectRef, OrdinaryRead, PropertyKey, ProxyBooleanResume, ProxyBooleanStep,
    ProxyGetResume, ProxyGetStep, ProxyOwnResume, ProxyOwnStep, Runtime, Value,
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

pub(in crate::engine::vm) struct BooleanResultPayload {
    pub(super) _object: ObjectRef,
    pub(super) _key: Option<PropertyKey>,
    pub(super) strict_delete: bool,
}
pub(in crate::engine::vm) struct DefineTypedPayload {
    pub(super) object: ObjectRef,
    pub(super) _descriptor: crate::engine::object::OwnedPropertyDescriptor,
    pub(super) resume: Box<Resume>,
}
pub(in crate::engine::vm) struct DefineLengthPayload {
    pub(super) object: ObjectRef,
    pub(super) key: PropertyKey,
    pub(super) descriptor: crate::engine::object::OwnedPropertyDescriptor,
    pub(super) resume: Box<Resume>,
}

pub(in crate::engine::vm) enum Resume {
    ComputedKey,
    PropertyKeyValue,
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
        resume: crate::engine::builtins::ArrayConstructorResume,
    },
    ArraySliceSet {
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
        resume: crate::engine::builtins::ArrayCopyResume,
    },
    ArrayConcatSet {
        resume: crate::engine::builtins::ArrayConcatResume,
    },

    Instance(crate::engine::builtins::InstanceResume),
    IteratorFrom(crate::engine::builtins::IteratorFromResume),
    IteratorWrap(crate::engine::builtins::IteratorWrapResume),
    IteratorConcat(crate::engine::builtins::IteratorConcatResume),
    ArrayBuild(crate::engine::builtins::ArrayBuildResume),
    ArrayBuildSet {
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
        resume: crate::engine::builtins::ArraySortResume,
    },
    ArrayIndexedSet {
        resume: crate::engine::builtins::ArrayIndexedResume,
    },
    ArrayReverseSet {
        resume: crate::engine::builtins::ArrayReverseResume,
    },

    ArrayNext(crate::engine::builtins::ArrayNextResume),
    ArrayMutation(crate::engine::builtins::ArrayMutationResume),
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
        payload: Box<BooleanResultPayload>,
    },
    ReadOwner(ObjectRef),
    Element(ElementResume),
    TypedElement(TypedWriteResume),
    SetTyped(SetResume),
    DefineTyped {
        payload: Box<DefineTypedPayload>,
    },
    Number(NumberResume),
    LengthNumber(ArrayLengthResume),
    SetLength(SetResume),
    DefineLength {
        payload: Box<DefineLengthPayload>,
    },
    WriteKey(set::WriteKeyInputs),
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

/// Allocated only after an actual selected callback must cross a boundary.
pub(in crate::engine::vm) struct SelectedRawCallback {
    pub(in crate::engine::vm) inputs: crate::engine::vm::call::ordinary::RawCallbackInputs,
    pub(in crate::engine::vm) selection: crate::engine::vm::call::ordinary::CallbackSelection,
    pub(in crate::engine::vm) overflow: bool,
    pub(in crate::engine::vm) resume: Resume,
}
/// A published activation crosses only for a genuinely unmigrated body.
pub(in crate::engine::vm) struct PreparedNativeBoundary {
    pub(in crate::engine::vm) call: crate::engine::vm::call::PreparedNativeCall,
    pub(in crate::engine::vm) kind: crate::engine::builtins::continuation::NativeOperation,
    pub(in crate::engine::vm) resume: Resume,
}

pub(in crate::engine::vm) enum Step {
    /// A fresh ToString diagnostic plus its still-owned concrete parent.
    CyclePublishedStringReply {
        value: Option<NativeConversion<crate::engine::value::JsString>>,
        resume: Option<Resume>,
    },
    StringReply {
        value: Option<NativeConversion<crate::engine::value::JsString>>,
        resume: Option<Resume>,
    },
    CallbackBoundary(Option<Box<SelectedRawCallback>>),
    PreparedNativeBoundary(Option<Box<PreparedNativeBoundary>>),
    /// Immediate typed replies do not reserve parent storage merely to cross
    /// an explicitly unmigrated outer consumer.
    NumberReply {
        value: Option<NativeConversion<f64>>,
        resume: Option<Resume>,
    },
    PrimitiveReply {
        value: Option<Completion>,
        resume: Option<Resume>,
    },
    /// This completion owns a freshly published collectible producer. The
    /// canonical State advance consumes the fact before the ordinary reply.
    CyclePublishedPrimitiveReply {
        value: Option<Completion>,
        resume: Option<Resume>,
    },
    ComputedError(Option<crate::engine::api::Error>),
    CyclePublishedNumber(Option<NativeConversion<f64>>),
    CyclePublishedElement(Option<NativeConversion<[u8; 8]>>),
    RawRead {
        read: Option<crate::engine::object::ReadStep>,
        key: crate::engine::atom::Atom,
        resume: Option<Resume>,
    },
    CyclePublishedComplete(Option<Completion>),
    CyclePublishedPrimitive {
        value: Option<JsValue>,
        hint: Option<ToPrimitiveHint>,
        resume: Option<Resume>,
    },
    RawValueReadRequest {
        realm: crate::engine::heap::ContextId,
        selected: Option<crate::engine::object::ReadStep>,
        key: crate::engine::atom::Atom,
        receiver: Option<JsValue>,
        resume: Option<Resume>,
    },
    RegExpExecProgress(Option<crate::engine::builtins::RegExpExecStep>),
    ArrayMutationProgress(Option<crate::engine::builtins::ArrayMutationStep>),
    ArrayMutationRead {
        read: Option<crate::engine::object::ReadStep>,
        resume: Option<crate::engine::builtins::ArrayMutationResume>,
    },
    ArrayMutationSharedDelete {
        word: Option<crate::engine::builtins::SharedTypedOwnWord>,
        resume: Option<crate::engine::builtins::ArrayMutationResume>,
    },
    RawReadRequest {
        selected: Option<crate::engine::object::ReadStep>,
        object: Option<crate::engine::heap::ObjectId>,
        key: crate::engine::atom::Atom,
        receiver: Option<JsValue>,
        resume: Option<Resume>,
    },
    RawCall {
        inputs: Option<crate::engine::vm::call::ordinary::RawCallbackInputs>,
        resume: Option<Resume>,
    },
    PrimitiveProgress(Option<crate::engine::value::conversion::primitive::PrimitiveStep>),
    NumberProgress(Option<NumberStep>),
    RootDescriptor(Option<crate::engine::vm::entry::DescriptorReply>),
    ModuleCallbackOperation {
        step: Option<Box<crate::engine::modules::callback::CallbackStep>>,
        resume: Option<Resume>,
    },
    ModuleBodyOperation {
        step: Option<Box<crate::engine::modules::body::BodyStep>>,
        resume: Option<Resume>,
    },
    ModuleLink {
        realm: Option<crate::engine::heap::ContextId>,
        callable: Option<crate::engine::object::CallableRef>,
        resume: Option<Resume>,
    },
    PromiseOperation {
        step: Option<Box<crate::engine::builtins::promise::operation::PromiseStep>>,
        resume: Option<Resume>,
    },
    IntrinsicPromiseResolve {
        value: Option<JsValue>,
        realm: Option<crate::engine::heap::ContextId>,
        resume: Option<Resume>,
    },
    ResumeFrame {
        activation: Option<Box<crate::engine::vm::suspend::RootedVmActivation>>,
        input: Option<crate::engine::vm::suspend::VmActivationResume>,
        resume: Option<Resume>,
    },
    ForInComplete {
        value: Option<JsValue>,
        done: Option<Option<bool>>,
    },
    TypedIteratorMethod {
        source: Option<JsValue>,
        resume: Option<Resume>,
    },
    TypedIteratorMethodComplete(
        Option<NativeConversion<Option<crate::engine::object::CallableRef>>>,
    ),
    TypedCollect {
        source: Option<JsValue>,
        method: Option<crate::engine::object::CallableRef>,
        element: Option<TypedArrayElementKind>,
        resume: Option<Resume>,
    },
    TypedCollectComplete(Option<NativeConversion<Vec<JsValue>>>),
    TypedCreate {
        constructor: Option<JsValue>,
        length: Option<u64>,
        resume: Option<Resume>,
    },
    NumericComplete {
        value: Option<JsValue>,
        previous: Option<Option<JsValue>>,
    },
    TypedSpeciesView {
        source: Option<ObjectRef>,
        element: Option<TypedArrayElementKind>,
        buffer: Option<ObjectRef>,
        offset: Option<u64>,
        length: Option<Option<u64>>,
        resume: Option<Resume>,
    },
    RegExpSpecies {
        regexp: Option<ObjectRef>,
        resume: Option<Resume>,
    },
    RegExpSpeciesComplete(Option<NativeConversion<crate::engine::vm::call::ConstructorRef>>),
    IndirectEval {
        source: Option<crate::engine::value::JsString>,
        resume: Option<Resume>,
    },
    Aggregate {
        iterable: Option<JsValue>,
        resume: Option<Resume>,
    },
    OrdinaryPrimitive {
        object: Option<crate::engine::heap::ObjectId>,
        hint: Option<ToPrimitiveHint>,
    },
    ConstructorSource {
        new_target: Option<JsValue>,
        resume: Option<Resume>,
    },
    ConstructorSourceComplete(
        Option<NativeConversion<crate::engine::vm::call::ConstructorPrototypeSource>>,
    ),
    TypedSpecies {
        source: Option<ObjectRef>,
        element: Option<TypedArrayElementKind>,
        length: Option<u64>,
        resume: Option<Resume>,
    },
    TypedSpeciesComplete(Option<NativeConversion<ObjectRef>>),
    ArrayCopy {
        object: Option<ObjectRef>,
        to: Option<u64>,
        from: Option<u64>,
        count: Option<u64>,
        backwards: Option<bool>,
        resume: Option<Resume>,
    },
    OrdinaryInstance {
        constructor: Option<crate::engine::object::CallableRef>,
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
    ParseIterator {
        result: Option<Completion>,
        resume: Option<Resume>,
    },
    String {
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
    ObjectTag {
        receiver: Option<JsValue>,
    },
    RegExpExec {
        regexp: Option<JsValue>,
        input: Option<JsValue>,
        resume: Option<Resume>,
    },
    IteratorCloseWithResume {
        iterator: Option<ObjectRef>,
        completion: Option<Completion>,
        resume: Option<Resume>,
    },
    NativeRawComplete(Option<crate::engine::vm::call::NativeInvokeOutcome>),
    ArraySpecies {
        source: Option<ObjectRef>,
        length: Option<u64>,
        resume: Option<Resume>,
    },
    ArrayPush {
        object: Option<crate::engine::heap::ObjectId>,
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
    IteratorNext {
        iterator: Option<ObjectRef>,
        method: Option<JsValue>,
        resume: Option<Resume>,
    },
    IteratorNextComplete(Option<crate::engine::builtins::ObjectIteratorStep>),
    IteratorCall {
        callable: Option<crate::engine::object::CallableRef>,
        iterator: Option<ObjectRef>,
        resume: Option<crate::engine::builtins::IteratorNextResume>,
    },
    IteratorClose {
        iterator: Option<ObjectRef>,
        completion: Option<Completion>,
    },
    Native {
        callable: Option<crate::engine::object::CallableRef>,
        target: Option<crate::engine::builtins::native::NativeFunctionId>,
        defining_realm: Option<crate::engine::heap::ContextId>,
        min_readable_args: Option<u8>,
        mode: Option<crate::engine::vm::call::NativeInvokeMode>,
        invocation: Option<crate::engine::vm::call::NativeInvocation>,
        arguments: Option<Vec<JsValue>>,
        resume: Option<Resume>,
    },
    Construct {
        target: Option<crate::engine::vm::call::ConstructorRef>,
        new_target: Option<crate::engine::vm::call::ConstructNewTarget>,
        arguments: Option<Vec<JsValue>>,
        resume: Option<Resume>,
    },
    ConstructProxy {
        target: Option<crate::engine::vm::call::ConstructorRef>,
        new_target: Option<crate::engine::vm::call::ConstructNewTarget>,
        arguments: Option<Vec<JsValue>>,
        resume: Option<Resume>,
    },
    ConstructorReady {
        request: Option<Box<BytecodeCallRequest>>,
        receiver: Option<Completion>,
        derived: Option<bool>,
        resume: Option<Resume>,
    },
    Arguments {
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
    ArgumentsComplete(Option<NativeConversion<Vec<JsValue>>>),
    ArgumentsProgress(Option<crate::engine::builtins::ArgumentsStep>),
    InvokeProgress(Option<crate::engine::builtins::InvokeStep>),
    ArgumentsReply {
        value: Option<NativeConversion<Vec<JsValue>>>,
        resume: Option<Resume>,
    },
    SnapshotEnumerable {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    OwnFlag {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        enumerable: Option<bool>,
        resume: Option<Resume>,
    },
    Keys {
        object: Option<ObjectRef>,
        resume: Option<Resume>,
    },
    KeysComplete(Option<NativeConversion<Vec<PropertyKey>>>),
    ReadValue {
        receiver: Option<JsValue>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    PreparedHas {
        probe: Option<crate::engine::object::PreparedHas>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    PreparedRead {
        read: Option<OrdinaryRead>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    Primitive {
        value: Option<JsValue>,
        hint: Option<crate::engine::vm::ToPrimitiveHint>,
        resume: Option<Resume>,
    },
    GetPrototype {
        object: Option<ObjectRef>,
        resume: Option<Resume>,
    },
    SetPrototype {
        object: Option<ObjectRef>,
        prototype: Option<Option<ObjectRef>>,
        resume: Option<Resume>,
    },
    Delete {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    PreventExtensions {
        object: Option<ObjectRef>,
        resume: Option<Resume>,
    },
    Element {
        element: Option<TypedArrayElementKind>,
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
    ElementComplete(Option<NativeConversion<[u8; 8]>>),
    TypedComplete(Option<NativeConversion<bool>>),
    Number {
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
    NumberComplete(Option<NativeConversion<f64>>),
    LengthComplete(Option<ArrayLengthConversion>),
    SetLength {
        value: Option<JsValue>,
        resume: Option<SetResume>,
    },
    /// The actual VM receiver/value own their transferred operand edges. The
    /// key is borrowed from the linked executable or owned final continuation.
    ValueSet {
        atom: crate::engine::atom::Atom,
        value: Option<JsValue>,
        receiver: Option<JsValue>,
    },
    WriteOperands {
        atom: Option<crate::engine::atom::Atom>,
        input: Option<set::WriteKeyInputs>,
    },
    SetProgress(Option<crate::engine::object::SetProgress>),
    // A real already-selected Set child keeps PreparedSet's budget/reserve
    // prefix. Direct VM ValueSet has no such prefix.
    PreparedSetProgress {
        progress: Option<crate::engine::object::SetProgress>,
        resume: Option<Resume>,
    },
    SetReply {
        action: Option<crate::engine::object::SetAction>,
        resume: Option<Resume>,
    },
    WriteError(Option<crate::engine::api::Error>),
    SetComplete(Option<PropertySetAction>),
    PreparedSet {
        step: Option<Box<SetStep>>,
        resume: Option<Resume>,
    },
    Set {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        value: Option<JsValue>,
        receiver: Option<JsValue>,
        resume: Option<Resume>,
    },
    SetProxy {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        value: Option<JsValue>,
        receiver: Option<JsValue>,
        resume: Option<Resume>,
    },
    Define {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        descriptor: Option<crate::engine::object::DefinitionInput>,
        resume: Option<Resume>,
    },
    DefineOrdinary {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        descriptor: Option<crate::engine::object::DefinitionInput>,
        resume: Option<Resume>,
    },
    Defined(Option<NativeConversion<InternalDefineResult>>),
    Complete(Option<Completion>),
    BooleanComplete(Option<NativeConversion<bool>>),
    OwnComplete(
        Option<NativeConversion<Option<crate::engine::object::OwnedCompletePropertyDescriptor>>>,
    ),
    Converted(Option<NativeConversion<crate::engine::object::OwnedPropertyDescriptor>>),
    Has {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    Read {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        receiver: Option<JsValue>,
        resume: Option<Resume>,
    },
    Call {
        target: Option<DirectCallTarget>,
        receiver: Option<JsValue>,
        arguments: Option<Vec<JsValue>>,
        resume: Option<Resume>,
    },
    Descriptor {
        object: Option<ObjectRef>,
        key: Option<PropertyKey>,
        resume: Option<Resume>,
    },
    Extensible {
        object: Option<ObjectRef>,
        resume: Option<Resume>,
    },
    Convert {
        value: Option<JsValue>,
        resume: Option<Resume>,
    },
}

impl Step {
    pub(super) fn has_raw_owner(&self) -> bool {
        match self {
            Self::ValueSet { .. }
            | Self::WriteOperands { .. }
            | Self::PreparedSetProgress { .. }
            | Self::SetProgress(Some(_))
            | Self::SetReply { .. } => true,
            Self::ArgumentsProgress(Some(_))
            | Self::InvokeProgress(Some(_))
            | Self::ArgumentsReply { .. }
            | Self::PrimitiveProgress(Some(_))
            | Self::NumberProgress(Some(_))
            | Self::NumberReply { .. }
            | Self::StringReply { .. }
            | Self::CyclePublishedStringReply { .. }
            | Self::PrimitiveReply { .. }
            | Self::CyclePublishedPrimitiveReply { .. }
            | Self::RawRead { .. }
            | Self::RawReadRequest { .. }
            | Self::RawValueReadRequest { .. }
            | Self::ArrayPush { .. }
            | Self::RegExpExecProgress(_)
            | Self::RegExpExec { .. }
            | Self::ArrayMutationProgress(_)
            | Self::ArrayMutationRead { .. }
            | Self::ArrayMutationSharedDelete { .. }
            | Self::CyclePublishedComplete(_)
            | Self::CyclePublishedPrimitive { .. }
            | Self::OrdinaryPrimitive { .. }
            | Self::RawCall { .. }
            | Self::CallbackBoundary(_)
            | Self::PreparedNativeBoundary(_)
            | Self::CyclePublishedNumber(Some(_))
            | Self::CyclePublishedElement(Some(_)) => true,
            Self::Arguments { resume, .. }
            | Self::String { resume, .. }
            | Self::Primitive { resume, .. }
            | Self::Number { resume, .. }
            | Self::Call { resume, .. }
            | Self::Read { resume, .. } => resume.as_ref().is_some_and(Resume::has_raw_owner),
            _ => false,
        }
    }

    /// Drain a request abandoned before its consumer takes the owned fields.
    /// Replacing with an empty terminal makes cleanup safe after partial takes.
    pub(super) fn release_owned(&mut self, runtime: &Runtime) {
        let release = |value| {
            if !runtime.skip_cleanup() {
                let _ = runtime.release_jsvalue(value);
            }
        };
        let release_completion = |value| {
            let (Completion::Return(value) | Completion::Throw(value)) = value;
            release(value);
        };
        match std::mem::replace(self, Self::Complete(None)) {
            Self::WriteOperands { atom, input } => {
                if let Some(mut input) = input
                    && input.retire_at_boundary(runtime).is_err()
                {
                    return;
                }
                if !runtime.skip_cleanup()
                    && let Some(atom) = atom
                {
                    release(JsValue::Symbol(crate::engine::atom::AtomIdx::from_raw(
                        atom.raw(),
                    )));
                }
            }
            Self::ValueSet {
                value, receiver, ..
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if !runtime.skip_cleanup()
                    && let Some(receiver) = receiver
                {
                    release(receiver);
                }
            }
            Self::SetProgress(value) => {
                if let Some(value) = value {
                    let _ = value.retire_at_boundary(runtime);
                }
            }
            Self::PreparedSetProgress { progress, resume } => {
                if let Some(progress) = progress
                    && progress.retire_at_boundary(runtime).is_err()
                {
                    return;
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::SetReply { action, resume } => {
                if let Some(action) = action
                    && action.retire_at_boundary(runtime).is_err()
                {
                    return;
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::WriteError(_) => {}
            Self::ComputedError(_) => {}
            Self::CallbackBoundary(value) => {
                if let Some(mut value) = value {
                    let _ = value.inputs.retire_at_boundary(runtime);
                    value.resume.release_owned(runtime);
                }
            }
            Self::PreparedNativeBoundary(value) => {
                if let Some(value) = value {
                    value.call.abandon_at_boundary(runtime);
                    value.resume.release_owned(runtime);
                }
            }
            Self::StringReply { value, resume }
            | Self::CyclePublishedStringReply { value, resume } => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::NumberReply { value, resume } => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::PrimitiveReply { value, resume }
            | Self::CyclePublishedPrimitiveReply { value, resume } => {
                if let Some(value) = value {
                    release_completion(value);
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::RegExpExecProgress(progress) => {
                if let Some(progress) = progress {
                    let _ = progress.retire_at_boundary(runtime);
                }
            }
            Self::ArrayMutationProgress(progress) => {
                if let Some(progress) = progress {
                    let _ = progress.retire_at_boundary(runtime);
                }
            }
            Self::ArrayMutationRead { read, resume } => {
                if let Some(
                    crate::engine::object::ReadStep::Ready(read)
                    | crate::engine::object::ReadStep::CyclePublished(read),
                ) = read
                {
                    if read.retire_at_boundary(runtime).is_err() {
                        return;
                    }
                }
                if let Some(resume) = resume {
                    let _ = resume.retire_at_boundary(runtime);
                }
            }
            Self::ArrayMutationSharedDelete { resume, .. } => {
                if let Some(resume) = resume {
                    let _ = resume.retire_at_boundary(runtime);
                }
            }
            Self::RawValueReadRequest {
                selected,
                receiver,
                resume,
                ..
            } => {
                if let Some(
                    crate::engine::object::ReadStep::Ready(read)
                    | crate::engine::object::ReadStep::CyclePublished(read),
                ) = selected
                {
                    if read.retire_at_boundary(runtime).is_err() {
                        return;
                    }
                }
                if let Some(receiver) = receiver {
                    if runtime.release_jsvalue(receiver).is_err() || runtime.is_poisoned() {
                        return;
                    }
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::RawReadRequest {
                selected,
                object,
                receiver,
                resume,
                ..
            } => {
                if let Some(
                    crate::engine::object::ReadStep::Ready(read)
                    | crate::engine::object::ReadStep::CyclePublished(read),
                ) = selected
                {
                    if read.retire_at_boundary(runtime).is_err() {
                        return;
                    }
                }
                if let Some(receiver) = receiver {
                    if runtime.release_jsvalue(receiver).is_err() || runtime.is_poisoned() {
                        return;
                    }
                }
                if let Some(object) = object {
                    if runtime.release_jsvalue(JsValue::Object(object)).is_err()
                        || runtime.is_poisoned()
                    {
                        return;
                    }
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::RawRead { read, resume, .. } => {
                if let Some(read) = read {
                    match read {
                        crate::engine::object::ReadStep::Ready(read)
                        | crate::engine::object::ReadStep::CyclePublished(read) => match read {
                            crate::engine::object::OwnedRead::Complete(Some(value)) => {
                                release(value)
                            }
                            crate::engine::object::OwnedRead::Complete(None) => {}
                            crate::engine::object::OwnedRead::Getter { function, receiver }
                            | crate::engine::object::OwnedRead::Proxy {
                                object: function,
                                receiver,
                            } => {
                                release(JsValue::Object(function));
                                release(receiver);
                            }
                        },
                        crate::engine::object::ReadStep::Shared(_) => {}
                    }
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::RawCall { inputs, resume } => {
                if let Some(mut inputs) = inputs {
                    let _ = inputs.retire_at_boundary(runtime);
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::PrimitiveProgress(step) => {
                if let Some(step) = step {
                    match step {
                        crate::engine::value::conversion::primitive::PrimitiveStep::Complete(value)
                        | crate::engine::value::conversion::primitive::PrimitiveStep::CyclePublished(value) => release_completion(value),
                        crate::engine::value::conversion::primitive::PrimitiveStep::Get {
                            resume,
                        }
                        | crate::engine::value::conversion::primitive::PrimitiveStep::Call {
                            resume,
                        } => resume.release_owned(runtime),
                    }
                }
            }
            Self::NumberProgress(step) => {
                if let Some(step) = step {
                    match step {
                        crate::engine::value::conversion::number::NumberStep::Complete(
                            NativeConversion::Throw(value),
                        )
                        | crate::engine::value::conversion::number::NumberStep::CyclePublished(
                            NativeConversion::Throw(value),
                        ) => release(value),
                        crate::engine::value::conversion::number::NumberStep::Complete(
                            NativeConversion::Value(_),
                        )
                        | crate::engine::value::conversion::number::NumberStep::CyclePublished(
                            NativeConversion::Value(_),
                        ) => {}
                        crate::engine::value::conversion::number::NumberStep::Read { resume }
                        | crate::engine::value::conversion::number::NumberStep::Call { resume } => {
                            resume.release_owned(runtime)
                        }
                    }
                }
            }
            Self::RootDescriptor(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::ModuleCallbackOperation { step, resume } => {
                if let Some(value) = step {
                    value.release(runtime);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ModuleBodyOperation { step, resume } => {
                if let Some(value) = step {
                    value.release(runtime);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ModuleLink {
                realm: _,
                callable: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::PromiseOperation { step, resume } => {
                if let Some(value) = step {
                    value.release(runtime);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::IntrinsicPromiseResolve {
                value,
                realm: _,
                resume,
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ResumeFrame {
                activation: _,
                input,
                resume,
            } => {
                if let Some(value) = input {
                    use crate::engine::vm::VmResume;
                    use crate::engine::vm::suspend::VmActivationResume;
                    match value {
                        VmActivationResume::Initial => {}
                        VmActivationResume::AwaitFulfill(value)
                        | VmActivationResume::AwaitReject(value)
                        | VmActivationResume::Generator(
                            VmResume::Next(value)
                            | VmResume::Return(value)
                            | VmResume::Throw(value),
                        ) => release(value),
                    }
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ForInComplete { value, done: _ } => {
                if let Some(value) = value {
                    release(value);
                }
            }
            Self::TypedIteratorMethod { source, resume } => {
                if let Some(value) = source {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::TypedIteratorMethodComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::TypedCollect {
                source,
                method: _,
                element: _,
                resume,
            } => {
                if let Some(value) = source {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::TypedCollectComplete(value) => {
                if let Some(value) = value {
                    match value {
                        NativeConversion::Value(values) => {
                            for value in values {
                                release(value);
                            }
                        }
                        NativeConversion::Throw(value) => release(value),
                    }
                }
            }
            Self::TypedCreate {
                constructor,
                length: _,
                resume,
            } => {
                if let Some(value) = constructor {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::NumericComplete { value, previous } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(Some(value)) = previous {
                    release(value);
                }
            }
            Self::TypedSpeciesView {
                source: _,
                element: _,
                buffer: _,
                offset: _,
                length: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::RegExpSpecies { regexp: _, resume } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::RegExpSpeciesComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::IndirectEval { source: _, resume } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Aggregate { iterable, resume } => {
                if let Some(value) = iterable {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::OrdinaryPrimitive { object, .. } => {
                if let Some(object) = object {
                    release(JsValue::Object(object));
                }
            }
            Self::ConstructorSource { new_target, resume } => {
                if let Some(value) = new_target {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ConstructorSourceComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::TypedSpecies {
                source: _,
                element: _,
                length: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::TypedSpeciesComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::ArrayCopy {
                object: _,
                to: _,
                from: _,
                count: _,
                backwards: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::OrdinaryInstance {
                constructor: _,
                value,
                resume,
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ParseIterator { result, resume } => {
                if let Some(value) = result {
                    release_completion(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::String { value, resume } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ObjectTag { receiver } => {
                if let Some(value) = receiver {
                    release(value);
                }
            }
            Self::RegExpExec {
                regexp,
                input,
                resume,
            } => {
                if let Some(value) = regexp {
                    release(value);
                }
                if let Some(value) = input {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::IteratorCloseWithResume {
                iterator: _,
                completion,
                resume,
            } => {
                if let Some(value) = completion {
                    release_completion(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::NativeRawComplete(value) => {
                if let Some(value) = value {
                    match value {
                        crate::engine::vm::call::NativeInvokeOutcome::Completion(value) => {
                            release_completion(value)
                        }
                        crate::engine::vm::call::NativeInvokeOutcome::IteratorNextRaw {
                            value,
                            ..
                        } => release(value),
                    }
                }
            }
            Self::ArraySpecies {
                source: _,
                length: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ArrayPush {
                object,
                value,
                resume,
            } => {
                if let Some(value) = value {
                    if runtime.release_jsvalue(value).is_err() || runtime.is_poisoned() {
                        return;
                    }
                }
                if let Some(object) = object {
                    if runtime.release_jsvalue(JsValue::Object(object)).is_err()
                        || runtime.is_poisoned()
                    {
                        return;
                    }
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::IteratorNext {
                iterator: _,
                method,
                resume,
            } => {
                if let Some(value) = method {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::IteratorNextComplete(value) => {
                if let Some(value) = value {
                    match value {
                        crate::engine::builtins::ObjectIteratorStep::Yield(value)
                        | crate::engine::builtins::ObjectIteratorStep::Throw(value) => {
                            release(value)
                        }
                        crate::engine::builtins::ObjectIteratorStep::Done => {}
                    }
                }
            }
            Self::IteratorCall {
                callable: _,
                iterator: _,
                resume: _,
            } => {}
            Self::IteratorClose {
                iterator: _,
                completion,
            } => {
                if let Some(value) = completion {
                    release_completion(value);
                }
            }
            Self::Native {
                callable: _,
                target: _,
                defining_realm: _,
                min_readable_args: _,
                mode: _,
                invocation,
                arguments,
                resume,
            } => {
                if let Some(value) = invocation {
                    let _ = value.release(runtime);
                }
                if let Some(values) = arguments {
                    for value in values {
                        release(value);
                    }
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Construct {
                target: _,
                new_target,
                arguments,
                resume,
            } => {
                if let Some(value) = new_target {
                    let _ = value.release(runtime);
                }
                if let Some(values) = arguments {
                    for value in values {
                        release(value);
                    }
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ConstructProxy {
                target: _,
                new_target,
                arguments,
                resume,
            } => {
                if let Some(value) = new_target {
                    let _ = value.release(runtime);
                }
                if let Some(values) = arguments {
                    for value in values {
                        release(value);
                    }
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ConstructorReady {
                request,
                receiver,
                derived: _,
                resume,
            } => {
                if let Some(mut value) = request {
                    let _ = value.release_owned_values(runtime);
                }
                if let Some(value) = receiver {
                    release_completion(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ArgumentsProgress(step) => {
                if let Some(step) = step {
                    let _ = step.retire_at_boundary(runtime);
                }
            }
            Self::InvokeProgress(step) => {
                if let Some(step) = step {
                    let _ = step.retire_at_boundary(runtime);
                }
            }
            Self::ArgumentsReply { value, resume } => {
                if let Some(value) = value {
                    match value {
                        NativeConversion::Value(values) => {
                            for value in values {
                                release(value);
                            }
                        }
                        NativeConversion::Throw(value) => release(value),
                    }
                }
                if let Some(resume) = resume {
                    resume.release_owned(runtime);
                }
            }
            Self::Arguments { value, resume } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ArgumentsComplete(value) => {
                if let Some(value) = value {
                    match value {
                        NativeConversion::Value(values) => {
                            for value in values {
                                release(value);
                            }
                        }
                        NativeConversion::Throw(value) => release(value),
                    }
                }
            }
            Self::SnapshotEnumerable {
                object: _,
                key: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::OwnFlag {
                object: _,
                key: _,
                enumerable: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Keys { object: _, resume } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::KeysComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::ReadValue {
                receiver,
                key: _,
                resume,
            } => {
                if let Some(value) = receiver {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::PreparedHas {
                probe: _,
                key: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::PreparedRead {
                read,
                key: _,
                resume,
            } => {
                if let Some(value) = read {
                    value.release(runtime);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Primitive {
                value,
                hint: _,
                resume,
            }
            | Self::CyclePublishedPrimitive {
                value,
                hint: _,
                resume,
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::GetPrototype { object: _, resume } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::SetPrototype {
                object: _,
                prototype: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Delete {
                object: _,
                key: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::PreventExtensions { object: _, resume } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Element {
                element: _,
                value,
                resume,
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::ElementComplete(value) | Self::CyclePublishedElement(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::TypedComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::Number { value, resume } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::NumberComplete(value) | Self::CyclePublishedNumber(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::LengthComplete(value) => {
                if let Some(ArrayLengthConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::SetLength { value, resume } => {
                if let Some(value) = value {
                    release(value);
                }
                if !runtime.skip_cleanup()
                    && let Some(resume) = resume
                {
                    let _ = resume.retire_at_boundary(runtime);
                }
            }
            Self::SetComplete(value) => {
                if let Some(value) = value {
                    SetStep::Complete(value).release(runtime);
                }
            }
            Self::PreparedSet { step, resume } => {
                if let Some(value) = step {
                    value.release(runtime);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Set {
                object: _,
                key: _,
                value,
                receiver,
                resume,
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = receiver {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::SetProxy {
                object: _,
                key: _,
                value,
                receiver,
                resume,
            } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = receiver {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Define {
                object: _,
                key: _,
                descriptor: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::DefineOrdinary {
                object: _,
                key: _,
                descriptor: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Defined(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::Complete(value) | Self::CyclePublishedComplete(value) => {
                if let Some(value) = value {
                    release_completion(value);
                }
            }
            Self::BooleanComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::OwnComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::Converted(value) => {
                if let Some(NativeConversion::Throw(value)) = value {
                    release(value);
                }
            }
            Self::Has {
                object: _,
                key: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Read {
                object: _,
                key: _,
                receiver,
                resume,
            } => {
                if let Some(value) = receiver {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Call {
                target: _,
                receiver,
                arguments,
                resume,
            } => {
                if let Some(value) = receiver {
                    release(value);
                }
                if let Some(values) = arguments {
                    for value in values {
                        release(value);
                    }
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Descriptor {
                object: _,
                key: _,
                resume,
            } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Extensible { object: _, resume } => {
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
            Self::Convert { value, resume } => {
                if let Some(value) = value {
                    release(value);
                }
                if let Some(value) = resume {
                    value.release_owned(runtime);
                }
            }
        }
    }
}

impl Resume {
    /// Raw domains retire with the caller's current State. Legacy outer
    /// domains remain present until their actual boundary owner is released.
    pub(super) fn retire_raw_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<(), crate::engine::api::RuntimeError> {
        match self {
            Self::WriteKey(input) => input.retire_in_state(state, poisoned),
            Self::Primitive(_)
            | Self::Number(_)
            | Self::Numeric(_)
            | Self::NumericPrimitive(_)
            | Self::Math(_)
            | Self::RegExpExec(_)
            | Self::ScalarText(_)
            | Self::OrdinarySet(_)
            | Self::SetTyped(_)
            | Self::SetLength(_)
            | Self::DatePrototype(_)
            | Self::ArrayMutation(_)
            | Self::Arguments(_)
            | Self::Invoke(_)
            | Self::DateConstructor(_)
            | Self::DateConstructorPrimitive(_) => match std::mem::replace(self, Self::Identity) {
                Self::Primitive(resume) => resume.retire_in_state(state, poisoned),
                Self::Number(resume) => resume.retire_in_state(state, poisoned),
                Self::Numeric(resume) | Self::NumericPrimitive(resume) => {
                    resume.retire_in_state(state, poisoned)
                }
                Self::Math(resume) => resume.retire_in_state(state, poisoned),
                Self::ScalarText(resume) => resume.retire_in_state(state, poisoned),
                Self::RegExpExec(resume) => resume.retire_in_state(state, poisoned),
                Self::DatePrototype(resume) => resume.retire_in_state(state, poisoned),
                Self::Arguments(resume) => resume.retire_in_state(state, poisoned),
                Self::Invoke(resume) => resume.retire_in_state(state, poisoned),
                Self::DateConstructor(resume) | Self::DateConstructorPrimitive(resume) => {
                    resume.retire_in_state(state, poisoned)
                }
                Self::OrdinarySet(resume) | Self::SetTyped(resume) | Self::SetLength(resume) => {
                    resume.retire_in_state(state, poisoned)
                }
                Self::ArrayMutation(resume) => resume.retire_in_state(state, poisoned),
                _ => unreachable!(),
            },
            Self::StringValue { resume, .. }
            | Self::OwnFlagReply { resume, .. }
            | Self::PrototypeGetReply(resume)
            | Self::PrototypeSetReply(resume)
            | Self::ConstructorPrototype { resume, .. } => {
                resume.retire_raw_in_state(state, poisoned)
            }
            Self::DefineTyped { payload } => payload.resume.retire_raw_in_state(state, poisoned),
            Self::DefineLength { payload } => payload.resume.retire_raw_in_state(state, poisoned),
            _ => Ok(()),
        }
    }
    fn retire_raw_at_boundary(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), crate::engine::api::RuntimeError> {
        match self {
            Self::WriteKey(input) => input.retire_at_boundary(runtime),
            Self::Primitive(_)
            | Self::Number(_)
            | Self::Numeric(_)
            | Self::NumericPrimitive(_)
            | Self::Math(_)
            | Self::RegExpExec(_)
            | Self::ScalarText(_)
            | Self::OrdinarySet(_)
            | Self::SetTyped(_)
            | Self::SetLength(_)
            | Self::DatePrototype(_)
            | Self::ArrayMutation(_)
            | Self::Arguments(_)
            | Self::Invoke(_)
            | Self::DateConstructor(_)
            | Self::DateConstructorPrimitive(_) => match std::mem::replace(self, Self::Identity) {
                Self::Primitive(resume) => resume.retire_at_boundary(runtime),
                Self::Number(resume) => resume.retire_at_boundary(runtime),
                Self::Numeric(resume) | Self::NumericPrimitive(resume) => {
                    resume.retire_at_boundary(runtime)
                }
                Self::Math(resume) => resume.retire_at_boundary(runtime),
                Self::ScalarText(resume) => resume.retire_at_boundary(runtime),
                Self::RegExpExec(resume) => resume.retire_at_boundary(runtime),
                Self::DatePrototype(resume) => resume.retire_at_boundary(runtime),
                Self::Arguments(resume) => resume.retire_at_boundary(runtime),
                Self::Invoke(resume) => resume.retire_at_boundary(runtime),
                Self::DateConstructor(resume) | Self::DateConstructorPrimitive(resume) => {
                    resume.retire_at_boundary(runtime)
                }
                Self::OrdinarySet(resume) | Self::SetTyped(resume) | Self::SetLength(resume) => {
                    resume.retire_at_boundary(runtime)
                }
                Self::ArrayMutation(resume) => resume.retire_at_boundary(runtime),
                _ => unreachable!(),
            },
            Self::StringValue { resume, .. }
            | Self::OwnFlagReply { resume, .. }
            | Self::PrototypeGetReply(resume)
            | Self::PrototypeSetReply(resume)
            | Self::ConstructorPrototype { resume, .. } => resume.retire_raw_at_boundary(runtime),
            Self::DefineTyped { payload } => payload.resume.retire_raw_at_boundary(runtime),
            Self::DefineLength { payload } => payload.resume.retire_raw_at_boundary(runtime),
            _ => Ok(()),
        }
    }
    pub(super) fn has_raw_owner(&self) -> bool {
        match self {
            Self::WriteKey(_) => true,
            Self::Primitive(_)
            | Self::Number(_)
            | Self::Numeric(_)
            | Self::NumericPrimitive(_)
            | Self::Math(_)
            | Self::RegExpExec(_)
            | Self::ScalarText(_)
            | Self::OrdinarySet(_)
            | Self::SetTyped(_)
            | Self::SetLength(_)
            | Self::DatePrototype(_)
            | Self::ArrayMutation(_)
            | Self::Arguments(_)
            | Self::Invoke(_)
            | Self::DateConstructor(_)
            | Self::DateConstructorPrimitive(_) => true,
            Self::StringValue { resume, .. }
            | Self::OwnFlagReply { resume, .. }
            | Self::PrototypeGetReply(resume)
            | Self::PrototypeSetReply(resume)
            | Self::ConstructorPrototype { resume, .. } => resume.has_raw_owner(),
            Self::DefineTyped { payload } => payload.resume.has_raw_owner(),
            Self::DefineLength { payload } => payload.resume.has_raw_owner(),
            _ => false,
        }
    }
    pub(super) fn release_owned(self, runtime: &Runtime) {
        self.release_owned_at_boundary(Some(runtime));
    }
    /// The Query owns the single Weak capability. A dead Runtime already
    /// quarantined its raw execution owners; raw records then drop as scalars.
    pub(super) fn release_owned_at_boundary(mut self, runtime: Option<&Runtime>) {
        // Legacy-only continuations do not acquire State. Wrapped raw children
        // are retired together under one lease, before legacy boundary cleanup.
        if self.has_raw_owner()
            && let Some(runtime) = runtime
            && !runtime.skip_cleanup()
        {
            let _unwind = runtime.unwind_guard();
            let result = if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
                self.retire_raw_in_state(&mut state, &runtime.0.poisoned)
            } else {
                // A normal external Query drop may overlap an admitted lease.
                // Use the existing reference coordinator, in the same owner order.
                self.retire_raw_at_boundary(runtime)
            };
            if result.is_err() {
                return;
            }
        }
        // This match executes only after the lease above ended. Legacy root
        // Drops cannot run under a held State borrow.
        match self {
            Self::ConstructorPrototype {
                mut request,
                resume,
            } => {
                let request_runtime = request.callable.as_object().runtime().clone();
                if !request_runtime.skip_cleanup() {
                    let _ = request.release_owned_values(&request_runtime);
                }
                resume.release_owned_at_boundary(runtime.or(Some(&request_runtime)));
            }
            Self::StringValue { resume, .. }
            | Self::OwnFlagReply { resume, .. }
            | Self::PrototypeGetReply(resume)
            | Self::PrototypeSetReply(resume) => resume.release_owned_at_boundary(runtime),
            Self::DefineTyped { payload } => payload.resume.release_owned_at_boundary(runtime),
            Self::DefineLength { payload } => payload.resume.release_owned_at_boundary(runtime),
            _ => {}
        }
    }
    pub(super) fn can_resume_in_state(&self) -> bool {
        matches!(
            self,
            Self::Primitive(_)
                | Self::RegExpExec(_)
                | Self::Number(_)
                | Self::NumericPrimitive(_)
                | Self::DatePrototype(_)
                | Self::Arguments(_)
                | Self::DateConstructor(_)
                | Self::DateConstructorPrimitive(_)
                | Self::Setter
                | Self::WriteKey(_)
                | Self::ArrayMutation(_)
                | Self::Identity
                | Self::ComputedKey
                | Self::PropertyKeyValue
        )
    }
    pub(in crate::engine::vm) fn resume_in_state(
        self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        host: &dyn crate::engine::host::HostServices,
        completion: Completion,
    ) -> Result<Step, crate::engine::api::RuntimeError> {
        match self {
            Self::RegExpExec(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::WriteKey(input) => input.reply_in_state(state, poisoned, completion),
            Self::Setter => {
                let action = match completion {
                    Completion::Return(value) => {
                        state.release_owned_jsvalue(poisoned, value)?;
                        crate::engine::object::SetAction::Complete
                    }
                    Completion::Throw(value) => crate::engine::object::SetAction::Throw(value),
                };
                Ok(Step::SetProgress(Some(
                    crate::engine::object::SetProgress::Complete(action),
                )))
            }
            Self::PropertyKeyValue => Ok(Step::Complete(Some(match completion {
                Completion::Return(value) => {
                    Completion::Return(state.property_key_primitive(poisoned, value)?)
                }
                completion => completion,
            }))),
            Self::Identity => Ok(Step::Complete(Some(completion))),
            Self::Primitive(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::Number(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::Arguments(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::NumericPrimitive(resume) => resume
                .primitive_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::ArrayMutation(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .map(Step::from),
            Self::DatePrototype(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::DateConstructor(resume) => resume
                .resume_in_state(state, poisoned, completion)
                .and_then(Step::try_from),
            Self::DateConstructorPrimitive(resume) => resume
                .primitive_in_state(state, poisoned, host, completion)
                .and_then(Step::try_from),
            mut resume => {
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                state.release_owned_jsvalue(poisoned, value)?;
                resume.retire_raw_in_state(state, poisoned)?;
                Err(crate::engine::api::RuntimeError::Invariant(
                    "unmigrated completion consumer entered State",
                ))
            }
        }
    }
    /// Keep the concrete StringValue wrapper armed until its child succeeds.
    /// Legacy parent roots are moved only into the published StringReply.
    pub(super) fn finish_string_value_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        completion: Completion,
    ) -> Result<Step, crate::engine::api::RuntimeError> {
        let Self::StringValue { realm, .. } = self else {
            let (Completion::Return(value) | Completion::Throw(value)) = completion;
            state.release_owned_jsvalue(poisoned, value)?;
            return Err(crate::engine::api::RuntimeError::Invariant(
                "ToString reply lost its wrapper",
            ));
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("tostring_state_reply");
        let result = state.finish_string_value_with_publication(poisoned, *realm, completion)?;
        let Self::StringValue { resume, .. } = std::mem::replace(self, Self::Identity) else {
            unreachable!()
        };
        Ok(match result {
            crate::engine::value::conversion::StringPrimitiveStep::CyclePublishedThrow(value) => {
                Step::CyclePublishedStringReply {
                    value: Some(NativeConversion::Throw(value)),
                    resume: Some(*resume),
                }
            }
            result => Step::StringReply {
                value: Some(result.into_conversion()),
                resume: Some(*resume),
            },
        })
    }
    pub(super) fn can_arguments_in_state(&self) -> bool {
        matches!(self, Self::Invoke(_))
    }
    pub(super) fn arguments_in_state(
        self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<Step, crate::engine::api::RuntimeError> {
        match self {
            Self::Invoke(resume) => resume
                .arguments_in_state(state, poisoned, result)
                .and_then(Step::try_from),
            mut resume => {
                match result {
                    NativeConversion::Value(values) => {
                        for value in values {
                            state.release_owned_jsvalue(poisoned, value)?;
                        }
                    }
                    NativeConversion::Throw(value) => {
                        state.release_owned_jsvalue(poisoned, value)?
                    }
                }
                resume.retire_raw_in_state(state, poisoned)?;
                Err(crate::engine::api::RuntimeError::Invariant(
                    "unmigrated argument-list consumer entered State",
                ))
            }
        }
    }
    pub(super) fn can_string_in_state(&self) -> bool {
        matches!(self, Self::ScalarText(_) | Self::DateConstructor(_))
    }
    pub(super) fn string_in_state(
        self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        host: &dyn crate::engine::host::HostServices,
        result: NativeConversion<crate::engine::value::JsString>,
    ) -> Result<Step, crate::engine::api::RuntimeError> {
        match self {
            Self::ScalarText(resume) => resume
                .string_in_state(state, poisoned, result)
                .and_then(Step::try_from),
            Self::DateConstructor(resume) => resume
                .string_in_state(state, poisoned, host, result)
                .and_then(Step::try_from),
            mut resume => {
                if let NativeConversion::Throw(value) = result {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
                resume.retire_raw_in_state(state, poisoned)?;
                Err(crate::engine::api::RuntimeError::Invariant(
                    "unmigrated String consumer entered State",
                ))
            }
        }
    }
    pub(super) fn can_number_in_state(&self) -> bool {
        matches!(
            self,
            Self::Math(_)
                | Self::Numeric(_)
                | Self::ScalarText(_)
                | Self::DatePrototype(_)
                | Self::Arguments(_)
                | Self::DateConstructor(_)
                | Self::ArrayMutation(_)
        )
    }
    pub(super) fn number_in_state(
        self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        host: &dyn crate::engine::host::HostServices,
        result: NativeConversion<f64>,
    ) -> Result<Step, crate::engine::api::RuntimeError> {
        match self {
            Self::Math(resume) => resume
                .number_in_state(state, poisoned, result)
                .and_then(Step::try_from),
            Self::Numeric(resume) => resume
                .number_in_state(state, poisoned, result)
                .and_then(Step::try_from),
            Self::ScalarText(resume) => resume
                .number_in_state(state, poisoned, result)
                .and_then(Step::try_from),
            Self::ArrayMutation(resume) => resume
                .number_in_state(state, poisoned, result)
                .map(Step::from),
            Self::DatePrototype(resume) => resume
                .number_in_state(state, poisoned, host, result)
                .and_then(Step::try_from),
            Self::Arguments(resume) => resume
                .number_in_state(state, poisoned, result)
                .and_then(Step::try_from),
            Self::DateConstructor(resume) => resume
                .number_in_state(state, poisoned, host, result)
                .and_then(Step::try_from),
            mut resume => {
                if let NativeConversion::Throw(value) = result {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
                resume.retire_raw_in_state(state, poisoned)?;
                Err(crate::engine::api::RuntimeError::Invariant(
                    "unmigrated Number consumer entered State",
                ))
            }
        }
    }
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
            Self::RootSet => Ok(Step::Complete(Some(match set_result(action)? {
                NativeConversion::Throw(value) => Completion::Throw(value),
                NativeConversion::Value(result) => {
                    Completion::Return(JsValue::Bool(matches!(result, InternalSetResult::Accepted)))
                }
            }))),

            Self::RegExpMatchAll(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::RegExpSplit(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::Environment(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::RegExpIteratorSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }

            Self::RegExpSearch(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::RegExpMatch(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),

            Self::ArrayConstructorSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::ArraySliceSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::IteratorTag(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::ArrayCopySet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::ArrayConcatSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::ArrayBuildSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::RegExpReplace(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::ArraySortSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::ArrayIndexedSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::ArrayReverseSet { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume
                    .set(runtime, key, set_result(action)?)
                    .and_then(Step::try_from)
            }
            Self::RegExpExec(resume) => resume
                .set_boundary(
                    runtime,
                    crate::engine::object::SetAction::from_boundary(action),
                )
                .and_then(Step::try_from),
            Self::ArrayMutation(resume) => resume
                .set_boundary(
                    runtime,
                    crate::engine::object::SetAction::from_boundary(action),
                )
                .map(Step::from),
            Self::Property(resume) => resume
                .set(runtime, set_result(action)?)
                .and_then(Step::try_from),
            Self::OrdinarySet(resume) => resume.forward(runtime, action).and_then(Step::try_from),
            Self::ProxySet(resume) => resume.set(set_result(action)?).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                SetStep::Complete(action).release(runtime);
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "Set result has no matching continuation",
                ))
            }
        }
    }
    pub(super) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RootDefine => Ok(Step::Complete(Some(match result {
                NativeConversion::Throw(value) => Completion::Throw(value),
                NativeConversion::Value(result) => Completion::Return(JsValue::Bool(matches!(
                    result,
                    InternalDefineResult::Defined
                ))),
            }))),

            Self::LiteralDefinition(resume) => resume.defined(result).and_then(Step::try_from),
            Self::PublicField => match Runtime::finish_public_class_field_definition(result)? {
                crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => {
                    Ok(Step::Complete(Some(Completion::Return(JsValue::Undefined))))
                }
                crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => {
                    Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                        "public field rejected without throwing",
                    ))
                }
                crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                    Ok(Step::Complete(Some(Completion::Throw(value))))
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
                .and_then(Step::try_from),

            Self::ArraySlice(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::IteratorTag(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::ArrayConcat(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::ArrayFlatten(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::ArrayBuild(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::ArrayCallback(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::ObjectIteration(resume) => {
                resume.defined(runtime, result).and_then(Step::try_from)
            }
            Self::Predicate(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::Definitions(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::Property(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::OrdinarySet(resume) => resume.defined(runtime, result).and_then(Step::try_from),
            Self::Define(resume) => resume.defined(result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "Define result has no matching continuation",
                ))
            }
        }
    }

    pub(super) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Import(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ForIn(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::Environment(resume) => resume.boolean(runtime, result).and_then(Step::try_from),

            Self::Bind(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::JsonParse(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::JsonStringify(resume) => resume.boolean(runtime, result).and_then(Step::try_from),

            Self::Error(resume) => resume.boolean(runtime, result).and_then(Step::try_from),

            Self::ArraySlice(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::IteratorTag(resume) => resume.boolean(result).and_then(Step::try_from),
            Self::ArrayCopy(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ArrayConcat(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ArrayFlatten(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ObjectCopy(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ArraySort(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ArrayIndexed(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ArrayReverse(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::ArrayMutation(resume) => resume.boolean(runtime, result).map(Step::from),
            Self::ArrayCallback(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::BooleanResult { payload } => runtime
                .finish_property_delete(result, payload.strict_delete)
                .map(|result| Step::Complete(Some(result))),
            Self::Definitions(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::Predicate(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::Keys(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::Property(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::BuiltinPrototype(resume) => {
                resume.boolean(runtime, result).and_then(Step::try_from)
            }
            Self::Prototype(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::Own(resume) => resume.extensible(result).and_then(Step::try_from),
            Self::Boolean(resume) => resume.boolean(runtime, result).and_then(Step::try_from),
            Self::Conversion(resume) => resume.has(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "Proxy Get received a boolean reply",
                ))
            }
        }
    }

    pub(super) fn suspended(
        self,
        runtime: &Runtime,
        outcome: crate::engine::vm::suspend::VmRunOutcome,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::AsyncGenerator(resume) => resume.body(outcome).map(Step::from),
            Self::Async(resume) => resume.body(outcome).map(Step::from),
            Self::GeneratorCreate(creation) => {
                creation.initial(runtime, outcome).and_then(Step::try_from)
            }
            Self::Generator(resume) => resume.resume(outcome).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                match outcome {
                    crate::engine::vm::suspend::VmRunOutcome::Complete(
                        Completion::Return(value) | Completion::Throw(value),
                    )
                    | crate::engine::vm::suspend::VmRunOutcome::Suspend { value, .. } => {
                        let _ = runtime.release_jsvalue(value);
                    }
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "ordinary callback returned a suspension",
                ))
            }
        }
    }

    pub(super) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::WriteKey(input) => input.reply_at_boundary(runtime, completion),
            abandoned @ (Self::RootDescriptor | Self::RootDefine | Self::RootSet) => {
                abandoned.release_owned(runtime);
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);

                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "typed root received an untyped reply",
                ))
            }
            Self::ModuleCallback(resume) => resume.resume(completion).and_then(Step::try_from),
            Self::ModuleEvaluation(resume) => resume.resume(completion).and_then(Step::try_from),
            Self::ModuleBody(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::ModuleLink(resume) => {
                crate::engine::modules::link::resume_reply(runtime, resume.resume(completion))
                    .and_then(Step::try_from)
            }
            Self::Import(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            #[cfg(feature = "test262-host")]
            Self::Test262Agent(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            #[cfg(feature = "test262-host")]
            Self::EvalScript(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::FromSync(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::AsyncGenerator(resume) => resume.resume(completion).map(Step::from),
            Self::Async(resume) => resume.resume(completion).map(Step::from),
            Self::Promise(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::GeneratorCreate(creation) => creation
                .initial(
                    runtime,
                    crate::engine::vm::suspend::VmRunOutcome::Complete(completion),
                )
                .and_then(Step::try_from),
            Self::GeneratorPrototype(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::Generator(resume) => resume
                .resume(crate::engine::vm::suspend::VmRunOutcome::Complete(
                    completion,
                ))
                .and_then(Step::try_from),
            Self::ForIn(_) => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "for-in requires typed reply",
            )),
            Self::Atomics(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::LiteralDefinition(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::PublicField => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "public field requires definition reply",
            )),

            Self::TypedCreate(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedCollect(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedIteratorMethod(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }

            Self::BufferSlice(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedWith(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Uint8Codec(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::VmNumeric(resume) => resume
                .resume(runtime, completion)
                .and_then(|value| Ok(Step::try_from(value)?))
                .map_err(crate::engine::api::runtime_error::RuntimeError::Engine),
            Self::TypedSearch(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedString(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedSlice(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::TypedMutation(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::StringFactory(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }

            Self::WeakConstructor(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "weak constructor requires prototype reply",
                ))
            }
            Self::RegExpMatchAll(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpSplit(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpIterator(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpSpecies(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::Environment(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpIteratorSet { .. } => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "RegExp iterator requires Set reply",
                ))
            }

            Self::Bind(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::FunctionText(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::DynamicFunction(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::JsonParse(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::JsonStringify(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::BufferConstructor(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::DataViewConstructor(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedSet(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::RegExpConstructor(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpSearch(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpMatch(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpCompile(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::StringProtocol(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
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
            Self::TypedSort(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Sum(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::PrimitiveConstructor(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::DateConstructor(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::DatePrototype(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::Error(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Aggregate(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::PrimitiveConstructorValue(resume) => resume
                .primitive(runtime, completion)
                .and_then(Step::try_from),
            Self::NumericPrimitive(resume) => resume
                .primitive(runtime, completion)
                .and_then(Step::try_from),
            Self::DateConstructorPrimitive(resume) => resume
                .primitive(runtime, completion)
                .and_then(Step::try_from),

            Self::MapCallback(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::SetEach(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::SetOperation(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::Collection(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::WeakComputed(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorInvalidCount(resume) => resume
                .invalid_count(runtime, completion)
                .and_then(Step::try_from),

            Self::ArrayConstructor(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArraySlice(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::TypedTraversal(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedSpecies(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::TypedIteration(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ConstructorSource(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArrayCopy(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::ArrayConcat(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArrayFlatten(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ObjectCopy(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::StringText(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::StringSearch(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::StringSplit(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::Instance(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::IteratorFrom(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorWrap(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorConcat(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArrayBuild(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::ArraySort(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::ArrayIndexed(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArrayReverse(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArrayString(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpExec(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::RegExpPresentation(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::RegExpReplace(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorConsume(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorHelper(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorCreate(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::StringValue { realm, resume } => {
                let _unwind = runtime.unwind_guard();
                let result = runtime.0.state.borrow_mut().finish_string_value(
                    &runtime.0.poisoned,
                    realm,
                    completion,
                );
                let result = match result {
                    Ok(result) => result,
                    Err(error) => {
                        resume.release_owned(runtime);
                        return Err(error);
                    }
                };
                runtime.check_poison()?;
                resume.string(runtime, result)
            }
            Self::ArrayNext(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::ArrayMutation(resume) => resume.resume(runtime, completion).map(Step::from),
            Self::ArrayCallback(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ArraySpecies(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::StringReplace(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::DataView(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::BufferMutation(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ObjectIteration(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorNext(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::IteratorClose(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ObjectIterationKey(resume) => {
                resume.key(runtime, completion).and_then(Step::try_from)
            }
            Self::Arguments(resume) => resume.read(runtime, completion).and_then(Step::try_from),
            Self::ProxyConstruct(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::ConstructorPrototype { request, resume } => {
                super::construct::prototype(runtime, request, completion, *resume)
                    .map_err(crate::engine::api::runtime_error::RuntimeError::Engine)
            }
            Self::PropertyKeyValue => {
                let result = match completion {
                    Completion::Return(value) => Completion::Return(
                        runtime
                            .0
                            .state
                            .borrow_mut()
                            .property_key_primitive(&runtime.0.poisoned, value)?,
                    ),
                    completion => completion,
                };
                Ok(Step::Complete(Some(result)))
            }
            Self::ComputedKey => {
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                runtime.release_jsvalue(value)?;
                runtime.check_poison()?;
                Err(crate::engine::api::RuntimeError::Invariant(
                    "computed key reply missed its Query",
                ))
            }
            Self::Identity => Ok(Step::Complete(Some(completion))),
            Self::ObjectString(resume) => {
                resume.resume(runtime, completion).and_then(Step::try_from)
            }
            Self::Definitions(resume) => resume.read(runtime, completion).and_then(Step::try_from),
            Self::PredicateKey(resume) => resume.key(runtime, completion).and_then(Step::try_from),
            Self::Keys(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::PropertyKey(resume) => resume.key(runtime, completion).and_then(Step::try_from),
            Self::Property(resume) => resume.read(runtime, completion).and_then(Step::try_from),
            Self::Primitive(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::BuiltinPrototype(_) => {
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "prototype builtin received an untyped reply",
                ))
            }
            Self::Prototype(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::PrototypeGetReply(resume) => {
                let result = match completion {
                    Completion::Return(JsValue::Object(object)) => NativeConversion::Value(Some(
                        ObjectRef::from_owned_handle(runtime.clone(), object),
                    )),
                    Completion::Return(JsValue::Null) => NativeConversion::Value(None),
                    Completion::Throw(value) => NativeConversion::Throw(value),
                    Completion::Return(value) => {
                        let _ = runtime.release_jsvalue(value);
                        resume.release_owned(runtime);
                        return Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                            "invalid GetPrototypeOf reply",
                        ));
                    }
                };
                resume.prototype(runtime, result)
            }
            Self::PrototypeSetReply(resume) => {
                let result = match completion {
                    Completion::Return(JsValue::Bool(value)) => NativeConversion::Value(value),
                    Completion::Throw(value) => NativeConversion::Throw(value),
                    Completion::Return(value) => {
                        let _ = runtime.release_jsvalue(value);
                        resume.release_owned(runtime);
                        return Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                            "invalid SetPrototypeOf reply",
                        ));
                    }
                };
                resume.boolean(runtime, result)
            }
            Self::ProxySet(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Define(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Setter => {
                // A cold callback still replies to the same raw Set producer.
                // Its assignment owner can be ResidentWrite or a Set parent;
                // the obsolete SetComplete route only recognized legacy Write.
                let _unwind = runtime.unwind_guard();
                let result = Self::Setter.resume_in_state(
                    &mut runtime.0.state.borrow_mut(),
                    &runtime.0.poisoned,
                    runtime.0.host_services.as_ref(),
                    completion,
                );
                runtime.check_poison()?;
                result
            }
            abandoned @ Self::BooleanResult { .. } => {
                abandoned.release_owned(runtime);
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);

                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "boolean operation received an untyped reply",
                ))
            }
            Self::ReadOwner(_owner) => Ok(Step::Complete(Some(completion))),
            Self::Element(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Number(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            abandoned @ (Self::IteratorConstructor(_)
            | Self::IteratorTag(_)
            | Self::ArrayConstructorSet { .. }
            | Self::ArraySliceSet { .. }
            | Self::ArrayCopySet { .. }
            | Self::ArrayConcatSet { .. }
            | Self::ArrayBuildSet { .. }
            | Self::ArraySortSet { .. }
            | Self::ArrayIndexedSet { .. }
            | Self::ArrayReverseSet { .. }
            | Self::Invoke(_)
            | Self::Predicate(_)
            | Self::OwnFlagReply { .. }
            | Self::TypedElement(_)
            | Self::SetTyped(_)
            | Self::DefineTyped { .. }
            | Self::LengthNumber(_)
            | Self::SetLength(_)
            | Self::DefineLength { .. }
            | Self::OrdinarySet(_)) => {
                abandoned.release_owned(runtime);
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);

                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "ordinary Set received an untyped reply",
                ))
            }
            Self::Get(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Call(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Own(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
            Self::Conversion(resume) => resume.read(runtime, completion).and_then(Step::try_from),
            Self::Boolean(resume) => resume.resume(runtime, completion).and_then(Step::try_from),
        }
    }
    pub(super) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<crate::engine::object::OwnedCompletePropertyDescriptor>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::RootDescriptor => Ok(Step::RootDescriptor(Some(
                runtime.public_descriptor_result(result)?,
            ))),
            Self::OwnFlagReply { enumerable, resume } => resume.boolean(
                runtime,
                match result {
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                    NativeConversion::Value(descriptor) => NativeConversion::Value(
                        descriptor.is_some_and(|descriptor| !enumerable || descriptor.enumerable()),
                    ),
                },
            ),
            Self::Predicate(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::Keys(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::Get(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::Property(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::Own(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::Boolean(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::OrdinarySet(resume) => {
                resume.descriptor(runtime, result).and_then(Step::try_from)
            }
            Self::ProxySet(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            Self::Define(resume) => resume.descriptor(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "descriptor conversion received an own-property reply",
                ))
            }
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
            Self::SetLength(resume) => resume
                .array_length(runtime, result)
                .and_then(Step::try_from),
            Self::DefineLength { payload } => {
                let DefineLengthPayload {
                    object,
                    key,
                    descriptor,
                    resume,
                } = *payload;
                let result = match result {
                    ArrayLengthConversion::Throw(value) => NativeConversion::Throw(value),
                    ArrayLengthConversion::Length(length) => match runtime
                        .apply_array_length_descriptor(
                            &object,
                            &key,
                            &descriptor.attributes_public()?,
                            length,
                        )? {
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
            resume => {
                resume.release_owned(runtime);
                if let ArrayLengthConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "Array length result has no matching continuation",
                ))
            }
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
                resume
                    .special(runtime, Some(result))
                    .and_then(Step::try_from)
            }
            Self::DefineTyped { payload } => {
                let DefineTypedPayload {
                    object,
                    _descriptor,
                    resume,
                } = *payload;
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
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "TypedArray result has no matching continuation",
                ))
            }
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
            Self::ForIn(resume) => resume.prototype(runtime, result).and_then(Step::try_from),
            Self::Instance(resume) => resume.prototype(runtime, result).and_then(Step::try_from),
            Self::Predicate(resume) => resume.prototype(runtime, result).and_then(Step::try_from),
            Self::BuiltinPrototype(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }
            Self::Prototype(resume) => resume.prototype(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "prototype result has no matching continuation",
                ))
            }
        }
    }
}

impl Resume {
    pub(super) fn converted(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::object::OwnedPropertyDescriptor>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Definitions(resume) => resume.converted(runtime, result).and_then(Step::try_from),
            Self::Own(resume) => resume.converted(runtime, result).and_then(Step::try_from),
            Self::Property(resume) => resume.converted(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "descriptor conversion has no matching operation",
                ))
            }
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
            Self::Test262Agent(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::Atomics(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::StringFactory(resume) => resume.number(runtime, result).and_then(Step::try_from),

            Self::JsonParse(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::JsonStringify(resume) => resume.number(runtime, result).and_then(Step::try_from),

            Self::TypedSort(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::Math(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::Global(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::Numeric(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ScalarText(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::DateConstructor(resume) => {
                resume.number(runtime, result).and_then(Step::try_from)
            }
            Self::DatePrototype(resume) => resume.number(runtime, result).and_then(Step::try_from),

            Self::SetOperation(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArraySlice(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayConcat(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayFlatten(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayBuild(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArraySort(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayIndexed(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayReverse(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayString(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::IteratorCreate(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayNext(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::ArrayMutation(resume) => resume.number(runtime, result).map(Step::from),
            Self::ArrayCallback(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::Arguments(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::LengthNumber(resume) => resume.number(runtime, result).and_then(Step::try_from),
            Self::Keys(resume) => resume.number(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "numeric reply has no matching continuation",
                ))
            }
        }
    }
    pub(super) fn keys(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Import(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            Self::ForIn(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            Self::JsonParse(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            Self::JsonStringify(resume) => resume.keys(runtime, result).and_then(Step::try_from),

            Self::ObjectCopy(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            Self::Definitions(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            Self::Keys(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            Self::Property(resume) => resume.keys(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "key-list reply has no matching continuation",
                ))
            }
        }
    }
}

impl Resume {
    pub(super) fn arguments(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Invoke(resume) => resume.arguments(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                runtime.check_poison()?;
                match result {
                    NativeConversion::Value(values) => {
                        for value in values {
                            runtime.release_jsvalue(value)?;
                            runtime.check_poison()?;
                        }
                    }
                    NativeConversion::Throw(value) => {
                        runtime.release_jsvalue(value)?;
                        runtime.check_poison()?;
                    }
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "argument list has no matching continuation",
                ))
            }
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
            Self::Promise(resume) => resume.next(runtime, result).and_then(Step::try_from),
            Self::Sum(resume) => resume.item(runtime, result).and_then(Step::try_from),
            Self::Aggregate(resume) => resume.item(runtime, result).and_then(Step::try_from),

            Self::SetOperation(resume) => resume.parsed(runtime, result).and_then(Step::try_from),
            Self::Collection(resume) => resume.next(runtime, result).and_then(Step::try_from),
            Self::IteratorWrap(resume) => resume.next(runtime, result).and_then(Step::try_from),
            Self::IteratorConcat(resume) => resume.next(runtime, result).and_then(Step::try_from),
            Self::ArrayBuild(resume) => resume.parsed(runtime, result).and_then(Step::try_from),
            Self::IteratorConsume(resume) => resume.next(runtime, result).and_then(Step::try_from),
            Self::IteratorHelper(resume) => resume.next(runtime, result).and_then(Step::try_from),
            Self::ObjectIteration(resume) => resume.next(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                match result {
                    crate::engine::builtins::ObjectIteratorStep::Yield(value)
                    | crate::engine::builtins::ObjectIteratorStep::Throw(value) => {
                        let _ = runtime.release_jsvalue(value);
                    }
                    crate::engine::builtins::ObjectIteratorStep::Done => {}
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "iterator result has no continuation",
                ))
            }
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
            Self::IteratorNext(resume) => resume.raw(runtime, result).and_then(Step::try_from),
            resume => resume.resume(runtime, Runtime::ordinary_native_completion(result)?),
        }
    }
}

impl Resume {
    pub(super) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::value::JsString>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Import(resume) => resume
                .resume(
                    runtime,
                    match result {
                        NativeConversion::Value(value) => {
                            Completion::Return(runtime.into_jsvalue(Value::String(value))?)
                        }
                        NativeConversion::Throw(value) => Completion::Throw(value),
                    },
                )
                .and_then(Step::try_from),

            #[cfg(feature = "test262-host")]
            resume @ (Self::EvalScript(_) | Self::Test262Agent(_)) => resume.resume(
                runtime,
                match result {
                    NativeConversion::Value(value) => {
                        Completion::Return(runtime.into_jsvalue(Value::String(value))?)
                    }
                    NativeConversion::Throw(value) => Completion::Throw(value),
                },
            ),

            Self::StringFactory(resume) => resume.string(runtime, result).and_then(Step::try_from),

            Self::RegExpIterator(resume) => resume.string(runtime, result).and_then(Step::try_from),

            Self::FunctionText(resume) => resume.string(runtime, result).and_then(Step::try_from),
            Self::DynamicFunction(resume) => {
                resume.string(runtime, result).and_then(Step::try_from)
            }
            Self::JsonParse(resume) => resume.string(runtime, result).and_then(Step::try_from),
            Self::JsonStringify(resume) => resume.string(runtime, result).and_then(Step::try_from),
            Self::JsonRaw(resume) => resume
                .string(runtime, result)
                .map(|result| Step::Complete(Some(result))),

            Self::PrimitiveConstructor(resume) => {
                resume.string(runtime, result).and_then(Step::try_from)
            }
            Self::Global(resume) => resume.string(runtime, result).and_then(Step::try_from),
            Self::ScalarText(resume) => resume.string(runtime, result).and_then(Step::try_from),
            Self::DateConstructor(resume) => {
                resume.string(runtime, result).and_then(Step::try_from)
            }
            Self::Error(resume) => resume.string(runtime, result).and_then(Step::try_from),

            Self::ArraySort(resume) => resume.string(runtime, result).and_then(Step::try_from),
            Self::ArrayString(resume) => resume.string(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "string result has no continuation",
                ))
            }
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
            Self::Promise(resume) => resume.prototype(runtime, result).and_then(Step::try_from),
            Self::TypedCreate(resume) => resume.prototype(runtime, result).and_then(Step::try_from),

            Self::WeakConstructor(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }
            Self::ObjectConstructor(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }
            Self::BufferConstructor(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }
            Self::DataViewConstructor(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }
            Self::RegExpConstructor(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }

            Self::Collection(resume) => resume.prototype(runtime, result).and_then(Step::try_from),
            Self::IteratorConstructor(resume) => {
                resume.prototype(runtime, result).and_then(Step::try_from)
            }
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "constructor prototype source has no continuation",
                ))
            }
        }
    }
    pub(super) fn element(
        self,
        runtime: &Runtime,
        result: NativeConversion<[u8; 8]>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.element(runtime, result).and_then(Step::try_from),

            Self::TypedMutation(resume) => resume.element(runtime, result).and_then(Step::try_from),

            Self::TypedSet(resume) => resume.element(runtime, result).and_then(Step::try_from),

            Self::TypedIteration(resume) => {
                resume.element(runtime, result).and_then(Step::try_from)
            }
            Self::TypedElement(resume) => resume.element(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "element result has no continuation",
                ))
            }
        }
    }
    pub(super) fn typed_species(
        self,
        runtime: &Runtime,
        result: NativeConversion<ObjectRef>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.created(runtime, result).and_then(Step::try_from),

            Self::TypedSlice(resume) => resume.species(runtime, result).and_then(Step::try_from),

            Self::TypedIteration(resume) => {
                resume.species(runtime, result).and_then(Step::try_from)
            }
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "typed species result has no continuation",
                ))
            }
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
            Self::RegExpMatchAll(resume) => {
                resume.species(runtime, result).and_then(Step::try_from)
            }
            Self::RegExpSplit(resume) => resume.species(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "RegExp species has no continuation",
                ))
            }
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
            Self::TypedCreate(resume) => resume.method(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                if let NativeConversion::Throw(value) = result {
                    let _ = runtime.release_jsvalue(value);
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "typed iterator method has no continuation",
                ))
            }
        }
    }
    pub(super) fn typed_collected(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::TypedCreate(resume) => resume.collected(runtime, result).and_then(Step::try_from),
            resume => {
                resume.release_owned(runtime);
                match result {
                    NativeConversion::Value(values) => {
                        for value in values {
                            let _ = runtime.release_jsvalue(value);
                        }
                    }
                    NativeConversion::Throw(value) => {
                        let _ = runtime.release_jsvalue(value);
                    }
                }
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "typed collected values have no continuation",
                ))
            }
        }
    }
}

#[cfg(test)]
#[test]
fn s11_protocol_layout_inventory() {
    assert!(
        std::mem::size_of::<Resume>() <= 32,
        "Resume {}",
        std::mem::size_of::<Resume>()
    );
    // Step is intentionally resident: dispatchers drain selected Option fields
    // through &mut Step, never take/rebuild the entire packet to retry dispatch.
    assert!(
        std::mem::size_of::<Step>() <= 192,
        "Step {}",
        std::mem::size_of::<Step>()
    );
    assert!(
        std::mem::size_of::<super::Next>() <= 64,
        "Next {}",
        std::mem::size_of::<super::Next>()
    );
    assert!(std::mem::size_of::<super::super::conversion_driver::ConversionTask>() <= 8);
    assert!(std::mem::size_of::<super::super::iterator_driver::PendingIterator>() <= 8);
    #[cfg(feature = "test262-host")]
    {
        assert!(
            std::mem::size_of::<crate::engine::api::test262_agent::operation::AgentResume>() <= 8
        );
        assert!(
            std::mem::size_of::<crate::engine::api::test262_host::operation::EvalScriptResume>()
                <= 8
        );
    }
}
