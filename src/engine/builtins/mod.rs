use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::builtins::native::{
    DateGetFieldKind, DateNativeKind, DateSetFieldKind, DateStringMethod, NativeFunctionId,
};
use crate::engine::code::bytecode_publish;
use crate::engine::code::function::UnlinkedFunction;
use crate::source::QuickJsSourceLocator;

use crate::engine::api::compile::Compilation;
use crate::engine::code::function::metadata::{
    ClosureSource, ClosureVariableKind, ClosureVariableName,
};
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::compiler::DEFAULT_EVAL_FILENAME;
use crate::engine::heap::roots::VarRefRoot;

use crate::engine::heap::{AutoInitProperty, ContextId, PropertySlot};
#[cfg(test)]
use crate::engine::host::HostServices;
use crate::engine::object::operations::{InternalSetResult, PropertySetRejection};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{
    CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
    WellKnownSymbol,
};
use crate::engine::realm::bindings::GlobalBindingCreationMode;
use crate::engine::value::conversion::NativeConversion;

use crate::engine::value::{JsString, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};
use crate::engine::vm::frames::ExplicitBacktraceLocation;

mod array;
mod array_buffer;
mod atomics;
pub(crate) use array_buffer::typed_array::CanonicalNumericIndex;
pub(crate) use array_buffer::typed_array::write::TypedWriteStep;

pub(crate) use array_buffer::typed_array::{
    element::{ElementResume, ElementStep},
    write::TypedWriteResume,
};
pub(crate) mod date;
mod error;
mod eval;

pub(crate) use eval::DirectEvalPreparation;

pub(crate) mod continuation;

pub(crate) use function::arguments::{ArgumentsResume, ArgumentsStep};

pub(crate) use function::invoke::{InvokeResume, InvokeStep};
mod iterator;
mod json;
mod map;
mod math;
mod object;

pub(crate) use object::definitions::{DefinitionsResume, DefinitionsStep};

pub(crate) use object::predicate::{PredicateResume, PredicateStep};

pub(crate) use object::property::{PropertyResume, PropertyStep};

pub(crate) use object::prototype::{BuiltinPrototypeResume, BuiltinPrototypeStep};

pub(crate) use object::string::{ObjectStringResume, ObjectStringStep};
pub(crate) mod promise;
mod proxy;
mod reflect;
mod regexp;
mod replacement;
mod set;
mod shared_array_buffer;
mod string;
pub mod uri;
mod weak_collection;
mod weak_ref;

/// Pinned QuickJS `JS_ToInt64Free` for an already numeric value.
///
/// Rust's float-to-integer cast saturates outside the signed range. QuickJS
/// instead preserves the low 64 bits while the binary exponent remains close
/// enough to the mantissa, and maps still larger magnitudes to zero.
fn quickjs_to_int64_free(number: f64) -> i64 {
    const EXPONENT_BIAS: u64 = 1023;
    const MANTISSA_BITS: u64 = 52;
    const MANTISSA_MASK: u64 = (1_u64 << MANTISSA_BITS) - 1;

    let bits = number.to_bits();
    let exponent = (bits >> MANTISSA_BITS) & 0x7ff;
    if exponent <= EXPONENT_BIAS + 62 {
        return number as i64;
    }
    if exponent <= EXPONENT_BIAS + 62 + 53 {
        let significand = (bits & MANTISSA_MASK) | (1_u64 << MANTISSA_BITS);
        let shift = u32::try_from(exponent - EXPONENT_BIAS - MANTISSA_BITS)
            .expect("QuickJS ToInt64 exponent shift fits u32");
        let signed = (significand << shift) as i64;
        return if bits >> 63 == 0 {
            signed
        } else {
            signed.wrapping_neg()
        };
    }
    0
}

impl Runtime {
    /// Perform ordinary throwing Set for builtin algorithms which publish
    /// values through `[[Set]]` rather than CreateDataProperty.
    pub(crate) fn set_property_or_throw(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> Result<Option<Value>, RuntimeError> {
        self.finish_set_property_or_throw(
            realm,
            key,
            self.internal_set(realm, object, key, value, Value::Object(object.clone()))?,
        )
    }

    pub(crate) fn finish_set_property_or_throw(
        &self,
        realm: ContextId,
        key: &PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<Option<Value>, RuntimeError> {
        match result {
            NativeConversion::Value(InternalSetResult::Accepted) => Ok(None),
            NativeConversion::Throw(value) => Ok(Some(value)),
            NativeConversion::Value(result) => {
                let error = match result {
                    InternalSetResult::RejectedProxyTrap => {
                        Error::new(ErrorKind::Type, "proxy: cannot set property")
                    }
                    InternalSetResult::Rejected(PropertySetRejection::ReadOnly) => {
                        self.native_atom_error(ErrorKind::Type, "'", key, "' is read-only")?
                    }
                    InternalSetResult::Rejected(PropertySetRejection::ArrayLengthReadOnly) => {
                        let length = self
                            .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
                        self.native_atom_error(ErrorKind::Type, "'", &length, "' is read-only")?
                    }
                    InternalSetResult::Rejected(PropertySetRejection::NotConfigurable) => {
                        Error::new(ErrorKind::Type, "not configurable")
                    }
                    InternalSetResult::Rejected(PropertySetRejection::NoSetter) => {
                        Error::new(ErrorKind::Type, "no setter for property")
                    }
                    InternalSetResult::Rejected(PropertySetRejection::NotExtensible) => {
                        Error::new(ErrorKind::Type, "object is not extensible")
                    }
                    InternalSetResult::Rejected(PropertySetRejection::NotObject) => {
                        Error::new(ErrorKind::Type, "not an object")
                    }
                    InternalSetResult::Accepted => unreachable!("accepted Set returned above"),
                };
                Ok(Some(self.new_native_error_from_error(
                    realm,
                    NativeErrorKind::Type,
                    &error,
                )?))
            }
        }
    }
}

pub(crate) mod dispatch;

pub(crate) mod buffer_access;

pub(crate) mod qjs_host;

pub(crate) mod qjs_value_printer;

pub(crate) mod function;

pub(crate) mod primitive;

pub(crate) mod native;

pub(crate) use array::callback::{
    CallbackResume as ArrayCallbackResume, CallbackStep as ArrayCallbackStep,
};

pub(crate) use array::mutation::{
    MutationResume as ArrayMutationResume, MutationStep as ArrayMutationStep,
};

pub(crate) use array::species::{
    SpeciesResume as ArraySpeciesResume, SpeciesStep as ArraySpeciesStep,
};

pub(crate) use array_buffer::{
    BufferMutationResume, BufferMutationStep, DataViewAccessResume, DataViewAccessStep,
};
pub(crate) use iterator::step::CloseStep as IteratorCloseStep;

pub(crate) use iterator::step::{
    CloseResume as IteratorCloseResume, NextResume as IteratorNextResume,
    NextStep as IteratorNextStep,
};

pub(crate) use object::iteration::{
    IterationResume as ObjectIterationResume, IterationStep as ObjectIterationStep,
};

pub(crate) use string::{StringReplaceResume, StringReplaceStep};

pub(crate) use array::mutation::MutationKind as ArrayMutationKind;

pub(crate) use iterator::array::{ArrayNextResume, ArrayNextStep};

pub(crate) use object::ObjectIteratorStep;

pub(crate) use array::sort::{SortResume as ArraySortResume, SortStep as ArraySortStep};

pub(crate) use array::indexed::{
    IndexedResume as ArrayIndexedResume, IndexedStep as ArrayIndexedStep,
};

pub(crate) use array::reverse::{
    ReverseResume as ArrayReverseResume, ReverseStep as ArrayReverseStep,
};

pub(crate) use array::string::{ArrayStringResume, ArrayStringStep};

pub(crate) use regexp::{RegExpExecResume, RegExpExecStep};

pub(crate) use regexp::{RegExpPresentationResume, RegExpPresentationStep};

pub(crate) use regexp::{RegExpReplaceResume, RegExpReplaceStep};

pub(crate) use iterator::consume::{
    ConsumeResume as IteratorConsumeResume, ConsumeStep as IteratorConsumeStep,
};

pub(crate) use iterator::helper::{
    HelperResume as IteratorHelperResume, HelperResumeStep as IteratorHelperStep,
};

pub(crate) use iterator::create::{
    CreateResume as IteratorCreateResume, CreateStep as IteratorCreateStep,
};

pub(crate) use object::string::ObjectStringKind;

pub(crate) use array::build::{BuildResume as ArrayBuildResume, BuildStep as ArrayBuildStep};

pub(crate) use function::instance::{InstanceResume, InstanceStep};

pub(crate) use iterator::concat::{
    ConcatResume as IteratorConcatResume, ConcatStep as IteratorConcatStep,
};

pub(crate) use iterator::from::{FromResume as IteratorFromResume, FromStep as IteratorFromStep};

pub(crate) use iterator::wrap::{WrapResume as IteratorWrapResume, WrapStep as IteratorWrapStep};

pub(crate) use array::copy::{CopyResume as ArrayCopyResume, CopyStep as ArrayCopyStep};

pub(crate) use array::concat::{ConcatResume as ArrayConcatResume, ConcatStep as ArrayConcatStep};

pub(crate) use array::flatten::{
    FlattenResume as ArrayFlattenResume, FlattenStep as ArrayFlattenStep,
};

pub(crate) use object::copy::{CopyResume as ObjectCopyResume, CopyStep as ObjectCopyStep};

pub(crate) use string::{StringTextResume, StringTextStep};

pub(crate) use string::{StringSearchResume, StringSearchStep};

pub(crate) use string::{StringSplitResume, StringSplitStep};

pub(crate) use array::constructor::{
    ConstructorResume as ArrayConstructorResume, ConstructorStep as ArrayConstructorStep,
};

pub(crate) use array::slice::{SliceResume as ArraySliceResume, SliceStep as ArraySliceStep};

pub(crate) use iterator::constructor::{
    ConstructorResume as IteratorConstructorResume, ConstructorStep as IteratorConstructorStep,
};

pub(crate) use iterator::entry::{
    TagSetterResume as IteratorTagResume, TagSetterStep as IteratorTagStep,
};

pub(crate) use array_buffer::typed_array::{TypedTraversalResume, TypedTraversalStep};

pub(crate) use array_buffer::typed_array::{TypedSpeciesResume, TypedSpeciesStep};

pub(crate) use array_buffer::typed_array::{TypedIterationResume, TypedIterationStep};

pub(crate) use iterator::collection::{CollectionResume, CollectionStep};

pub(crate) use map::callback::{
    CallbackResume as MapCallbackResume, CallbackStep as MapCallbackStep,
};

pub(crate) use set::callback::{EachResume as SetEachResume, EachStep as SetEachStep};

pub(crate) use set::operations::{SetResume as SetOperationResume, SetStep as SetOperationStep};

pub(crate) use weak_collection::computed::{
    ComputedResume as WeakComputedResume, ComputedStep as WeakComputedStep,
};

pub(crate) use math::operation::{MathResume, MathStep};

pub(crate) use math::sum::{SumResume, SumStep};

pub(crate) use primitive::constructor::{PrimitiveConstructorResume, PrimitiveConstructorStep};

pub(crate) use primitive::globals::{GlobalResume, GlobalStep};

pub(crate) use primitive::numeric::{NumericResume, NumericStep};

pub(crate) use primitive::text::{ScalarTextResume, ScalarTextStep};

pub(crate) use date::{DateConstructorResume, DateConstructorStep};

pub(crate) use date::{DatePrototypeResume, DatePrototypeStep};

pub(crate) use error::operation::{ErrorResume, ErrorStep};

pub(crate) use error::aggregate::{AggregateResume, AggregateStep};

pub(crate) use array_buffer::typed_array::{TypedSortResume, TypedSortStep};

pub(crate) use object::constructor::{ObjectConstructorResume, ObjectConstructorStep};

pub(crate) use function::bind::{BindResume, BindStep};

pub(crate) use function::text::{FunctionTextResume, FunctionTextStep};

pub(crate) use function::dynamic::{DynamicFunctionResume, DynamicFunctionStep};

pub(crate) use json::{JsonParseResume, JsonParseStep};

pub(crate) use json::{JsonStringifyResume, JsonStringifyStep};

pub(crate) use array_buffer::{BufferConstructorResume, BufferConstructorStep};

pub(crate) use array_buffer::{DataViewConstructorResume, DataViewConstructorStep};

pub(crate) use array_buffer::typed_array::{TypedSetResume, TypedSetStep};

pub(crate) use regexp::{RegExpConstructorResume, RegExpConstructorStep};

pub(crate) use regexp::{RegExpSearchResume, RegExpSearchStep};

pub(crate) use regexp::{RegExpMatchResume, RegExpMatchStep};

pub(crate) use regexp::{RegExpCompileResume, RegExpCompileStep};

pub(crate) use string::{StringProtocolResume, StringProtocolStep};

pub(crate) use json::JsonRawResume;

pub(crate) use regexp::{RegExpMatchAllResume, RegExpMatchAllStep};

pub(crate) use regexp::{RegExpSplitResume, RegExpSplitStep};

pub(crate) use regexp::{RegExpIteratorResume, RegExpIteratorStep};

pub(crate) use regexp::{RegExpSpeciesResume, RegExpSpeciesStep};

pub(crate) use weak_ref::constructor::{WeakConstructorResume, WeakConstructorStep};

pub(crate) use array_buffer::typed_array::{TypedSearchResume, TypedSearchStep};

pub(crate) use array_buffer::typed_array::{TypedStringResume, TypedStringStep};

pub(crate) use array_buffer::typed_array::{TypedSliceResume, TypedSliceStep};

pub(crate) use array_buffer::typed_array::{TypedMutationResume, TypedMutationStep};

pub(crate) use string::{StringFactoryResume, StringFactoryStep};

pub(crate) use array_buffer::{BufferSliceResume, BufferSliceStep};

pub(crate) use array_buffer::typed_array::{TypedWithResume, TypedWithStep};

pub(crate) use array_buffer::typed_array::{Uint8CodecResume, Uint8CodecStep};

pub(crate) use array_buffer::typed_array::{TypedCreateResume, TypedCreateStep};

pub(crate) use array_buffer::typed_array::{TypedCollectResume, TypedCollectStep};

pub(crate) use array_buffer::typed_array::{TypedIteratorMethodResume, TypedIteratorMethodStep};

pub(crate) use atomics::{AtomicsResume, AtomicsStep};
