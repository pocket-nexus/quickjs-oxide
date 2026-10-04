//! Math argument conversion advances in source order while numerical kernels stay pure.
//!
//! Owned primitive calls borrow NativeActivation argv and allocate no Math argv.
//! At the first object, only the remaining suffix becomes continuation-owned;
//! NativeActivation retains the original arguments across callbacks. Profiling
//! distinguishes these two starts, without claiming native argument storage or
//! every primitive conversion is allocation-free.
use super::{quickjs_binary, quickjs_max, quickjs_min, quickjs_unary};
use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use std::cell::Cell;

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{MathBinaryKind, MathMinMaxKind, MathUnaryKind},
    heap::ContextId,
    value::{JsValue, conversion::NativeConversion, number::operations::Number},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum MathKind {
    Unary(MathUnaryKind),
    Binary(MathBinaryKind),
    MinMax(MathMinMaxKind),
    Hypot,
    Imul,
    Clz32,
}
impl MathKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::MathUnary(kind) => Self::Unary(kind),
            NativeFunctionId::MathBinary(kind) => Self::Binary(kind),
            NativeFunctionId::MathMinMax(kind) => Self::MinMax(kind),
            NativeFunctionId::MathHypot => Self::Hypot,
            NativeFunctionId::MathImul => Self::Imul,
            NativeFunctionId::MathClz32 => Self::Clz32,
            _ => return None,
        })
    }
}
pub(crate) enum MathStep {
    Complete(Completion),
    CyclePublished(Completion),
    Number { value: JsValue, resume: MathResume },
}
pub(crate) struct MathResume(Box<MathResumeState>);
impl std::ops::Deref for MathResume {
    type Target = MathResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for MathResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<MathResume>() <= 8);
pub(crate) struct MathResumeState {
    kind: MathKind,
    arguments: std::collections::VecDeque<JsValue>,
    result: Option<f64>,
    count: usize,
}
impl MathStep {
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
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                release(value)
            }
            Self::Number { value, resume } => {
                release(value)?;
                resume.retire_at_boundary(runtime)
            }
        }
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: MathKind,
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
        kind: MathKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant("Math requires generic invocation"));
        }
        let count = match kind {
            MathKind::Unary(_) | MathKind::Clz32 => 1,
            MathKind::Binary(_) | MathKind::Imul => 2,
            _ => arguments.actual_arg_count,
        };
        let values = arguments
            .readable
            .get(..count)
            .ok_or(RuntimeError::Invariant("Math argv was not padded"))?;

        {
            let mut resume = MathResumeState {
                kind,
                arguments: std::collections::VecDeque::new(),
                result: None,
                count,
            };
            for (index, value) in values.iter().enumerate() {
                if matches!(value, JsValue::Object(_)) {
                    // The native activation owns original argv. A suspended
                    // continuation needs only the not-yet-converted suffix.
                    resume
                        .arguments
                        .try_reserve(values.len() - index)
                        .map_err(|_| {
                            RuntimeError::Invariant("Math remaining argv allocation failed")
                        })?;
                    for remaining_value in &values[index..] {
                        let value = match state.dup_jsvalue(remaining_value) {
                            Ok(value) => value,
                            Err(error) => {
                                resume.retire_in_state(state, poisoned)?;
                                return Err(error);
                            }
                        };
                        resume.arguments.push_back(value);
                    }
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "math_remaining_arguments_owned",
                    );
                    return MathResume(Box::new(resume)).next();
                }
                // Object arguments above retain the shared waiting protocol.
                // NativeActivation already owns this primitive: borrow it in
                // the same conversion kernel used by NumberStep completion.
                let result = match state
                    .number_from_primitive_jsvalue_with_publication(poisoned, realm, value)?
                {
                    crate::engine::value::conversion::NumberPrimitiveStep::Value(number) => {
                        NativeConversion::Value(number)
                    }
                    crate::engine::value::conversion::NumberPrimitiveStep::CyclePublishedThrow(
                        value,
                    ) => {
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_owned_execution_event(
                            "math_completed_without_argument_storage",
                        );
                        return Ok(Self::CyclePublished(Completion::Throw(value)));
                    }
                };
                if let Some(completion) = resume.accept_number(result)? {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "math_completed_without_argument_storage",
                    );
                    return Ok(Self::Complete(completion));
                }
            }
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "math_completed_without_argument_storage",
            );
            resume.finish()
        }
    }
}
impl MathResume {
    fn next(mut self) -> Result<MathStep, RuntimeError> {
        if let Some(value) = self.0.arguments.pop_front() {
            return Ok(MathStep::Number {
                value,
                resume: self,
            });
        }
        self.0.finish()
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<MathStep, RuntimeError> {
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
    ) -> Result<MathStep, RuntimeError> {
        if let Some(completion) = self.accept_number(result)? {
            let (value, thrown) = match completion {
                Completion::Return(value) => (value, false),
                Completion::Throw(value) => (value, true),
            };
            let mut output = OwnedValueGuard::new(state, poisoned, value);
            let (state, output) = output.parts();
            self.0.retire_in_state(state, poisoned)?;
            let value = output.take().expect("Math result owner");
            Ok(MathStep::Complete(if thrown {
                Completion::Throw(value)
            } else {
                Completion::Return(value)
            }))
        } else {
            self.next()
        }
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0.retire_in_state(state, poisoned)
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
            if runtime.skip_cleanup() {
                return Err(RuntimeError::Poisoned);
            }
            runtime.release_jsvalue(value)?;
            if runtime.is_poisoned() {
                Err(RuntimeError::Poisoned)
            } else {
                Ok(())
            }
        })
    }
}
impl MathResumeState {
    fn retire_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        // Pop one consumed edge at a time; an interrupted suffix remains in its owner.
        while let Some(value) = self.arguments.pop_front() {
            release(value)?;
        }
        Ok(())
    }

    fn finish(self) -> Result<MathStep, RuntimeError> {
        let value = match self.kind {
            MathKind::MinMax(kind) if self.result.is_none() => {
                return Ok(MathStep::Complete(Completion::Return(JsValue::Float(
                    match kind {
                        MathMinMaxKind::Min => f64::INFINITY,
                        MathMinMaxKind::Max => f64::NEG_INFINITY,
                    },
                ))));
            }
            MathKind::Hypot if self.count == 0 => {
                return Ok(MathStep::Complete(Completion::Return(JsValue::Int(0))));
            }
            MathKind::Hypot if self.count == 1 => self
                .result
                .ok_or(RuntimeError::Invariant("Math hypot result missing"))?
                .abs(),
            _ => self
                .result
                .ok_or(RuntimeError::Invariant("Math result missing"))?,
        };
        Ok(MathStep::Complete(Completion::Return(
            Number::compact(value).into(),
        )))
    }
    /// One numerical accumulation kernel for immediate and suspended inputs.
    fn accept_number(
        &mut self,
        result: NativeConversion<f64>,
    ) -> Result<Option<Completion>, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Some(Completion::Throw(value)));
            }
        };
        self.result = Some(match self.kind {
            MathKind::Unary(kind) => quickjs_unary(kind, value),
            MathKind::Clz32 => {
                return Ok(Some(Completion::Return(JsValue::Int(
                    Runtime::to_uint32_number(value).leading_zeros() as i32,
                ))));
            }
            MathKind::Binary(kind) => {
                if let Some(left) = self.result {
                    quickjs_binary(kind, left, value)
                } else {
                    value
                }
            }
            MathKind::Imul => {
                if let Some(left) = self.result {
                    let product = Runtime::to_uint32_number(left)
                        .wrapping_mul(Runtime::to_uint32_number(value));
                    return Ok(Some(Completion::Return(JsValue::Int(i32::from_ne_bytes(
                        product.to_ne_bytes(),
                    )))));
                } else {
                    value
                }
            }
            MathKind::MinMax(kind) => {
                if let Some(left) = self.result {
                    if left.is_nan() {
                        left
                    } else if value.is_nan() {
                        value
                    } else {
                        match kind {
                            MathMinMaxKind::Min => quickjs_min(left, value),
                            MathMinMaxKind::Max => quickjs_max(left, value),
                        }
                    }
                } else {
                    value
                }
            }
            MathKind::Hypot => {
                if let Some(left) = self.result {
                    left.hypot(value)
                } else {
                    value
                }
            }
        });
        Ok(None)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    step: MathStep,
) -> Result<Completion, RuntimeError> {
    let mut step = MathBoundaryGuard {
        runtime,
        step: Some(step),
    };
    loop {
        match step.step.take().expect("Math boundary progress") {
            MathStep::Complete(result) => return Ok(result),
            MathStep::CyclePublished(result) => {
                step.step = Some(MathStep::CyclePublished(result));
                runtime.collect_if_requested()?;
                let MathStep::CyclePublished(result) =
                    step.step.take().expect("published Math completion")
                else {
                    unreachable!()
                };
                return Ok(result);
            }
            MathStep::Number { value, resume } => {
                let mut owner = MathBoundaryGuard {
                    runtime,
                    step: Some(MathStep::Number {
                        value: JsValue::Undefined,
                        resume,
                    }),
                };
                let result = runtime.native_to_number_jsvalue(realm, value)?;
                let MathStep::Number { resume, .. } = owner.step.take().expect("Math parent")
                else {
                    unreachable!()
                };
                step.step = Some(resume.number(runtime, result)?);
            }
        }
    }
}
impl MathStep {
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                state.release_owned_jsvalue(poisoned, value)
            }
            Self::Number { value, resume } => {
                state.release_owned_jsvalue(poisoned, value)?;
                resume.retire_in_state(state, poisoned)
            }
        }
    }
}
struct MathBoundaryGuard<'a> {
    runtime: &'a Runtime,
    step: Option<MathStep>,
}
impl Drop for MathBoundaryGuard<'_> {
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

#[cfg(all(test, feature = "profiling"))]
mod tests;

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<MathStep>() <= 64);
