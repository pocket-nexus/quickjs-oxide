//! Branded scalar formatting and BigInt widths preserve argument conversion order.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    builtins::native::{BigIntAsNKind, NativeFunctionId, NumberFormatKind, PrimitiveKind},
    heap::{
        ContextId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
use std::cell::Cell;
#[derive(Clone, Copy)]
pub(crate) enum NumericKind {
    ToString(PrimitiveKind),
    Format(NumberFormatKind),
    BigIntAsN(BigIntAsNKind),
}
impl NumericKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::PrimitivePrototypeToString(kind) => Some(Self::ToString(kind)),
            NativeFunctionId::NumberPrototypeFormat(kind) => Some(Self::Format(kind)),
            NativeFunctionId::BigIntAsN(kind) => Some(Self::BigIntAsN(kind)),
            _ => None,
        }
    }
}
pub(crate) enum NumericStep {
    Complete(Completion),
    Number {
        value: JsValue,
        resume: NumericResume,
    },
    Primitive {
        value: JsValue,
        resume: NumericResume,
    },
}
#[derive(Clone, Copy)]
enum Phase {
    Radix,
    Digits,
    Width,
    BigInt,
}
pub(crate) struct NumericResume(Box<NumericResumeState>);
pub(crate) struct NumericResumeState {
    realm: ContextId,
    kind: NumericKind,
    value: JsValue,
    argument_undefined: bool,
    phase: Phase,
    bits: u64,
}
const _: () = assert!(std::mem::size_of::<NumericResume>() <= 8);

impl NumericStep {
    /// The actual boundary may overlap an admitted State lease. Coordinate
    /// those releases through the existing FIFO rather than quarantining a
    /// normal borrow conflict; destructive failure stops the remaining owners.
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            return self.retire_in_state(&mut state, &runtime.0.poisoned);
        }
        let release = |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        };
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value)) => release(value),
            Self::Number { value, resume } | Self::Primitive { value, resume } => {
                release(value)?;
                resume.retire_at_boundary(runtime)
            }
        }
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: NumericKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            kind,
            invocation,
            arguments,
        )
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: NumericKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "scalar numeric method requires generic invocation",
            ));
        };
        let argument = arguments.readable.first().unwrap_or(&JsValue::Undefined);
        let value = match kind {
            NumericKind::BigIntAsN(_) => state.dup_jsvalue(
                arguments
                    .readable
                    .get(1)
                    .ok_or(RuntimeError::Invariant("BigInt width argv was not padded"))?,
            )?,
            _ => {
                let brand = match kind {
                    NumericKind::ToString(kind) => kind,
                    _ => PrimitiveKind::Number,
                };
                match state.primitive_this_value_jsvalue(poisoned, realm, brand, this_value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                }
            }
        };
        let mut payload = OwnedValueGuard::new(state, poisoned, value);
        let (state, payload) = payload.parts();
        if let NumericKind::ToString(brand) = kind {
            if !matches!(brand, PrimitiveKind::Number | PrimitiveKind::BigInt)
                || matches!(argument, JsValue::Undefined)
            {
                return state
                    .finish_branded_to_string(
                        poisoned,
                        realm,
                        brand,
                        payload.take().expect("scalar brand owner"),
                        10,
                    )
                    .map(Self::Complete);
            }
        }
        let phase = match kind {
            NumericKind::ToString(_) => Phase::Radix,
            NumericKind::Format(_) => Phase::Digits,
            NumericKind::BigIntAsN(_) => Phase::Width,
        };
        // Keep the payload guarded until the fallible request duplicate succeeds.
        // The same domain allocation precedes that duplicate as before.
        let mut resume = NumericResume(Box::new(NumericResumeState {
            realm,
            kind,
            value: JsValue::Undefined,
            argument_undefined: matches!(argument, JsValue::Undefined),
            phase,
            bits: 0,
        }));
        match kind {
            NumericKind::Format(NumberFormatKind::LocaleString)
            | NumericKind::Format(NumberFormatKind::Precision)
                if matches!(argument, JsValue::Undefined)
                    || matches!(kind, NumericKind::Format(NumberFormatKind::LocaleString)) =>
            {
                resume.0.value = payload.take().expect("scalar brand owner");
                resume.format_in_state(state, poisoned, 0)
            }
            _ => {
                let argument = state.dup_jsvalue(argument)?;
                resume.0.value = payload.take().expect("scalar brand owner");
                Ok(Self::Number {
                    value: argument,
                    resume,
                })
            }
        }
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value)) => {
                state.release_owned_jsvalue(poisoned, value)
            }
            Self::Number { value, resume } | Self::Primitive { value, resume } => {
                state.release_owned_jsvalue(poisoned, value)?;
                resume.retire_in_state(state, poisoned)
            }
        }
    }
}

impl NumericResume {
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        state.release_owned_jsvalue(
            poisoned,
            std::mem::replace(&mut self.0.value, JsValue::Undefined),
        )
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        runtime.release_jsvalue(std::mem::replace(&mut self.0.value, JsValue::Undefined))?;
        if runtime.is_poisoned() {
            Err(RuntimeError::Poisoned)
        } else {
            Ok(())
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<NumericStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        )
    }
    pub(crate) fn number_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<f64>,
    ) -> Result<NumericStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                let mut output = OwnedValueGuard::new(state, poisoned, value);
                let (state, output) = output.parts();
                self.retire_in_state(state, poisoned)?;
                return Ok(NumericStep::Complete(Completion::Throw(
                    output.take().expect("numeric throw"),
                )));
            }
        };
        match self.0.phase {
            Phase::Digits => self.format_in_state(
                state,
                poisoned,
                crate::engine::value::number::to_int32_sat(value),
            ),
            Phase::Radix => {
                let payload = std::mem::replace(&mut self.0.value, JsValue::Undefined);
                let mut payload = OwnedValueGuard::new(state, poisoned, payload);
                let (state, payload) = payload.parts();
                let radix = crate::engine::value::number::to_int32_sat(value);
                if !(2..=36).contains(&radix) {
                    let value = state.new_native_error_from_message(
                        poisoned,
                        self.0.realm,
                        NativeErrorKind::Range,
                        NativeErrorMessage::from_utf8("radix must be between 2 and 36"),
                    )?;
                    state
                        .release_owned_jsvalue(poisoned, payload.take().expect("radix payload"))?;
                    return Ok(NumericStep::Complete(Completion::Throw(JsValue::Object(
                        value,
                    ))));
                }
                let NumericKind::ToString(kind) = self.0.kind else {
                    return Err(RuntimeError::Invariant("scalar radix kind mismatch"));
                };
                state
                    .finish_branded_to_string(
                        poisoned,
                        self.0.realm,
                        kind,
                        payload.take().expect("radix payload"),
                        radix as u32,
                    )
                    .map(NumericStep::Complete)
            }
            Phase::Width => {
                let bits = state.index_from_number(poisoned, self.0.realm, value);
                let bits = match bits {
                    Ok(NativeConversion::Value(bits)) => bits,
                    Ok(NativeConversion::Throw(value)) => {
                        let mut output = OwnedValueGuard::new(state, poisoned, value);
                        let (state, output) = output.parts();
                        self.retire_in_state(state, poisoned)?;
                        return Ok(NumericStep::Complete(Completion::Throw(
                            output.take().expect("width throw"),
                        )));
                    }
                    Err(error) => {
                        if !poisoned.get() {
                            self.retire_in_state(state, poisoned)?;
                        }
                        return Err(error);
                    }
                };
                self.0.bits = bits;
                self.0.phase = Phase::BigInt;
                let value = std::mem::replace(&mut self.0.value, JsValue::Undefined);
                Ok(NumericStep::Primitive {
                    value,
                    resume: self,
                })
            }
            _ => {
                self.retire_in_state(state, poisoned)?;
                Err(RuntimeError::Invariant(
                    "scalar numeric reply phase mismatch",
                ))
            }
        }
    }
    fn format_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        digits: i32,
    ) -> Result<NumericStep, RuntimeError> {
        let value = std::mem::replace(&mut self.0.value, JsValue::Undefined);
        let mut payload = OwnedValueGuard::new(state, poisoned, value);
        let (state, payload) = payload.parts();
        let NumericKind::Format(kind) = self.0.kind else {
            return Err(RuntimeError::Invariant("scalar formatter kind mismatch"));
        };
        let number = payload
            .as_ref()
            .expect("Number format payload")
            .as_number()
            .ok_or(RuntimeError::Invariant(
                "Number formatter brand returned non-number",
            ))?;
        let result = match kind {
            NumberFormatKind::LocaleString => {
                crate::engine::value::number::to_string_radix(number, 10)
            }
            NumberFormatKind::Fixed => crate::engine::value::number::to_fixed(number, digits),
            NumberFormatKind::Exponential => crate::engine::value::number::to_exponential(
                number,
                (!self.0.argument_undefined).then_some(digits),
            ),
            NumberFormatKind::Precision => crate::engine::value::number::to_precision(
                number,
                (!self.0.argument_undefined).then_some(digits),
            ),
        };
        let result = state.finish_number_format(poisoned, self.0.realm, result);
        if poisoned.get() {
            return Err(result.err().unwrap_or(RuntimeError::Poisoned));
        }
        state.release_owned_jsvalue(poisoned, payload.take().expect("Number format payload"))?;
        result.map(NumericStep::Complete)
    }
    pub(crate) fn primitive(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<NumericStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        self.primitive_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        )
    }
    pub(crate) fn primitive_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Completion,
    ) -> Result<NumericStep, RuntimeError> {
        let (value, thrown) = match result {
            Completion::Return(value) => (value, false),
            Completion::Throw(value) => (value, true),
        };
        let mut reply = OwnedValueGuard::new(state, poisoned, value);
        let (state, reply) = reply.parts();
        if !matches!(self.0.phase, Phase::BigInt) {
            self.retire_in_state(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "BigInt width primitive phase mismatch",
            ));
        }
        if thrown {
            self.retire_in_state(state, poisoned)?;
            return Ok(NumericStep::Complete(Completion::Throw(
                reply.take().expect("BigInt throw"),
            )));
        }
        let result = state.bigint_from_primitive_jsvalue(
            poisoned,
            self.0.realm,
            reply.as_ref().expect("BigInt primitive reply"),
        );
        if poisoned.get() {
            return Err(result.err().unwrap_or(RuntimeError::Poisoned));
        }
        state.release_owned_jsvalue(poisoned, reply.take().expect("BigInt primitive reply"))?;
        let value = match result? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(NumericStep::Complete(Completion::Throw(value)));
            }
        };
        let NumericKind::BigIntAsN(kind) = self.0.kind else {
            return Err(RuntimeError::Invariant("BigInt width kind mismatch"));
        };
        let result = match kind {
            BigIntAsNKind::AsUintN => value.as_uint_n(self.0.bits),
            BigIntAsNKind::AsIntN => value.as_int_n(self.0.bits),
        };
        Ok(NumericStep::Complete(match result {
            Ok(value) => Completion::Return(match value.as_i64() {
                Some(value) => JsValue::ShortBigInt(value),
                None => JsValue::BigInt(state.heap.allocate_bigint(value)?),
            }),
            Err(_) => Completion::Throw(JsValue::Object(state.new_native_error_from_message(
                poisoned,
                self.0.realm,
                NativeErrorKind::Range,
                NativeErrorMessage::from_utf8("BigInt is too large to allocate"),
            )?)),
        }))
    }
}

/// Existing external synchronous consumer owns the actual domain while JS runs.
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    step: NumericStep,
) -> Result<Completion, RuntimeError> {
    let mut step = NumericBoundaryGuard {
        runtime,
        step: Some(step),
    };
    loop {
        match step.step.take().expect("numeric boundary progress") {
            NumericStep::Complete(result) => return Ok(result),
            NumericStep::Number { value, resume } => {
                // Install the resume before the fallible child conversion takes input.
                let mut owner = NumericBoundaryGuard {
                    runtime,
                    step: Some(NumericStep::Number {
                        value: JsValue::Undefined,
                        resume,
                    }),
                };
                let result = runtime.native_to_number_jsvalue(realm, value)?;
                let NumericStep::Number { resume, .. } = owner.step.take().expect("numeric parent")
                else {
                    unreachable!()
                };
                step.step = Some(resume.number(runtime, result)?);
            }
            NumericStep::Primitive { value, resume } => {
                let mut owner = NumericBoundaryGuard {
                    runtime,
                    step: Some(NumericStep::Primitive {
                        value: JsValue::Undefined,
                        resume,
                    }),
                };
                let result = runtime.to_primitive_jsvalue(
                    realm,
                    value,
                    crate::engine::vm::ToPrimitiveHint::Number,
                )?;
                let NumericStep::Primitive { resume, .. } =
                    owner.step.take().expect("numeric parent")
                else {
                    unreachable!()
                };
                step.step = Some(resume.primitive(runtime, result)?);
            }
        }
    }
}
struct NumericBoundaryGuard<'a> {
    runtime: &'a Runtime,
    step: Option<NumericStep>,
}
impl Drop for NumericBoundaryGuard<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if let Some(step) = self.step.take() {
            let _ = step.retire_at_boundary(self.runtime);
        }
    }
}
const _: () = assert!(std::mem::size_of::<NumericStep>() <= 64);
