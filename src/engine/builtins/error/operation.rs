//! Error construction and stringification retain intermediate values in observable order.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ErrorConstructorKind,
    heap::ContextId,
    object::{DescriptorField, ObjectRef, OwnedPropertyDescriptor, PropertyKey},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ErrorKind {
    Constructor(ErrorConstructorKind),
    ToString,
}
impl ErrorKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ErrorConstructor(kind) => Some(Self::Constructor(kind)),
            NativeFunctionId::ErrorPrototypeToString => Some(Self::ToString),
            _ => None,
        }
    }
}
pub(crate) enum ErrorStep {
    Complete(Completion),
    Read { resume: ErrorResume },
    String { resume: ErrorResume },
    Has { resume: ErrorResume },
    Aggregate { resume: ErrorResume },
}
enum Phase {
    Prototype,
    Message,
    CauseHas,
    Cause,
    Aggregate,
    NameRead,
    Name,
    TextRead,
    Text,
}
pub(crate) struct ErrorResume(Box<ErrorResumeState>);
impl std::ops::Deref for ErrorResume {
    type Target = ErrorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ErrorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ErrorResume>() <= 8);
pub(crate) struct ErrorResumeState {
    runtime: Runtime,
    pending_effect: ErrorStepPending,
    realm: ContextId,
    kind: ErrorKind,
    phase: Phase,
    object: Option<ObjectRef>,
    new_target: JsValue,
    arguments: Vec<JsValue>,
    actual: usize,
    name: JsString,
    name_value: JsValue,
}
impl Drop for ErrorResumeState {
    fn drop(&mut self) {
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.new_target, JsValue::Undefined));
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.name_value, JsValue::Undefined));
        for value in self.arguments.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
        for value in [
            self.pending_effect.read_receiver.take(),
            self.pending_effect.string_value.take(),
            self.pending_effect.aggregate_iterable.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl ErrorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ErrorKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let mut resume = ErrorResume(Box::new(ErrorResumeState {
            runtime: runtime.clone(),
            pending_effect: ErrorStepPending::default(),
            realm,
            kind,
            phase: Phase::Prototype,
            object: None,
            new_target: JsValue::Undefined,
            arguments: Vec::new(),
            actual: arguments.actual_arg_count,
            name: JsString::from_static("Error"),
            name_value: JsValue::Undefined,
        }));
        resume
            .0
            .arguments
            .try_reserve_exact(arguments.readable.len())
            .map_err(|_| RuntimeError::Invariant("Error argument allocation failed"))?;
        for argument in &arguments.readable {
            resume.0.arguments.push(runtime.dup_jsvalue(argument)?);
        }
        match kind {
            ErrorKind::Constructor(_) => {
                let NativeInvocation::Construct { new_target } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Error constructor requires constructor-or-function invocation",
                    ));
                };
                resume.new_target = if matches!(new_target, JsValue::Undefined) {
                    JsValue::Object(runtime.active_function()?.into_handle())
                } else {
                    runtime.dup_jsvalue(new_target)?
                };
                Ok({
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
                    let __pending_field_receiver = runtime.dup_jsvalue(&resume.new_target)?;
                    let __pending_field_resume = resume;
                    Self::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            ErrorKind::ToString => {
                let NativeInvocation::Call { this_value } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Error string requires generic invocation",
                    ));
                };
                let JsValue::Object(object) = this_value else {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                let object = ObjectRef::from_borrowed_handle(runtime.clone(), *object)?;
                resume.object = Some(object.clone());
                resume.phase = Phase::NameRead;
                Ok({
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Name)?;
                    let __pending_field_receiver = JsValue::Object(object.into_handle());
                    let __pending_field_resume = resume;
                    Self::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
        }
    }
}
impl ErrorResume {
    fn object(&self) -> Result<ObjectRef, RuntimeError> {
        self.0
            .object
            .clone()
            .ok_or(RuntimeError::Invariant("Error operation object missing"))
    }
    fn aggregate_kind(&self) -> bool {
        matches!(
            self.0.kind,
            ErrorKind::Constructor(ErrorConstructorKind::Native(NativeErrorKind::Aggregate))
        )
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ErrorStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(ErrorStep::Complete(Completion::Throw(value))),
        };
        match self.0.phase {
            Phase::Prototype => {
                let ErrorKind::Constructor(kind) = self.0.kind else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant("Error prototype kind mismatch"));
                };
                let prototype = if let JsValue::Object(object) = value {
                    ObjectRef::from_owned_handle(runtime.clone(), object)
                } else {
                    runtime.release_jsvalue(value)?;
                    let realm = match runtime
                        .function_realm_from_jsvalue(self.0.realm, &self.0.new_target)?
                    {
                        NativeConversion::Value(realm) => realm,
                        NativeConversion::Throw(value) => {
                            return Ok(ErrorStep::Complete(Completion::Throw(value)));
                        }
                    };
                    let prototype = {
                        let state = runtime.0.state.borrow();
                        let context = state.heap.context(realm)?;
                        match kind {
                            ErrorConstructorKind::Error => context
                                .error_prototype
                                .ok_or(RuntimeError::Invariant("realm has no Error prototype"))?,
                            ErrorConstructorKind::Native(kind) => {
                                context.native_error_prototypes[kind.index()].ok_or(
                                    RuntimeError::Invariant("realm has no native Error prototype"),
                                )?
                            }
                        }
                    };
                    ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
                };
                self.0.object = Some(runtime.new_error_object(&prototype)?);
                let message = self
                    .0
                    .arguments
                    .get(usize::from(self.aggregate_kind()))
                    .ok_or(RuntimeError::Invariant("Error message argv missing"))?;
                if matches!(message, JsValue::Undefined) {
                    self.cause(runtime)
                } else {
                    self.0.phase = Phase::Message;
                    Ok({
                        let __pending_field_value = runtime.dup_jsvalue(message)?;
                        let __pending_field_resume = self;
                        __pending_field_resume.stringify(runtime, __pending_field_value)?
                    })
                }
            }
            Phase::Cause => {
                self.define_field(runtime, "cause", value)?;
                self.aggregate(runtime)
            }
            Phase::Aggregate => {
                if !matches!(value, JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "AggregateError iterable returned non-object",
                    ));
                }
                self.define_field(runtime, "errors", value)?;
                self.finish(runtime)
            }
            Phase::NameRead => {
                if matches!(value, JsValue::Undefined) {
                    self.text(runtime)
                } else {
                    self.0.phase = Phase::Name;
                    Ok({
                        let __pending_field_value = value;
                        let __pending_field_resume = self;
                        __pending_field_resume.stringify(runtime, __pending_field_value)?
                    })
                }
            }
            Phase::TextRead => {
                self.0.phase = Phase::Text;
                if matches!(value, JsValue::Undefined) {
                    self.string(runtime, NativeConversion::Value(JsString::from_static("")))
                } else {
                    Ok({
                        let __pending_field_value = value;
                        let __pending_field_resume = self;
                        __pending_field_resume.stringify(runtime, __pending_field_value)?
                    })
                }
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant("Error value reply phase mismatch"))
            }
        }
    }
    fn define_field(
        &self,
        runtime: &Runtime,
        name: &str,
        value: JsValue,
    ) -> Result<(), RuntimeError> {
        let mut descriptor = OwnedPropertyDescriptor::new(runtime);
        descriptor.value = DescriptorField::Present(value);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(false);
        descriptor.configurable = DescriptorField::Present(true);
        let object = self.object()?;
        let key = runtime.intern_property_key(name)?;
        if !runtime.define_ordinary_owned_property(&object, &key, &descriptor)? {
            return Err(RuntimeError::Invariant(
                "function intrinsic property definition was rejected",
            ));
        }
        Ok(())
    }
    fn stringify(self, runtime: &Runtime, value: JsValue) -> Result<ErrorStep, RuntimeError> {
        if matches!(value, JsValue::String(_)) {
            self.string_value(runtime, value)
        } else {
            Ok(ErrorStep::request_string(value, self))
        }
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<ErrorStep, RuntimeError> {
        match result {
            NativeConversion::Value(value) => {
                self.string_value(runtime, runtime.into_jsvalue(Value::String(value))?)
            }
            NativeConversion::Throw(value) => Ok(ErrorStep::Complete(Completion::Throw(value))),
        }
    }
    fn string_value(
        mut self,
        runtime: &Runtime,
        value: JsValue,
    ) -> Result<ErrorStep, RuntimeError> {
        self.0.pending_effect.string_value = Some(value);
        let Some(JsValue::String(id)) = self.0.pending_effect.string_value.as_ref() else {
            return Err(RuntimeError::Invariant(
                "Error string reply was not a string",
            ));
        };
        let text = runtime.0.state.borrow().heap.string(*id)?.clone();
        match self.0.phase {
            Phase::Message => {
                let value = self.0.pending_effect.string_value.take().unwrap();
                self.define_field(runtime, "message", value)?;
                self.cause(runtime)
            }
            Phase::Name => {
                self.0.name = text;
                let value = self.0.pending_effect.string_value.take().unwrap();
                runtime.release_jsvalue(std::mem::replace(&mut self.0.name_value, value))?;
                self.text(runtime)
            }
            Phase::Text => {
                let result = if self.0.name.is_empty() {
                    self.0.pending_effect.string_value.take().unwrap()
                } else if text.is_empty() {
                    if matches!(self.0.name_value, JsValue::Undefined) {
                        runtime.into_jsvalue(Value::String(self.0.name.clone()))?
                    } else {
                        std::mem::replace(&mut self.0.name_value, JsValue::Undefined)
                    }
                } else {
                    runtime.into_jsvalue(Value::String(
                        self.0
                            .name
                            .try_concat(&JsString::from_static(": "))?
                            .try_concat(&text)?,
                    ))?
                };
                Ok(ErrorStep::Complete(Completion::Return(result)))
            }
            _ => Err(RuntimeError::Invariant("Error string reply phase mismatch")),
        }
    }
    fn text(mut self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        self.0.phase = Phase::TextRead;
        Ok({
            let __pending_field_key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Message)?;
            let __pending_field_receiver = JsValue::Object(self.object()?.into_handle());
            let __pending_field_resume = self;
            ErrorStep::request_read(
                __pending_field_receiver,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
    fn cause(mut self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        let index = usize::from(self.aggregate_kind()) + 1;
        if self.0.actual > index
            && let Some(JsValue::Object(options)) = self.0.arguments.get(index)
        {
            let object = ObjectRef::from_borrowed_handle(runtime.clone(), *options)?;
            self.0.phase = Phase::CauseHas;
            return Ok({
                let __pending_field_object = object;
                let __pending_field_key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Cause)?;
                let __pending_field_resume = self;
                ErrorStep::request_has(
                    __pending_field_object,
                    __pending_field_key,
                    __pending_field_resume,
                )
            });
        }
        self.aggregate(runtime)
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ErrorStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::CauseHas) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Error cause boolean phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ErrorStep::Complete(Completion::Throw(value)));
            }
        };
        if !value {
            return self.aggregate(runtime);
        }
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Cause)?;
        let receiver =
            runtime.dup_jsvalue(&self.0.arguments[usize::from(self.aggregate_kind()) + 1])?;
        self.0.phase = Phase::Cause;
        Ok({
            let __pending_field_receiver = receiver;
            let __pending_field_key = key;
            let __pending_field_resume = self;
            ErrorStep::request_read(
                __pending_field_receiver,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
    fn aggregate(mut self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        if self.aggregate_kind() {
            self.0.phase = Phase::Aggregate;
            Ok({
                let __pending_field_iterable =
                    runtime.dup_jsvalue(self.0.arguments.first().ok_or(
                        RuntimeError::Invariant("AggregateError errors argv missing"),
                    )?)?;
                let __pending_field_resume = self;
                ErrorStep::request_aggregate(__pending_field_iterable, __pending_field_resume)
            })
        } else {
            self.finish(runtime)
        }
    }
    fn finish(self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        let object = self.object()?;
        let value = JsValue::Object(object.object_id());
        runtime.ensure_error_backtrace_jsvalue(&value, true, None)?;
        Ok(ErrorStep::Complete(Completion::Return(JsValue::Object(
            object.into_handle(),
        ))))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ErrorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ErrorStep::Complete(result) => return Ok(result),
            ErrorStep::Read { mut resume } => {
                let receiver = resume.take_read_receiver();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?,
                )?
            }
            ErrorStep::String { mut resume } => {
                let value = resume.take_string_value();
                resume.string(runtime, runtime.native_to_js_string_jsvalue(realm, value)?)?
            }
            ErrorStep::Has { mut resume } => {
                let object = resume.take_has_object();
                let key = resume.take_has_key();
                resume.boolean(
                    runtime,
                    runtime.internal_has_property(realm, &object, &key)?,
                )?
            }
            ErrorStep::Aggregate { mut resume } => {
                let iterable = resume.take_aggregate_iterable();
                resume.resume(
                    runtime,
                    super::aggregate::finish(
                        runtime,
                        realm,
                        super::aggregate::AggregateStep::start(runtime, realm, iterable)?,
                    )?,
                )?
            }
        };
    }
}

#[derive(Default)]
struct ErrorStepPending {
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    string_value: Option<JsValue>,
    has_object: Option<ObjectRef>,
    has_key: Option<PropertyKey>,
    aggregate_iterable: Option<JsValue>,
}
impl ErrorStep {
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: ErrorResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_string(value: JsValue, mut resume: ErrorResume) -> Self {
        resume.0.pending_effect.string_value = Some(value);
        Self::String { resume }
    }
    pub(crate) fn request_has(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: ErrorResume,
    ) -> Self {
        resume.0.pending_effect.has_object = Some(object);
        resume.0.pending_effect.has_key = Some(key);
        Self::Has { resume }
    }
    pub(crate) fn request_aggregate(iterable: JsValue, mut resume: ErrorResume) -> Self {
        resume.0.pending_effect.aggregate_iterable = Some(iterable);
        Self::Aggregate { resume }
    }
}
impl ErrorResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("ErrorStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("ErrorStep Read key")
    }
    pub(crate) fn take_string_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .string_value
            .take()
            .expect("ErrorStep String value")
    }
    pub(crate) fn take_has_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .has_object
            .take()
            .expect("ErrorStep Has object")
    }
    pub(crate) fn take_has_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .has_key
            .take()
            .expect("ErrorStep Has key")
    }
    pub(crate) fn take_aggregate_iterable(&mut self) -> JsValue {
        self.0
            .pending_effect
            .aggregate_iterable
            .take()
            .expect("ErrorStep Aggregate iterable")
    }
}
const _: () = assert!(std::mem::size_of::<ErrorStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ErrorStep>() <= 64);
