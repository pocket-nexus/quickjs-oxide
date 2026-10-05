//! Scheduling for a pending addition or unary-plus conversion. Domain phases remain in
//! value/conversion; only frame installation and reply routing live here.
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::object::CallableRef;
use crate::engine::value::JsValue;
use crate::engine::value::conversion::primitive::{PrimitiveResume, PrimitiveScope, PrimitiveStep};
use crate::engine::vm::call::{BytecodeCallRequest, CallableExecution};
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{FrameId, ReturnTarget};
use crate::engine::vm::{Completion, ToPrimitiveHint};

mod owners;
use owners::TaskScope;
pub(in crate::engine::vm) use owners::{ConversionSlotScope, ConversionWaitScope};

enum Finish {
    Predicate(Option<Box<super::predicate_driver::Input>>),
    SuperProperty(Option<Box<super::super_property_driver::Input>>),
    Plus,
    PropertyKey,
    PropertyWrite {
        base: JsValue,
        value: JsValue,
    },
    PropertyRead {
        base: JsValue,
        keep_receiver: bool,
        keep_key: bool,
        fallthrough: super::execute::FallthroughPc,
    },
    AddLeft(JsValue),
    AddRight(JsValue),
}

/// The same resident state moves between task and wait as one pointer.
pub(super) struct ConversionWait(ConversionTask);
pub(super) struct ConversionTask(Option<Box<ConversionState>>);
pub(super) struct ConversionState {
    finish: Finish,
    frame: FrameId,
    identity: u64,
    step: Option<PrimitiveStep>,
    resume: Option<PrimitiveResume>,
}
impl ConversionTask {
    pub(super) fn release_owned(mut self, runtime: &Runtime) {
        if self.0.is_none() || runtime.skip_cleanup() {
            return;
        }
        let _unwind = runtime.unwind_guard();
        if let Some(step) = self.step.take() {
            match step {
                PrimitiveStep::Complete(Completion::Return(value) | Completion::Throw(value)) => {
                    release_conversion_value(runtime, value);
                }
                PrimitiveStep::Get { resume } | PrimitiveStep::Call { resume } => {
                    resume.release_owned(runtime)
                }
            }
        }
        if let Some(resume) = self.resume.take() {
            resume.release_owned(runtime);
        }
        let finish = std::mem::replace(&mut self.finish, Finish::Plus);
        match finish {
            Finish::PropertyWrite { base, value } => {
                release_conversion_value(runtime, base);
                release_conversion_value(runtime, value);
            }
            Finish::PropertyRead { base, .. } | Finish::AddLeft(base) | Finish::AddRight(base) => {
                release_conversion_value(runtime, base);
            }
            Finish::SuperProperty(Some(input)) => input.release_edges(runtime),
            Finish::SuperProperty(None)
            | Finish::Predicate(_)
            | Finish::Plus
            | Finish::PropertyKey => {}
        }
    }
}
fn release_conversion_value(runtime: &Runtime, value: JsValue) {
    if runtime.skip_cleanup() {
        return;
    }
    let _ = runtime
        .0
        .state
        .borrow_mut()
        .release_owned_jsvalue(&runtime.0.poisoned, value);
}
impl ConversionWait {
    pub(super) fn release_owned(self, runtime: &Runtime) {
        self.0.release_owned(runtime);
    }
}
impl std::ops::Deref for ConversionTask {
    type Target = ConversionState;
    fn deref(&self) -> &Self::Target {
        self.0
            .as_ref()
            .expect("completed conversion has no resident state")
    }
}
impl std::ops::DerefMut for ConversionTask {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
            .as_mut()
            .expect("completed conversion has no resident state")
    }
}
impl std::ops::Deref for ConversionWait {
    type Target = ConversionState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
const _: () = assert!(size_of::<ConversionTask>() <= 8);
const _: () = assert!(size_of::<ConversionWait>() <= 8);

pub(super) enum Progress {
    Predicate(Box<super::predicate_driver::Input>),
    SuperProperty(Box<super::super_property_driver::Input>),
    Ready(ConversionTask),
    Entered,
    Complete(Completion),
    PropertyRead {
        keep_receiver: bool,
        keep_key: bool,
        fallthrough: super::execute::FallthroughPc,
    },
    PropertyWrite(Box<super::property_write_driver::ConvertedWrite>),
}

fn property_key_primitive(runtime: &Runtime, value: JsValue) -> Result<JsValue, Error> {
    if matches!(value, JsValue::Symbol(_) | JsValue::String(_)) {
        return Ok(value);
    }
    let mut state = runtime.0.state.borrow_mut();
    let mut primitive = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
        &mut state,
        &runtime.0.poisoned,
        value,
    );
    let (state, primitive) = primitive.parts();
    let text = state.primitive_to_js_string(primitive.as_ref().expect("primitive key owner"))?;
    state
        .release_owned_jsvalue(
            &runtime.0.poisoned,
            primitive.take().expect("primitive key owner"),
        )
        .map_err(runtime_error_to_vm_error)?;
    state
        .heap
        .allocate_string(text)
        .map(JsValue::String)
        .map_err(|error| Error::internal(error.to_string()))
}

fn add_completion(
    runtime: &Runtime,
    realm: crate::engine::heap::ContextId,
    left: JsValue,
    right: JsValue,
) -> Result<Completion, Error> {
    match super::numeric::add_primitives(runtime, left, right) {
        Ok(value) => Ok(Completion::Return(value)),
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(Completion::Throw(
                runtime
                    .new_native_error_from_error_jsvalue(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            ))
        }
    }
}

pub(super) enum PrimitiveCompletion {
    Completed,
    Throw(JsValue),
    Declined,
}

/// The fault PC is published before entry. The input borrow owns no values
/// across parsing, allocation, error materialization or final-owner release.
pub(super) fn complete_primitives(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    addition: bool,
    next_operation: &mut u64,
) -> Result<PrimitiveCompletion, Error> {
    let frame = execution.frames.current_mut(id)?;
    let body = &mut *frame.cold;
    let executable = &*body.executable;
    let realm = executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&body.window);
    let mut transaction = execution.slots.frame_transaction(&mut body.window)?;
    let (left, right) = {
        let mut slots = transaction.slots();
        // Internal values carry no runtime branding; authenticate every operand
        // slot in the original left-to-right order before the identity issue.
        let mut has_object = false;
        for offset in (0..=usize::from(addition)).rev() {
            let value = slots.peek(offset)?;
            has_object |= matches!(value, JsValue::Object(_));
        }
        *next_operation = next_operation
            .checked_add(1)
            .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
        if has_object {
            return Ok(PrimitiveCompletion::Declined);
        }
        let right = slots.pop().expect("validated primitive conversion operand");
        let left = if addition {
            Some(
                slots
                    .pop()
                    .expect("validated primitive conversion left operand"),
            )
        } else {
            None
        };
        (left, right)
    };
    // End the authenticated window before String/BigInt allocation or release.
    let completion = if let Some(left) = left {
        add_completion(runtime, realm, left, right)?
    } else {
        match super::numeric::unary_plus_primitive(runtime, right) {
            Ok(value) => Completion::Return(value),
            Err(error) => {
                let Some(kind) =
                    crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
                else {
                    return Err(error);
                };
                Completion::Throw(
                    runtime
                        .new_native_error_from_error_jsvalue(realm, kind, &error)
                        .map_err(runtime_error_to_vm_error)?,
                )
            }
        }
    };
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(
        "conversion_completed_without_task",
    );
    Ok(match completion {
        Completion::Return(value) => {
            let mut pending = Some(value);
            {
                let mut slots = transaction.slots();
                slots.push_pending(&mut pending)?;
            }
            frame.resume_pc = frame.next_pc()?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            PrimitiveCompletion::Completed
        }
        Completion::Throw(value) => PrimitiveCompletion::Throw(value),
    })
}

impl ConversionTask {
    #[inline(always)]
    fn new(
        _runtime: &Runtime,
        finish: Finish,
        frame: FrameId,
        identity: u64,
        step: PrimitiveStep,
    ) -> Self {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("conversion_task_allocated");
        Self(Some(Box::new(ConversionState {
            finish,
            frame,
            identity,
            step: Some(step),
            resume: None,
        })))
    }
    #[allow(clippy::too_many_arguments)]
    fn start_owned_input(
        runtime: &Runtime,
        finish: Finish,
        frame: FrameId,
        identity: u64,
        realm: crate::engine::heap::ContextId,
        value: JsValue,
        hint: ToPrimitiveHint,
    ) -> Result<Self, Error> {
        let mut task = TaskScope::new(
            runtime,
            Self::new(
                runtime,
                finish,
                frame,
                identity,
                PrimitiveStep::Complete(Completion::Return(JsValue::Undefined)),
            ),
        );
        task.step = None;
        task.step = Some(PrimitiveResume::start(runtime, realm, value, hint)?);
        Ok(task.take())
    }
    fn with_step(mut self, step: PrimitiveStep) -> Self {
        self.step = Some(step);
        self
    }
    fn waiting(mut self, resume: PrimitiveResume) -> ConversionWait {
        self.resume = Some(resume);
        ConversionWait(self)
    }

    #[cfg(feature = "profiling")]
    pub(super) fn operand_count(&self) -> usize {
        if self.0.is_none() {
            return 0;
        }
        match &self.finish {
            Finish::SuperProperty(input) => input
                .as_ref()
                .expect("super conversion input")
                .operand_count(),
            Finish::Plus | Finish::PropertyKey => 1,
            Finish::PropertyWrite { .. } => 3,
            _ => 2,
        }
    }

    pub(super) fn start(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        addition: bool,
        property_key: bool,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(frame)?;
        let right = execution.slots.pop(&mut parent.window)?;
        if property_key && !addition && !matches!(right, JsValue::Object(_)) {
            let value = property_key_primitive(runtime, right)?;
            #[cfg(feature = "profiling")]
            let depth = execution.slots.depth(&parent.window) + 1;
            execution.slots.push(&mut parent.window, value)?;
            parent.resume_pc = parent.next_pc()?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            return Ok(Self(None));
        }
        let (value, finish, hint) = if addition {
            (
                execution.slots.pop(&mut parent.window)?,
                Finish::AddLeft(right),
                ToPrimitiveHint::Default,
            )
        } else if property_key {
            (right, Finish::PropertyKey, ToPrimitiveHint::String)
        } else {
            (right, Finish::Plus, ToPrimitiveHint::Number)
        };
        Self::start_owned_input(
            runtime,
            finish,
            frame,
            identity,
            parent.executable.realm,
            value,
            hint,
        )
    }

    pub(super) fn start_predicate(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        input: Box<super::predicate_driver::Input>,
    ) -> Result<Self, Error> {
        let realm = execution.frames.current_mut(frame)?.executable.realm;
        let value = runtime
            .dup_jsvalue(&input.key)
            .map_err(runtime_error_to_vm_error)?;
        Self::start_owned_input(
            runtime,
            Finish::Predicate(Some(input)),
            frame,
            identity,
            realm,
            value,
            ToPrimitiveHint::String,
        )
    }

    pub(super) fn start_super_property(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        input: Box<super::super_property_driver::Input>,
    ) -> Result<Self, Error> {
        let realm = execution.frames.current_mut(frame)?.executable.realm;
        let value = runtime
            .dup_jsvalue(&input.key)
            .map_err(runtime_error_to_vm_error)?;
        Self::start_owned_input(
            runtime,
            Finish::SuperProperty(Some(input)),
            frame,
            identity,
            realm,
            value,
            ToPrimitiveHint::String,
        )
    }

    pub(super) fn start_property_write(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(frame)?;
        let value = execution.slots.pop(&mut parent.window)?;
        let key = execution.slots.pop(&mut parent.window)?;
        let base = execution.slots.pop(&mut parent.window)?;
        Self::start_owned_input(
            runtime,
            Finish::PropertyWrite { base, value },
            frame,
            identity,
            parent.executable.realm,
            key,
            ToPrimitiveHint::String,
        )
    }

    pub(super) fn start_property_read(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        keep_receiver: bool,
        keep_key: bool,
        fallthrough: super::execute::FallthroughPc,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(frame)?;
        execution.slots.peek(&parent.window, 1)?;
        execution.slots.peek(&parent.window, 0)?;
        let key = execution.slots.pop(&mut parent.window)?;
        let base = execution.slots.pop(&mut parent.window)?;
        Self::start_owned_input(
            runtime,
            Finish::PropertyRead {
                base,
                keep_receiver,
                keep_key,
                fallthrough,
            },
            frame,
            identity,
            parent.executable.realm,
            key,
            ToPrimitiveHint::String,
        )
    }

    pub(super) fn reply(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        target: ReturnTarget,
        completion: Completion,
    ) -> Result<Self, Error> {
        let selected = (|| {
            let frame = target.frame()?;
            let parent = execution.frames.current_mut(frame)?;
            let wait = parent
                .cold
                .conversion
                .take()
                .ok_or_else(|| Error::internal("conversion reply has no pending owner"))?;
            let mut wait = ConversionWaitScope::new(runtime, wait);
            if target.operation != Some(super::frame::OperationTarget::Conversion(wait.identity)) {
                return Err(Error::internal("conversion reply identity mismatch"));
            }
            Ok((frame, wait.take()))
        })();
        let (frame, wait) = match selected {
            Ok(selected) => selected,
            Err(error) => {
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let _ = runtime.release_jsvalue(value);
                return Err(error);
            }
        };
        Self::from_wait(runtime, frame, wait, completion)
    }

    pub(super) fn from_wait(
        runtime: &Runtime,
        frame: FrameId,
        wait: ConversionWait,
        completion: Completion,
    ) -> Result<Self, Error> {
        let mut task = TaskScope::new(runtime, wait.0);
        task.frame = frame;
        let Some(resume) = task.resume.take() else {
            let (Completion::Return(value) | Completion::Throw(value)) = completion;
            let _ = runtime.release_jsvalue(value);
            return Err(Error::internal("conversion wait lost its resume"));
        };
        task.step = Some(
            resume
                .resume(runtime, completion)
                .map_err(runtime_error_to_vm_error)?,
        );
        Ok(task.take())
    }

    pub(super) fn advance(
        self,
        runtime: &Runtime,
        execution: &mut RunningExecution,
    ) -> Result<Progress, Error> {
        let mut task = TaskScope::new(runtime, self);
        if task.0.is_none() {
            return Ok(Progress::Entered);
        }
        let frame = task.frame;
        let realm = execution.frames.current_mut(frame)?.executable.realm;
        let step = task
            .step
            .take()
            .ok_or_else(|| Error::internal("conversion task lost its step"))?;
        match step {
            PrimitiveStep::Complete(completion) => {
                let completion = match completion {
                    Completion::Throw(value) => Completion::Throw(value),
                    Completion::Return(value) => {
                        // Domain completion guarantees a primitive: this call
                        // cannot recursively perform another ToPrimitive.
                        if matches!(value, JsValue::Object(_)) {
                            let _ = runtime.release_jsvalue(value);
                            return Err(Error::internal("conversion returned an object"));
                        }
                        match &mut task.finish {
                            Finish::AddLeft(right) => {
                                let right = std::mem::replace(right, JsValue::Undefined);
                                if !matches!(right, JsValue::Object(_)) {
                                    #[cfg(feature = "profiling")]
                                    crate::engine::api::profiling::record_owned_execution_event(
                                        "add_completed_with_primitive_rhs",
                                    );
                                    return Ok(Progress::Complete(add_completion(
                                        runtime, realm, value, right,
                                    )?));
                                }
                                task.finish = Finish::AddRight(value);
                                return Ok(Progress::Ready(task.with_step(
                                    PrimitiveResume::start(
                                        runtime,
                                        realm,
                                        right,
                                        ToPrimitiveHint::Default,
                                    )?,
                                )));
                            }
                            Finish::AddRight(left) => add_completion(
                                runtime,
                                realm,
                                std::mem::replace(left, JsValue::Undefined),
                                value,
                            )?,
                            Finish::Predicate(input) => {
                                let mut input = input.take().expect("predicate conversion input");
                                let previous = std::mem::replace(&mut input.key, value);
                                runtime
                                    .release_jsvalue(previous)
                                    .map_err(runtime_error_to_vm_error)?;
                                return Ok(Progress::Predicate(input));
                            }
                            Finish::SuperProperty(input) => {
                                let mut input = input.take().expect("super conversion input");
                                let previous = std::mem::replace(&mut input.key, value);
                                runtime
                                    .release_jsvalue(previous)
                                    .map_err(runtime_error_to_vm_error)?;
                                return Ok(Progress::SuperProperty(input));
                            }
                            Finish::PropertyWrite {
                                base,
                                value: assigned,
                            } => {
                                let base = std::mem::replace(base, JsValue::Undefined);
                                let assigned = std::mem::replace(assigned, JsValue::Undefined);
                                return Ok(Progress::PropertyWrite(Box::new(
                                    super::property_write_driver::ConvertedWrite {
                                        base: Some(base),
                                        key: Some(value),
                                        value: Some(assigned),
                                        runtime: runtime.clone(),
                                    },
                                )));
                            }
                            Finish::PropertyRead {
                                base,
                                keep_receiver,
                                keep_key,
                                fallthrough,
                            } => {
                                // Converted numeric keys use String/Symbol when retained,
                                // unlike an original direct Int operand. Publish the reply
                                // into the existing parent window; no operand box is needed.
                                let key = if *keep_key {
                                    property_key_primitive(runtime, value)?
                                } else {
                                    value
                                };
                                let mut state = runtime.0.state.borrow_mut();
                                let mut key = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                                    &mut state, &runtime.0.poisoned, key,
                                );
                                let (state, key) = key.parts();
                                let mut base_owner = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                                    state, &runtime.0.poisoned, std::mem::replace(base, JsValue::Undefined),
                                );
                                let (_, base_owner) = base_owner.parts();
                                let parent = execution.frames.current_mut(frame)?;
                                let mut transaction =
                                    execution.slots.frame_transaction(&mut parent.cold.window)?;
                                let mut slots = transaction.slots();
                                slots.push_pending(base_owner)?;
                                slots.push_pending(key)?;
                                return Ok(Progress::PropertyRead {
                                    keep_receiver: *keep_receiver,
                                    keep_key: *keep_key,
                                    fallthrough: *fallthrough,
                                });
                            }
                            Finish::PropertyKey => {
                                Completion::Return(property_key_primitive(runtime, value)?)
                            }
                            Finish::Plus => {
                                match super::numeric::unary_plus_primitive(runtime, value) {
                                    Ok(value) => Completion::Return(value),
                                    Err(error) => {
                                        let Some(kind) = crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind()) else { return Err(error); };
                                        Completion::Throw(
                                            runtime
                                                .new_native_error_from_error_jsvalue(
                                                    realm, kind, &error,
                                                )
                                                .map_err(runtime_error_to_vm_error)?,
                                        )
                                    }
                                }
                            }
                        }
                    }
                };
                Ok(Progress::Complete(completion))
            }
            PrimitiveStep::Get { resume } => {
                let mut resume = PrimitiveScope::new(runtime, resume);
                let (effect, atom) = resume.take_state_read();
                match effect {
                    crate::engine::object::StateReadEffect::Getter { callee, receiver } => {
                        let key = crate::engine::object::PropertyKey::from_owned_atom(
                            runtime.clone(),
                            atom,
                        );
                        drop(key);
                        let getter = CallableRef::from_validated_object(
                            crate::engine::object::ObjectRef::from_owned_handle(
                                runtime.clone(),
                                callee,
                            ),
                        );
                        invoke(
                            runtime,
                            execution,
                            task.take(),
                            getter,
                            receiver,
                            Vec::new(),
                            resume.take(),
                        )
                    }
                    effect @ crate::engine::object::StateReadEffect::Shared(_) => {
                        let completion = runtime
                            .finish_primitive_read(realm, effect, atom)
                            .map_err(runtime_error_to_vm_error)?;
                        Ok(Progress::Ready(
                            task.with_step(
                                resume
                                    .take()
                                    .resume(runtime, completion)
                                    .map_err(runtime_error_to_vm_error)?,
                            ),
                        ))
                    }
                    effect @ crate::engine::object::StateReadEffect::Proxy { .. } => {
                        match super::proxy_get_driver::start_conversion_state_read(
                            runtime,
                            execution,
                            frame,
                            effect,
                            atom,
                            task.take().waiting(resume.take()),
                        )? {
                            super::proxy_get_driver::Progress::Conversion(task) => {
                                Ok(Progress::Ready(task))
                            }
                            super::proxy_get_driver::Progress::Call(
                                super::driver::CallStep::Entered,
                            ) => Ok(Progress::Entered),
                            super::proxy_get_driver::Progress::Call(
                                super::driver::CallStep::Complete(completion),
                            ) => Ok(Progress::Complete(completion)),
                            super::proxy_get_driver::Progress::Call(
                                super::driver::CallStep::Bridge,
                            ) => Err(Error::internal(
                                "conversion property query attempted replay",
                            )),
                        }
                    }
                }
            }
            PrimitiveStep::Call { mut resume } => {
                let callable = resume.take_callable(runtime);
                let receiver = resume.take_receiver();
                let arguments = resume.take_arguments();
                invoke(
                    runtime,
                    execution,
                    task.take(),
                    callable,
                    receiver,
                    arguments,
                    resume,
                )
            }
        }
    }
}

// Own the call operands until a normalized callback or prepared frame accepts them.
struct ConversionInvocation<'a> {
    runtime: &'a Runtime,
    receiver: JsValue,
    arguments: Vec<JsValue>,
}
impl ConversionInvocation<'_> {
    fn take_receiver(&mut self) -> JsValue {
        std::mem::replace(&mut self.receiver, JsValue::Undefined)
    }
    fn take_arguments(&mut self) -> Vec<JsValue> {
        std::mem::take(&mut self.arguments)
    }
}
impl Drop for ConversionInvocation<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        let receiver = self.take_receiver();
        release_conversion_value(self.runtime, receiver);
        for value in self.arguments.drain(..) {
            release_conversion_value(self.runtime, value);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    task: ConversionTask,
    callable: CallableRef,
    receiver: JsValue,
    arguments: Vec<JsValue>,
    resume: PrimitiveResume,
) -> Result<Progress, Error> {
    let mut task = TaskScope::new(runtime, task);
    let mut resume = PrimitiveScope::new(runtime, resume);
    let mut operands = ConversionInvocation {
        runtime,
        receiver,
        arguments,
    };
    let frame = task.frame;
    let identity = task.identity;
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    let super::call::NormalizedCallback {
        callable,
        receiver,
        arguments,
        classification,
    } = match super::call::normalize_callback(
        runtime,
        realm,
        callable,
        operands.take_receiver(),
        operands.take_arguments(),
    )? {
        crate::engine::value::conversion::NativeConversion::Value(call) => call,
        crate::engine::value::conversion::NativeConversion::Throw(value) => {
            // Transfer the callback's owned exception directly to the continuation.
            return Ok(Progress::Ready(
                task.with_step(
                    resume
                        .take()
                        .resume(runtime, Completion::Throw(value))
                        .map_err(runtime_error_to_vm_error)?,
                ),
            ));
        }
    };
    operands.receiver = receiver;
    operands.arguments = arguments;
    let is_proxy = matches!(classification, CallableExecution::Proxy);
    let is_native = matches!(classification, CallableExecution::Native { .. });
    let is_resumable = if let CallableExecution::Bytecode { bytecode, .. } = &classification {
        runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode(bytecode.bytecode_id())
            .map_err(|error| Error::internal(error.to_string()))?
            .metadata
            .function_kind
            != FunctionKind::Normal
    } else {
        false
    };
    if is_proxy || is_native || is_resumable {
        let wait = task.take().waiting(resume.take());
        let progress = if is_proxy {
            super::proxy_get_driver::start_conversion_call(
                runtime,
                execution,
                frame,
                callable.as_object().try_clone()?,
                operands.take_receiver(),
                operands.take_arguments(),
                wait,
            )?
        } else {
            super::proxy_get_driver::start_native_conversion_call(
                runtime,
                execution,
                frame,
                callable,
                operands.take_receiver(),
                operands.take_arguments(),
                wait,
            )?
        };
        return match progress {
            super::proxy_get_driver::Progress::Conversion(task) => Ok(Progress::Ready(task)),
            super::proxy_get_driver::Progress::Call(super::driver::CallStep::Entered) => {
                Ok(Progress::Entered)
            }
            super::proxy_get_driver::Progress::Call(super::driver::CallStep::Complete(
                completion,
            )) => Ok(Progress::Complete(completion)),
            super::proxy_get_driver::Progress::Call(super::driver::CallStep::Bridge) => {
                Err(Error::internal("conversion callback attempted replay"))
            }
        };
    }
    if let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = classification
    {
        let kind = runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode(bytecode.bytecode_id())
            .map_err(|error| Error::internal(error.to_string()))?
            .metadata
            .function_kind;
        if kind == FunctionKind::Normal {
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                let completion = runtime
                    .bytecode_stack_overflow_completion(realm, &bytecode)
                    .map_err(runtime_error_to_vm_error)?;
                return Ok(Progress::Ready(
                    task.with_step(
                        resume
                            .take()
                            .resume(runtime, completion)
                            .map_err(runtime_error_to_vm_error)?,
                    ),
                ));
            }
            let request = BytecodeCallRequest {
                callable,
                receiver: operands.take_receiver(),
                arguments: operands.take_arguments(),
                new_target: JsValue::Undefined,
                bytecode,
                closure_slots,
                caller_realm: realm,
                return_to: ReturnTarget {
                    value_use: crate::engine::vm::frame::ReturnValue::Push,
                    owner: crate::engine::vm::frame::ReturnOwner::Frame(frame),
                    tail: false,
                    operation: Some(super::frame::OperationTarget::Conversion(identity)),
                },
            };
            let entry = request.prepare(runtime, &mut execution.call_storage)?;
            let parent = execution.frames.current_mut(frame)?;
            if parent.cold.conversion.is_some() {
                return Err(Error::internal(
                    "conversion overwrote an unanswered request",
                ));
            }
            parent.cold.conversion = Some(task.take().waiting(resume.take()));
            super::driver::push_frame(runtime, execution, entry)?;
            return Ok(Progress::Entered);
        }
    }
    Err(Error::internal(
        "conversion callback lost its normalized classification",
    ))
}

#[cfg(all(test, feature = "profiling"))]
mod primitive_store_tests {
    use super::*;
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn converted_read_reply_uses_parent_storage_and_retains_normalized_key() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(base) = context.eval("({'0':7})").unwrap() else {
            panic!("object")
        };
        let (mut execution, id) =
            super::super::property_driver::read_completion_tests::read_fixture(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]++})",
                crate::engine::code::exec_opcode::Opcode::GetArrayEl3Dense,
            );
        let frame = execution.frames.current_mut(id).unwrap();
        let fallthrough = super::super::execute::FallthroughPc::from_decoded(
            frame
                .executable
                .exec
                .decode_published(frame.resume_pc as u32)
                .unwrap(),
        );
        let task = ConversionTask::new(
            &runtime,
            Finish::PropertyRead {
                base: JsValue::Object(base.into_handle()),
                keep_receiver: true,
                keep_key: true,
                fallthrough,
            },
            id,
            1,
            PrimitiveStep::Complete(Completion::Return(JsValue::Int(0))),
        );
        assert!(matches!(
            task.advance(&runtime, &mut execution).unwrap(),
            Progress::PropertyRead {
                keep_receiver: true,
                keep_key: true,
                ..
            }
        ));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert!(matches!(
            execution.slots.peek(&frame.window, 1).unwrap(),
            JsValue::Object(_)
        ));
        let JsValue::String(key) = execution.slots.peek(&frame.window, 0).unwrap() else {
            panic!("converted numeric key must be normalized before retention")
        };
        assert_eq!(
            runtime.0.state.borrow().heap.string(*key).unwrap(),
            &crate::engine::value::JsString::from_static("0")
        );
        assert!(matches!(
            super::super::property_driver::read_progress(
                &runtime,
                &mut execution,
                id,
                super::super::property_driver::ReadKey::Computed { keep_key: true },
                true,
                fallthrough
            )
            .unwrap(),
            super::super::property_driver::PropertyProgress::Completed
        ));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 3);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Int(7)
        );
        assert!(matches!(
            execution.slots.peek(&frame.window, 1).unwrap(),
            JsValue::String(_)
        ));
        assert_eq!(frame.resume_pc, fallthrough.index());
    }

    #[test]
    fn conversion_task_resides_across_both_operands_and_skips_primitive_property_keys() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(
            context
                .eval("var residentLeft={valueOf(){return 1}},residentRight={valueOf(){return 2}}")
                .unwrap(),
        );
        let profile = CostProfile::start();
        assert_eq!(
            context.eval("residentLeft+residentRight").unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("conversion_task_allocated")
                .copied()
                .unwrap_or(0),
            0
        );
        drop(profile);
        let profile = CostProfile::start();
        assert_eq!(
            context.eval("({[true]:1,[1.25]:2,[null]:3}).true").unwrap(),
            Value::Int(1)
        );
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("conversion_task_allocated")
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn primitive_store_keeps_conversion_capture_and_throw_observations() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let s='', b=1n;
            for(let i=0;i<20;i++){ s+='x'; b+=2n; }
            let capture=()=>s;
            s+='y';
            let old=s,trace='';
            try { s+=Symbol(); } catch(e) { if(e instanceof TypeError)trace+='throw'; }
            finally { trace+='finally'; }
            let a='left';
            a+= {valueOf(){a='changed';return 'right';}};
            let constant='a', read=constant;
            try { const x='a'; x+='b'; } catch(e) { if(e instanceof TypeError)read+='!'; }
            return s===old && capture()===old && s.length===21 && b===41n
                && trace==='throwfinally' && a==='leftright' && read==='a!';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}
