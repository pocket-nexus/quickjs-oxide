//! Closed reply routing between domain states and the owned scheduler.

/// Generates one `TryFrom<DomainStep> for Step` adapter from a table of arms.
///
/// The header names the domain step type and its default `Resume` variant.
/// Each `Variant => shape;` item expands to one match arm that moves the
/// domain payload into the matching `Step` fields, exactly as a hand-written
/// arm would; `@ Resume` overrides the continuation variant. Irregular arms
/// stay hand-written inside the trailing `raw { ... }` block.
///
/// Shapes taking from `{ mut resume }`:
/// - `read(object, key)` / `read(pair)`: `Step::Read` with an object receiver.
/// - `set(object, key, value)` / `set_keyed(...) @ SetResume`: `Step::Set`.
/// - `construct(target, arguments)`: validated `Step::Construct`.
/// - `state_read(pair)`: `Step::StateRead`.
/// - `Step { field: take [as wrap], ..; field: expr, .. }`: generic moves.
///
/// Shapes destructuring `{ fields.., resume }`:
/// - `{ object, key } => read`.
/// - `{ fields.. } => Step [{ field: expr, .. }]`: generic moves.
macro_rules! step_from {
    (@arms [$(#[$meta:meta])*] [$ty:ty] $R:ident [$($acc:tt)*] $(raw { $($raw:tt)* })?) => {
        $(#[$meta])*
        impl TryFrom<$ty> for Step {
            type Error = crate::engine::api::RuntimeError;
            fn try_from(step: $ty) -> Result<Self, Self::Error> {
                type T = $ty;
                Ok(match step {
                    $($acc)*
                    $($($raw)*)?
                })
            }
        }
    };
    (@resume $R:ident $RV:ident; $resume:ident) => { Resume::$RV($resume) };
    (@resume $R:ident; $resume:ident) => { Resume::$R($resume) };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => read($object:ident, $key:ident) $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let object = resume.$object();
                let key = resume.$key();
                Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => read($pair:ident) $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let (object, key) = resume.$pair();
                Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => state_read($pair:ident) $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let (effect, atom) = resume.$pair();
                Self::StateRead {
                    effect: Some(effect),
                    atom: Some(atom),
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => set($object:ident, $key:ident, $value:ident) $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let object = resume.$object();
                let key = resume.$key();
                let value = resume.$value();
                let receiver = match object.try_clone() {
                    Ok(receiver) => receiver,
                    Err(error) => {
                        let _ = object.runtime().release_jsvalue(value);
                        return Err(error);
                    }
                };
                Self::Set {
                    receiver: Some(JsValue::Object(receiver.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    value: Some(value),
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => set_keyed($object:ident, $key:ident, $value:ident) @ $RV:ident; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let object = resume.$object();
                let key = resume.$key();
                let value = resume.$value();
                let receiver = match object.try_clone() {
                    Ok(receiver) => receiver,
                    Err(error) => {
                        let _ = object.runtime().release_jsvalue(value);
                        return Err(error);
                    }
                };
                let scheduler_key = match key.try_clone() {
                    Ok(key) => key,
                    Err(error) => {
                        let _ = object.runtime().release_jsvalue(value);
                        return Err(error);
                    }
                };
                Self::Set {
                    receiver: Some(JsValue::Object(receiver.into_handle())),
                    object: Some(object),
                    key: Some(scheduler_key),
                    value: Some(value),
                    resume: Some(Resume::$RV {
                        resume: resume.with_scheduler_set_key(key),
                    }),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => call($callable:ident, $receiver:ident $(, $arguments:ident)?) $(@ $RV:ident)?;
        $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let callable = resume.$callable();
                let receiver = resume.$receiver();
                let arguments = step_from!(@take resume $($arguments)?);
                Self::Call {
                    target: Some(crate::engine::vm::call::DirectCallTarget::Callable(callable)),
                    receiver: Some(receiver),
                    arguments: Some(arguments),
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@take $resume:ident $take:ident) => { $resume.$take() };
    (@take $resume:ident) => { Vec::new() };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => construct($target:ident, $arguments:ident) $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                let target = resume.$target();
                let new_target = target.try_clone()?;
                let arguments = resume.$arguments();
                Self::Construct {
                    new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                        new_target,
                    )),
                    target: Some(target),
                    arguments: Some(arguments),
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident { object, key } => read $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { object, key, resume } => Self::Read {
                receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                object: Some(object),
                key: Some(key),
                resume: Some(step_from!(@resume $R $($RV)?; resume)),
            },
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident => $S:ident {
            $($field:ident: $take:ident $(as $wrap:path)?),+ $(; $($cf:ident: $ce:expr),+)?
        } $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { mut resume } => {
                $(
                    let $field = resume.$take();
                    $(let $field = $wrap($field);)?
                )+
                Self::$S {
                    $($field: Some($field),)+
                    $($($cf: Some($ce),)+)?
                    resume: Some(step_from!(@resume $R $($RV)?; resume)),
                }
            }
        ] $($rest)*);
    };
    (@arms $m:tt $t:tt $R:ident [$($acc:tt)*]
        $V:ident { $($bind:ident),* } => $S:ident $({ $($cf:ident: $ce:expr),+ })?
        $(@ $RV:ident)?; $($rest:tt)*) => {
        step_from!(@arms $m $t $R [$($acc)*
            T::$V { $($bind,)* resume } => Self::$S {
                $($bind: Some($bind),)*
                $($($cf: Some($ce),)+)?
                resume: Some(step_from!(@resume $R $($RV)?; resume)),
            },
        ] $($rest)*);
    };
    ($(#[$meta:meta])* $ty:ty => $R:ident; $($items:tt)*) => {
        step_from!(@arms [$(#[$meta])*] [$ty] $R [] $($items)*);
    };
}

/// Declares `Step` and generates `Step::release_owned` from one table.
///
/// Every field is stored as `Option<Type>` so dispatchers can drain fields in
/// place. `=> kind` names how an abandoned field releases its owned values;
/// fields without a kind are dropped with the packet. A variant followed by
/// `=> { ... }` (or `=> |payload| { ... }` for tuple variants) releases its
/// fields with that hand-written block instead.
macro_rules! step_enum {
    (@pat $field:ident $kind:ident) => { $field };
    (@pat $field:ident) => { _ };
    (@release $runtime:ident value $field:ident) => {
        if let Some(value) = $field {
            let _ = $runtime.release_jsvalue(value);
        }
    };
    (@release $runtime:ident resume $field:ident) => {
        if let Some(value) = $field {
            value.release_owned($runtime);
        }
    };
    (@release $runtime:ident completion $field:ident) => {
        if let Some(value) = $field {
            let (Completion::Return(value) | Completion::Throw(value)) = value;
            let _ = $runtime.release_jsvalue(value);
        }
    };
    (@release $runtime:ident values $field:ident) => {
        if let Some(values) = $field {
            for value in values {
                let _ = $runtime.release_jsvalue(value);
            }
        }
    };
    (@release $runtime:ident release $field:ident) => {
        if let Some(value) = $field {
            value.release($runtime);
        }
    };
    (@release $runtime:ident try_release $field:ident) => {
        if let Some(value) = $field {
            let _ = value.release($runtime);
        }
    };
    (@release $runtime:ident throw $field:ident) => {
        if let Some(NativeConversion::Throw(value)) = $field {
            let _ = $runtime.release_jsvalue(value);
        }
    };
    (@release $runtime:ident throw_values $field:ident) => {
        if let Some(value) = $field {
            match value {
                NativeConversion::Value(values) => {
                    for value in values {
                        let _ = $runtime.release_jsvalue(value);
                    }
                }
                NativeConversion::Throw(value) => {
                    let _ = $runtime.release_jsvalue(value);
                }
            }
        }
    };
    (@release $runtime:ident some_value $field:ident) => {
        if let Some(Some(value)) = $field {
            let _ = $runtime.release_jsvalue(value);
        }
    };
    (@release $runtime:ident request $field:ident) => {
        if let Some(mut value) = $field {
            let _ = value.release_owned_values($runtime);
        }
    };
    (@release $runtime:ident length_throw $field:ident) => {
        if let Some(ArrayLengthConversion::Throw(value)) = $field {
            let _ = $runtime.release_jsvalue(value);
        }
    };
    (@release $runtime:ident set_action $field:ident) => {
        if let Some(value) = $field {
            SetStep::Complete(value).release($runtime);
        }
    };
    (@munch $runtime:ident [$($variants:tt)*] [$($arms:tt)*]) => {
        pub(super) enum Step {
            $($variants)*
        }

        impl Step {
            /// Drain a request abandoned before its consumer takes the owned fields.
            /// Replacing with an empty terminal makes cleanup safe after partial takes.
            pub(super) fn release_owned(&mut self, $runtime: &Runtime) {
                match std::mem::replace(self, Self::Complete(None)) {
                    $($arms)*
                }
            }
        }
    };
    (@munch $runtime:ident [$($variants:tt)*] [$($arms:tt)*]
        $V:ident { $($field:ident: $ty:ty),* $(,)? } => $body:block, $($rest:tt)*) => {
        step_enum!(@munch $runtime [$($variants)* $V { $($field: Option<$ty>),* },]
            [$($arms)* Self::$V { $($field),* } => $body] $($rest)*);
    };
    (@munch $runtime:ident [$($variants:tt)*] [$($arms:tt)*]
        $V:ident { $($field:ident: $ty:ty $(=> $kind:ident)?),* $(,)? }, $($rest:tt)*) => {
        step_enum!(@munch $runtime [$($variants)* $V { $($field: Option<$ty>),* },]
            [$($arms)* Self::$V { $($field: step_enum!(@pat $field $($kind)?)),* } => {
                $($(step_enum!(@release $runtime $kind $field);)?)*
            }] $($rest)*);
    };
    (@munch $runtime:ident [$($variants:tt)*] [$($arms:tt)*]
        $V:ident($ty:ty) => |$payload:ident| $body:block, $($rest:tt)*) => {
        step_enum!(@munch $runtime [$($variants)* $V(Option<$ty>),]
            [$($arms)* Self::$V($payload) => $body] $($rest)*);
    };
    (@munch $runtime:ident [$($variants:tt)*] [$($arms:tt)*]
        $V:ident($ty:ty => $kind:ident), $($rest:tt)*) => {
        step_enum!(@munch $runtime [$($variants)* $V(Option<$ty>),]
            [$($arms)* Self::$V(value) => step_enum!(@release $runtime $kind value),] $($rest)*);
    };
    (@munch $runtime:ident [$($variants:tt)*] [$($arms:tt)*]
        $V:ident($ty:ty), $($rest:tt)*) => {
        step_enum!(@munch $runtime [$($variants)* $V(Option<$ty>),]
            [$($arms)* Self::$V(_) => {}] $($rest)*);
    };
    (release_owned($runtime:ident); $($table:tt)*) => {
        step_enum!(@munch $runtime [] [] $($table)*);
    };
}

/// Routes one reply to each listed `Resume` variant's domain continuation,
/// converting the domain's next step; the trailing arms stay hand-written.
macro_rules! forward {
    ($resume:ident {
        $($method:ident $args:tt => $($(#[$meta:meta])* $V:ident),+;)*
        $(@set_keyed($runtime:ident, $result:expr) => $($K:ident),+;)?
    } $($rest:tt)*) => {
        match $resume {
            $($($(#[$meta])* Self::$V(resume) => resume.$method $args.and_then(Step::try_from),)+)*
            $($(Self::$K { mut resume } => {
                let key = resume.take_scheduler_set_key();
                resume.set($runtime, key, $result).and_then(Step::try_from)
            })+)?
            $($rest)*
        }
    };
}

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
#[cfg(test)]
use crate::engine::test_support::test_context;
use crate::engine::vm::ToPrimitiveHint;

use crate::engine::object::operations::ArrayLengthConversion;
use crate::engine::object::{ArrayLengthResume, ArrayLengthStep};
use crate::engine::value::conversion::number::{NumberResume, NumberStep};

use crate::engine::builtins::native::TypedArrayElementKind;
use crate::engine::builtins::{ElementResume, ElementStep, TypedWriteResume, TypedWriteStep};

use crate::engine::object::{ProxyPrototypeResume, ProxyPrototypeStep};

pub(super) struct BooleanResultPayload {
    pub(super) _object: ObjectRef,
    pub(super) _key: Option<PropertyKey>,
    pub(super) strict_delete: bool,
}
pub(super) struct DefineTypedPayload {
    pub(super) object: ObjectRef,
    pub(super) _descriptor: crate::engine::object::OwnedPropertyDescriptor,
    pub(super) resume: Box<Resume>,
}
pub(super) struct DefineLengthPayload {
    pub(super) object: ObjectRef,
    pub(super) key: PropertyKey,
    pub(super) descriptor: crate::engine::object::OwnedPropertyDescriptor,
    pub(super) resume: Box<Resume>,
}

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
    ArrayMutationSet {
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

step_enum! {
    release_owned(runtime);
    RootDescriptor(crate::engine::vm::entry::DescriptorReply => throw),
    ModuleCallbackOperation { step: Box<crate::engine::modules::callback::CallbackStep> => release,
        resume: Resume => resume },
    ModuleBodyOperation { step: Box<crate::engine::modules::body::BodyStep> => release,
        resume: Resume => resume },
    ModuleLink { realm: crate::engine::heap::ContextId,
        callable: crate::engine::object::CallableRef, resume: Resume => resume },
    PromiseOperation {
        step: Box<crate::engine::builtins::promise::operation::PromiseStep> => release,
        resume: Resume => resume },
    IntrinsicPromiseResolve { value: JsValue => value, realm: crate::engine::heap::ContextId,
        resume: Resume => resume },
    ResumeFrame { activation: Box<crate::engine::vm::suspend::RootedVmActivation>,
        input: crate::engine::vm::suspend::VmActivationResume, resume: Resume } => {
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
                ) => {
                    let _ = runtime.release_jsvalue(value);
                }
            }
        }
        if let Some(value) = resume {
            value.release_owned(runtime);
        }
        drop(activation);
    },
    ForInComplete { value: JsValue => value, done: Option<bool> },
    TypedIteratorMethod { source: JsValue => value, resume: Resume => resume },
    TypedIteratorMethodComplete(
        NativeConversion<Option<crate::engine::object::CallableRef>> => throw
    ),
    TypedCollect { source: JsValue => value, method: crate::engine::object::CallableRef,
        element: TypedArrayElementKind, resume: Resume => resume },
    TypedCollectComplete(NativeConversion<Vec<JsValue>> => throw_values),
    TypedCreate { constructor: JsValue => value, length: u64, resume: Resume => resume },
    NumericComplete { value: JsValue => value, previous: Option<JsValue> => some_value },
    TypedSpeciesView { source: ObjectRef, element: TypedArrayElementKind, buffer: ObjectRef,
        offset: u64, length: Option<u64>, resume: Resume => resume },
    RegExpSpecies { regexp: ObjectRef, resume: Resume => resume },
    RegExpSpeciesComplete(NativeConversion<crate::engine::vm::call::ConstructorRef> => throw),
    IndirectEval { source: crate::engine::value::JsString, resume: Resume => resume },
    Aggregate { iterable: JsValue => value, resume: Resume => resume },
    OrdinaryPrimitive { object: ObjectRef, hint: ToPrimitiveHint },
    ConstructorSource { new_target: JsValue => value, resume: Resume => resume },
    ConstructorSourceComplete(
        NativeConversion<crate::engine::vm::call::ConstructorPrototypeSource> => throw
    ),
    TypedSpecies { source: ObjectRef, element: TypedArrayElementKind, length: u64,
        resume: Resume => resume },
    TypedSpeciesComplete(NativeConversion<ObjectRef> => throw),
    ArrayCopy { object: ObjectRef, to: u64, from: u64, count: u64, backwards: bool,
        resume: Resume => resume },
    OrdinaryInstance { constructor: crate::engine::object::CallableRef, value: JsValue => value,
        resume: Resume => resume },
    ParseIterator { result: Completion => completion, resume: Resume => resume },
    String { value: JsValue => value, resume: Resume => resume },
    ObjectTag { receiver: JsValue => value },
    RegExpExec { regexp: JsValue => value, input: JsValue => value, resume: Resume => resume },
    IteratorCloseWithResume { iterator: ObjectRef, completion: Completion => completion,
        resume: Resume => resume },
    NativeRawComplete(crate::engine::vm::call::NativeInvokeOutcome) => |value| {
        if let Some(value) = value {
            match value {
                crate::engine::vm::call::NativeInvokeOutcome::Completion(
                    Completion::Return(value) | Completion::Throw(value),
                )
                | crate::engine::vm::call::NativeInvokeOutcome::IteratorNextRaw {
                    value, ..
                } => {
                    let _ = runtime.release_jsvalue(value);
                }
            }
        }
    },
    ArraySpecies { source: ObjectRef, length: u64, resume: Resume => resume },
    ArrayPush { object: ObjectRef, value: JsValue => value, resume: Resume => resume },
    IteratorNext { iterator: ObjectRef, method: JsValue => value, resume: Resume => resume },
    IteratorNextComplete(crate::engine::builtins::ObjectIteratorStep) => |value| {
        if let Some(value) = value {
            match value {
                crate::engine::builtins::ObjectIteratorStep::Yield(value)
                | crate::engine::builtins::ObjectIteratorStep::Throw(value) => {
                    let _ = runtime.release_jsvalue(value);
                }
                crate::engine::builtins::ObjectIteratorStep::Done => {}
            }
        }
    },
    IteratorCall { callable: crate::engine::object::CallableRef, iterator: ObjectRef,
        resume: crate::engine::builtins::IteratorNextResume },
    IteratorClose { iterator: ObjectRef, completion: Completion => completion },
    Native { callable: crate::engine::object::CallableRef,
        target: crate::engine::builtins::native::NativeFunctionId,
        defining_realm: crate::engine::heap::ContextId, min_readable_args: u8,
        mode: crate::engine::vm::call::NativeInvokeMode,
        invocation: crate::engine::vm::call::NativeInvocation => try_release,
        arguments: Vec<JsValue> => values, resume: Resume => resume },
    Construct { target: crate::engine::vm::call::ConstructorRef,
        new_target: crate::engine::vm::call::ConstructNewTarget => try_release,
        arguments: Vec<JsValue> => values, resume: Resume => resume },
    ConstructProxy { target: crate::engine::vm::call::ConstructorRef,
        new_target: crate::engine::vm::call::ConstructNewTarget => try_release,
        arguments: Vec<JsValue> => values, resume: Resume => resume },
    ConstructorReady { request: Box<BytecodeCallRequest> => request,
        receiver: Completion => completion, derived: bool, resume: Resume => resume },
    Arguments { value: JsValue => value, resume: Resume => resume },
    ArgumentsComplete(NativeConversion<Vec<JsValue>> => throw_values),
    SnapshotEnumerable { object: ObjectRef, key: PropertyKey, resume: Resume => resume },
    OwnFlag { object: ObjectRef, key: PropertyKey, enumerable: bool, resume: Resume => resume },
    Keys { object: ObjectRef, resume: Resume => resume },
    KeysComplete(NativeConversion<Vec<PropertyKey>> => throw),
    ReadValue { receiver: JsValue => value, key: PropertyKey, resume: Resume => resume },
    PreparedHas { probe: crate::engine::object::PreparedHas, key: PropertyKey,
        resume: Resume => resume },
    StateRead { effect: crate::engine::object::StateReadEffect, atom: crate::engine::atom::Atom,
        resume: Resume } => {
        if !runtime.skip_cleanup() {
            let mut state = runtime.0.state.borrow_mut();
            if let Some(effect) = effect {
                let _ = effect.release_in_state(&mut state, runtime);
            }
            if !runtime.0.poisoned.get()
                && let Some(atom) = atom
                && state.release_atoms([atom]).is_err()
            {
                runtime.0.poisoned.set(true);
            }
        }
        if let Some(resume) = resume {
            resume.release_owned(runtime);
        }
    },
    PreparedOwn { method: crate::engine::object::internal_methods::StateMethodStep,
        atom: crate::engine::atom::Atom, resume: Resume } => {
        if !runtime.skip_cleanup() {
            let mut state = runtime.0.state.borrow_mut();
            if let Some(method) = method
                && method.release_in_state(runtime, &mut state).is_err()
            {
                runtime.0.poisoned.set(true);
            }
            if !runtime.0.poisoned.get()
                && let Some(atom) = atom
                && state.release_atoms([atom]).is_err()
            {
                runtime.0.poisoned.set(true);
            }
        }
        if let Some(resume) = resume {
            resume.release_owned(runtime);
        }
    },
    PreparedRead { read: OrdinaryRead => release, key: PropertyKey, resume: Resume => resume },
    Primitive { value: JsValue => value, hint: crate::engine::vm::ToPrimitiveHint,
        resume: Resume => resume },
    GetPrototype { object: ObjectRef, resume: Resume => resume },
    SetPrototype { object: ObjectRef, prototype: Option<ObjectRef>, resume: Resume => resume },
    Delete { object: ObjectRef, key: PropertyKey, resume: Resume => resume },
    PreventExtensions { object: ObjectRef, resume: Resume => resume },
    Element { element: TypedArrayElementKind, value: JsValue => value, resume: Resume => resume },
    ElementComplete(NativeConversion<[u8; 8]> => throw),
    TypedComplete(NativeConversion<bool> => throw),
    Number { value: JsValue => value, resume: Resume => resume },
    NumberComplete(NativeConversion<f64> => throw),
    LengthComplete(ArrayLengthConversion => length_throw),
    SetLength { value: JsValue => value, resume: SetResume },
    SetComplete(PropertySetAction => set_action),
    PreparedSet { step: Box<SetStep> => release, resume: Resume => resume },
    SetContinue(SetResume),
    SetSpecial { object: ObjectRef, key: PropertyKey, value: JsValue => value,
        receiver: JsValue => value, resume: SetResume },
    Set { object: ObjectRef, key: PropertyKey, value: JsValue => value, receiver: JsValue => value,
        resume: Resume => resume },
    SetProxy { object: ObjectRef, key: PropertyKey, value: JsValue => value,
        receiver: JsValue => value, resume: Resume => resume },
    Define { object: ObjectRef, key: PropertyKey,
        descriptor: crate::engine::object::DefinitionInput, resume: Resume => resume },
    DefineOrdinary { object: ObjectRef, key: PropertyKey,
        descriptor: crate::engine::object::DefinitionInput, resume: Resume => resume },
    Defined(NativeConversion<InternalDefineResult> => throw),
    Complete(Completion => completion),
    BooleanComplete(NativeConversion<bool> => throw),
    OwnComplete(
        NativeConversion<Option<crate::engine::object::OwnedCompletePropertyDescriptor>> => throw
    ),
    Converted(NativeConversion<crate::engine::object::OwnedPropertyDescriptor> => throw),
    Has { object: ObjectRef, key: PropertyKey, resume: Resume => resume },
    Read { object: ObjectRef, key: PropertyKey, receiver: JsValue => value,
        resume: Resume => resume },
    Call { target: DirectCallTarget, receiver: JsValue => value, arguments: Vec<JsValue> => values,
        resume: Resume => resume },
    Descriptor { object: ObjectRef, key: PropertyKey, resume: Resume => resume },
    Extensible { object: ObjectRef, resume: Resume => resume },
    Convert { value: JsValue => value, resume: Resume => resume },
}

impl Resume {
    /// Real continuation owners can contain a raw constructor request.
    /// Raw domain owners use this context for explicit State cleanup. Legacy
    /// domain destructors still run outside a held State access during migration.
    pub(super) fn release_owned(self, runtime: &Runtime) {
        if runtime.skip_cleanup() {
            return;
        }
        match self {
            Self::ConstructorPrototype {
                mut request,
                resume,
            } => {
                let _ = request.release_owned_values(runtime);
                resume.release_owned(runtime);
            }
            Self::StringValue { resume, .. }
            | Self::OwnFlagReply { resume, .. }
            | Self::PrototypeGetReply(resume)
            | Self::PrototypeSetReply(resume) => resume.release_owned(runtime),
            Self::Get(resume) => resume.release_owned(runtime),
            Self::DefineTyped { payload } => payload.resume.release_owned(runtime),
            Self::DefineLength { payload } => payload.resume.release_owned(runtime),
            _ => {}
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
        forward! {
            self {
                set(runtime, set_result(action)?) => RegExpMatchAll, RegExpSplit, Environment,
                    RegExpSearch, RegExpMatch, IteratorTag, RegExpReplace, Property;
                forward(action) => OrdinarySet;
                set(set_result(action)?) => ProxySet;
                @set_keyed(runtime, set_result(action)?) => RegExpIteratorSet, ArrayConstructorSet,
                    ArraySliceSet, ArrayCopySet, ArrayConcatSet, ArrayBuildSet, ArraySortSet,
                    ArrayIndexedSet, ArrayReverseSet, ArrayMutationSet;
            }
            Self::RootSet => Ok(Step::Complete(Some(match set_result(action)? {
                NativeConversion::Throw(value) => Completion::Throw(value),
                NativeConversion::Value(result) => {
                    Completion::Return(JsValue::Bool(matches!(result, InternalSetResult::Accepted)))
                }
            }))),
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
        forward! {
            self {
                defined(result) => LiteralDefinition, Define;
                defined(runtime, result) => IteratorTag, ArrayConcat, ArrayFlatten, ArrayBuild,
                    ArrayCallback, ObjectIteration, Predicate, Definitions, Property, OrdinarySet,
                    ArraySlice;
            }
            Self::RootDefine => Ok(Step::Complete(Some(match result {
                NativeConversion::Throw(value) => Completion::Throw(value),
                NativeConversion::Value(result) => Completion::Return(JsValue::Bool(matches!(
                    result,
                    InternalDefineResult::Defined
                ))),
            }))),
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
        forward! {
            self {
                boolean(runtime, result) => Import, ForIn, Environment, Bind, JsonParse,
                    JsonStringify, Error, ArraySlice, ArrayCopy, ArrayConcat, ArrayFlatten,
                    ObjectCopy, ArraySort, ArrayIndexed, ArrayReverse, ArrayMutation, ArrayCallback,
                    Predicate, Keys, Property, BuiltinPrototype, Prototype, Boolean, Definitions;
                boolean(result) => IteratorTag;
                extensible(result) => Own;
                has(runtime, result) => Conversion;
            }
            Self::BooleanResult { payload } => runtime
                .finish_property_delete(result, payload.strict_delete)
                .map(|result| Step::Complete(Some(result))),
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
        forward! {
            self {
                resume(completion) => ModuleCallback, ModuleEvaluation;
                resume(runtime, completion) => ModuleBody, Import,
                    #[cfg(feature = "test262-host")] Test262Agent,
                    #[cfg(feature = "test262-host")] EvalScript, FromSync, Promise, Atomics,
                    LiteralDefinition, TypedCreate, TypedCollect, TypedIteratorMethod, BufferSlice,
                    TypedWith, Uint8Codec, TypedString, TypedSlice, TypedMutation, StringFactory,
                    RegExpMatchAll, RegExpSplit, RegExpIterator, RegExpSpecies, Environment, Bind,
                    FunctionText, DynamicFunction, JsonParse, JsonStringify, BufferConstructor,
                    DataViewConstructor, TypedSet, RegExpConstructor, RegExpSearch, RegExpMatch,
                    RegExpCompile, StringProtocol, TypedSort, Sum, PrimitiveConstructor,
                    DateConstructor, DatePrototype, Error, Aggregate, SetEach, SetOperation,
                    Collection, WeakComputed, ArraySlice, TypedTraversal, TypedSpecies,
                    TypedIteration, ConstructorSource, ArrayCopy, ArrayConcat, ArrayFlatten,
                    ObjectCopy, StringText, StringSearch, StringSplit, Instance, IteratorFrom,
                    IteratorWrap, IteratorConcat, ArrayBuild, ArraySort, ArrayIndexed, ArrayReverse,
                    ArrayString, RegExpExec, RegExpPresentation, RegExpReplace, IteratorConsume,
                    IteratorHelper, IteratorCreate, ArrayNext, ArrayMutation, ArrayCallback,
                    ArraySpecies, StringReplace, DataView, BufferMutation, ObjectIteration,
                    IteratorNext, IteratorClose, ProxyConstruct, ObjectString, Keys, Primitive,
                    Prototype, ProxySet, Define, Element, Number, Get, Call, Own, Boolean,
                    GeneratorPrototype, TypedSearch, MapCallback, ArrayConstructor;
                key(runtime, completion) => ObjectIterationKey, PredicateKey, PropertyKey;
                read(runtime, completion) => Arguments, Definitions, Property, Conversion;
                primitive(runtime, completion) => PrimitiveConstructorValue, NumericPrimitive,
                    DateConstructorPrimitive;
                invalid_count(runtime, completion) => IteratorInvalidCount;
                resume(crate::engine::vm::suspend::VmRunOutcome::Complete(completion,)) =>
                    Generator;
            }
            abandoned @ (Self::RootDescriptor | Self::RootDefine | Self::RootSet) => {
                abandoned.release_owned(runtime);
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);

                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "typed root received an untyped reply",
                ))
            }
            Self::ModuleLink(resume) => {
                crate::engine::modules::link::resume_reply(runtime, resume.resume(completion))
                    .and_then(Step::try_from)
            }
            Self::AsyncGenerator(resume) => resume.resume(completion).map(Step::from),
            Self::Async(resume) => resume.resume(completion).map(Step::from),
            Self::GeneratorCreate(creation) => creation
                .initial(
                    runtime,
                    crate::engine::vm::suspend::VmRunOutcome::Complete(completion),
                )
                .and_then(Step::try_from),
            Self::ForIn(_) => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "for-in requires typed reply",
            )),
            Self::PublicField => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "public field requires definition reply",
            )),
            Self::VmNumeric(resume) => resume
                .resume(runtime, completion)
                .and_then(|value| Ok(Step::try_from(value)?))
                .map_err(crate::engine::api::runtime_error::RuntimeError::Engine),
            Self::WeakConstructor(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "weak constructor requires prototype reply",
                ))
            }
            Self::RegExpIteratorSet { .. } => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "RegExp iterator requires Set reply",
                ))
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
            Self::StringValue { realm, resume } => {
                let result = match completion {
                    Completion::Return(value) => {
                        let result = runtime.string_from_primitive_jsvalue(realm, &value);
                        runtime.release_jsvalue(value)?;
                        result?
                    }
                    Completion::Throw(value) => NativeConversion::Throw(value),
                };
                resume.string(runtime, result)
            }
            Self::ConstructorPrototype { request, resume } => {
                super::construct::prototype(runtime, request, completion, *resume)
                    .map_err(crate::engine::api::runtime_error::RuntimeError::Engine)
            }
            Self::Identity => Ok(Step::Complete(Some(completion))),
            Self::BuiltinPrototype(_) => {
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "prototype builtin received an untyped reply",
                ))
            }
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
            Self::Setter => Ok(Step::SetComplete(Some(match completion {
                Completion::Return(value) => {
                    runtime.release_jsvalue(value)?;
                    PropertySetAction::Complete
                }
                Completion::Throw(value) => PropertySetAction::Throw(value),
            }))),
            abandoned @ Self::BooleanResult { .. } => {
                abandoned.release_owned(runtime);
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);

                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "boolean operation received an untyped reply",
                ))
            }
            Self::ReadOwner(_owner) => Ok(Step::Complete(Some(completion))),
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
            | Self::OrdinarySet(_)) => {
                abandoned.release_owned(runtime);
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);

                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "ordinary Set received an untyped reply",
                ))
            }
        }
    }
    pub(super) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<crate::engine::object::OwnedCompletePropertyDescriptor>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                descriptor(runtime, result) => Predicate, Keys, Get, Property, Own, Boolean,
                    OrdinarySet, ProxySet, Define;
            }
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

    pub(super) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                prototype(runtime, result) => ForIn, Instance, Predicate, BuiltinPrototype,
                    Prototype;
            }
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

    pub(super) fn converted(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::object::OwnedPropertyDescriptor>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                converted(runtime, result) => Definitions, Own, Property;
            }
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

    pub(super) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                number(runtime, result) => #[cfg(feature = "test262-host")] Test262Agent, Atomics,
                    StringFactory, JsonParse, JsonStringify, TypedSort, Math, Global, Numeric,
                    ScalarText, DateConstructor, DatePrototype, SetOperation, ArraySlice,
                    ArrayConcat, ArrayFlatten, ArrayBuild, ArraySort, ArrayIndexed, ArrayReverse,
                    ArrayString, IteratorCreate, ArrayNext, ArrayMutation, ArrayCallback, Arguments,
                    LengthNumber, Keys;
            }
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
        forward! {
            self {
                keys(runtime, result) => Import, ForIn, JsonParse, JsonStringify, ObjectCopy,
                    Definitions, Keys, Property;
            }
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

    pub(super) fn arguments(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Invoke(resume) => resume.arguments(runtime, result).and_then(Step::try_from),
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
                    "argument list has no matching continuation",
                ))
            }
        }
    }

    pub(super) fn iterator_next(
        self,
        runtime: &Runtime,
        result: crate::engine::builtins::ObjectIteratorStep,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                next(runtime, result) => Promise, Collection, IteratorWrap, IteratorConcat,
                    IteratorConsume, IteratorHelper, ObjectIteration;
                item(runtime, result) => Sum, Aggregate;
                parsed(runtime, result) => SetOperation, ArrayBuild;
            }
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

    fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::value::JsString>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                string(runtime, result) => StringFactory, RegExpIterator, FunctionText,
                    DynamicFunction, JsonParse, JsonStringify, Global, ScalarText, DateConstructor,
                    Error, ArraySort, ArrayString, PrimitiveConstructor;
            }
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
            Self::JsonRaw(resume) => resume
                .string(runtime, result)
                .map(|result| Step::Complete(Some(result))),
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

    pub(super) fn constructor_source(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::vm::call::ConstructorPrototypeSource>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                prototype(runtime, result) => Promise, TypedCreate, WeakConstructor,
                    ObjectConstructor, BufferConstructor, DataViewConstructor, RegExpConstructor,
                    Collection, IteratorConstructor;
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
        forward! {
            self {
                element(runtime, result) => TypedCreate, TypedMutation, TypedSet, TypedIteration,
                    TypedElement;
            }
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
        forward! {
            self {
                created(runtime, result) => TypedCreate;
                species(runtime, result) => TypedSlice, TypedIteration;
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

    pub(super) fn regexp_species(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::vm::call::ConstructorRef>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        forward! {
            self {
                species(runtime, result) => RegExpMatchAll, RegExpSplit;
            }
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

// Reply transport layout is hot-path sensitive; generated tables must not move it.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<Step>() == 184);
    assert!(size_of::<Resume>() == 32);
    assert!(size_of::<super::PendingProxyGet>() == 192);
};

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

#[cfg(test)]
#[test]
fn selected_own_carrier_cancellation_releases_exact_method_and_key() {
    use crate::engine::{
        api::Value,
        object::internal_methods::{StateMethodStep, StateOwnPrefix},
    };
    let (runtime, mut context) = test_context();
    let Value::Object(proxy) = context
        .eval("new Proxy({}, {get getOwnPropertyDescriptor(){throw 42}})")
        .unwrap()
    else {
        panic!()
    };
    let strong = std::rc::Rc::strong_count(&runtime.0);
    let mut pending = {
        let mut state = runtime.0.state.borrow_mut();
        let StateOwnPrefix::Effect(method) =
            StateOwnPrefix::select(&runtime, &mut state, context.realm, proxy.object_id()).unwrap()
        else {
            panic!()
        };
        assert!(matches!(method, StateMethodStep::Read { .. }));
        let atom = state.atoms.intern("own-selected-key").unwrap();
        Step::PreparedOwn {
            method: Some(method),
            atom: Some(atom),
            resume: Some(Resume::RootDescriptor),
        }
    };
    assert_eq!(runtime.0.proxy_method_depth.get(), 1);
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
    pending.release_owned(&runtime);
    assert!(matches!(pending, Step::Complete(None)));
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}
