//! Closed registration of native algorithms that yield owned domain requests.
use super::native::NativeFunctionId;
use super::object::{
    definitions::{DefinitionsKind, DefinitionsStep},
    predicate::{PredicateKind, PredicateStep},
    property::{PropertyKind, PropertyStep},
    prototype::{BuiltinPrototypeKind, BuiltinPrototypeStep},
    string::{ObjectStringKind, ObjectStringStep},
};
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    vm::call::{NativeArguments, NativeInvocation, NativeInvokeOutcome},
};

mod output;
mod registry;

/// Closed synchronous entry families. The ordinary native ABI remains shared;
/// only the generic waiting dispatcher is absent from this corridor.
pub(crate) enum SynchronousNative {
    Pure(NativeFunctionId),
    PrimitiveConstructor(super::native::PrimitiveKind),
    Math(super::math::operation::MathKind),
}
impl SynchronousNative {
    pub(crate) fn start(
        self,
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
        callable: &crate::engine::object::CallableRef,
    ) -> Result<crate::engine::vm::Completion, RuntimeError> {
        match self {
            Self::Pure(target) => runtime.dispatch_synchronous_native_borrowed(
                callable, target, realm, invocation, arguments,
            ),
            Self::PrimitiveConstructor(kind) => match super::PrimitiveConstructorStep::start(
                runtime, realm, kind, invocation, arguments,
            )? {
                super::PrimitiveConstructorStep::Complete(result) => Ok(result),
                _ => Err(RuntimeError::Invariant(
                    "synchronous primitive constructor unexpectedly waited",
                )),
            },
            Self::Math(kind) => {
                match super::MathStep::start(runtime, realm, kind, invocation, arguments)? {
                    super::MathStep::Complete(result) => Ok(result),
                    _ => Err(RuntimeError::Invariant(
                        "synchronous Math unexpectedly required conversion",
                    )),
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum NativeOperation {
    #[cfg(test)]
    ActiveFrameProbe,
    ModuleCallback(NativeFunctionId),
    #[cfg(feature = "test262-host")]
    Test262Agent(super::native::Test262AgentKind),
    #[cfg(feature = "test262-host")]
    EvalScript,
    HostOutput(NativeFunctionId),
    AsyncGenerator(NativeFunctionId),
    FromSync(NativeFunctionId),
    AsyncResume(crate::engine::heap::AsyncFunctionResumeKind),
    Promise(NativeFunctionId),
    GeneratorResume(super::native::GeneratorResumeKind),
    Atomics(super::native::AtomicsNativeKind),
    TypedCreate(super::native::TypedArrayNativeKind),
    SharedBufferConstructor,
    SharedBufferGrow,
    BufferSlice(super::array_buffer::BufferSliceKind),
    TypedWith,
    Uint8Codec(super::native::Uint8ArrayCodecKind),

    TypedSearch(super::array_buffer::typed_array::TypedSearchKind),
    TypedString(super::native::ArrayJoinKind),
    TypedSlice(super::array_buffer::typed_array::TypedSliceKind),
    TypedMutation(super::array_buffer::typed_array::TypedMutationKind),
    StringFactory(super::string::StringFactoryKind),

    WeakConstructor(super::weak_ref::WeakIntrinsicKind),
    RegExpMatchAll,
    RegExpSplit,
    RegExpIterator,

    GlobalEval,
    JsonRaw,
    ObjectConstructor,
    Bind,
    FunctionText,
    DynamicFunction(super::native::DynamicFunctionKind),
    JsonParse,
    JsonStringify,
    BufferConstructor,
    DataViewConstructor,
    TypedSet,
    RegExpConstructor,
    RegExpSearch,
    RegExpMatch,
    RegExpCompile,
    StringProtocol(super::string::StringProtocolKind),

    TypedSort(bool),
    Math(super::math::operation::MathKind),
    Sum,
    PrimitiveConstructor(super::native::PrimitiveKind),
    Global(super::primitive::globals::GlobalKind),
    Numeric(super::primitive::numeric::NumericKind),
    ScalarText(super::primitive::text::ScalarTextKind),
    DateConstructor(super::native::DateNativeKind),
    DatePrototype(super::native::DateNativeKind),
    Error(super::error::operation::ErrorKind),

    MapCallback(super::map::callback::CallbackKind),
    SetEach,
    SetOperation(super::set::operations::SetOperation),
    Collection(super::iterator::collection::CollectionKind),
    WeakComputed,
    Pure(NativeFunctionId),
    ArrayConstructor,
    ArraySlice(super::array::slice::SliceKind),
    IteratorConstructor,
    IteratorAccessor,
    IteratorTag,
    TypedTraversal(super::array_buffer::typed_array::TypedTraversalKind),
    TypedIteration(super::native::ArrayIterationKind),
    ArrayConcat,
    ArrayFlatten(super::native::ArrayFlattenKind),
    StringText(super::string::StringTextKind),
    StringSearch(super::string::StringSearchKind),
    StringSplit,
    Instance,
    IteratorFrom,
    IteratorWrap(crate::engine::heap::IteratorResumeKind),
    IteratorConcat(super::iterator::concat::ConcatKind),
    ArrayBuild(super::array::build::BuildKind),
    ArraySpeciesGetter,
    ArraySort(bool),
    ArrayIndexed(super::array::indexed::IndexedKind),
    ArrayReverse,
    ArrayString(super::array::string::ArrayStringKind),
    RegExpExec(super::native::RegExpNativeKind),
    RegExpPresentation(super::native::RegExpNativeKind),
    RegExpReplace,
    IteratorConsume(super::iterator::consume::ConsumeKind),
    IteratorHelper(crate::engine::heap::IteratorResumeKind),
    IteratorCreate(crate::engine::heap::IteratorHelperKind),

    ArrayNext,
    PureIterator(NativeFunctionId),
    ArrayMutation(super::array::mutation::MutationKind),
    ArrayCallback(super::array::callback::CallbackKind),
    ObjectIteration(super::object::iteration::IterationKind),
    StringReplace(super::native::StringReplaceKind),
    DataView(super::native::DataViewNativeKind),
    BufferMutation(super::native::ArrayBufferNativeKind),
    Invoke(super::function::invoke::InvokeKind),
    Prototype(BuiltinPrototypeKind),
    Property(PropertyKind),
    String(ObjectStringKind),
    Proxy(NativeFunctionId),
    ObjectValueOf,
    ObjectIs,
    Definitions(DefinitionsKind),
    Predicate(PredicateKind),
}
pub(crate) enum NativeStep {
    ModuleCallback(crate::engine::modules::callback::CallbackStep),
    #[cfg(feature = "test262-host")]
    Test262Agent(crate::engine::api::test262_agent::operation::AgentStep),
    #[cfg(feature = "test262-host")]
    EvalScript(crate::engine::api::test262_host::operation::EvalScriptStep),
    AsyncGenerator(crate::engine::vm::async_generator::AsyncGeneratorStep),
    FromSync(crate::engine::vm::async_from_sync_iterator::FromSyncStep),
    Async(crate::engine::vm::async_function::AsyncStep),
    Promise(super::promise::operation::PromiseStep),
    GeneratorResume(crate::engine::vm::generator::GeneratorStep),
    Atomics(super::AtomicsStep),
    TypedCreate(super::TypedCreateStep),
    BufferSlice(super::BufferSliceStep),
    TypedWith(super::TypedWithStep),
    Uint8Codec(super::Uint8CodecStep),

    TypedSearch(super::TypedSearchStep),
    TypedString(super::TypedStringStep),
    TypedSlice(super::TypedSliceStep),
    TypedMutation(super::TypedMutationStep),
    StringFactory(super::StringFactoryStep),

    WeakConstructor(super::WeakConstructorStep),
    RegExpMatchAll(super::RegExpMatchAllStep),
    RegExpSplit(super::RegExpSplitStep),
    RegExpIterator(super::RegExpIteratorStep),

    GlobalEval(crate::engine::value::JsString),
    JsonRaw {
        value: crate::engine::value::JsValue,
        resume: super::JsonRawResume,
    },
    ObjectConstructor(super::ObjectConstructorStep),
    Bind(super::BindStep),
    FunctionText(super::FunctionTextStep),
    DynamicFunction(super::DynamicFunctionStep),
    JsonParse(super::JsonParseStep),
    JsonStringify(super::JsonStringifyStep),
    BufferConstructor(super::BufferConstructorStep),
    DataViewConstructor(super::DataViewConstructorStep),
    TypedSet(super::TypedSetStep),
    RegExpConstructor(super::RegExpConstructorStep),
    RegExpSearch(super::RegExpSearchStep),
    RegExpMatch(super::RegExpMatchStep),
    RegExpCompile(super::RegExpCompileStep),
    StringProtocol(super::StringProtocolStep),

    TypedSort(super::TypedSortStep),
    Math(super::MathStep),
    Sum(super::SumStep),
    PrimitiveConstructor(super::PrimitiveConstructorStep),
    Global(super::GlobalStep),
    Numeric(super::NumericStep),
    ScalarText(super::ScalarTextStep),
    DateConstructor(super::DateConstructorStep),
    DatePrototype(super::DatePrototypeStep),
    Error(super::ErrorStep),

    MapCallback(super::MapCallbackStep),
    SetEach(super::SetEachStep),
    SetOperation(super::SetOperationStep),
    Collection(super::CollectionStep),
    WeakComputed(super::WeakComputedStep),
    ArrayConstructor(super::ArrayConstructorStep),
    ArraySlice(super::ArraySliceStep),
    IteratorConstructor(super::IteratorConstructorStep),
    IteratorTag(super::IteratorTagStep),
    TypedTraversal(super::TypedTraversalStep),
    TypedIteration(super::TypedIterationStep),

    ArrayConcat(super::ArrayConcatStep),
    ArrayFlatten(super::ArrayFlattenStep),
    StringText(super::StringTextStep),
    StringSearch(super::StringSearchStep),
    StringSplit(super::StringSplitStep),

    Instance(super::InstanceStep),
    IteratorFrom(super::IteratorFromStep),
    IteratorWrap(super::IteratorWrapStep),
    IteratorConcat(super::IteratorConcatStep),
    ArrayBuild(super::ArrayBuildStep),
    ArraySort(super::ArraySortStep),
    ArrayIndexed(super::ArrayIndexedStep),
    ArrayReverse(super::ArrayReverseStep),
    ArrayString(super::ArrayStringStep),
    RegExpExec(super::RegExpExecStep),
    RegExpPresentation(super::RegExpPresentationStep),
    RegExpReplace(super::RegExpReplaceStep),
    IteratorConsume(super::IteratorConsumeStep),
    IteratorHelper(super::IteratorHelperStep),
    IteratorCreate(super::IteratorCreateStep),

    ArrayNext(super::ArrayNextStep),
    Raw(crate::engine::vm::call::NativeInvokeOutcome),
    ArrayMutation(super::ArrayMutationStep),
    ArrayCallback(super::ArrayCallbackStep),
    ObjectIteration(super::ObjectIterationStep),
    StringReplace(super::StringReplaceStep),
    DataView(super::DataViewAccessStep),
    BufferMutation(super::BufferMutationStep),
    Invoke(super::function::invoke::InvokeStep),
    Prototype(BuiltinPrototypeStep),
    Property(PropertyStep),
    String(ObjectStringStep),
    Complete(crate::engine::vm::Completion),
    Definitions(DefinitionsStep),
    Predicate(PredicateStep),
}
impl NativeOperation {
    pub(crate) fn synchronous(
        &self,
        arguments: &[crate::engine::value::JsValue],
    ) -> Option<SynchronousNative> {
        match self {
            Self::Pure(target) => Some(SynchronousNative::Pure(*target)),
            Self::PrimitiveConstructor(kind)
                if matches!(kind, super::native::PrimitiveKind::Boolean)
                    || !matches!(
                        arguments.first(),
                        Some(crate::engine::value::JsValue::Object(_))
                    ) =>
            {
                Some(SynchronousNative::PrimitiveConstructor(*kind))
            }
            Self::Math(kind) => {
                use super::math::operation::MathKind;
                let count = match kind {
                    MathKind::Unary(_) | MathKind::Clz32 => 1,
                    MathKind::Binary(_) | MathKind::Imul => 2,
                    _ => arguments.len(),
                };
                arguments
                    .iter()
                    .take(count)
                    .all(|value| !matches!(value, crate::engine::value::JsValue::Object(_)))
                    .then_some(SynchronousNative::Math(*kind))
            }
            _ => None,
        }
    }

    pub(crate) fn start_into(
        self,
        rt: &Runtime,
        realm: ContextId,
        inv: &NativeInvocation,
        args: &NativeArguments,
        callable: &crate::engine::object::CallableRef,
        mut waiting: impl FnMut(NativeStep),
    ) -> Result<Option<NativeInvokeOutcome>, RuntimeError> {
        use super::native::TypedArrayNativeKind;
        use super::*;
        let step = match self {
            #[cfg(test)]
            Self::ActiveFrameProbe => {
                NativeStep::Invoke(rt.prepare_active_frame_probe(realm, args)?)
            }
            Self::AsyncGenerator(target) => NativeStep::AsyncGenerator(
                crate::engine::vm::async_generator::AsyncGeneratorStep::start(
                    rt, realm, target, inv, args,
                )?,
            ),
            Self::FromSync(target) => NativeStep::FromSync(
                crate::engine::vm::async_from_sync_iterator::FromSyncStep::start(
                    rt, realm, target, inv, args,
                )?,
            ),
            Self::AsyncResume(kind) => NativeStep::Async(rt.start_async_function_resume(
                realm,
                kind,
                inv.dup(rt)?,
                args,
            )?),
            Self::Promise(target) => NativeStep::Promise(
                super::promise::operation::PromiseStep::start(rt, realm, target, inv, args)?,
            ),
            Self::GeneratorResume(kind) => NativeStep::GeneratorResume(
                rt.start_generator_prototype_resume(realm, kind, inv.dup(rt)?, args)?,
            ),
            Self::Atomics(kind) => NativeStep::Atomics(AtomicsStep::start(rt, realm, kind, args)?),
            Self::TypedCreate(kind) => NativeStep::TypedCreate(match kind {
                TypedArrayNativeKind::Constructor(element) => {
                    TypedCreateStep::constructor(rt, realm, element, inv, args)?
                }
                TypedArrayNativeKind::From => TypedCreateStep::from(rt, realm, inv, args)?,
                TypedArrayNativeKind::Of => TypedCreateStep::of(rt, realm, inv, args)?,
                _ => {
                    return Err(RuntimeError::Invariant(
                        "invalid TypedArray creation registration",
                    ));
                }
            }),

            Self::BufferSlice(kind) => {
                NativeStep::BufferSlice(BufferSliceStep::start(rt, realm, kind, inv, args)?)
            }
            Self::TypedWith => NativeStep::TypedWith(TypedWithStep::start(rt, realm, inv, args)?),
            Self::Uint8Codec(kind) => {
                NativeStep::Uint8Codec(Uint8CodecStep::start(rt, realm, kind, inv, args)?)
            }
            Self::SharedBufferConstructor => NativeStep::BufferConstructor(
                BufferConstructorStep::start_shared(rt, realm, inv, args)?,
            ),
            Self::SharedBufferGrow => {
                NativeStep::BufferMutation(BufferMutationStep::start_grow(rt, realm, inv, args)?)
            }

            Self::TypedSearch(kind) => {
                NativeStep::TypedSearch(TypedSearchStep::start(rt, realm, kind, inv, args)?)
            }
            Self::TypedString(kind) => {
                NativeStep::TypedString(TypedStringStep::start(rt, realm, kind, inv, args)?)
            }
            Self::TypedSlice(kind) => {
                NativeStep::TypedSlice(TypedSliceStep::start(rt, realm, kind, inv, args)?)
            }
            Self::TypedMutation(kind) => {
                NativeStep::TypedMutation(TypedMutationStep::start(rt, realm, kind, inv, args)?)
            }
            Self::StringFactory(kind) => {
                NativeStep::StringFactory(StringFactoryStep::start(rt, realm, kind, args)?)
            }
            Self::WeakConstructor(kind) => {
                NativeStep::WeakConstructor(WeakConstructorStep::start(rt, realm, kind, inv, args)?)
            }
            Self::RegExpMatchAll => {
                NativeStep::RegExpMatchAll(RegExpMatchAllStep::start(rt, realm, inv, args)?)
            }
            Self::RegExpSplit => {
                NativeStep::RegExpSplit(RegExpSplitStep::start(rt, realm, inv, args)?)
            }
            Self::RegExpIterator => {
                NativeStep::RegExpIterator(RegExpIteratorStep::start(rt, realm, inv)?)
            }
            Self::ObjectConstructor => {
                NativeStep::ObjectConstructor(ObjectConstructorStep::start(rt, realm, inv, args)?)
            }
            Self::Bind => NativeStep::Bind(BindStep::start(rt, realm, inv, args)?),
            Self::FunctionText => {
                NativeStep::FunctionText(FunctionTextStep::start(rt, realm, inv)?)
            }
            Self::DynamicFunction(kind) => {
                NativeStep::DynamicFunction(DynamicFunctionStep::start(rt, realm, kind, inv, args)?)
            }
            Self::JsonParse => NativeStep::JsonParse(JsonParseStep::start(rt, realm, args)?),
            Self::JsonStringify => {
                NativeStep::JsonStringify(JsonStringifyStep::start(rt, realm, args)?)
            }
            Self::BufferConstructor => {
                NativeStep::BufferConstructor(BufferConstructorStep::start(rt, realm, inv, args)?)
            }
            Self::DataViewConstructor => NativeStep::DataViewConstructor(
                DataViewConstructorStep::start(rt, realm, inv, args)?,
            ),
            Self::TypedSet => NativeStep::TypedSet(TypedSetStep::start(rt, realm, inv, args)?),
            Self::RegExpConstructor => {
                NativeStep::RegExpConstructor(RegExpConstructorStep::start(rt, realm, inv, args)?)
            }
            Self::RegExpSearch => {
                NativeStep::RegExpSearch(RegExpSearchStep::start(rt, realm, inv, args)?)
            }
            Self::RegExpMatch => {
                NativeStep::RegExpMatch(RegExpMatchStep::start(rt, realm, inv, args)?)
            }
            Self::RegExpCompile => {
                NativeStep::RegExpCompile(RegExpCompileStep::start(rt, realm, inv, args)?)
            }
            Self::StringProtocol(kind) => {
                NativeStep::StringProtocol(StringProtocolStep::start(rt, realm, kind, inv, args)?)
            }
            Self::GlobalEval => {
                let argument = &args.readable[0];
                if let crate::engine::value::JsValue::String(id) = argument {
                    NativeStep::GlobalEval(rt.0.state.borrow().heap.string(*id)?.clone())
                } else {
                    NativeStep::Complete(Completion::Return(rt.dup_jsvalue(argument)?))
                }
            }
            Self::JsonRaw => NativeStep::JsonRaw {
                value: rt.dup_jsvalue(&args.readable[0])?,
                resume: super::JsonRawResume::new(realm),
            },

            Self::TypedSort(copying) => {
                NativeStep::TypedSort(TypedSortStep::start(rt, realm, copying, inv, args)?)
            }
            Self::Math(kind) => {
                return Ok(output::deliver(
                    MathStep::start(rt, realm, kind, inv, args)?,
                    &mut waiting,
                ));
            }
            Self::Sum => NativeStep::Sum(SumStep::start(rt, realm, inv, args)?),
            Self::PrimitiveConstructor(kind) => {
                return Ok(output::deliver(
                    PrimitiveConstructorStep::start(rt, realm, kind, inv, args)?,
                    &mut waiting,
                ));
            }
            Self::Global(kind) => {
                return Ok(output::deliver(
                    GlobalStep::start(rt, realm, kind, inv, args)?.advance_primitive(rt, realm)?,
                    &mut waiting,
                ));
            }
            Self::Numeric(kind) => {
                NativeStep::Numeric(NumericStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ScalarText(kind) => {
                NativeStep::ScalarText(ScalarTextStep::start(rt, realm, kind, inv, args)?)
            }
            Self::DateConstructor(kind) => {
                NativeStep::DateConstructor(DateConstructorStep::start(rt, realm, kind, inv, args)?)
            }
            Self::DatePrototype(kind) => {
                NativeStep::DatePrototype(DatePrototypeStep::start(rt, realm, kind, inv, args)?)
            }
            Self::Error(kind) => NativeStep::Error(ErrorStep::start(rt, realm, kind, inv, args)?),

            Self::MapCallback(kind) => {
                NativeStep::MapCallback(MapCallbackStep::start(rt, realm, kind, inv, args)?)
            }
            Self::SetEach => NativeStep::SetEach(SetEachStep::start(rt, realm, inv, args)?),
            Self::SetOperation(kind) => {
                NativeStep::SetOperation(SetOperationStep::start(rt, realm, kind, inv, args)?)
            }
            Self::Collection(kind) => {
                NativeStep::Collection(CollectionStep::start(rt, realm, kind, inv, args)?)
            }
            Self::WeakComputed => {
                NativeStep::WeakComputed(WeakComputedStep::start(rt, realm, inv, args)?)
            }

            #[cfg(feature = "test262-host")]
            Self::Test262Agent(kind) => NativeStep::Test262Agent(
                crate::engine::api::test262_agent::operation::AgentStep::start(
                    rt,
                    realm,
                    kind,
                    inv.dup(rt)?,
                    args,
                )?,
            ),
            #[cfg(feature = "test262-host")]
            Self::EvalScript => NativeStep::EvalScript(
                crate::engine::api::test262_host::operation::EvalScriptStep::start(
                    rt,
                    realm,
                    inv.dup(rt)?,
                    args,
                )?,
            ),
            Self::ModuleCallback(target) => {
                NativeStep::ModuleCallback(crate::engine::modules::callback::CallbackStep::start(
                    rt,
                    realm,
                    target,
                    inv.dup(rt)?,
                    args,
                )?)
            }
            Self::HostOutput(target) => {
                NativeStep::Complete(rt.call_qjs_output(target, inv.dup(rt)?, args)?)
            }
            Self::Pure(target) => {
                // This registered domain cannot wait. Keep its completion in
                // the small result channel, without constructing NativeStep.
                let completion =
                    rt.dispatch_synchronous_native_borrowed(callable, target, realm, inv, args)?;
                return Ok(Some(NativeInvokeOutcome::Completion(completion)));
            }
            Self::ArrayConstructor => {
                NativeStep::ArrayConstructor(ArrayConstructorStep::start(rt, realm, inv, args)?)
            }
            Self::ArraySlice(kind) => {
                NativeStep::ArraySlice(ArraySliceStep::start(rt, realm, kind, inv, args)?)
            }
            Self::IteratorConstructor => {
                NativeStep::IteratorConstructor(IteratorConstructorStep::start(rt, realm, inv)?)
            }
            Self::IteratorAccessor => NativeStep::IteratorConstructor(
                IteratorConstructorStep::accessor(rt, realm, callable, inv, args)?,
            ),
            Self::IteratorTag => {
                NativeStep::IteratorTag(IteratorTagStep::start(rt, realm, inv, args)?)
            }
            Self::TypedTraversal(kind) => {
                NativeStep::TypedTraversal(TypedTraversalStep::start(rt, realm, kind, inv, args)?)
            }
            Self::TypedIteration(kind) => {
                NativeStep::TypedIteration(TypedIterationStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ArrayConcat => {
                NativeStep::ArrayConcat(ArrayConcatStep::start(rt, realm, inv, args)?)
            }
            Self::ArrayFlatten(kind) => {
                NativeStep::ArrayFlatten(ArrayFlattenStep::start(rt, realm, kind, inv, args)?)
            }
            Self::StringText(kind) => {
                NativeStep::StringText(StringTextStep::start(rt, realm, kind, inv, args)?)
            }
            Self::StringSearch(kind) => {
                NativeStep::StringSearch(StringSearchStep::start(rt, realm, kind, inv, args)?)
            }
            Self::StringSplit => {
                NativeStep::StringSplit(StringSplitStep::start(rt, realm, inv, args)?)
            }
            Self::Instance => NativeStep::Instance(InstanceStep::native(rt, realm, inv, args)?),
            Self::IteratorFrom => {
                NativeStep::IteratorFrom(IteratorFromStep::start(rt, realm, inv, args)?)
            }
            Self::IteratorWrap(kind) => {
                NativeStep::IteratorWrap(IteratorWrapStep::start(rt, realm, kind, inv)?)
            }
            Self::IteratorConcat(kind) => {
                NativeStep::IteratorConcat(IteratorConcatStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ArrayBuild(kind) => {
                NativeStep::ArrayBuild(ArrayBuildStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ArraySpeciesGetter => {
                NativeStep::Complete(rt.call_array_species_getter(inv.dup(rt)?)?)
            }
            Self::ArraySort(kind) => {
                NativeStep::ArraySort(ArraySortStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ArrayIndexed(kind) => {
                NativeStep::ArrayIndexed(ArrayIndexedStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ArrayReverse => {
                NativeStep::ArrayReverse(ArrayReverseStep::start(rt, realm, inv)?)
            }
            Self::ArrayString(kind) => NativeStep::ArrayString(ArrayStringStep::start(
                rt,
                realm,
                kind,
                inv,
                args,
                crate::engine::value::JsString::MAX_LEN,
            )?),
            Self::RegExpExec(kind) => {
                return Ok(output::deliver(
                    RegExpExecStep::start(rt, realm, kind, inv, args)?,
                    &mut waiting,
                ));
            }
            Self::RegExpPresentation(kind) => {
                NativeStep::RegExpPresentation(RegExpPresentationStep::start(rt, realm, kind, inv)?)
            }
            Self::RegExpReplace => {
                return Ok(output::deliver(
                    RegExpReplaceStep::start(rt, realm, inv, args)?,
                    &mut waiting,
                ));
            }
            Self::IteratorConsume(kind) => {
                NativeStep::IteratorConsume(IteratorConsumeStep::start(rt, realm, kind, inv, args)?)
            }
            Self::IteratorHelper(kind) => {
                NativeStep::IteratorHelper(IteratorHelperStep::start(rt, realm, kind, inv)?)
            }
            Self::IteratorCreate(kind) => {
                NativeStep::IteratorCreate(IteratorCreateStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ArrayNext => {
                return start_array_next_into(rt, realm, inv, &mut waiting);
            }
            Self::PureIterator(target) => NativeStep::Raw(match target {
                NativeFunctionId::StringIteratorNext => {
                    rt.call_string_iterator_next_raw(realm, inv.dup(rt)?)?
                }
                NativeFunctionId::MapIteratorNext => {
                    rt.call_map_iterator_next_raw(realm, inv.dup(rt)?)?
                }
                NativeFunctionId::SetIteratorNext => {
                    rt.call_set_iterator_next_raw(realm, inv.dup(rt)?)?
                }
                _ => unreachable!("closed pure iterator registration"),
            }),
            Self::ArrayMutation(kind) => {
                return Ok(output::deliver(
                    ArrayMutationStep::start(rt, realm, kind, inv, args)?,
                    &mut waiting,
                ));
            }
            Self::ArrayCallback(kind) => {
                NativeStep::ArrayCallback(ArrayCallbackStep::start(rt, realm, kind, inv, args)?)
            }
            Self::ObjectIteration(kind) => {
                NativeStep::ObjectIteration(ObjectIterationStep::start(rt, realm, kind, inv, args)?)
            }
            Self::StringReplace(kind) => {
                return Ok(output::deliver(
                    StringReplaceStep::start(rt, realm, kind, inv, args)?,
                    &mut waiting,
                ));
            }
            Self::DataView(kind) => {
                NativeStep::DataView(DataViewAccessStep::start(rt, realm, kind, inv, args)?)
            }
            Self::BufferMutation(kind) => {
                NativeStep::BufferMutation(BufferMutationStep::start(rt, realm, kind, inv, args)?)
            }
            Self::Proxy(target) => NativeStep::Complete(match target {
                NativeFunctionId::ProxyConstructor => {
                    rt.call_proxy_constructor(realm, inv.dup(rt)?, args)?
                }
                NativeFunctionId::ProxyRevocable => {
                    rt.call_proxy_revocable(realm, inv.dup(rt)?, args)?
                }
                NativeFunctionId::ProxyRevoke => rt.call_proxy_revoke(inv.dup(rt)?)?,
                _ => unreachable!("closed Proxy native registration"),
            }),
            Self::Invoke(kind) => NativeStep::Invoke(super::function::invoke::InvokeStep::start(
                rt, realm, kind, inv, args,
            )?),
            Self::ObjectValueOf => {
                NativeStep::Complete(rt.call_object_prototype_value_of(realm, inv.dup(rt)?)?)
            }
            Self::ObjectIs => NativeStep::Complete(rt.call_object_is(inv.dup(rt)?, args)?),
            Self::String(kind) => {
                NativeStep::String(ObjectStringStep::start(rt, realm, kind, inv)?)
            }
            Self::Definitions(kind) => {
                NativeStep::Definitions(DefinitionsStep::start(rt, realm, kind, args)?)
            }
            Self::Predicate(kind) => {
                NativeStep::Predicate(PredicateStep::start(rt, realm, kind, inv, args)?)
            }
            Self::Prototype(kind) => NativeStep::Prototype(BuiltinPrototypeStep::start_invocation(
                rt, realm, kind, inv, args,
            )?),
            Self::Property(kind) => {
                NativeStep::Property(PropertyStep::start(rt, realm, kind, args)?)
            }
        };
        Ok(match step {
            NativeStep::Complete(completion) => Some(NativeInvokeOutcome::Completion(completion)),
            NativeStep::Raw(result) => Some(result),
            step => {
                waiting(step);
                None
            }
        })
    }
}

/// One Array-next domain entry shared by generic and already-classified native
/// callers. Invocation adaptation and activation ownership remain with the VM.
#[inline(never)]
pub(crate) fn start_array_next_into(
    runtime: &Runtime,
    realm: ContextId,
    invocation: &NativeInvocation,
    mut waiting: impl FnMut(NativeStep),
) -> Result<Option<crate::engine::vm::call::NativeInvokeOutcome>, RuntimeError> {
    Ok(output::deliver(
        super::ArrayNextStep::start(runtime, realm, invocation)?.advance_local(runtime, realm)?,
        &mut waiting,
    ))
}

// Inline domain payloads leave room for the NativeStep discriminant.
macro_rules! assert_inline_payloads {
    ($($ty:ty),* $(,)?) => {
        $(const _: () = assert!(std::mem::size_of::<$ty>() <= 56);)*
    };
}
assert_inline_payloads! {
    crate::engine::modules::callback::CallbackStep,
    crate::engine::vm::async_generator::AsyncGeneratorStep,
    crate::engine::vm::async_from_sync_iterator::FromSyncStep,
    crate::engine::vm::async_function::AsyncStep, super::promise::operation::PromiseStep,
    crate::engine::vm::generator::GeneratorStep, super::AtomicsStep, super::TypedCreateStep,
    super::BufferSliceStep, super::TypedWithStep, super::Uint8CodecStep, super::TypedSearchStep,
    super::TypedStringStep, super::TypedSliceStep, super::TypedMutationStep,
    super::StringFactoryStep, super::WeakConstructorStep, super::RegExpMatchAllStep,
    super::RegExpSplitStep, super::RegExpIteratorStep, crate::engine::value::Value,
    super::ObjectConstructorStep, super::BindStep, super::FunctionTextStep,
    super::DynamicFunctionStep, super::JsonParseStep, super::JsonStringifyStep,
    super::BufferConstructorStep, super::DataViewConstructorStep, super::TypedSetStep,
    super::RegExpConstructorStep, super::RegExpSearchStep, super::RegExpMatchStep,
    super::RegExpCompileStep, super::StringProtocolStep, super::TypedSortStep, super::MathStep,
    super::SumStep, super::PrimitiveConstructorStep, super::GlobalStep, super::NumericStep,
    super::ScalarTextStep, super::DateConstructorStep, super::DatePrototypeStep, super::ErrorStep,
    super::MapCallbackStep, super::SetEachStep, super::SetOperationStep, super::CollectionStep,
    super::WeakComputedStep, super::ArrayConstructorStep, super::ArraySliceStep,
    super::IteratorConstructorStep, super::IteratorTagStep, super::TypedTraversalStep,
    super::TypedIterationStep, super::ArrayConcatStep, super::ArrayFlattenStep,
    super::StringTextStep, super::StringSearchStep, super::StringSplitStep, super::InstanceStep,
    super::IteratorFromStep, super::IteratorWrapStep, super::IteratorConcatStep,
    super::ArrayBuildStep, super::ArraySortStep, super::ArrayIndexedStep, super::ArrayReverseStep,
    super::ArrayStringStep, super::RegExpExecStep, super::RegExpPresentationStep,
    super::RegExpReplaceStep, super::IteratorConsumeStep, super::IteratorHelperStep,
    super::IteratorCreateStep, super::ArrayNextStep, crate::engine::vm::call::NativeInvokeOutcome,
    super::ArrayMutationStep, super::ArrayCallbackStep, super::ObjectIterationStep,
    super::StringReplaceStep, super::DataViewAccessStep, super::BufferMutationStep,
    super::function::invoke::InvokeStep, BuiltinPrototypeStep, PropertyStep, ObjectStringStep,
    crate::engine::vm::Completion, DefinitionsStep, PredicateStep,
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<NativeStep>() <= 64);

#[cfg(test)]
mod inventory_tests;
