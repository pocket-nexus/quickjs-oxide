//! Exact typed replies for the constructor raw owner.
use super::*;
impl DateConstructorResume {
    pub(crate) fn primitive(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let result = self.primitive_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn primitive_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let result = self.primitive_admitted(state, poisoned, host, result);
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    fn primitive_admitted(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let mut owner = DateConstructorGuard::new(state, poisoned, self);
        let (throwing, value) = match result {
            Completion::Return(value) => (false, value),
            Completion::Throw(value) => (true, value),
        };
        let mut input = OwnedValueGuard::new(owner.state, poisoned, value);
        if !matches!(
            owner.resume.as_ref().expect("Date single owner").0.phase,
            Phase::Single
        ) {
            return Err(RuntimeError::Invariant("Date primitive phase mismatch"));
        }
        if throwing {
            let value = input.parts().1.take().expect("Date primitive throw");
            drop(input);
            return owner.complete(Completion::Throw(value));
        }
        let (state, incoming) = input.parts();
        let resume = owner.resume.as_mut().expect("Date single owner");
        let previous = std::mem::replace(
            &mut resume.0.converted,
            incoming.take().expect("Date primitive reply"),
        );
        state.release_owned_jsvalue(poisoned, previous)?;
        drop(input);
        let resume = owner.resume.as_mut().expect("Date single owner");
        resume.0.value = if let JsValue::String(id) = resume.0.converted {
            parsed_date_value(parse_date_string(owner.state.heap.string(id)?), |instant| {
                host.timezone_offset_minutes(instant)
            })
        } else {
            match owner.state.number_from_primitive_jsvalue(
                poisoned,
                resume.0.realm,
                &resume.0.converted,
            )? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return owner.complete_step(DateConstructorStep::CyclePublished(
                        Completion::Throw(value),
                    ));
                }
            }
        };
        resume.0.value = time_clip(resume.0.value);
        let output = owner.prototype();
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        output
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let result = self.string_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn string_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: NativeConversion<JsString>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let result = self.string_admitted(state, poisoned, host, result);
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    fn string_admitted(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: NativeConversion<JsString>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let mut owner = DateConstructorGuard::new(state, poisoned, self);
        let output = match result {
            NativeConversion::Throw(value) => {
                let input = OwnedValueGuard::new(owner.state, poisoned, value);
                if !matches!(
                    owner.resume.as_ref().expect("Date parse owner").0.phase,
                    Phase::Parse
                ) {
                    return Err(RuntimeError::Invariant("Date parse phase mismatch"));
                }
                let mut input = input;
                let value = input.parts().1.take().expect("Date parse throw");
                drop(input);
                owner.complete(Completion::Throw(value))
            }
            NativeConversion::Value(string) => {
                if !matches!(
                    owner.resume.as_ref().expect("Date parse owner").0.phase,
                    Phase::Parse
                ) {
                    return Err(RuntimeError::Invariant("Date parse phase mismatch"));
                }
                let value = parsed_date_value(parse_date_string(&string), |instant| {
                    host.timezone_offset_minutes(instant)
                });
                owner.complete(Completion::Return(Number::compact(value).into()))
            }
        };
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        output
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let result = self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn number_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: NativeConversion<f64>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let result = self.number_admitted(state, poisoned, host, result);
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    fn number_admitted(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: NativeConversion<f64>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let mut owner = DateConstructorGuard::new(state, poisoned, self);
        let output = match result {
            NativeConversion::Throw(value) => {
                let mut input = OwnedValueGuard::new(owner.state, poisoned, value);
                if !matches!(
                    owner.resume.as_ref().expect("Date fields owner").0.phase,
                    Phase::Fields
                ) {
                    return Err(RuntimeError::Invariant("Date field phase mismatch"));
                }
                let value = input.parts().1.take().expect("Date field throw");
                drop(input);
                owner.complete(Completion::Throw(value))
            }
            NativeConversion::Value(value) => {
                let resume = owner.resume.as_mut().expect("Date fields owner");
                if !matches!(resume.0.phase, Phase::Fields) {
                    return Err(RuntimeError::Invariant("Date field phase mismatch"));
                }
                resume.0.fields[resume.0.index] = value;
                resume.0.index += 1;
                owner.fields(host)
            }
        };
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        output
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let result = self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn resume_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let result = self.resume_admitted(state, poisoned, result);
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    fn resume_admitted(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let mut owner = DateConstructorGuard::new(state, poisoned, self);
        let (throwing, value) = match result {
            Completion::Return(value) => (false, value),
            Completion::Throw(value) => (true, value),
        };
        let mut incoming = OwnedValueGuard::new(owner.state, poisoned, value);
        if !matches!(
            owner.resume.as_ref().expect("Date prototype owner").0.phase,
            Phase::Prototype
        ) {
            return Err(RuntimeError::Invariant("Date prototype phase mismatch"));
        }
        if throwing {
            let value = incoming.parts().1.take().expect("Date prototype throw");
            drop(incoming);
            return owner.complete(Completion::Throw(value));
        }
        owner
            .resume
            .as_mut()
            .expect("Date prototype owner")
            .0
            .prototype = incoming.parts().1.take();
        drop(incoming);
        let resume = owner.resume.as_mut().expect("Date prototype owner");
        let prototype_id = match resume.0.prototype.as_ref().expect("Date prototype reply") {
            JsValue::Object(object) => *object,
            _ => {
                owner.state.release_owned_jsvalue(
                    poisoned,
                    resume.0.prototype.take().expect("Date nonobject prototype"),
                )?;
                let realm = match owner.state.function_realm_from_jsvalue(
                    poisoned,
                    resume.0.realm,
                    &resume.0.new_target,
                )? {
                    FunctionRealmOutcome::Value(realm) => realm,
                    FunctionRealmOutcome::CyclePublishedThrow(value) => {
                        return owner.complete_step(DateConstructorStep::CyclePublished(
                            Completion::Throw(value),
                        ));
                    }
                };
                let object = owner
                    .state
                    .heap
                    .context(realm)?
                    .date_prototype
                    .ok_or(RuntimeError::Invariant("realm has no Date prototype"))?;
                // A genuine fallback producer held through shape publication.
                owner.state.heap.retain_object(object)?;
                resume.0.prototype = Some(JsValue::Object(object));
                object
            }
        };
        let object = owner
            .state
            .new_date_object(poisoned, prototype_id, resume.0.value)?;
        let mut output = DateConstructorStepGuard::new(
            owner.state,
            poisoned,
            DateConstructorStep::CyclePublished(Completion::Return(JsValue::Object(object))),
        );
        output.state.release_owned_jsvalue(
            poisoned,
            owner
                .resume
                .as_mut()
                .expect("Date prototype owner")
                .0
                .prototype
                .take()
                .expect("Date prototype producer"),
        )?;
        owner
            .resume
            .take()
            .expect("Date constructor owner")
            .retire_in_state(output.state, poisoned)?;
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        Ok(output.take())
    }

    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0
            .retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })
    }
}
