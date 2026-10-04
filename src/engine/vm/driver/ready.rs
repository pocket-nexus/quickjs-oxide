//! Resume the single execution stream after operations that do not wait or call JS.
//! Opcode semantics live in `execute_frame`; this module completes continuations.
use super::{CallStep, VmAction};
use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::vm::Completion;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::FrameId;

pub(super) enum Boundary {
    Exit(VmAction),
    /// An existing property/query helper scheduled work; revisit the outer
    /// driver's pending-call/frame checks instead of assuming this frame ran.
    Entered,
    /// Operand domains were checked and next_operation was advanced once.
    /// The outer driver constructs the waiting task without repeating either.
    Conversion(VmAction),
    /// An actual resident Query boundary already produced this conversion task.
    QueryConversion(crate::engine::vm::conversion_driver::ConversionTask),
    Complete(Completion),
}

#[inline(never)]
pub(super) fn run(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    mut id: FrameId,
    next_operation: &mut u64,
) -> Result<Boundary, Error> {
    #[cfg(feature = "profiling")]
    let mut entered = false;
    loop {
        // The concrete shared mutex seed/reply already selected a leaf word. Its
        // resident commit is not a new allocation/scheduler pressure boundary.
        // Remaining legacy producer boundaries keep their original service.
        if !matches!(
            execution.selected_named_read,
            Some(
                crate::engine::vm::property_driver::SelectedNamedRead::Shared(_)
                    | crate::engine::vm::property_driver::SelectedNamedRead::SharedReady(_)
            )
        ) {
            runtime
                .collect_if_requested()
                .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        }
        #[cfg(feature = "profiling")]
        {
            if entered {
                record_event("frame_authentication_reentry");
            }
            entered = true;
        }
        // The state borrow ends before materialization and every legacy helper.
        // Unwind likewise drops it before RunningExecution's cleanup guard.
        let result = {
            // Public roots dropped by a legacy adapter coordinate outside the
            // exclusive segment. No public-root Drop occurs inside this loop.
            runtime
                .drain_deferred_references()
                .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
            let mut state = runtime.0.state.borrow_mut();
            crate::engine::vm::execute::execute_frame_in_state(runtime, &mut state, execution, id)
        };
        // A segment can install and retire several ordinary frames. Its cold
        // action and fault PC belong to the actual current frame at exit.
        id = execution
            .frames
            .current_id()
            .ok_or_else(|| invariant("execution segment lost current frame"))?;
        // A failed owned release may have partially changed heap cleanup.
        // Do not materialize diagnostics or run another adapter on that state.
        if runtime.0.poisoned.get() {
            return Err(result
                .err()
                .unwrap_or_else(|| Error::internal("runtime is poisoned")));
        }
        #[cfg(feature = "profiling")]
        record_segment_boundary(&result);
        #[cfg(feature = "profiling")]
        record_exit(&result);
        // Ordinary Call/Return need no observable activation. Cold operations
        // may allocate an error, release an observable owner or invoke code.
        if result.as_ref().map_or(true, VmAction::observes_activation)
            && !matches!(
                execution.selected_named_read,
                Some(crate::engine::vm::property_driver::SelectedNamedRead::Shared(_))
            )
        {
            execution.frames.materialize(runtime)?;
        }
        let exit = result?;
        match exit {
            VmAction::Materialize => continue,
            VmAction::Pure(operation) => {
                // Pure leaves can complete or throw, but cannot install another
                // frame or a pending conversion. Keep their successful result
                // in this loop instead of redispatching through the cold driver.
                match crate::engine::vm::frame_operations::pure(runtime, execution, id, operation)?
                {
                    CallStep::Entered => {
                        #[cfg(feature = "profiling")]
                        record_event("pure_completed_in_same_frame");
                    }
                    CallStep::Complete(completion) => return Ok(Boundary::Complete(completion)),
                    CallStep::Bridge => return Err(invariant("pure operation attempted replay")),
                }
            }
            VmAction::Call {
                arguments,
                method,
                tail,
                fallthrough,
            } => {
                let selected_native = execution.selected_native.take();
                if let Some(boundary) = enter_call(
                    runtime,
                    execution,
                    &mut id,
                    arguments,
                    method,
                    tail,
                    selected_native,
                    fallthrough,
                )? {
                    return Ok(boundary);
                }
            }
            VmAction::NativeProgress => {
                return resume_native_boundary(runtime, execution);
            }
            VmAction::Complete => match super::ordinary::finish(runtime, execution, id)? {
                super::ordinary::ReturnProgress::NativeThrow => {
                    execution.frames.materialize(runtime)?;
                    return Ok(Boundary::Exit(VmAction::Throw));
                }
                super::ordinary::ReturnProgress::NativeBoundary => {
                    execution.frames.materialize(runtime)?;
                    return resume_native_boundary(runtime, execution);
                }
                super::ordinary::ReturnProgress::Declined => return Ok(Boundary::Exit(exit)),
                super::ordinary::ReturnProgress::Returned => {
                    id = execution.frames.current_id().unwrap()
                }
                super::ordinary::ReturnProgress::Property(CallStep::Entered) => {
                    return Ok(Boundary::Entered);
                }
                super::ordinary::ReturnProgress::Property(CallStep::Complete(completion)) => {
                    return Ok(Boundary::Complete(completion));
                }
                super::ordinary::ReturnProgress::Property(CallStep::Bridge) => {
                    return Err(invariant("property return attempted replay"));
                }
            },
            #[cfg(all(test, feature = "profiling"))]
            VmAction::ReleaseOperand { .. } => {
                if !crate::engine::vm::frame_operations::complete_owned_slot(
                    runtime, execution, id, exit,
                )? {
                    return Err(invariant(
                        "direct slot completion changed its frame protocol",
                    ));
                }
            }

            VmAction::Numeric { kind, fallthrough } => {
                use crate::engine::vm::frame_operations::NumericProgress;
                let Some(progress) =
                    crate::engine::vm::frame_operations::try_complete_primitive_numeric(
                        runtime,
                        execution,
                        id,
                        kind,
                        fallthrough,
                    )?
                else {
                    #[cfg(feature = "profiling")]
                    record_event("numeric_primitive_declined");
                    return Ok(Boundary::Exit(exit));
                };
                match progress {
                    NumericProgress::Completed => {
                        #[cfg(feature = "profiling")]
                        record_event("numeric_completed_in_same_frame");
                        #[cfg(feature = "profiling")]
                        if kind.primitive_arithmetic() {
                            record_event("numeric_completed_with_carried_fallthrough");
                        }
                    }
                    NumericProgress::Deferred(CallStep::Entered) => return Ok(Boundary::Entered),
                    NumericProgress::Deferred(CallStep::Complete(completion)) => {
                        return Ok(Boundary::Complete(completion));
                    }
                    NumericProgress::Deferred(CallStep::Bridge) => {
                        return Err(invariant("numeric operation attempted replay"));
                    }
                }
            }
            VmAction::ConvertPlus | VmAction::ConvertAdd => {
                let addition = exit == VmAction::ConvertAdd;
                use crate::engine::vm::conversion_driver::PrimitiveCompletion;
                match crate::engine::vm::conversion_driver::complete_primitives(
                    runtime,
                    execution,
                    id,
                    addition,
                    next_operation,
                )? {
                    PrimitiveCompletion::Completed => {}
                    PrimitiveCompletion::Throw(value) => {
                        return Ok(Boundary::Complete(Completion::Throw(value)));
                    }
                    PrimitiveCompletion::Declined => return Ok(Boundary::Conversion(exit)),
                }
            }

            VmAction::GetField {
                index,
                keep_receiver,
                fallthrough,
            } => {
                #[cfg(feature = "profiling")]
                record_event("property_read_action_exit");
                #[cfg(feature = "profiling")]
                record_event("driver_handoff.named_read");
                let selected = execution.selected_named_read.take();
                let selected = match selected {
                    Some(crate::engine::vm::property_driver::SelectedNamedRead::Shared(word)) => {
                        // Ended State before this actual mutex. The seed owns
                        // exact backing/index/word; no Get selection is replayed.
                        let word = word
                            .read()
                            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
                        let read = runtime
                            .0
                            .state
                            .borrow_mut()
                            .own_typed_read_word(word)
                            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
                        execution.selected_named_read = Some(
                            crate::engine::vm::property_driver::SelectedNamedRead::SharedReady(
                                read,
                            ),
                        );
                        #[cfg(feature = "profiling")]
                        record_event("core.named_read_shared_mutex");
                        continue;
                    }
                    selected => selected,
                };
                let progress = crate::engine::vm::property_driver::read_progress_selected(
                    runtime,
                    execution,
                    id,
                    crate::engine::vm::property_driver::ReadKey::Static(index),
                    keep_receiver,
                    fallthrough,
                    selected,
                )?;
                #[cfg(feature = "profiling")]
                if matches!(
                    progress,
                    crate::engine::vm::property_driver::PropertyProgress::Completed
                ) {
                    record_event("property_read_completed_with_carried_fallthrough");
                }
                if let Some(boundary) = property_boundary(progress) {
                    return Ok(boundary);
                }
            }
            VmAction::GetElement {
                keep_receiver,
                keep_key,
                fallthrough,
            } => {
                #[cfg(feature = "profiling")]
                record_event("property_read_action_exit");
                let frame = execution.frames.current_mut(id)?;
                if !matches!(
                    execution.slots.peek(&frame.window, 1)?,
                    crate::engine::value::JsValue::Null | crate::engine::value::JsValue::Undefined
                ) && matches!(
                    execution.slots.peek(&frame.window, 0)?,
                    crate::engine::value::JsValue::Object(_)
                ) {
                    return Ok(Boundary::Exit(exit));
                }
                let progress = crate::engine::vm::property_driver::read_progress(
                    runtime,
                    execution,
                    id,
                    crate::engine::vm::property_driver::ReadKey::Computed { keep_key },
                    keep_receiver,
                    fallthrough,
                )?;
                #[cfg(feature = "profiling")]
                if matches!(
                    progress,
                    crate::engine::vm::property_driver::PropertyProgress::Completed
                ) {
                    record_event("property_read_completed_with_carried_fallthrough");
                }
                if let Some(boundary) = property_boundary(progress) {
                    return Ok(boundary);
                }
            }
            VmAction::SetProperty(key) => {
                let frame = execution.frames.current_mut(id)?;
                if key.is_none()
                    && matches!(
                        execution.slots.peek(&frame.window, 1)?,
                        crate::engine::value::JsValue::Object(_)
                    )
                {
                    return Ok(Boundary::Exit(exit));
                }
                let progress = crate::engine::vm::property_write_driver::write_progress(
                    runtime, execution, id, key,
                )?;
                if let Some(boundary) = property_boundary(progress) {
                    return Ok(boundary);
                }
            }
            _ => return Ok(Boundary::Exit(exit)),
        }
    }
}

fn resume_native_boundary(
    runtime: &Runtime,
    execution: &mut RunningExecution,
) -> Result<Boundary, Error> {
    let packet = execution
        .selected_native_query
        .take()
        .ok_or_else(|| invariant("resident native action lost its selected query"))?;
    match crate::engine::vm::proxy_get_driver::resume_resident_boundary(runtime, execution, packet)?
    {
        crate::engine::vm::proxy_get_driver::Progress::Call(CallStep::Entered) => {
            Ok(Boundary::Entered)
        }
        crate::engine::vm::proxy_get_driver::Progress::Call(CallStep::Complete(completion)) => {
            Ok(Boundary::Complete(completion))
        }
        crate::engine::vm::proxy_get_driver::Progress::Call(CallStep::Bridge) => {
            Err(invariant("resident native query attempted replay"))
        }
        crate::engine::vm::proxy_get_driver::Progress::Conversion(task) => {
            Ok(Boundary::QueryConversion(task))
        }
    }
}

fn property_boundary(
    progress: crate::engine::vm::property_driver::PropertyProgress,
) -> Option<Boundary> {
    use crate::engine::vm::property_driver::PropertyProgress;
    match progress {
        PropertyProgress::Completed => None,
        PropertyProgress::Deferred(CallStep::Entered) => Some(Boundary::Entered),
        PropertyProgress::Deferred(CallStep::Complete(completion)) => {
            Some(Boundary::Complete(completion))
        }
        PropertyProgress::Deferred(CallStep::Bridge) => Some(Boundary::Exit(VmAction::Bridge)),
    }
}

// Keep the decoded call facts explicit across the ordinary/native boundary.
#[allow(clippy::too_many_arguments)]
fn enter_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: &mut FrameId,
    arguments: u16,
    method: bool,
    tail: bool,
    selected_native: Option<crate::engine::object::LinkedNativeSelection>,
    fallthrough: crate::engine::vm::execute::FallthroughPc,
) -> Result<Option<Boundary>, Error> {
    Ok(
        match super::ordinary::enter_selected(
            runtime,
            execution,
            *id,
            arguments,
            method,
            tail,
            selected_native,
            fallthrough,
        )? {
            super::ordinary::Entry::Ordinary => {
                *id = execution.frames.current_id().unwrap();
                None
            }
            super::ordinary::Entry::NativeReady => None,
            super::ordinary::Entry::NativeBoundary => {
                Some(resume_native_boundary(runtime, execution)?)
            }
            super::ordinary::Entry::NativeComplete => Some(Boundary::Complete(Completion::Return(
                execution
                    .pending
                    .take()
                    .ok_or_else(|| invariant("tail call lost its owned reply"))?,
            ))),
            super::ordinary::Entry::NativeThrow => {
                let frame = execution.frames.current_mut(*id)?;
                let value = execution.slots.pop(&mut frame.window)?;
                Some(Boundary::Complete(Completion::Throw(value)))
            }
            super::ordinary::Entry::Native(CallStep::Entered) => Some(Boundary::Entered),
            super::ordinary::Entry::Native(CallStep::Complete(completion)) => {
                Some(Boundary::Complete(completion))
            }
            super::ordinary::Entry::Native(CallStep::Bridge) => {
                Some(Boundary::Exit(VmAction::Bridge))
            }
            super::ordinary::Entry::General => {
                execution.frames.materialize(runtime)?;
                Some(Boundary::Exit(VmAction::Call {
                    arguments,
                    method,
                    tail,
                    fallthrough,
                }))
            }
        },
    )
}

/// Error allocation and diagnostic-only dispatch must not widen the ordinary
/// ready loop. Profiling helpers are absent from the ordinary build.
#[cold]
#[inline(never)]
fn invariant(message: &'static str) -> Error {
    Error::internal(message)
}

#[cfg(feature = "profiling")]
#[cold]
#[inline(never)]
fn record_exit(result: &Result<VmAction, Error>) {
    use crate::engine::api::profiling::record_owned_execution_layout as layout;
    layout::<VmAction>("VmAction");
    layout::<Result<VmAction, Error>>("Result<VmAction, Error>");
    layout::<crate::engine::vm::frame::Frame>("Frame");
    record_event("core.frame_executor_exit");
    match result {
        Ok(VmAction::Call { .. }) => record_event("core.call_frame_executor_exit"),
        Ok(VmAction::Complete) => record_event("core.return_frame_executor_exit"),
        _ => {}
    }
    record_event(match result {
        Ok(exit) => exit.diagnostic_name(),
        Err(_) => "execute_continuation.EngineError",
    });
    if matches!(result, Ok(VmAction::Numeric { .. })) {
        record_event("numeric_action_exit");
    }
}

#[cfg(feature = "profiling")]
#[cold]
#[inline(never)]
fn record_event(event: &'static str) {
    crate::engine::api::profiling::record_owned_execution_event(event);
}

#[cfg(feature = "profiling")]
fn record_segment_boundary(result: &Result<VmAction, Error>) {
    let boundary = match result {
        Ok(VmAction::Call { .. }) => "core.legacy_boundary.ordinary_call",
        Ok(VmAction::Complete) => "core.legacy_boundary.ordinary_return",
        Ok(VmAction::GetField { .. } | VmAction::GetElement { .. }) => {
            "core.legacy_boundary.property_read"
        }
        Ok(VmAction::Pure(_)) => "core.legacy_boundary.pure",
        Ok(VmAction::Numeric { .. } | VmAction::ConvertAdd | VmAction::ConvertPlus) => {
            "core.legacy_boundary.numeric"
        }
        Ok(VmAction::Materialize) => "core.legacy_boundary.materialize",
        Err(_) => "core.legacy_boundary.error",
        Ok(_) => "core.legacy_boundary.other",
    };
    crate::engine::api::profiling::record_owned_execution_event(boundary);
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn synchronous_leaves_resume_and_throw_without_losing_the_current_frame() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"
                function same(a,b) { return a === b; }
                const object = new Proxy({}, { get() { throw 'unexpected conversion'; } });
                let count = 0;
                for (let i=0;i<32;i++) {
                    if (same(object,object) && typeof object === 'object') count++;
                }
                class NeedsNew {}
                let trace = '';
                try { NeedsNew(); }
                catch (error) { trace += error instanceof TypeError ? 'caught' : 'wrong'; }
                finally { trace += ':finally'; }
                count === 32 && trace === 'caught:finally' && same(object,object);
            "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert!(
                events
                    .get("pure_completed_in_same_frame")
                    .copied()
                    .unwrap_or(0)
                    > 0
            );
            assert!(
                events
                    .get("strict_comparison.local_value")
                    .copied()
                    .unwrap_or(0)
                    >= 32
            );
        }
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[cfg(test)]
mod shared_named_read_tests {
    use crate::engine::{
        api::{Runtime, Value},
        code::exec_opcode::Opcode,
        heap::GcPolicy,
        value::JsValue,
        vm::{
            execute::{VmAction, execute_frame},
            property_driver::{SelectedNamedRead, read_completion_tests::read_fixture},
        },
    };
    #[test]
    fn shared_named_word_crosses_its_mutex_then_commits_once_without_leaf_pressure_service() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let Value::Object(cycle) = context
            .eval("(()=>{let o={};o.self=o;return o})()")
            .unwrap()
        else {
            panic!("cycle")
        };
        let cycle = cycle.into_handle();
        runtime.release_jsvalue(JsValue::Object(cycle)).unwrap();
        let input = context.eval("(()=>{let a=new BigUint64Array(new SharedArrayBuffer(8));a[0]=18446744073709551615n;return a})()").unwrap();
        let input = runtime.into_jsvalue(input).unwrap();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){const {'0':v}=o;return v})",
            Opcode::GetField2Cached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution.slots.push(&mut frame.window, input).unwrap();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::GetField { .. }
        ));
        assert!(matches!(
            &execution.selected_named_read,
            Some(SelectedNamedRead::Shared(_))
        ));
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut operation = 0;
        let result = super::run(&runtime, &mut execution, id, &mut operation).unwrap();
        assert!(matches!(result, super::Boundary::Exit(VmAction::Complete)));
        assert!(execution.selected_named_read.is_none());
        let state = runtime.0.state.borrow();
        let Some(JsValue::BigInt(output)) = execution.pending.as_ref() else {
            panic!("owned bigint word")
        };
        assert_eq!(
            state.heap.bigint(*output).unwrap().to_string(),
            "18446744073709551615"
        );
        assert!(state.heap.object(cycle).is_ok());
        assert!(runtime.0.gc_pressure.requested());
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("core.named_read_shared_mutex"), Some(&1));
            assert!(!events.contains_key("gc.automatic.started"));
        }
    }
}
