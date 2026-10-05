//! Proxy [[GetOwnProperty]] and its observable invariant-query order.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
    proxy_gopd_descriptor_is_compatible,
};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::property::validate_and_apply_property_descriptor;
use crate::engine::object::{ObjectRef, OwnedCompletePropertyDescriptor, PropertyKey};
use crate::engine::value::{JsValue, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

mod prefix;
pub(crate) use prefix::StateOwnPrefix;

type Descriptor = Option<OwnedCompletePropertyDescriptor>;

pub(crate) enum ProxyOwnStep {
    StateRead { resume: ProxyOwnResume },
    Complete(NativeConversion<Descriptor>),
    Call { resume: ProxyOwnResume },
    Descriptor { resume: ProxyOwnResume },
    Extensible { resume: ProxyOwnResume },
    Convert { resume: ProxyOwnResume },
}

pub(crate) struct ProxyOwnResume(Box<ProxyOwnResumeState>);
impl std::ops::Deref for ProxyOwnResume {
    type Target = ProxyOwnResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ProxyOwnResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ProxyOwnResume>() <= 8);
pub(crate) struct ProxyOwnResumeState {
    pending_effect: ProxyOwnStepPending,
    realm: ContextId,
    phase: Phase,
}

enum Phase {
    Method {
        resume: MethodResume,
        key: PropertyKey,
    },
    Forward {
        _rooted: RootedProxy,
    },
    Trap {
        rooted: RootedProxy,
        key: PropertyKey,
    },
    Target {
        rooted: RootedProxy,
        result: Option<ObjectRef>,
    },
    Extensible {
        rooted: RootedProxy,
        target: Descriptor,
    },
    Converted {
        _rooted: RootedProxy,
        target: Descriptor,
        extensible: bool,
    },
}

impl ProxyOwnStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
    ) -> Result<Self, RuntimeError> {
        runtime.validate_object_and_key(&object, &key)?;
        let mut state = runtime.0.state.borrow_mut();
        let selected =
            super::StateOwnPrefix::select(runtime, &mut state, realm, object.object_id())?;
        match selected {
            StateOwnPrefix::Direct(object) => {
                let descriptor =
                    match prefix::own_descriptor_in_state(runtime, &mut state, object, key.atom())?
                    {
                        prefix::StateOwnDescriptor::Complete(descriptor) => descriptor,
                        prefix::StateOwnDescriptor::Shared(token) => {
                            drop(state);
                            let (element, bytes) = token.read()?;
                            let mut state = runtime.0.state.borrow_mut();
                            let value = state.decode_typed_index(element, bytes)?;
                            Some(
                            crate::engine::object::StateOwnedCompleteDescriptor::from_owned_data(
                                value, true, true, true,
                            ),
                        )
                        }
                    };
                Ok(Self::Complete(NativeConversion::Value(
                    descriptor.map(|value| value.into_legacy(runtime)),
                )))
            }
            StateOwnPrefix::Throw(value) => Ok(Self::Complete(NativeConversion::Throw(value))),
            StateOwnPrefix::Effect(selected) => {
                drop(state);
                Self::start_selected_method(runtime, realm, key, selected)
            }
        }
    }
    /// This is a cold compatibility boundary for an actual selected Own effect.
    /// It consumes the exact method choice after State access has ended.
    pub(crate) fn start_selected_method(
        runtime: &Runtime,
        realm: ContextId,
        key: PropertyKey,
        selected: super::StateMethodStep,
    ) -> Result<Self, RuntimeError> {
        method(
            runtime,
            realm,
            key,
            MethodStep::from_state(runtime, selected)?,
        )
    }
}

fn method(
    runtime: &Runtime,
    realm: ContextId,
    key: PropertyKey,
    step: MethodStep,
) -> Result<ProxyOwnStep, RuntimeError> {
    Ok(match step {
        MethodStep::Throw(value) => ProxyOwnStep::Complete(NativeConversion::Throw(value.take())),
        MethodStep::Complete { mut resume } => {
            let rooted = resume.take_completed_rooted();
            let target = resume.take_completed_target();
            drop(resume);
            match target {
                None => ProxyOwnStep::request_descriptor(
                    rooted.target.try_clone()?,
                    key,
                    ProxyOwnResume(Box::new(ProxyOwnResumeState {
                        pending_effect: ProxyOwnStepPending::new(runtime.clone()),
                        realm,
                        phase: Phase::Forward { _rooted: rooted },
                    })),
                ),
                Some(target) => {
                    let (receiver, arguments) = {
                        use crate::engine::heap::runtime::owned_values::{
                            OwnedValueGuard, OwnedValuesGuard,
                        };
                        let mut state = runtime.0.state.borrow_mut();
                        let receiver =
                            state.dup_jsvalue(&JsValue::Object(rooted.handler.object_id()))?;
                        let mut receiver =
                            OwnedValueGuard::new(&mut state, &runtime.0.poisoned, receiver);
                        let (state, receiver) = receiver.parts();
                        let mut arguments =
                            OwnedValuesGuard::new(state, &runtime.0.poisoned, Vec::new());
                        let (state, arguments) = arguments.parts();
                        arguments.try_reserve(2).map_err(|_| {
                            RuntimeError::Invariant("Own trap argument allocation failed")
                        })?;
                        arguments
                            .push(state.dup_jsvalue(&JsValue::Object(rooted.target.object_id()))?);
                        arguments.push(state.property_key_argument_in_state(key.atom())?);
                        (receiver.take().unwrap(), std::mem::take(arguments))
                    };
                    ProxyOwnStep::request_call(
                        target,
                        receiver,
                        arguments,
                        ProxyOwnResume(Box::new(ProxyOwnResumeState {
                            pending_effect: ProxyOwnStepPending::new(runtime.clone()),
                            realm,
                            phase: Phase::Trap { rooted, key },
                        })),
                    )
                }
            }
        }
        MethodStep::Read { resume } => ProxyOwnStep::StateRead {
            resume: ProxyOwnResume(Box::new(ProxyOwnResumeState {
                pending_effect: ProxyOwnStepPending::new(runtime.clone()),
                realm,
                phase: Phase::Method { resume, key },
            })),
        },
    })
}

impl ProxyOwnResume {
    pub(crate) fn take_state_read(
        &mut self,
    ) -> (
        crate::engine::object::StateReadEffect,
        crate::engine::atom::Atom,
    ) {
        let Phase::Method { resume, .. } = &mut self.0.phase else {
            unreachable!("selected method read phase")
        };
        resume.take_state_read()
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => {
                return Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        match self.0.phase {
            Phase::Method { resume, key } => method(
                runtime,
                self.0.realm,
                key,
                resume.resume(runtime, Completion::Return(value))?,
            ),
            Phase::Trap { rooted, key } => {
                if !matches!(value, JsValue::Undefined | JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Ok(ProxyOwnStep::Complete(runtime.proxy_invariant_throw(
                        self.0.realm,
                        "getOwnPropertyDescriptor",
                    )?));
                }
                Ok(ProxyOwnStep::request_descriptor(
                    rooted.target.try_clone()?,
                    key,
                    Self(Box::new(ProxyOwnResumeState {
                        pending_effect: ProxyOwnStepPending::new(runtime.clone()),
                        realm: self.0.realm,
                        phase: Phase::Target {
                            rooted,
                            result: match value {
                                JsValue::Object(id) => {
                                    Some(ObjectRef::from_owned_handle(runtime.clone(), id))
                                }
                                JsValue::Undefined => None,
                                _ => unreachable!(),
                            },
                        },
                    })),
                ))
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy descriptor continuation received a value reply",
            )),
        }
    }

    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        descriptor: NativeConversion<Descriptor>,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let target = match descriptor {
            NativeConversion::Value(target) => target,
            NativeConversion::Throw(value) => {
                return Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Forward { .. } => Ok(ProxyOwnStep::Complete(NativeConversion::Value(target))),
            Phase::Target { rooted, result } => {
                if result.is_none() {
                    if let Some(target) = target
                        && (!target.configurable()
                            || !runtime.raw_extensible_bit(&rooted.target)?)
                    {
                        return Ok(ProxyOwnStep::Complete(runtime.proxy_invariant_throw(
                            self.0.realm,
                            "getOwnPropertyDescriptor",
                        )?));
                    }
                    return Ok(ProxyOwnStep::Complete(NativeConversion::Value(None)));
                }
                // QuickJS queries target extensibility before reading any
                // fields from the descriptor returned by the trap.
                let mut pending = ProxyOwnStepPending::new(runtime.clone());
                pending.extensible_result = Some(JsValue::Object(
                    result.expect("object trap result").into_handle(),
                ));
                Ok(ProxyOwnStep::request_extensible(
                    rooted.target.try_clone()?,
                    Self(Box::new(ProxyOwnResumeState {
                        pending_effect: pending,
                        realm: self.0.realm,
                        phase: Phase::Extensible { rooted, target },
                    })),
                ))
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy value continuation received a descriptor reply",
            )),
        }
    }

    pub(crate) fn extensible(
        self,
        result: NativeConversion<bool>,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let mut state = self.0;
        let value = state
            .pending_effect
            .extensible_result
            .take()
            .expect("ProxyOwnStep Extensible result");
        let realm = state.realm;
        let Phase::Extensible { rooted, target } = state.phase else {
            let _ = state.pending_effect.runtime.release_jsvalue(value);
            if let NativeConversion::Throw(thrown) = result {
                let _ = state.pending_effect.runtime.release_jsvalue(thrown);
            }
            return Err(RuntimeError::Invariant(
                "Proxy descriptor continuation received an extensibility reply",
            ));
        };
        let runtime = rooted.proxy.runtime().clone();
        match result {
            NativeConversion::Throw(thrown) => {
                runtime.release_jsvalue(value)?;
                Ok(ProxyOwnStep::Complete(NativeConversion::Throw(thrown)))
            }
            NativeConversion::Value(extensible) => Ok(ProxyOwnStep::request_convert(
                value,
                Self(Box::new(ProxyOwnResumeState {
                    pending_effect: ProxyOwnStepPending::new(runtime),
                    realm,
                    phase: Phase::Converted {
                        _rooted: rooted,
                        target,
                        extensible,
                    },
                })),
            )),
        }
    }

    pub(crate) fn converted(
        self,
        runtime: &Runtime,
        result: NativeConversion<crate::engine::object::OwnedPropertyDescriptor>,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let Phase::Converted {
            _rooted,
            target,
            extensible,
        } = self.0.phase
        else {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Proxy descriptor continuation received a conversion reply",
            ));
        };
        let result = match result {
            NativeConversion::Throw(value) => {
                return Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)));
            }
            NativeConversion::Value(result) => result,
        };
        let record = result.raw_record();
        let complete = validate_and_apply_property_descriptor(
            true,
            &record,
            None,
            &crate::engine::heap::RawValue::Undefined,
            |a, b| {
                crate::engine::value::collection_key::same_value(
                    &runtime.0.state.borrow().heap,
                    a,
                    b,
                )
            },
        )
        .map_err(|_| {
            RuntimeError::Invariant("validated Proxy descriptor could not be completed")
        })?;
        let result = OwnedCompletePropertyDescriptor::from_raw(runtime, &complete)?;
        if !proxy_gopd_descriptor_is_compatible(target.as_ref(), &result, extensible) {
            return Ok(ProxyOwnStep::Complete(runtime.proxy_invariant_throw(
                self.0.realm,
                "getOwnPropertyDescriptor",
            )?));
        }
        Ok(ProxyOwnStep::Complete(NativeConversion::Value(Some(
            result,
        ))))
    }
}

struct ProxyOwnStepPending {
    runtime: Runtime,
    call_target: Option<DirectCallTarget>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    descriptor_object: Option<ObjectRef>,
    descriptor_key: Option<PropertyKey>,
    extensible_object: Option<ObjectRef>,
    extensible_result: Option<JsValue>,
    convert_value: Option<JsValue>,
}
impl ProxyOwnStepPending {
    fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            call_target: None,
            call_receiver: None,
            call_arguments: None,
            descriptor_object: None,
            descriptor_key: None,
            extensible_object: None,
            extensible_result: None,
            convert_value: None,
        }
    }
}
impl Drop for ProxyOwnStepPending {
    /// Release the internal edges still held when the request is abandoned.
    /// Consumption goes through `Option::take`; releases are defer-safe and
    /// nothrow, and never run JavaScript.
    fn drop(&mut self) {
        if let Some(value) = self.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
        if let Some(value) = self.extensible_result.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.convert_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl ProxyOwnStep {
    pub(crate) fn request_call(
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: ProxyOwnResume,
    ) -> Self {
        resume.0.pending_effect.call_target = Some(target);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_descriptor(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: ProxyOwnResume,
    ) -> Self {
        resume.0.pending_effect.descriptor_object = Some(object);
        resume.0.pending_effect.descriptor_key = Some(key);
        Self::Descriptor { resume }
    }
    pub(crate) fn request_extensible(object: ObjectRef, mut resume: ProxyOwnResume) -> Self {
        resume.0.pending_effect.extensible_object = Some(object);
        Self::Extensible { resume }
    }
    pub(crate) fn request_convert(value: JsValue, mut resume: ProxyOwnResume) -> Self {
        resume.0.pending_effect.convert_value = Some(value);
        Self::Convert { resume }
    }
}
impl ProxyOwnResume {
    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .pending_effect
            .call_target
            .take()
            .expect("ProxyOwnStep Call target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("ProxyOwnStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("ProxyOwnStep Call arguments")
    }
    pub(crate) fn take_descriptor_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .descriptor_object
            .take()
            .expect("ProxyOwnStep Descriptor object")
    }
    pub(crate) fn take_descriptor_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .descriptor_key
            .take()
            .expect("ProxyOwnStep Descriptor key")
    }
    pub(crate) fn take_extensible_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .extensible_object
            .take()
            .expect("ProxyOwnStep Extensible object")
    }
    pub(crate) fn take_convert_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .convert_value
            .take()
            .expect("ProxyOwnStep Convert value")
    }
}
const _: () = assert!(std::mem::size_of::<ProxyOwnStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ProxyOwnStep>() <= 64);
