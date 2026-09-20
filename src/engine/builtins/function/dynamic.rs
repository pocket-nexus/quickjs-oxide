//! Dynamic Function source fragments and eval results survive every conversion and prototype lookup.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::DynamicFunctionKind,
    code::dynamic_source::DynamicSourceBuilder,
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum DynamicFunctionStep {
    Complete(Completion),
    String { resume: DynamicFunctionResume },
    Eval { resume: DynamicFunctionResume },
    Read { resume: DynamicFunctionResume },
}
enum Phase {
    Parameters,
    Body,
    Eval,
    Prototype,
}
pub(crate) struct DynamicFunctionResume(Box<DynamicFunctionResumeState>);
impl std::ops::Deref for DynamicFunctionResume {
    type Target = DynamicFunctionResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for DynamicFunctionResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<DynamicFunctionResume>() <= 8);
pub(crate) struct DynamicFunctionResumeState {
    pending_effect: DynamicFunctionStepPending,
    realm: ContextId,
    kind: DynamicFunctionKind,
    new_target: JsValue,
    arguments: Vec<JsValue>,
    index: usize,
    source: Option<DynamicSourceBuilder>,
    phase: Phase,
    value: Value,
}
impl DynamicFunctionStep {
    pub(crate) fn start(
        _runtime: &Runtime,
        realm: ContextId,
        kind: DynamicFunctionKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "Function constructor requires constructor-or-function invocation",
            ));
        };
        let mut source = DynamicSourceBuilder::new();
        source.push_str("(")?;
        if matches!(
            kind,
            DynamicFunctionKind::Async | DynamicFunctionKind::AsyncGenerator
        ) {
            source.push_str("async ")?;
        }
        source.push_str("function")?;
        if matches!(
            kind,
            DynamicFunctionKind::Generator | DynamicFunctionKind::AsyncGenerator
        ) {
            source.push_str("*")?;
        }
        source.push_str(" anonymous(")?;
        let mut owned_arguments = Vec::new();
        owned_arguments
            .try_reserve_exact(arguments.actual_arg_count)
            .map_err(|_| RuntimeError::Invariant("Function constructor argv allocation failed"))?;
        for value in &arguments.readable[..arguments.actual_arg_count] {
            owned_arguments.push(_runtime.dup_jsvalue(value)?);
        }
        DynamicFunctionResume(Box::new(DynamicFunctionResumeState {
            pending_effect: DynamicFunctionStepPending::default(),
            realm,
            kind,
            new_target: _runtime.dup_jsvalue(new_target)?,
            arguments: owned_arguments,
            index: 0,
            source: Some(source),
            phase: Phase::Parameters,
            value: Value::Undefined,
        }))
        .parameter(_runtime)
    }
}
impl DynamicFunctionResume {
    fn source(&mut self) -> Result<&mut DynamicSourceBuilder, RuntimeError> {
        self.0
            .source
            .as_mut()
            .ok_or(RuntimeError::Invariant("Function source builder missing"))
    }
    fn parameter(mut self, runtime: &Runtime) -> Result<DynamicFunctionStep, RuntimeError> {
        if self.0.index < self.0.arguments.len().saturating_sub(1) {
            if self.0.index != 0 {
                self.source()?.push_str(",")?;
            }
            return Ok({
                let __pending_field_value = runtime.dup_jsvalue(&self.0.arguments[self.0.index])?;
                let __pending_field_resume = self;
                DynamicFunctionStep::request_string(__pending_field_value, __pending_field_resume)
            });
        }
        self.source()?.push_str("\n) {\n")?;
        if let Some(value) = self.0.arguments.last() {
            let value = runtime.dup_jsvalue(value)?;
            self.0.phase = Phase::Body;
            return Ok({
                let __pending_field_value = value;
                let __pending_field_resume = self;
                DynamicFunctionStep::request_string(__pending_field_value, __pending_field_resume)
            });
        }
        self.eval()
    }
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<DynamicFunctionStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(DynamicFunctionStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        self.source()?.push_js_string(&value)?;
        match self.0.phase {
            Phase::Parameters => {
                self.0.index += 1;
                self.parameter(runtime)
            }
            Phase::Body => self.eval(),
            _ => Err(RuntimeError::Invariant(
                "Function source string reply phase mismatch",
            )),
        }
    }
    fn eval(mut self) -> Result<DynamicFunctionStep, RuntimeError> {
        self.source()?.push_str("\n})")?;
        let source = self
            .0
            .source
            .take()
            .ok_or(RuntimeError::Invariant("Function source builder missing"))?
            .finish()?;
        self.0.phase = Phase::Eval;
        Ok({
            let __pending_field_source = source;
            let __pending_field_resume = self;
            DynamicFunctionStep::request_eval(__pending_field_source, __pending_field_resume)
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DynamicFunctionStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => runtime.root_and_release_jsvalue(value)?,
            Completion::Throw(value) => {
                return Ok(DynamicFunctionStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Eval => {
                if matches!(self.0.new_target, JsValue::Undefined) {
                    return Ok(DynamicFunctionStep::Complete(Completion::Return(
                        runtime.into_jsvalue(value)?,
                    )));
                }
                self.0.value = value;
                self.0.phase = Phase::Prototype;
                Ok(DynamicFunctionStep::request_read(
                    runtime.dup_jsvalue(&self.0.new_target)?,
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?,
                    self,
                ))
            }
            Phase::Prototype => {
                let prototype = if let Value::Object(object) = value {
                    object
                } else {
                    let new_target = std::mem::replace(&mut self.0.new_target, JsValue::Undefined);
                    let new_target = runtime.root_and_release_jsvalue(new_target)?;
                    let realm =
                        match runtime.function_realm_from_value(self.0.realm, &new_target)? {
                            NativeConversion::Value(realm) => realm,
                            NativeConversion::Throw(value) => {
                                return Ok(DynamicFunctionStep::Complete(Completion::Throw(
                                    runtime.into_jsvalue(value)?,
                                )));
                            }
                        };
                    let prototype = {
                        let state = runtime.0.state.borrow();
                        let context = state.heap.context(realm)?;
                        match self.0.kind {
                            DynamicFunctionKind::Normal => context.function_prototype,
                            DynamicFunctionKind::Generator => context.generator.ok_or(RuntimeError::Invariant("dynamic GeneratorFunction realm has no Generator intrinsics"))?.function_prototype,
                            DynamicFunctionKind::Async => context.async_function.ok_or(RuntimeError::Invariant("dynamic AsyncFunction realm has no AsyncFunction intrinsics"))?.function_prototype,
                            DynamicFunctionKind::AsyncGenerator => context.async_generator.ok_or(RuntimeError::Invariant("dynamic AsyncGeneratorFunction realm has no AsyncGenerator intrinsics"))?.function_prototype,
                        }
                    };
                    ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
                };
                let Value::Object(function) = self.0.value else {
                    return Ok(DynamicFunctionStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                if !runtime.set_prototype_of(&function, Some(&prototype))? {
                    return Ok(DynamicFunctionStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "prototype is immutable",
                        )?,
                    )));
                }
                Ok(DynamicFunctionStep::Complete(Completion::Return(
                    JsValue::Object(function.into_handle()),
                )))
            }
            _ => Err(RuntimeError::Invariant(
                "Function constructor value reply phase mismatch",
            )),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: DynamicFunctionStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            DynamicFunctionStep::Complete(result) => return Ok(result),
            DynamicFunctionStep::String { mut resume } => {
                let value = runtime.root_and_release_jsvalue(resume.take_string_value())?;
                resume.string(
                    runtime,
                    runtime.native_to_dynamic_source_fragment(realm, &value)?,
                )?
            }
            DynamicFunctionStep::Eval { mut resume } => {
                let source = resume.take_eval_source();
                resume.resume(
                    runtime,
                    runtime.execute_indirect_string_eval(realm, &source)?,
                )?
            }
            DynamicFunctionStep::Read { mut resume } => {
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
        };
    }
}

#[derive(Default)]
struct DynamicFunctionStepPending {
    string_value: Option<JsValue>,
    eval_source: Option<JsString>,
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
}
impl DynamicFunctionStep {
    pub(crate) fn request_string(value: JsValue, mut resume: DynamicFunctionResume) -> Self {
        resume.0.pending_effect.string_value = Some(value);
        Self::String { resume }
    }
    pub(crate) fn request_eval(source: JsString, mut resume: DynamicFunctionResume) -> Self {
        resume.0.pending_effect.eval_source = Some(source);
        Self::Eval { resume }
    }
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: DynamicFunctionResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
}
impl DynamicFunctionResume {
    pub(crate) fn take_string_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .string_value
            .take()
            .expect("DynamicFunctionStep String value")
    }
    pub(crate) fn take_eval_source(&mut self) -> JsString {
        self.0
            .pending_effect
            .eval_source
            .take()
            .expect("DynamicFunctionStep Eval source")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("DynamicFunctionStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("DynamicFunctionStep Read key")
    }
}
const _: () = assert!(std::mem::size_of::<DynamicFunctionStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<DynamicFunctionStep>() <= 64);
