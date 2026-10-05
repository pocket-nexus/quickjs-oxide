//! ToPrimitive consumes synchronous State lookups before publishing an effect.
//! A pending phase owns raw edges; its execution scope supplies cleanup context.
use super::*;
use crate::engine::atom::{Atom, pinned::PinnedAtom};
use crate::engine::heap::{
    ObjectId,
    runtime::{RuntimeState, owned_values::OwnedValueGuard},
};
use crate::engine::object::{CallableRef, ObjectRef, ReadBoundary, StateReadEffect};

pub(crate) enum PrimitiveStep {
    Get { resume: PrimitiveResume },
    Call { resume: PrimitiveResume },
    Complete(Completion),
}
const _: () = assert!(size_of::<PrimitiveStep>() <= 64);

pub(crate) struct PrimitiveResume(Box<PrimitiveResumeState>);
const _: () = assert!(size_of::<PrimitiveResume>() <= 8);
struct PrimitiveResumeState {
    object: ObjectId,
    realm: ContextId,
    domain_id: u64,
    hint: ToPrimitiveHint,
    phase: Phase,
    read: Option<StateReadEffect>,
    atom: Option<Atom>,
    callable: Option<ObjectId>,
    receiver: Option<JsValue>,
    arguments: Vec<JsValue>,
}

#[derive(Clone, Copy)]
enum Phase {
    ExoticMethod,
    ExoticResult,
    OrdinaryMethod(bool),
    OrdinaryResult(bool),
}

/// First entry stays on the Rust stack. Only a selected real effect needs a
/// resident allocation; replies reuse that same allocation.
enum Machine {
    Local(PrimitiveResumeState),
    Pending(Box<PrimitiveResumeState>),
}
impl std::ops::Deref for Machine {
    type Target = PrimitiveResumeState;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(state) => state,
            Self::Pending(state) => state,
        }
    }
}
impl std::ops::DerefMut for Machine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Local(state) => state,
            Self::Pending(state) => state,
        }
    }
}
struct MachineGuard<'a> {
    state: &'a mut RuntimeState,
    runtime: &'a Runtime,
    machine: Option<Machine>,
}
impl Drop for MachineGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.runtime.0.poisoned.set(true);
        }
        if self.runtime.0.poisoned.get() {
            return;
        }
        if let Some(mut machine) = self.machine.take()
            && machine.release_in_state(self.state, self.runtime).is_err()
        {
            self.runtime.0.poisoned.set(true);
        }
    }
}
impl PrimitiveResumeState {
    fn release_in_state(
        &mut self,
        state: &mut RuntimeState,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        let poisoned = &runtime.0.poisoned;
        if let Some(effect) = self.read.take() {
            effect.release_in_state(state, runtime)?;
        }
        if let Some(atom) = self.atom.take() {
            state.atoms.release(atom)?;
        }
        if let Some(callee) = self.callable.take() {
            state.release_owned_jsvalue(poisoned, JsValue::Object(callee))?;
        }
        if let Some(receiver) = self.receiver.take() {
            state.release_owned_jsvalue(poisoned, receiver)?;
        }
        for argument in self.arguments.drain(..) {
            state.release_owned_jsvalue(poisoned, argument)?;
        }
        state.release_owned_jsvalue(poisoned, JsValue::Object(self.object))
    }
}
impl MachineGuard<'_> {
    /// A result cannot leave the machine before its remaining owners finish
    /// cleanup. A destructive cleanup failure quarantines even a normal reply.
    fn finish(
        self,
        result: Result<PrimitiveStep, RuntimeError>,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let poisoned = &self.runtime.0.poisoned;
        drop(self);
        if poisoned.get() {
            Err(RuntimeError::Poisoned)
        } else {
            result
        }
    }
    fn publish(&mut self, call: bool) -> PrimitiveStep {
        let machine = self.machine.take().expect("owned primitive machine");
        let state = match machine {
            Machine::Local(state) => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "primitive.real_effect_resume_created",
                );
                Box::new(state)
            }
            Machine::Pending(state) => state,
        };
        let resume = PrimitiveResume(state);
        if call {
            PrimitiveStep::Call { resume }
        } else {
            PrimitiveStep::Get { resume }
        }
    }
    fn type_error(&mut self, message: &str) -> Result<PrimitiveStep, RuntimeError> {
        let realm = self.machine.as_ref().expect("primitive owner").realm;
        let error = self.state.new_native_error_from_message(
            &self.runtime.0.poisoned,
            realm,
            NativeErrorKind::Type,
            crate::engine::api::error::NativeErrorMessage::from_utf8(message),
        )?;
        Ok(PrimitiveStep::Complete(Completion::Throw(JsValue::Object(
            error,
        ))))
    }
    fn read_method(&mut self, second: bool) -> Result<PrimitiveStep, RuntimeError> {
        let machine = self.machine.as_mut().expect("primitive owner");
        let string_first = matches!(machine.hint, ToPrimitiveHint::String);
        machine.phase = Phase::OrdinaryMethod(second);
        let name = if string_first != second {
            PinnedAtom::ToString
        } else {
            PinnedAtom::ValueOf
        };
        self.read(self.state.pinned_atoms.get(name))
    }
    fn read(&mut self, atom: Atom) -> Result<PrimitiveStep, RuntimeError> {
        let machine = self.machine.as_ref().expect("primitive owner");
        let receiver = JsValue::Object(machine.object);
        let mut boundary = None;
        let value = self.state.select_value_read_in_state(
            &self.runtime.0.poisoned,
            machine.realm,
            &receiver,
            atom,
            machine.domain_id,
            &mut boundary,
            None,
        )?;
        if let Some(value) = value {
            return self.reply(Completion::Return(value));
        }
        let boundary = boundary.ok_or(RuntimeError::Invariant(
            "primitive method read omitted its boundary",
        ))?;
        if matches!(boundary, ReadBoundary::Absent) {
            return self.reply(Completion::Return(JsValue::Undefined));
        }
        let effect = match crate::engine::object::internal_methods::resolve_read_boundary_in_state(
            self.runtime,
            self.state,
            machine.realm,
            atom,
            &receiver,
            boundary,
        )? {
            crate::engine::object::ProxyGetStep::Complete(completion) => {
                return self.reply(completion);
            }
            crate::engine::object::ProxyGetStep::Effect(effect) => effect,
        };
        let needs_key = matches!(effect, StateReadEffect::Proxy { .. });
        let machine = self.machine.as_mut().expect("primitive owner");
        machine.read = Some(effect);
        if needs_key {
            self.state.atoms.retain(atom)?;
            machine.atom = Some(atom);
        }
        Ok(self.publish(false))
    }
    fn reply(&mut self, completion: Completion) -> Result<PrimitiveStep, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => {
                return Ok(PrimitiveStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        enum Reply {
            Call,
            Complete(JsValue),
            Ordinary(bool),
            Error(&'static str),
        }
        let reply = {
            let mut incoming = OwnedValueGuard::new(self.state, &self.runtime.0.poisoned, value);
            let (state, value) = incoming.parts();
            let machine = self.machine.as_mut().expect("primitive owner");
            match machine.phase {
                phase @ (Phase::ExoticMethod | Phase::OrdinaryMethod(_)) => {
                    let callable = match value.as_ref().expect("method reply owner") {
                        JsValue::Object(method)
                            if state.object_id_has_call_capability(*method)? =>
                        {
                            Some(*method)
                        }
                        _ => None,
                    };
                    if let Some(callable) = callable {
                        let receiver = state.dup_jsvalue(&JsValue::Object(machine.object))?;
                        machine.receiver = Some(receiver);
                        if matches!(phase, Phase::ExoticMethod) {
                            let hint = match machine.hint {
                                ToPrimitiveHint::String => "string",
                                ToPrimitiveHint::Number => "number",
                                ToPrimitiveHint::Default => "default",
                            };
                            machine.arguments.try_reserve(1).map_err(|_| {
                                RuntimeError::Invariant("primitive argument allocation failed")
                            })?;
                            machine.arguments.push(JsValue::String(
                                state.heap.allocate_string(JsString::from_static(hint))?,
                            ));
                            machine.phase = Phase::ExoticResult;
                        } else if let Phase::OrdinaryMethod(second) = phase {
                            machine.phase = Phase::OrdinaryResult(second);
                        }
                        machine.callable = Some(callable);
                        value.take(); // Transfer the method result's edge to the callee.
                        Reply::Call
                    } else {
                        match phase {
                            Phase::ExoticMethod
                                if matches!(
                                    value.as_ref(),
                                    Some(JsValue::Undefined | JsValue::Null)
                                ) =>
                            {
                                Reply::Ordinary(false)
                            }
                            Phase::ExoticMethod => Reply::Error("not a function"),
                            Phase::OrdinaryMethod(false) => Reply::Ordinary(true),
                            Phase::OrdinaryMethod(true) => Reply::Error("toPrimitive"),
                            _ => unreachable!(),
                        }
                    }
                }
                phase @ (Phase::ExoticResult | Phase::OrdinaryResult(_)) => {
                    if !matches!(value.as_ref(), Some(JsValue::Object(_))) {
                        Reply::Complete(value.take().expect("primitive result owner"))
                    } else {
                        match phase {
                            Phase::ExoticResult | Phase::OrdinaryResult(true) => {
                                Reply::Error("toPrimitive")
                            }
                            Phase::OrdinaryResult(false) => Reply::Ordinary(true),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }; // The incoming-owner guard ends before the next State operation.
        if self.runtime.0.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        match reply {
            Reply::Call => Ok(self.publish(true)),
            Reply::Complete(value) => Ok(PrimitiveStep::Complete(Completion::Return(value))),
            Reply::Ordinary(second) => self.read_method(second),
            Reply::Error(message) => self.type_error(message),
        }
    }
}

impl PrimitiveResume {
    pub(crate) fn realm(&self) -> ContextId {
        self.0.realm
    }
    pub(crate) fn take_state_read(&mut self) -> (StateReadEffect, Option<Atom>) {
        (
            self.0.read.take().expect("primitive selected read"),
            self.0.atom.take(),
        )
    }
    pub(crate) fn take_callable(&mut self, runtime: &Runtime) -> CallableRef {
        CallableRef::from_validated_object(ObjectRef::from_owned_handle(
            runtime.clone(),
            self.0.callable.take().expect("primitive call callee"),
        ))
    }
    pub(crate) fn take_receiver(&mut self) -> JsValue {
        self.0.receiver.take().expect("primitive call receiver")
    }
    pub(crate) fn take_arguments(&mut self) -> Vec<JsValue> {
        std::mem::take(&mut self.0.arguments)
    }
    pub(crate) fn release_in_state(
        mut self,
        state: &mut RuntimeState,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        self.0.release_in_state(state, runtime)
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        if runtime.skip_cleanup() {
            return;
        }
        let _unwind = runtime.unwind_guard();
        if self
            .release_in_state(&mut runtime.0.state.borrow_mut(), runtime)
            .is_err()
        {
            runtime.0.poisoned.set(true);
        }
    }
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let JsValue::Object(object) = value else {
            return Ok(PrimitiveStep::Complete(Completion::Return(value)));
        };
        let mut state = runtime.0.state.borrow_mut();
        let atom = state.well_known_symbols[&WellKnownSymbol::ToPrimitive];
        let mut guard = MachineGuard {
            state: &mut state,
            runtime,
            machine: Some(Machine::Local(PrimitiveResumeState {
                object,
                realm,
                domain_id: runtime.domain_id(),
                hint,
                phase: Phase::ExoticMethod,
                read: None,
                atom: None,
                callable: None,
                receiver: None,
                arguments: Vec::new(),
            })),
        };
        let result = guard.read(atom);
        guard.finish(result)
    }
    pub(crate) fn ordinary(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let object = object.into_handle();
        let mut state = runtime.0.state.borrow_mut();
        let mut guard = MachineGuard {
            state: &mut state,
            runtime,
            machine: Some(Machine::Local(PrimitiveResumeState {
                object,
                realm,
                domain_id: runtime.domain_id(),
                hint,
                phase: Phase::OrdinaryMethod(false),
                read: None,
                atom: None,
                callable: None,
                receiver: None,
                arguments: Vec::new(),
            })),
        };
        let result = guard.read_method(false);
        guard.finish(result)
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let mut state = runtime.0.state.borrow_mut();
        let mut guard = MachineGuard {
            state: &mut state,
            runtime,
            machine: Some(Machine::Pending(self.0)),
        };
        let result = guard.reply(completion);
        guard.finish(result)
    }
}

/// Outside-State scope for an unpublished real effect. Taking it transfers
/// cleanup to execution storage; an error releases through the caller's Runtime.
pub(crate) struct PrimitiveScope<'a> {
    runtime: &'a Runtime,
    resume: Option<PrimitiveResume>,
}
impl<'a> PrimitiveScope<'a> {
    pub(crate) fn new(runtime: &'a Runtime, resume: PrimitiveResume) -> Self {
        Self {
            runtime,
            resume: Some(resume),
        }
    }
    pub(crate) fn take(&mut self) -> PrimitiveResume {
        self.resume.take().expect("primitive scope owner")
    }
}
impl std::ops::Deref for PrimitiveScope<'_> {
    type Target = PrimitiveResume;
    fn deref(&self) -> &Self::Target {
        self.resume.as_ref().expect("primitive scope owner")
    }
}
impl std::ops::DerefMut for PrimitiveScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resume.as_mut().expect("primitive scope owner")
    }
}
impl Drop for PrimitiveScope<'_> {
    fn drop(&mut self) {
        if let Some(resume) = self.resume.take() {
            resume.release_owned(self.runtime);
        }
    }
}

impl Runtime {
    /// Remaining embedding boundary consumes an already selected effect.
    pub(crate) fn finish_primitive_read(
        &self,
        realm: ContextId,
        effect: StateReadEffect,
        atom: Option<Atom>,
    ) -> Result<Completion, RuntimeError> {
        self.finish_state_read_effect(realm, effect, atom)
    }
    pub(super) fn finish_primitive_steps(
        &self,
        realm: ContextId,
        mut step: PrimitiveStep,
    ) -> Result<Completion, RuntimeError> {
        loop {
            step = match step {
                PrimitiveStep::Complete(completion) => return Ok(completion),
                PrimitiveStep::Get { resume } => {
                    let mut resume = PrimitiveScope::new(self, resume);
                    let (effect, atom) = resume.take_state_read();
                    let completion = self.finish_primitive_read(realm, effect, atom)?;
                    resume.take().resume(self, completion)?
                }
                PrimitiveStep::Call { resume } => {
                    let mut resume = PrimitiveScope::new(self, resume);
                    let callable = resume.take_callable(self);
                    let receiver = resume.take_receiver();
                    let arguments = resume.take_arguments();
                    let completion =
                        self.call_internal_jsvalue(realm, &callable, receiver, arguments)?;
                    resume.take().resume(self, completion)?
                }
            };
        }
    }
    pub(crate) fn ordinary_to_primitive(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        hint: ToPrimitiveHint,
    ) -> Result<Completion, RuntimeError> {
        let step = PrimitiveResume::ordinary(self, realm, object.try_clone()?, hint)?;
        self.finish_primitive_steps(realm, step)
    }
}

#[cfg(test)]
mod state_phase_tests {
    use super::*;
    use crate::engine::api::Value;

    #[cfg(feature = "profiling")]
    #[test]
    fn synchronous_method_prefix_publishes_only_the_actual_call() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let value = context.eval("({valueOf(){return 7}})").unwrap();
        let Value::Object(object) = &value else {
            panic!("object")
        };
        let object = object.object_id();
        let input = runtime.unroot_value(&value).unwrap();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        let profile = crate::engine::api::profiling::CostProfile::start();
        let _core = crate::engine::api::profiling::CoreExecutionScope::enter();
        let PrimitiveStep::Call { resume } =
            PrimitiveResume::start(&runtime, context.realm, input, ToPrimitiveHint::Number)
                .unwrap()
        else {
            panic!("selected valueOf call")
        };
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("primitive.real_effect_resume_created"), Some(&1));
        assert_eq!(events.get("core.runtime_clone").copied().unwrap_or(0), 0);
        assert_eq!(events.get("query.read.acquired").copied().unwrap_or(0), 0);
        resume.release_owned(&runtime);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(object)
                .unwrap(),
            1
        );
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn synchronous_proxy_method_prefix_does_not_publish_a_primitive_phase() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let value = context
            .eval("new Proxy({[Symbol.toPrimitive]:null,valueOf:null,toString:null},{})")
            .unwrap();
        let input = runtime.unroot_value(&value).unwrap();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        let profile = crate::engine::api::profiling::CostProfile::start();
        let _core = crate::engine::api::profiling::CoreExecutionScope::enter();
        let PrimitiveStep::Complete(Completion::Throw(error)) =
            PrimitiveResume::start(&runtime, context.realm, input, ToPrimitiveHint::Number)
                .unwrap()
        else {
            panic!("synchronous ToPrimitive rejection")
        };
        let events = profile.snapshot().owned_execution_events;
        for event in [
            "primitive.real_effect_resume_created",
            "get_resume_allocation",
            "query.read.acquired",
            "core.runtime_clone",
        ] {
            assert_eq!(events.get(event).copied().unwrap_or(0), 0, "{event}");
        }
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        runtime.release_jsvalue(error).unwrap();
    }

    #[test]
    fn getter_replies_reuse_the_resident_phase_and_never_repeat_selection() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let value = context.eval("({get [Symbol.toPrimitive](){return undefined}, get valueOf(){return function(){return 7}}})").unwrap();
        let input = runtime.unroot_value(&value).unwrap();
        let PrimitiveStep::Get { resume } =
            PrimitiveResume::start(&runtime, context.realm, input, ToPrimitiveHint::Number)
                .unwrap()
        else {
            panic!("exotic getter")
        };
        let mut resume = PrimitiveScope::new(&runtime, resume);
        let address = &*resume.0 as *const PrimitiveResumeState;
        let (effect, atom) = resume.take_state_read();
        let completion = runtime
            .finish_primitive_read(context.realm, effect, atom)
            .unwrap();
        let PrimitiveStep::Get { resume } = resume.take().resume(&runtime, completion).unwrap()
        else {
            panic!("ordinary getter")
        };
        let mut resume = PrimitiveScope::new(&runtime, resume);
        assert_eq!(&*resume.0 as *const PrimitiveResumeState, address);
        let (effect, atom) = resume.take_state_read();
        let completion = runtime
            .finish_primitive_read(context.realm, effect, atom)
            .unwrap();
        let PrimitiveStep::Call { resume } = resume.take().resume(&runtime, completion).unwrap()
        else {
            panic!("ordinary method")
        };
        assert_eq!(&*resume.0 as *const PrimitiveResumeState, address);
        assert!(matches!(
            runtime
                .finish_primitive_steps(context.realm, PrimitiveStep::Call { resume })
                .unwrap(),
            Completion::Return(JsValue::Int(7))
        ));
    }

    #[test]
    fn rejected_resume_scope_cleans_object_and_selected_receiver() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(value) = context
            .eval("({get [Symbol.toPrimitive](){return function(){return 7}}})")
            .unwrap()
        else {
            panic!("object")
        };
        let object = value.object_id();
        let PrimitiveStep::Get { resume } = PrimitiveResume::start(
            &runtime,
            context.realm,
            JsValue::Object(value.into_handle()),
            ToPrimitiveHint::Number,
        )
        .unwrap() else {
            panic!("getter")
        };
        drop(PrimitiveScope::new(&runtime, resume));
        assert!(runtime.0.state.borrow().heap.object(object).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn destructive_reply_cleanup_does_not_advance_to_the_next_method() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(method) = context
            .eval("var method = function(){return '7'}; method")
            .unwrap()
        else {
            panic!("method")
        };
        let Value::Object(object) = context
            .eval("({get valueOf(){return undefined},toString:method})")
            .unwrap()
        else {
            panic!("object")
        };
        let PrimitiveStep::Get { mut resume } = PrimitiveResume::ordinary(
            &runtime,
            context.realm,
            object.try_clone().unwrap(),
            ToPrimitiveHint::Number,
        )
        .unwrap() else {
            panic!("valueOf getter")
        };
        let (effect, atom) = resume.take_state_read();
        let _ = runtime
            .finish_primitive_read(context.realm, effect, atom)
            .unwrap();
        let method_id = method.object_id();
        let before = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(method_id)
            .unwrap();
        let invalid = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_string(JsString::from_static("invalid"))
            .unwrap();
        runtime.release_jsvalue(JsValue::String(invalid)).unwrap();
        assert!(matches!(
            resume.resume(&runtime, Completion::Return(JsValue::String(invalid))),
            Err(RuntimeError::Poisoned)
        ));
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(method_id)
                .unwrap(),
            before
        );
    }

    #[test]
    fn successful_reply_is_rejected_if_final_machine_cleanup_poisons() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let value = context.eval("({valueOf(){return 7}})").unwrap();
        let PrimitiveStep::Call { mut resume } = PrimitiveResume::start(
            &runtime,
            context.realm,
            runtime.unroot_value(&value).unwrap(),
            ToPrimitiveHint::Number,
        )
        .unwrap() else {
            panic!("method")
        };
        let invalid = runtime.new_object(None).unwrap().into_handle();
        runtime.release_jsvalue(JsValue::Object(invalid)).unwrap();
        let previous = std::mem::replace(&mut resume.0.object, invalid);
        runtime.release_jsvalue(JsValue::Object(previous)).unwrap();
        assert!(matches!(
            resume.resume(&runtime, Completion::Return(JsValue::Int(7))),
            Err(RuntimeError::Poisoned)
        ));
        assert!(runtime.0.poisoned.get());
        drop(value); // Poisoned root cleanup cannot retry the failed release.
    }

    #[test]
    fn destructive_cleanup_failure_poisons_before_visiting_remaining_owners() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let value = context.eval("({valueOf(){return 7}})").unwrap();
        let PrimitiveStep::Call { mut resume } = PrimitiveResume::start(
            &runtime,
            context.realm,
            runtime.unroot_value(&value).unwrap(),
            ToPrimitiveHint::Number,
        )
        .unwrap() else {
            panic!("method")
        };
        let invalid = runtime.new_object(None).unwrap().into_handle();
        runtime.release_jsvalue(JsValue::Object(invalid)).unwrap();
        let previous = resume.0.callable.replace(invalid).unwrap();
        runtime.release_jsvalue(JsValue::Object(previous)).unwrap();
        let object = resume.0.object;
        let before = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(object)
            .unwrap();
        drop(PrimitiveScope::new(&runtime, resume));
        assert!(runtime.0.poisoned.get());
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(object)
                .unwrap(),
            before
        );
        drop(value); // Poisoned public-root cleanup must not traverse the heap.
    }
}
