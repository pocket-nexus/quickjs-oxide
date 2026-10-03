//! A generator's executing state belongs to its suspended native operation.
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::object::ObjectRef;
use crate::engine::vm::call::NativeInvokeOutcome;
use crate::engine::vm::suspend::{RootedVmActivation, VmActivationResume, VmRunOutcome};

pub(crate) enum GeneratorStep {
    Complete(NativeInvokeOutcome),
    Run {
        activation: Box<RootedVmActivation>,
        input: VmActivationResume,
        resume: Box<GeneratorResume>,
    },
}

impl GeneratorStep {
    pub(super) fn finish(self, runtime: &Runtime) -> Result<NativeInvokeOutcome, RuntimeError> {
        match self {
            Self::Complete(result) => Ok(result),
            Self::Run {
                activation,
                input,
                resume,
            } => {
                let outcome = activation.run(runtime, input)?;
                match resume.resume(outcome)? {
                    Self::Complete(result) => Ok(result),
                    Self::Run { .. } => Err(RuntimeError::Invariant(
                        "generator resume requested another frame",
                    )),
                }
            }
        }
    }
}

pub(crate) struct GeneratorResume {
    pub(super) runtime: Runtime,
    pub(super) generator: ObjectRef,
    pub(super) active: bool,
}

impl GeneratorResume {
    // The waiting continuation already owns this guard in a Box; retain that ownership until resume finishes.
    #[allow(clippy::boxed_local)]
    pub(crate) fn resume(
        mut self: Box<Self>,
        outcome: VmRunOutcome,
    ) -> Result<GeneratorStep, RuntimeError> {
        let result = self
            .runtime
            .finish_generator_resume(&self.generator, outcome)?;
        self.active = false;
        Ok(GeneratorStep::Complete(result))
    }
}

impl Drop for GeneratorResume {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        // Normal abandonment completes the object. Panic or corrupted cleanup
        // instead quarantines the runtime without traversing it again.
        if self.active
            && self
                .runtime
                .complete_executing_generator(&self.generator)
                .is_err()
        {
            self.runtime.0.poisoned.set(true);
        }
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::{api::runtime::Runtime, value::Value};

    #[test]
    fn recursive_delegation_and_finally_use_one_owned_driver() {
        std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(|| {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let Value::Object(function) = context.eval("(function(){function* g(n){try{if(n) return yield* g(n-1); yield 42;}finally{if(!n)yield [1,2].map(x=>x+1)[1];}} var i=g(1000); var a=i.next();var b=i.return(9);var c=i.next();return a.value===42&&!a.done&&b.value===3&&!b.done&&c.value===9&&c.done;})").unwrap() else {panic!("expected function")};
            let callable = runtime.as_callable(&function).unwrap().unwrap();
            let profile = CostProfile::start();
            assert_eq!(context.call(&callable, Value::Undefined, &[]).unwrap(), Value::Bool(true));
            let _cost = profile.snapshot();
        }).unwrap().join().unwrap();
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<GeneratorStep>() <= 64);

#[cfg(all(test, not(target_family = "wasm"), panic = "unwind"))]
mod quarantine_tests {
    use super::*;
    use crate::engine::heap::GeneratorState;
    use crate::engine::value::Value;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn generator_resume_abandonment_respects_quarantine() {
        let Ok(case) = std::env::var("QJS_GENERATOR_RESUME_CHILD") else {
            for case in ["normal", "poison", "unwind", "error"] {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "engine::vm::generator::resume::quarantine_tests::generator_resume_abandonment_respects_quarantine",
                        "--nocapture",
                    ])
                    .env("QJS_GENERATOR_RESUME_CHILD", case)
                    .status()
                    .unwrap();
                assert!(status.success(), "generator cleanup {case}: {status}");
            }
            return;
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(generator) = context.eval("(function*(){yield 1;})()").unwrap() else {
            panic!("expected generator")
        };
        {
            let mut state = runtime.0.state.borrow_mut();
            let (_, _, cleanup) = state
                .heap
                .begin_generator_resume(generator.object_id())
                .unwrap();
            state.apply_cleanup(cleanup).unwrap();
        }
        let resume = GeneratorResume {
            runtime: runtime.clone(),
            generator: generator.try_clone().unwrap(),
            active: true,
        };
        let count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(generator.object_id())
            .unwrap();
        match case.as_str() {
            "normal" => {
                drop(resume);
                assert!(!runtime.is_poisoned());
                assert_eq!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .generator_snapshot(generator.object_id())
                        .unwrap()
                        .0,
                    GeneratorState::Completed
                );
                assert_eq!(context.eval("1 + 2").unwrap(), Value::number(3.0));
            }
            "poison" | "unwind" => {
                let state = runtime.0.state.borrow_mut();
                if case == "poison" {
                    runtime.0.poisoned.set(true);
                    drop(resume);
                } else {
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| {
                            let _resume = resume;
                            panic!("injected generator abandonment panic");
                        }))
                        .is_err()
                    );
                }
                assert!(runtime.is_poisoned());
                assert_eq!(
                    state
                        .heap
                        .generator_snapshot(generator.object_id())
                        .unwrap()
                        .0,
                    GeneratorState::Executing
                );
                assert_eq!(
                    state
                        .heap
                        .object_strong_count(generator.object_id())
                        .unwrap(),
                    count
                );
                drop(state);
                assert!(matches!(runtime.new_context(), Err(RuntimeError::Poisoned)));
            }
            "error" => {
                // A repeated completion violates the executing guard's cleanup
                // contract; its owned generator root must then remain untouched.
                runtime
                    .0
                    .state
                    .borrow_mut()
                    .heap
                    .complete_generator(generator.object_id())
                    .unwrap();
                drop(resume);
                assert!(runtime.is_poisoned());
                assert_eq!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object_strong_count(generator.object_id())
                        .unwrap(),
                    count
                );
                assert!(matches!(runtime.new_context(), Err(RuntimeError::Poisoned)));
            }
            _ => panic!("unknown generator cleanup case"),
        }
    }
}
