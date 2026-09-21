//! Iterator's abstract constructor and constructor accessor share owned replies.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::NativeFunctionId,
    heap::{ContextId, ObjectPayload},
    object::{CallableRef, DescriptorField, ObjectRef, OwnedPropertyDescriptor},
    value::{JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{ConstructorPrototypeSource, NativeArguments, NativeInvocation},
    },
};
pub(crate) enum ConstructorStep {
    Complete(Completion),
    Prototype {
        new_target: JsValue,
        resume: ConstructorResume,
    },
}
pub(crate) struct ConstructorResume;
impl ConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            if matches!(
                invocation,
                NativeInvocation::Getter { .. } | NativeInvocation::Setter { .. }
            ) {
                return Err(RuntimeError::Invariant(
                    "Iterator constructor received an accessor invocation",
                ));
            }
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "constructor requires 'new'",
                )?,
            )));
        };
        let new_target_value = runtime.dup_jsvalue(new_target)?;
        let JsValue::Object(new_target_id) = &new_target_value else {
            runtime.release_jsvalue(new_target_value)?;
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "constructor requires 'new'",
                )?,
            )));
        };
        let new_target = crate::engine::object::ObjectRef::from_borrowed_handle(
            runtime.clone(),
            *new_target_id,
        )?;
        let native_iterator = {
            let state = runtime.0.state.borrow();
            matches!(&state.heap.object(new_target.object_id())?.payload, ObjectPayload::NativeFunction { data, .. } if data.target == NativeFunctionId::IteratorConstructor)
        };
        if native_iterator {
            runtime.release_jsvalue(new_target_value)?;
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "abstract class not constructable",
                )?,
            )));
        }
        Ok(Self::Prototype {
            new_target: new_target_value,
            resume: ConstructorResume,
        })
    }
    pub(crate) fn accessor(
        runtime: &Runtime,
        realm: ContextId,
        callable: &CallableRef,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Iterator constructor accessor did not receive a generic invocation",
            ));
        };
        let defining_realm = {
            let state = runtime.0.state.borrow();
            let ObjectPayload::NativeFunction { data, .. } =
                &state.heap.object(callable.as_object().object_id())?.payload
            else {
                return Err(RuntimeError::Invariant(
                    "Iterator constructor accessor callable lost its native payload",
                ));
            };
            if data.target != NativeFunctionId::IteratorConstructorAccessor {
                return Err(RuntimeError::Invariant(
                    "Iterator constructor accessor callable changed target",
                ));
            }
            data.realm.ok_or(RuntimeError::Invariant(
                "Iterator constructor accessor lost its defining realm",
            ))?
        };
        if arguments.actual_arg_count == 0 {
            let constructor = runtime.iterator_realm_data(defining_realm)?.constructor;
            return Ok(Self::Complete(Completion::Return(runtime.into_jsvalue(
                Value::Object(ObjectRef::from_borrowed_handle(
                    runtime.clone(),
                    constructor,
                )?),
            )?)));
        }
        let Some(JsValue::Object(value_id)) = arguments.readable.first() else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        let JsValue::Object(receiver_id) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        let receiver = ObjectRef::from_borrowed_handle(runtime.clone(), *receiver_id)?;
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?;
        let mut descriptor = OwnedPropertyDescriptor::new(runtime);
        descriptor.value =
            DescriptorField::Present(runtime.dup_jsvalue(&JsValue::Object(*value_id))?);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(false);
        descriptor.configurable = DescriptorField::Present(true);
        let completion = if matches!(
            runtime.define_owned_property_in_realm(Some(realm), &receiver, &key, &descriptor)?,
            crate::engine::object::operations::PropertyDefineOutcome::Defined(true)
        ) {
            Completion::Return(JsValue::Undefined)
        } else {
            Completion::Throw(runtime.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "cannot define property",
            )?)
        };
        Ok(Self::Complete(completion))
    }
}
impl ConstructorResume {
    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        reply: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<ConstructorStep, RuntimeError> {
        let prototype = match reply {
            NativeConversion::Throw(value) => {
                return Ok(ConstructorStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(prototype)) => prototype,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                let prototype = runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .context(realm)?
                    .iterator_prototype;
                ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
            }
        };
        Ok(ConstructorStep::Complete(Completion::Return(
            runtime.into_jsvalue(Value::Object(runtime.new_iterator_object(&prototype)?))?,
        )))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ConstructorStep::Complete(result) => return Ok(result),
            ConstructorStep::Prototype { new_target, resume } => resume.prototype(
                runtime,
                runtime.constructor_prototype_source(realm, new_target)?,
            )?,
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ConstructorStep>() <= 64);
