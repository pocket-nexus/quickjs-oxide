//! Primitive constructors finish coercion before observing a supplied new.target prototype.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::PrimitiveKind,
    heap::ContextId,
    object::ObjectRef,
    object::PropertyKey,
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum PrimitiveConstructorStep {
    Complete(Completion),
    Primitive { resume: PrimitiveConstructorResume },
    String { resume: PrimitiveConstructorResume },
    Read { resume: PrimitiveConstructorResume },
}
enum Phase {
    Value,
    Prototype,
}
pub(crate) struct PrimitiveConstructorResume(Box<PrimitiveConstructorResumeState>);
impl std::ops::Deref for PrimitiveConstructorResume {
    type Target = PrimitiveConstructorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for PrimitiveConstructorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<PrimitiveConstructorResume>() <= 8);
pub(crate) struct PrimitiveConstructorResumeState {
    runtime: Runtime,
    pending_effect: PrimitiveConstructorStepPending,
    realm: ContextId,
    kind: PrimitiveKind,
    new_target: JsValue,
    value: JsValue,
    phase: Phase,
}
impl Drop for PrimitiveConstructorResumeState {
    /// Release the internal edges still owned when the request is abandoned.
    /// Drained fields are `None`/`Undefined` here; releases are defer-safe.
    fn drop(&mut self) {
        for value in [
            self.pending_effect.primitive_value.take(),
            self.pending_effect.string_value.take(),
            self.pending_effect.read_receiver.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        let new_target = std::mem::replace(&mut self.new_target, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(new_target);
        let value = std::mem::replace(&mut self.value, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(value);
    }
}
impl PrimitiveConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: PrimitiveKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "primitive constructor requires constructor-or-function invocation",
            ));
        };
        let mut resume = PrimitiveConstructorResume(Box::new(PrimitiveConstructorResumeState {
            runtime: runtime.clone(),
            pending_effect: PrimitiveConstructorStepPending::default(),
            realm,
            kind,
            new_target: runtime.dup_jsvalue(new_target)?,
            value: JsValue::Undefined,
            phase: Phase::Value,
        }));
        resume.value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("primitive constructor argv was not padded"),
        )?)?;
        if matches!(kind, PrimitiveKind::Symbol | PrimitiveKind::BigInt)
            && !matches!(resume.new_target, JsValue::Undefined)
        {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_not_constructor_error_jsvalue(realm, &resume.new_target)?,
            )));
        }
        match kind {
            PrimitiveKind::Boolean => {
                let value = JsValue::Bool(runtime.value_to_boolean_jsvalue(&resume.value)?);
                resume.converted(runtime, value)
            }
            PrimitiveKind::Number if arguments.actual_arg_count == 0 => {
                resume.converted(runtime, JsValue::Int(0))
            }
            PrimitiveKind::String if arguments.actual_arg_count == 0 => resume.converted(
                runtime,
                runtime.into_jsvalue(Value::String(JsString::from_static("")))?,
            ),
            PrimitiveKind::Symbol if matches!(resume.value, JsValue::Undefined) => {
                let symbol = runtime.new_symbol(None)?;
                Ok(Self::Complete(Completion::Return(
                    runtime.unroot_value(&Value::Symbol(symbol))?,
                )))
            }
            PrimitiveKind::String
                if matches!(resume.0.new_target, JsValue::Undefined)
                    && matches!(resume.value, JsValue::Symbol(_)) =>
            {
                let JsValue::Symbol(index) = &resume.value else {
                    unreachable!()
                };
                let atom = runtime.0.state.borrow().atoms.brand(*index)?;
                let symbol =
                    crate::engine::object::SymbolRef::from_borrowed_atom(runtime.clone(), atom)?;
                let description = runtime.symbol_descriptive_string(&symbol)?;
                resume.converted(runtime, runtime.into_jsvalue(Value::String(description))?)
            }
            PrimitiveKind::String | PrimitiveKind::Symbol => {
                let argument = std::mem::replace(&mut resume.value, JsValue::Undefined);
                if !matches!(argument, JsValue::Object(_)) {
                    if kind == PrimitiveKind::String && matches!(argument, JsValue::String(_)) {
                        return resume.converted(runtime, argument);
                    }
                    let result = runtime.native_to_js_string_jsvalue(realm, argument)?;
                    resume.string(runtime, result)
                } else {
                    Ok(Self::request_string(argument, resume))
                }
            }
            PrimitiveKind::Number | PrimitiveKind::BigInt => {
                let argument = std::mem::replace(&mut resume.value, JsValue::Undefined);
                if !matches!(argument, JsValue::Object(_)) {
                    resume.primitive(runtime, Completion::Return(argument))
                } else {
                    Ok(Self::request_primitive(argument, resume))
                }
            }
        }
    }
}
impl PrimitiveConstructorResume {
    pub(crate) fn primitive(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Value) {
            return Err(RuntimeError::Invariant(
                "primitive constructor coercion phase mismatch",
            ));
        }
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        self.0.value = value;
        let value = match self.0.kind {
            PrimitiveKind::Number => {
                match runtime.number_constructor_from_primitive(self.0.realm, &self.0.value)? {
                    NativeConversion::Value(value) => runtime.into_jsvalue(Value::number(value))?,
                    NativeConversion::Throw(value) => {
                        return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            PrimitiveKind::BigInt => {
                match runtime.bigint_constructor_from_primitive(self.0.realm, &self.0.value)? {
                    NativeConversion::Value(value) => {
                        if matches!(self.0.value, JsValue::BigInt(_) | JsValue::ShortBigInt(_)) {
                            runtime.dup_jsvalue(&self.0.value)?
                        } else {
                            runtime.into_jsvalue(Value::BigInt(value))?
                        }
                    }
                    NativeConversion::Throw(value) => {
                        return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "primitive constructor coercion kind mismatch",
                ));
            }
        };
        self.converted(runtime, value)
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Value) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "primitive constructor string phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        if self.0.kind == PrimitiveKind::Symbol {
            let symbol = runtime.new_symbol(Some(value))?;
            return Ok(PrimitiveConstructorStep::Complete(Completion::Return(
                runtime.unroot_value(&Value::Symbol(symbol))?,
            )));
        }
        self.converted(runtime, runtime.into_jsvalue(Value::String(value))?)
    }
    fn converted(
        mut self,
        runtime: &Runtime,
        value: JsValue,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if matches!(self.0.new_target, JsValue::Undefined) {
            return Ok(PrimitiveConstructorStep::Complete(Completion::Return(
                value,
            )));
        }
        let previous = std::mem::replace(&mut self.0.value, value);
        runtime.release_jsvalue(previous)?;
        self.0.phase = Phase::Prototype;
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
        Ok(PrimitiveConstructorStep::request_read(
            runtime.dup_jsvalue(&self.0.new_target)?,
            key,
            self,
        ))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Prototype) {
            return Err(RuntimeError::Invariant(
                "primitive constructor prototype phase mismatch",
            ));
        }
        let result_value = match result {
            Completion::Return(value) => Some(value),
            Completion::Throw(value) => {
                let new_target = std::mem::replace(&mut self.0.new_target, JsValue::Undefined);
                runtime.release_jsvalue(new_target)?;
                let primitive = std::mem::replace(&mut self.0.value, JsValue::Undefined);
                runtime.release_jsvalue(primitive)?;
                return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        let prototype = match result_value {
            Some(JsValue::Object(id)) => {
                let new_target = std::mem::replace(&mut self.0.new_target, JsValue::Undefined);
                runtime.release_jsvalue(new_target)?;
                ObjectRef::from_owned_handle(runtime.clone(), id)
            }
            other => {
                if let Some(value) = other {
                    runtime.release_jsvalue(value)?;
                }
                let realm = match runtime
                    .function_realm_from_jsvalue(self.0.realm, &self.0.new_target)?
                {
                    NativeConversion::Value(realm) => realm,
                    NativeConversion::Throw(value) => {
                        return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
                    }
                };
                runtime.primitive_prototype_for_realm(realm, self.0.kind)?
            }
        };
        let value = std::mem::replace(&mut self.0.value, JsValue::Undefined);
        Ok(PrimitiveConstructorStep::Complete(Completion::Return(
            JsValue::Object(
                runtime
                    .new_primitive_object_jsvalue(&prototype, self.0.kind, value)?
                    .into_handle(),
            ),
        )))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: PrimitiveConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            PrimitiveConstructorStep::Complete(result) => return Ok(result),
            PrimitiveConstructorStep::Primitive { mut resume } => {
                let value = resume.take_primitive_value();
                resume.primitive(
                    runtime,
                    runtime.to_primitive_jsvalue(
                        realm,
                        value,
                        crate::engine::vm::ToPrimitiveHint::Number,
                    )?,
                )?
            }
            PrimitiveConstructorStep::String { mut resume } => {
                let value = resume.take_string_value();
                resume.string(runtime, runtime.native_to_js_string_jsvalue(realm, value)?)?
            }
            PrimitiveConstructorStep::Read { mut resume } => {
                let receiver = resume.take_read_receiver();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?,
                )?
            }
        };
    }
}

#[cfg(test)]
mod local_completion_tests {
    use super::*;

    #[test]
    fn primitive_constructor_finishes_without_a_conversion_request() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let invocation = NativeInvocation::Construct {
            new_target: JsValue::Undefined,
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Int(42)],
        };
        let result = PrimitiveConstructorStep::start(
            &runtime,
            context.realm,
            PrimitiveKind::String,
            &invocation,
            &arguments,
        )
        .unwrap();
        let PrimitiveConstructorStep::Complete(Completion::Return(value)) = result else {
            panic!("primitive constructor must complete");
        };
        let value = runtime.root_and_release_jsvalue(value).unwrap();
        assert!(matches!(value, Value::String(value) if value == JsString::from_static("42")));
    }

    #[test]
    fn local_constructor_conversion_keeps_symbol_and_new_target_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let trace='', symbol=Symbol('x'), caught=false;
            const value={toString(){trace+='v';return 'x';}};
            const target=new Proxy(function(){},{get(t,k,r){if(k==='prototype')trace+='p';return Reflect.get(t,k,r);}});
            let object=Reflect.construct(String,[value],target);
            try{new String(symbol)}catch(e){caught=e instanceof TypeError;}
            return String(42)==='42'&&String() === '' && String(undefined)==='undefined'
                && String(symbol)==='Symbol(x)'&&caught&&trace==='vp'
                && String.prototype.valueOf.call(object)==='x'&&Number(42n)===42&&BigInt('42')===42n;
        })()"#).unwrap(),Value::Bool(true));
    }
}

#[derive(Default)]
struct PrimitiveConstructorStepPending {
    primitive_value: Option<JsValue>,
    string_value: Option<JsValue>,
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
}
impl PrimitiveConstructorStep {
    pub(crate) fn request_primitive(
        value: JsValue,
        mut resume: PrimitiveConstructorResume,
    ) -> Self {
        resume.0.pending_effect.primitive_value = Some(value);
        Self::Primitive { resume }
    }
    pub(crate) fn request_string(value: JsValue, mut resume: PrimitiveConstructorResume) -> Self {
        resume.0.pending_effect.string_value = Some(value);
        Self::String { resume }
    }
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: PrimitiveConstructorResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
}
impl PrimitiveConstructorResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .primitive_value
            .take()
            .expect("PrimitiveConstructorStep Primitive value")
    }
    pub(crate) fn take_string_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .string_value
            .take()
            .expect("PrimitiveConstructorStep String value")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("PrimitiveConstructorStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("PrimitiveConstructorStep Read key")
    }
}
const _: () = assert!(std::mem::size_of::<PrimitiveConstructorStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<PrimitiveConstructorStep>() <= 64);
