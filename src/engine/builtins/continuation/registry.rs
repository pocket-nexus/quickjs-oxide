//! The closed native registry: one row per selector family.

use super::NativeOperation;
use super::NativeOperation as Op;
use crate::engine::builtins::array::{
    build::BuildKind, callback::CallbackKind as ArrayCallback, indexed::IndexedKind,
    mutation::MutationKind, slice::SliceKind, string::ArrayStringKind,
};
use crate::engine::builtins::array_buffer::BufferSliceKind;
use crate::engine::builtins::array_buffer::typed_array::{
    TypedMutationKind, TypedSearchKind, TypedSliceKind, TypedTraversalKind,
};
use crate::engine::builtins::error::operation::ErrorKind as ErrorOperation;
use crate::engine::builtins::function::invoke::InvokeKind;
use crate::engine::builtins::iterator::{
    collection::CollectionKind, concat::ConcatKind, consume::ConsumeKind,
};
use crate::engine::builtins::map::callback::CallbackKind as MapCallback;
use crate::engine::builtins::math::operation::MathKind;
use crate::engine::builtins::native::{
    ArrayBufferNativeKind as Ab, ArraySliceKind, AtomicsNativeKind as At, DataViewNativeKind as Dv,
    DateNativeKind as Date, FinalizationRegistryNativeKind as Fr, GlobalUriCodecKind as Uri,
    JsonNativeKind as Json, MapNativeKind as M, NativeCProto, NativeFunctionDescriptor,
    NativeFunctionId, NativeFunctionId as F, ObjectExtensibilityKind as Extensibility,
    PromiseNativeKind as P, ReflectKind as R, RegExpNativeKind as Re, SetNativeKind as S,
    SharedArrayBufferNativeKind as Sab, SymbolRegistryKind, TypedArrayNativeKind as Ta,
    WeakMapNativeKind as Wm, WeakRefNativeKind as Wr, WeakSetNativeKind as Ws,
};
use crate::engine::builtins::object::{
    definitions::DefinitionsKind, iteration::IterationKind, predicate::PredicateKind,
    property::PropertyKind, prototype::BuiltinPrototypeKind, string::ObjectStringKind,
};
use crate::engine::builtins::primitive::{
    globals::GlobalKind, numeric::NumericKind, text::ScalarTextKind,
};
use crate::engine::builtins::set::operations::SetOperation;
use crate::engine::builtins::string::{
    StringFactoryKind, StringProtocolKind, StringSearchKind, StringTextKind,
};
use crate::engine::builtins::weak_ref::WeakIntrinsicKind;

/// The closed native registry: one row per selector family, giving its
/// QuickJS C prototype and the operation that owns its algorithm. Both
/// `NativeFunctionId::descriptor` and `NativeOperation::for_target` are
/// generated from these rows, so they cannot disagree or miss a selector.
macro_rules! native_registry {
    (|$target:ident| $($(#[$attr:meta])* $pat:pat => $cproto:ident, $op:expr;)*) => {
        impl NativeFunctionId {
            /// Static handler-family metadata. Per-object realm, readable
            /// arity and the mutable constructor bit remain outside it.
            #[must_use]
            #[allow(unused_variables)]
            pub const fn descriptor(self) -> NativeFunctionDescriptor {
                let cproto = match self {
                    $($(#[$attr])* $pat => NativeCProto::$cproto,)*
                };
                NativeFunctionDescriptor { cproto }
            }
        }
        impl NativeOperation {
            pub(crate) fn for_target($target: NativeFunctionId) -> Option<Self> {
                Some(match $target {
                    $($(#[$attr])* $pat => $op,)*
                })
            }
        }
    };
}

native_registry! { |target|
    F::FunctionPrototype | F::ThrowTypeError => Generic, Op::Pure(target);
    F::FunctionConstructor(kind) => ConstructorOrFunctionMagic, Op::DynamicFunction(kind);
    F::GeneratorPrototypeResume(kind) => IteratorNext, Op::GeneratorResume(kind);
    F::AsyncGeneratorPrototypeResume(_) => GenericMagic, Op::AsyncGenerator(target);
    F::ArrayConstructor => ConstructorOrFunction, Op::ArrayConstructor;
    F::ArrayIsArray | F::ArrayPrototypeIterator(_) => Generic, Op::Pure(target);
    F::ArrayFrom => Generic, Op::ArrayBuild(BuildKind::From);
    F::ArrayOf => Generic, Op::ArrayBuild(BuildKind::Of);
    F::ArraySpeciesGetter => Getter, Op::ArraySpeciesGetter;
    F::ArrayPrototypeAt => Generic, Op::ArrayIndexed(IndexedKind::At);
    F::ArrayPrototypeWith => Generic, Op::ArrayIndexed(IndexedKind::With);
    F::ArrayPrototypeConcat => Generic, Op::ArrayConcat;
    F::ArrayPrototypeIteration(kind) => GenericMagic,
        Op::ArrayCallback(ArrayCallback::Iteration(kind));
    F::ArrayPrototypeReduce(kind) => GenericMagic, Op::ArrayCallback(ArrayCallback::Reduce(kind));
    F::ArrayPrototypeFill => Generic, Op::ArrayIndexed(IndexedKind::Fill);
    F::ArrayPrototypeFind(kind) => GenericMagic, Op::ArrayCallback(ArrayCallback::Find(kind));
    F::ArrayPrototypeSearch(kind) => Generic, Op::ArrayIndexed(IndexedKind::Search(kind));
    F::ArrayPrototypeJoin(kind) => GenericMagic, Op::ArrayString(ArrayStringKind::Join(kind));
    F::ArrayPrototypeToString => Generic, Op::ArrayString(ArrayStringKind::ToString);
    F::ArrayPrototypePop(kind) => GenericMagic, Op::ArrayMutation(MutationKind::Pop(kind));
    F::ArrayPrototypePush(kind) => GenericMagic, Op::ArrayMutation(MutationKind::Push(kind));
    F::ArrayPrototypeReverse => Generic, Op::ArrayReverse;
    F::ArrayPrototypeToReversed => Generic, Op::ArrayIndexed(IndexedKind::ToReversed);
    F::ArrayPrototypeSort => Generic, Op::ArraySort(false);
    F::ArrayPrototypeToSorted => Generic, Op::ArraySort(true);
    F::ArrayPrototypeSlice(ArraySliceKind::Slice) => GenericMagic, Op::ArraySlice(SliceKind::Slice);
    F::ArrayPrototypeSlice(ArraySliceKind::Splice) => GenericMagic,
        Op::ArraySlice(SliceKind::Splice);
    F::ArrayPrototypeToSpliced => Generic, Op::ArraySlice(SliceKind::ToSpliced);
    F::ArrayPrototypeCopyWithin => Generic, Op::ArrayIndexed(IndexedKind::CopyWithin);
    F::ArrayPrototypeFlatten(kind) => GenericMagic, Op::ArrayFlatten(kind);
    F::ArrayIteratorNext => IteratorNext, Op::ArrayNext;
    F::FunctionPrototypeCall => Generic, Op::Invoke(InvokeKind::Call);
    F::FunctionPrototypeApply => Generic, Op::Invoke(InvokeKind::Apply);
    F::FunctionPrototypeBind => Generic, Op::Bind;
    F::FunctionPrototypeToString => Generic, Op::FunctionText;
    F::FunctionPrototypeHasInstance => Generic, Op::Instance;
    F::FunctionPrototypeFileName => Getter, Op::Pure(target);
    F::FunctionPrototypePosition(_) => GetterMagic, Op::Pure(target);
    F::ObjectConstructor => ConstructorOrFunction, Op::ObjectConstructor;
    F::ObjectCreate => Generic, Op::Definitions(DefinitionsKind::Create);
    F::ObjectGetPrototypeOf => GenericMagic, Op::Prototype(BuiltinPrototypeKind::ObjectGet);
    F::ObjectSetPrototypeOf => Generic, Op::Prototype(BuiltinPrototypeKind::ObjectSet);
    F::ObjectDefineProperty => GenericMagic, Op::Property(PropertyKind::ObjectDefine);
    F::ObjectDefineProperties => Generic, Op::Definitions(DefinitionsKind::Define);
    F::ObjectGetOwnPropertyKeys(kind) => Generic, Op::Property(PropertyKind::ObjectOwnKeys(kind));
    F::ObjectGroupBy => GenericMagic, Op::ObjectIteration(IterationKind::Group);
    F::ObjectKeys(kind) => GenericMagic, Op::Property(PropertyKind::ObjectKeys(kind));
    F::ObjectExtensibility(Extensibility::IsExtensible) => GenericMagic,
        Op::Property(PropertyKind::ObjectExtensible);
    F::ObjectExtensibility(Extensibility::PreventExtensions) => GenericMagic,
        Op::Property(PropertyKind::ObjectPrevent);
    F::ObjectGetOwnPropertyDescriptor => GenericMagic, Op::Property(PropertyKind::ObjectDescriptor);
    F::ObjectGetOwnPropertyDescriptors => Generic, Op::Property(PropertyKind::ObjectDescriptors);
    F::ObjectIs => Generic, Op::ObjectIs;
    F::ObjectAssign => Generic, Op::Property(PropertyKind::Assign);
    F::ObjectIntegrity(kind) => GenericMagic, Op::Property(PropertyKind::Integrity(kind));
    F::ObjectFromEntries => Generic, Op::ObjectIteration(IterationKind::Entries);
    F::ObjectHasOwn => Generic, Op::Predicate(PredicateKind::HasOwn);
    F::ObjectPrototypeToString => Generic, Op::String(ObjectStringKind::Tag);
    F::ObjectPrototypeToLocaleString => Generic, Op::String(ObjectStringKind::Locale);
    F::ObjectPrototypeValueOf => Generic, Op::ObjectValueOf;
    F::ObjectPrototypeHasOwnProperty => Generic, Op::Predicate(PredicateKind::PrototypeHasOwn);
    F::ObjectPrototypeIsPrototypeOf => Generic, Op::Prototype(BuiltinPrototypeKind::IsPrototype);
    F::ObjectPrototypePropertyIsEnumerable => Generic, Op::Predicate(PredicateKind::Enumerable);
    F::ObjectPrototypeProtoGetter => Getter, Op::Prototype(BuiltinPrototypeKind::Getter);
    F::ObjectPrototypeProtoSetter => Setter, Op::Prototype(BuiltinPrototypeKind::Setter);
    F::ObjectPrototypeDefineAccessor(kind) => GenericMagic,
        Op::Predicate(PredicateKind::Define(kind));
    F::ObjectPrototypeLookupAccessor(kind) => GenericMagic,
        Op::Predicate(PredicateKind::Lookup(kind));
    F::ProxyConstructor => Constructor, Op::Proxy(target);
    F::ProxyRevocable | F::ProxyRevoke => Generic, Op::Proxy(target);
    F::Json(Json::Parse) => Generic, Op::JsonParse;
    F::Json(Json::Stringify) => Generic, Op::JsonStringify;
    F::Json(Json::RawJson) => Generic, Op::JsonRaw;
    F::Json(Json::IsRawJson) => Generic, Op::Pure(target);
    F::Reflect(R::Apply) => Generic, Op::Invoke(InvokeKind::ReflectApply);
    F::Reflect(R::Construct) => Generic, Op::Invoke(InvokeKind::ReflectConstruct);
    F::Reflect(R::DefineProperty) => GenericMagic, Op::Property(PropertyKind::Define);
    F::Reflect(R::DeleteProperty) => Generic, Op::Property(PropertyKind::Delete);
    F::Reflect(R::Get) => Generic, Op::Property(PropertyKind::Get);
    F::Reflect(R::GetOwnPropertyDescriptor) => GenericMagic, Op::Property(PropertyKind::Descriptor);
    F::Reflect(R::GetPrototypeOf) => GenericMagic, Op::Prototype(BuiltinPrototypeKind::ReflectGet);
    F::Reflect(R::Has) => Generic, Op::Property(PropertyKind::Has);
    F::Reflect(R::IsExtensible) => GenericMagic, Op::Property(PropertyKind::Extensible);
    F::Reflect(R::OwnKeys) => Generic, Op::Property(PropertyKind::Keys);
    F::Reflect(R::PreventExtensions) => GenericMagic, Op::Property(PropertyKind::Prevent);
    F::Reflect(R::Set) => Generic, Op::Property(PropertyKind::Set);
    F::Reflect(R::SetPrototypeOf) => Generic, Op::Prototype(BuiltinPrototypeKind::ReflectSet);
    F::Date(kind @ (Date::Now | Date::Parse | Date::Utc)) => Generic, Op::DateConstructor(kind);
    F::Date(kind @ Date::Constructor) => ConstructorOrFunction, Op::DateConstructor(kind);
    F::Date(Date::TimeValue | Date::TimezoneOffset) => Generic, Op::Pure(target);
    F::Date(kind @ (Date::ToPrimitive | Date::SetTime | Date::SetYear | Date::ToJson)) => Generic,
        Op::DatePrototype(kind);
    F::Date(Date::String(_) | Date::GetField(_)) => GenericMagic, Op::Pure(target);
    F::Date(kind @ Date::SetField(_)) => GenericMagic, Op::DatePrototype(kind);
    F::RegExp(Re::Constructor) => ConstructorOrFunction, Op::RegExpConstructor;
    F::RegExp(Re::Escape) => Generic, Op::Pure(target);
    F::RegExp(Re::Species) => Getter, Op::Pure(target);
    F::RegExp(kind @ (Re::Exec | Re::Test)) => Generic, Op::RegExpExec(kind);
    F::RegExp(Re::Compile) => Generic, Op::RegExpCompile;
    F::RegExp(kind @ Re::ToString) => Generic, Op::RegExpPresentation(kind);
    F::RegExp(kind @ (Re::Source | Re::Flags)) => Getter, Op::RegExpPresentation(kind);
    F::RegExp(kind @ Re::Flag(_)) => GetterMagic, Op::RegExpPresentation(kind);
    F::RegExp(Re::Replace) => Generic, Op::RegExpReplace;
    F::RegExp(Re::Match) => Generic, Op::RegExpMatch;
    F::RegExp(Re::MatchAll) => Generic, Op::RegExpMatchAll;
    F::RegExp(Re::Search) => Generic, Op::RegExpSearch;
    F::RegExp(Re::Split) => Generic, Op::RegExpSplit;
    F::Map(M::Constructor) => Constructor, Op::Collection(CollectionKind::Map);
    F::Map(M::Species | M::Size) => Getter, Op::Pure(target);
    F::Map(M::GroupBy) | F::Set(S::GroupBy) => Generic,
        Op::ObjectIteration(IterationKind::MapGroup);
    F::Map(M::Set | M::Get | M::Has | M::Delete | M::Clear | M::Iterator(_)) => Generic,
        Op::Pure(target);
    F::Map(M::GetOrInsert) => Generic, Op::MapCallback(MapCallback::Insert { computed: false });
    F::Map(M::GetOrInsertComputed) => Generic,
        Op::MapCallback(MapCallback::Insert { computed: true });
    F::Map(M::ForEach) => Generic, Op::MapCallback(MapCallback::Each);
    F::MapIteratorNext | F::SetIteratorNext | F::StringIteratorNext => IteratorNext,
        Op::PureIterator(target);
    F::Set(S::Constructor) => Constructor, Op::Collection(CollectionKind::Set);
    F::Set(S::Species | S::Size) => Getter, Op::Pure(target);
    F::Set(S::Add | S::Has | S::Delete | S::Clear | S::Iterator(_)) => Generic, Op::Pure(target);
    F::Set(S::ForEach) => Generic, Op::SetEach;
    F::Set(S::IsDisjointFrom) => Generic, Op::SetOperation(SetOperation::Disjoint);
    F::Set(S::IsSubsetOf) => Generic, Op::SetOperation(SetOperation::Subset);
    F::Set(S::IsSupersetOf) => Generic, Op::SetOperation(SetOperation::Superset);
    F::Set(S::Intersection) => Generic, Op::SetOperation(SetOperation::Intersection);
    F::Set(S::Difference) => Generic, Op::SetOperation(SetOperation::Difference);
    F::Set(S::SymmetricDifference) => Generic, Op::SetOperation(SetOperation::SymmetricDifference);
    F::Set(S::Union) => Generic, Op::SetOperation(SetOperation::Union);
    F::WeakMap(Wm::Constructor) => Constructor, Op::Collection(CollectionKind::WeakMap);
    F::WeakMap(Wm::GetOrInsertComputed) => Generic, Op::WeakComputed;
    F::WeakMap(Wm::Set | Wm::Get | Wm::GetOrInsert | Wm::Has | Wm::Delete) => Generic,
        Op::Pure(target);
    F::WeakSet(Ws::Constructor) => Constructor, Op::Collection(CollectionKind::WeakSet);
    F::WeakSet(Ws::Add | Ws::Has | Ws::Delete) => Generic, Op::Pure(target);
    F::WeakRef(Wr::Constructor) => ConstructorOrFunction,
        Op::WeakConstructor(WeakIntrinsicKind::WeakRef);
    F::WeakRef(Wr::Deref) => Generic, Op::Pure(target);
    F::FinalizationRegistry(Fr::Constructor) => ConstructorOrFunction,
        Op::WeakConstructor(WeakIntrinsicKind::FinalizationRegistry);
    F::FinalizationRegistry(Fr::Register | Fr::Unregister) => Generic, Op::Pure(target);
    F::ArrayBuffer(Ab::Constructor) => Constructor, Op::BufferConstructor;
    F::ArrayBuffer(Ab::IsView) => Generic, Op::Pure(target);
    F::ArrayBuffer(Ab::Species | Ab::Detached) => Getter, Op::Pure(target);
    F::ArrayBuffer(Ab::ByteLength | Ab::MaxByteLength | Ab::Resizable) => GetterMagic,
        Op::Pure(target);
    F::ArrayBuffer(kind @ (Ab::Resize | Ab::Transfer | Ab::TransferToFixedLength)) => GenericMagic,
        Op::BufferMutation(kind);
    F::ArrayBuffer(Ab::Slice) => GenericMagic, Op::BufferSlice(BufferSliceKind::Array);
    F::SharedArrayBuffer(Sab::Constructor) => Constructor, Op::SharedBufferConstructor;
    F::SharedArrayBuffer(Sab::Species) => Getter, Op::Pure(target);
    F::SharedArrayBuffer(Sab::ByteLength | Sab::MaxByteLength | Sab::Growable) => GetterMagic,
        Op::Pure(target);
    F::SharedArrayBuffer(Sab::Grow) => GenericMagic, Op::SharedBufferGrow;
    F::SharedArrayBuffer(Sab::Slice) => GenericMagic, Op::BufferSlice(BufferSliceKind::Shared);
    F::DataView(Dv::Constructor) => Constructor, Op::DataViewConstructor;
    F::DataView(Dv::Buffer | Dv::ByteLength | Dv::ByteOffset) => Getter, Op::Pure(target);
    F::DataView(kind @ (Dv::Get(_) | Dv::Set(_))) => GenericMagic, Op::DataView(kind);
    F::TypedArray(Ta::BaseConstructor) => ConstructorOrFunction, Op::Pure(target);
    F::TypedArray(kind @ Ta::Constructor(_)) => ConstructorMagic, Op::TypedCreate(kind);
    F::TypedArray(Ta::Uint8Codec(kind)) => Generic, Op::Uint8Codec(kind);
    F::TypedArray(kind @ (Ta::From | Ta::Of)) => Generic, Op::TypedCreate(kind);
    F::TypedArray(Ta::Species | Ta::Length | Ta::Buffer | Ta::ByteLength | Ta::ByteOffset
        | Ta::ToStringTag) => Getter,
        Op::Pure(target);
    F::TypedArray(Ta::At) => Generic, Op::TypedSearch(TypedSearchKind::At);
    F::TypedArray(Ta::With) => Generic, Op::TypedWith;
    F::TypedArray(Ta::Set) => Generic, Op::TypedSet;
    F::TypedArray(Ta::Iterator(_)) => GenericMagic, Op::Pure(target);
    F::TypedArray(Ta::CopyWithin) => Generic, Op::TypedMutation(TypedMutationKind::CopyWithin);
    F::TypedArray(Ta::Iteration(kind)) => GenericMagic, Op::TypedIteration(kind);
    F::TypedArray(Ta::Reduce(kind)) => GenericMagic,
        Op::TypedTraversal(TypedTraversalKind::Reduce(kind));
    F::TypedArray(Ta::Fill) => Generic, Op::TypedMutation(TypedMutationKind::Fill);
    F::TypedArray(Ta::Find(kind)) => GenericMagic,
        Op::TypedTraversal(TypedTraversalKind::Find(kind));
    F::TypedArray(Ta::Reverse | Ta::ToReversed) => Generic, Op::Pure(target);
    F::TypedArray(Ta::Slice) => Generic, Op::TypedSlice(TypedSliceKind::Slice);
    F::TypedArray(Ta::Subarray) => Generic, Op::TypedSlice(TypedSliceKind::Subarray);
    F::TypedArray(Ta::Sort) => Generic, Op::TypedSort(false);
    F::TypedArray(Ta::ToSorted) => Generic, Op::TypedSort(true);
    F::TypedArray(Ta::Join(kind)) => GenericMagic, Op::TypedString(kind);
    F::TypedArray(Ta::Search(kind)) => GenericMagic, Op::TypedSearch(TypedSearchKind::Search(kind));
    F::Atomics(kind @ At::Operation(_)) => GenericMagic, Op::Atomics(kind);
    F::Atomics(kind @ (At::Store | At::IsLockFree | At::Pause | At::Wait | At::Notify)) => Generic,
        Op::Atomics(kind);
    F::Promise(P::Constructor) => Constructor, Op::Promise(target);
    F::Promise(P::Species) => Getter, Op::Promise(target);
    F::Promise(
        P::Then | P::Catch | P::Finally | P::Resolve | P::Reject | P::All | P::AllSettled
        | P::Any | P::Try | P::Race | P::WithResolvers
    ) => Generic, Op::Promise(target);
    F::AsyncFunctionResume(kind) => Generic, Op::AsyncResume(kind);
    F::AsyncGeneratorResume(_) => Generic, Op::AsyncGenerator(target);
    F::PromiseResolving(_) | F::PromiseCapabilityExecutor | F::PromiseFinallyHandler(_)
    | F::PromiseFinallyThunk(_) | F::PromiseAllResolveElement | F::PromiseAllSettledElement(_)
    | F::PromiseAnyRejectElement => Generic, Op::Promise(target);
    F::ModuleEvaluation(_) | F::DynamicImportHandler(_) => Generic, Op::ModuleCallback(target);
    F::AsyncFromSyncIteratorResume(_) => GenericMagic, Op::FromSync(target);
    F::AsyncFromSyncIteratorUnwrap | F::AsyncFromSyncIteratorClose => Generic, Op::FromSync(target);
    F::PrimitiveConstructor(kind) => ConstructorOrFunctionMagic, Op::PrimitiveConstructor(kind);
    F::StringStatic(kind) => Generic, Op::StringFactory(StringFactoryKind::Static(kind));
    #[cfg(feature = "test262-host")]
    F::StringCodePointRange => Generic, Op::StringFactory(StringFactoryKind::CodePointRange);
    #[cfg(feature = "test262-host")]
    F::Test262DetachArrayBuffer | F::Test262CreateRealm | F::Test262IsHtmlDda
        | F::Test262Gc => Generic,
        Op::Pure(target);
    #[cfg(feature = "test262-host")]
    F::Test262EvalScript => Generic, Op::EvalScript;
    #[cfg(feature = "test262-host")]
    F::Test262Agent(kind) => Generic, Op::Test262Agent(kind);
    F::QjsPrint | F::QjsConsoleLog => Generic, Op::HostOutput(target);
    F::PrimitivePrototypeToString(kind) => Generic, Op::Numeric(NumericKind::ToString(kind));
    F::PrimitivePrototypeValueOf(_) => Generic, Op::Pure(target);
    F::StringPrototypeCharAt(kind) => GenericMagic, Op::ScalarText(ScalarTextKind::CharAt(kind));
    F::StringPrototypeCharCodeAt => Generic, Op::ScalarText(ScalarTextKind::CharCodeAt);
    F::StringPrototypeConcat => Generic, Op::ScalarText(ScalarTextKind::Concat);
    F::StringPrototypeCodePointAt => Generic, Op::ScalarText(ScalarTextKind::CodePointAt);
    F::StringPrototypeWellFormed(kind) => Generic, Op::ScalarText(ScalarTextKind::WellFormed(kind));
    F::StringPrototypeIndexOf(kind) => GenericMagic,
        Op::StringSearch(StringSearchKind::Index(kind));
    F::StringPrototypeIncludes(kind) => GenericMagic,
        Op::StringSearch(StringSearchKind::Includes(kind));
    F::StringPrototypeReplace(kind) => GenericMagic, Op::StringReplace(kind);
    F::StringPrototypeMatch => GenericMagic, Op::StringProtocol(StringProtocolKind::Match);
    F::StringPrototypeMatchAll => GenericMagic, Op::StringProtocol(StringProtocolKind::MatchAll);
    F::StringPrototypeSearch => GenericMagic, Op::StringProtocol(StringProtocolKind::Search);
    F::StringPrototypeSplit => Generic, Op::StringSplit;
    F::MathMinMax(kind) => GenericMagic, Op::Math(MathKind::MinMax(kind));
    F::MathUnary(kind) => UnaryF64, Op::Math(MathKind::Unary(kind));
    F::MathBinary(kind) => BinaryF64, Op::Math(MathKind::Binary(kind));
    F::MathHypot => Generic, Op::Math(MathKind::Hypot);
    F::MathRandom => Generic, Op::Pure(target);
    F::MathImul => Generic, Op::Math(MathKind::Imul);
    F::MathClz32 => Generic, Op::Math(MathKind::Clz32);
    F::MathSumPrecise => Generic, Op::Sum;
    F::StringPrototypeSubrange(kind) => Generic, Op::StringSearch(StringSearchKind::Subrange(kind));
    F::StringPrototypeRepeat => Generic, Op::StringText(StringTextKind::Repeat);
    F::StringPrototypePad(kind) => GenericMagic, Op::StringText(StringTextKind::Pad(kind));
    F::StringPrototypeTrim(kind) => GenericMagic, Op::StringText(StringTextKind::Trim(kind));
    F::StringPrototypeCase(kind) => GenericMagic, Op::StringText(StringTextKind::Case(kind));
    F::StringPrototypeNormalize => Generic, Op::StringText(StringTextKind::Normalize);
    F::StringPrototypeLocaleCompare => Generic, Op::StringText(StringTextKind::LocaleCompare);
    F::StringPrototypeCreateHtml(kind) => GenericMagic, Op::StringText(StringTextKind::Html(kind));
    F::IteratorConstructor => ConstructorOrFunction, Op::IteratorConstructor;
    F::IteratorConcat => Generic, Op::IteratorConcat(ConcatKind::Create);
    F::IteratorFrom => Generic, Op::IteratorFrom;
    F::IteratorConstructorAccessor => Generic, Op::IteratorAccessor;
    F::IteratorPrototypeCreateHelper(kind) => Generic, Op::IteratorCreate(kind);
    F::IteratorPrototypeConsume(kind) => Generic, Op::IteratorConsume(ConsumeKind::Predicate(kind));
    F::IteratorPrototypeReduce => Generic, Op::IteratorConsume(ConsumeKind::Reduce);
    F::IteratorPrototypeToArray => Generic, Op::IteratorConsume(ConsumeKind::Array);
    F::IteratorPrototypeIterator => Generic, Op::Pure(target);
    F::IteratorPrototypeToStringTagGetter => Getter, Op::Pure(target);
    F::IteratorPrototypeToStringTagSetter => Setter, Op::IteratorTag;
    F::IteratorHelperResume(kind) => Generic, Op::IteratorHelper(kind);
    F::IteratorWrapResume(kind) => Generic, Op::IteratorWrap(kind);
    F::IteratorConcatNext => IteratorNext, Op::IteratorConcat(ConcatKind::Next);
    F::IteratorConcatReturn => Generic, Op::IteratorConcat(ConcatKind::Return);
    F::StringPrototypeIterator => Generic, Op::ScalarText(ScalarTextKind::Iterator);
    F::RegExpStringIteratorNext => IteratorNext, Op::RegExpIterator;
    F::SymbolRegistry(SymbolRegistryKind::For) => Generic, Op::Global(GlobalKind::SymbolFor);
    F::SymbolRegistry(SymbolRegistryKind::KeyFor) => Generic, Op::Pure(target);
    F::SymbolPrototypeDescription => Getter, Op::Pure(target);
    F::BigIntAsN(kind) => GenericMagic, Op::Numeric(NumericKind::BigIntAsN(kind));
    F::GlobalEval => Generic, Op::GlobalEval;
    F::GlobalNumberParse(kind) => Generic, Op::Global(GlobalKind::Parse(kind));
    F::GlobalNumberPredicate(kind) => Generic, Op::Global(GlobalKind::Predicate(kind));
    F::GlobalUriCodec(kind @ (Uri::Escape | Uri::Unescape)) => Generic,
        Op::Global(GlobalKind::Uri(kind));
    F::GlobalUriCodec(kind) => GenericMagic, Op::Global(GlobalKind::Uri(kind));
    F::NumberPredicate(_) => Generic, Op::Pure(target);
    F::NumberPrototypeFormat(kind) => Generic, Op::Numeric(NumericKind::Format(kind));
    F::ErrorConstructor(kind) => ConstructorOrFunctionMagic,
        Op::Error(ErrorOperation::Constructor(kind));
    F::ErrorPrototypeToString => Generic, Op::Error(ErrorOperation::ToString);
    F::ErrorIsError => Generic, Op::Pure(target);
    #[cfg(test)]
    F::ArgumentProbe => Generic, Op::Pure(target);
    #[cfg(test)]
    F::ConstructorProbe => Constructor, Op::Pure(target);
    #[cfg(test)]
    F::ConstructorOrFunctionProbe => ConstructorOrFunction, Op::Pure(target);
    #[cfg(test)]
    F::ActiveFrameProbe => Generic, Op::ActiveFrameProbe;
}
