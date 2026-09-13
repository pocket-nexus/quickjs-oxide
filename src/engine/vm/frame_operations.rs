//! Outlined frame and binding operations. Keeping their temporary values out
//! of the resident driver frame leaves native stack room for callback reentry.

use super::Completion;
use super::driver::{CallStep, prepare_captured_reuse};
use super::exception::runtime_error_to_vm_error;
use super::execution::RunningExecution;
use super::frame::FrameId;
use super::run::RunExit;
use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::value::Value;
use crate::engine::value::conversion::NativeConversion;

#[inline(never)]
pub(super) fn step(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mut exit: RunExit,
) -> Result<Option<CallStep>, Error> {
    let original = exit;
    let mut forwarded = None;
    if let RunExit::ReleaseOperand { keep_top } = exit {
        let frame = execution.frames.current_mut(id)?;
        // Hot preflight has not changed an owner. Validate both operands before
        // moving either, then let the ordinary Drop path drain deferred work.
        execution.slots.peek(&frame.window, usize::from(keep_top))?;
        #[cfg(feature = "profiling")]
        let depth = execution.slots.depth(&frame.window);
        let kept = if keep_top {
            Some(execution.slots.pop(&mut frame.window)?)
        } else {
            None
        };
        let released = execution.slots.pop(&mut frame.window)?;
        if let Some(kept) = kept {
            execution.slots.push(&mut frame.window, kept)?;
        }
        // Publish the surviving stack before dropping the last temporary root.
        drop(released);
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("release resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        return Ok(Some(CallStep::Entered));
    }
    if exit == RunExit::HomeObject {
        let frame = execution.frames.current_mut(id)?;
        let home = runtime
            .bytecode_function_home_object(&frame.cold.function)
            .map_err(runtime_error_to_vm_error)?
            .ok_or_else(|| Error::internal("bytecode requested an uninstalled HomeObject"))?;
        #[cfg(feature = "profiling")]
        let depth = execution.slots.depth(&frame.window);
        execution
            .slots
            .push(&mut frame.window, Value::Object(home))?;
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("HomeObject resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        return Ok(Some(CallStep::Entered));
    }
    if exit == RunExit::GetSuper {
        let frame = execution.frames.current_mut(id)?;
        let value = execution.slots.peek(&frame.window, 0)?;
        if let Value::Object(object) = value {
            if object.belongs_to(runtime) {
                let value = runtime
                    .get_prototype_of(object)
                    .map_err(runtime_error_to_vm_error)?
                    .map_or(Value::Null, Value::Object);
                #[cfg(feature = "profiling")]
                let depth = execution.slots.depth(&frame.window);
                execution.slots.pop(&mut frame.window)?;
                execution.slots.push(&mut frame.window, value)?;
                frame.resume_pc = frame
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("super resume PC overflow"))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(depth);
                return Ok(Some(CallStep::Entered));
            }
        }
        exit = RunExit::Bridge;
    }
    if let RunExit::BindingError {
        index,
        redeclaration,
    } = exit
    {
        forwarded = Some(super::exception::binding_error(
            runtime,
            execution,
            id,
            index,
            redeclaration,
        )?);
        exit = RunExit::Complete;
    }
    if let RunExit::PrivateInitialize { index, kind } = exit {
        match super::private_bindings::step(runtime, execution, id, index, kind)? {
            None => return Ok(Some(CallStep::Entered)),
            Some(completion) => {
                forwarded = Some(completion);
                exit = RunExit::Complete;
            }
        }
    }
    if let RunExit::PrivateAccess { source, access } = exit {
        match super::private_access::step(runtime, execution, id, source, access)? {
            super::private_access::Outcome::Done | super::private_access::Outcome::Entered => {
                return Ok(Some(CallStep::Entered));
            }
            super::private_access::Outcome::Throw(value) => {
                forwarded = Some(Completion::Throw(value));
                exit = RunExit::Complete;
            }
            super::private_access::Outcome::Bridge => exit = RunExit::Bridge,
        }
    }
    if let RunExit::StrictEquality(negate) = exit {
        super::run::strict_comparison(execution, id, negate)?;
        return Ok(Some(CallStep::Entered));
    }
    if matches!(exit, RunExit::Arguments(_) | RunExit::Rest(_)) {
        match super::arguments_driver::step(runtime, execution, id, exit)? {
            None => return Ok(Some(CallStep::Entered)),
            Some(completion) => {
                forwarded = Some(completion);
                exit = RunExit::Complete;
            }
        }
    }
    if let RunExit::SetName(index) = exit {
        match super::property_keys::set_name(runtime, execution, id, index)? {
            None => return Ok(Some(CallStep::Entered)),
            Some(value) => {
                forwarded = Some(Completion::Throw(value));
                exit = RunExit::Complete;
            }
        }
    }

    if let RunExit::InstantiateClosure(index) = exit {
        super::closure_driver::instantiate(runtime, execution, id, index)?;
        return Ok(Some(CallStep::Entered));
    }
    if let RunExit::ResetCaptured(index) = exit {
        let frame = execution.frames.current_mut(id)?;
        let reusable = std::mem::take(&mut frame.cold.reusable_captured_locals[usize::from(index)]);
        let super::bindings::FrameBinding::Captured(root) =
            execution.slots.local(&frame.window, index)?
        else {
            return Err(Error::internal("captured reset lost its cell"));
        };
        super::bindings::reset_captured_binding(runtime, root, reusable)?;
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("reset resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(
            execution.slots.depth(&frame.window),
        );
        return Ok(Some(CallStep::Entered));
    }
    if let RunExit::CloseCaptured(index) = exit {
        let frame = execution.frames.current_mut(id)?;
        frame.cold.reusable_captured_locals[usize::from(index)] = false;
        super::bindings::close_frame_binding(
            runtime,
            execution.slots.local_mut(&frame.window, index)?,
            frame.executable.local_definitions[usize::from(index)].kind,
        )?;
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("close resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(
            execution.slots.depth(&frame.window),
        );
        return Ok(Some(CallStep::Entered));
    }
    if matches!(
        exit,
        RunExit::Catch(_) | RunExit::DropCatch | RunExit::NipCatch
    ) {
        let frame = execution.frames.current_mut(id)?;
        let depth = execution.slots.depth(&frame.window);
        if let RunExit::Catch(target) = exit {
            if target as usize >= frame.executable.code.len() {
                return Err(Error::internal("catch target is out of bounds"));
            }
            frame
                .cold
                .regions
                .try_reserve(1)
                .map_err(|_| Error::internal("catch region allocation failed"))?;
            frame.cold.regions.push(super::VmUnwindRegion::Catch {
                target: target as usize,
                stack_depth: depth,
            });
        } else {
            let Some(super::VmUnwindRegion::Catch { stack_depth, .. }) =
                frame.cold.regions.last().copied()
            else {
                return Err(Error::internal(
                    "catch cleanup has no innermost catch region",
                ));
            };
            if exit == RunExit::DropCatch {
                if depth != stack_depth {
                    return Err(Error::internal(
                        "DropCatch did not reach its catch entry depth",
                    ));
                }
            } else {
                if depth <= stack_depth {
                    return Err(Error::internal(
                        "NipCatch has no value above its catch marker",
                    ));
                }
                prepare_captured_reuse(frame, &execution.slots)?;
                let value = execution.slots.pop(&mut frame.window)?;
                while execution.slots.depth(&frame.window) > stack_depth {
                    drop(execution.slots.pop(&mut frame.window)?);
                }
                execution.slots.push(&mut frame.window, value)?;
            }
            frame.cold.regions.pop();
        }
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("catch resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        return Ok(Some(CallStep::Entered));
    }
    if exit == RunExit::Throw {
        let frame = execution.frames.current_mut(id)?;
        #[cfg(feature = "profiling")]
        let depth = execution.slots.depth(&frame.window);
        forwarded = Some(Completion::Throw(execution.slots.pop(&mut frame.window)?));
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        exit = RunExit::Complete;
    }
    if let RunExit::Binding {
        source,
        index,
        write,
        checked,
        keep,
    } = exit
    {
        let frame = execution.frames.current_mut(id)?;
        use super::run::BindingSource;
        use crate::engine::code::function::metadata::{
            ClosureSource, ClosureVariable, ClosureVariableName,
        };
        let (root, descriptor) = match source {
            BindingSource::Closure => (
                frame
                    .cold
                    .closure_slots
                    .get(usize::from(index))
                    .ok_or_else(|| Error::internal("closure variable index is out of bounds"))?
                    .clone(),
                frame.executable.closure_variables[usize::from(index)],
            ),
            BindingSource::Local | BindingSource::Argument => {
                let (binding, definition, source) = if source == BindingSource::Local {
                    (
                        execution.slots.local(&frame.window, index)?,
                        frame.executable.local_definitions[usize::from(index)],
                        ClosureSource::ParentLocal(index),
                    )
                } else {
                    (
                        execution.slots.parameter(&frame.window, index)?,
                        frame.executable.argument_definitions[usize::from(index)],
                        ClosureSource::ParentArgument(index),
                    )
                };
                let super::bindings::FrameBinding::Captured(root) = binding else {
                    return Err(Error::internal("captured access lost its cell"));
                };
                (
                    root.clone(),
                    ClosureVariable {
                        source,
                        name: definition
                            .name
                            .map_or(ClosureVariableName::None, ClosureVariableName::Atom),
                        is_lexical: definition.is_lexical,
                        is_const: definition.is_const,
                        kind: definition.kind,
                    },
                )
            }
        };
        let strip_debug = frame.executable.metadata.strip_variable_debug;
        let realm = frame.executable.realm;
        #[cfg(feature = "profiling")]
        let depth = execution.slots.depth(&frame.window);
        let value = if write {
            Some(if keep {
                super::stack::copy_value(execution.slots.peek(&frame.window, 0)?)?
            } else {
                execution.slots.pop(&mut frame.window)?
            })
        } else {
            None
        };
        // Own the cell and input before touching heap storage; no window or
        // Runtime state borrow survives the operation.
        let result = if let Some(value) = value {
            if checked {
                super::bindings::write_checked_closure(
                    runtime,
                    &root,
                    descriptor,
                    strip_debug,
                    value,
                )
            } else {
                runtime
                    .write_var_ref(&root, value)
                    .map_err(|error| Error::internal(error.to_string()))
            }
            .map(|()| None)
        } else if checked {
            super::bindings::read_checked_closure(runtime, &root, descriptor, strip_debug).map(Some)
        } else {
            runtime
                .read_var_ref(&root)
                .map(Some)
                .map_err(|error| Error::internal(error.to_string()))
        };
        match result {
            Ok(value) => {
                let frame = execution.frames.current_mut(id)?;
                if let Some(value) = value {
                    execution.slots.push(&mut frame.window, value)?;
                }
                frame.resume_pc = frame
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("closure access resume PC overflow"))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(depth);
                return Ok(Some(CallStep::Entered));
            }
            Err(error) => {
                let Some(kind) =
                    crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
                else {
                    return Err(error);
                };
                let value = runtime
                    .new_native_error_from_error(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?;
                forwarded = Some(Completion::Throw(value));
                exit = RunExit::Complete;
            }
        }
    }
    if let RunExit::LexicalUninitialized(index) = exit {
        let frame = execution.frames.current_mut(id)?;
        let definition = frame.executable.local_definitions[usize::from(index)];
        let error = super::bindings::lexical_uninitialized_error(
            runtime,
            definition.name,
            !frame.executable.metadata.strip_variable_debug,
        )?;
        let value = runtime
            .new_native_error_from_error(
                frame.executable.realm,
                crate::engine::api::error::NativeErrorKind::Reference,
                &error,
            )
            .map_err(runtime_error_to_vm_error)?;
        forwarded = Some(Completion::Throw(value));
        exit = RunExit::Complete;
    }
    if let RunExit::InitializeDerived(index) = exit {
        let frame = execution.frames.current_mut(id)?;
        let definition = frame
            .executable
            .frame_layout()
            .locals()
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| Error::internal("local definition index is out of bounds"))?;
        #[cfg(feature = "profiling")]
        let depth = execution.slots.depth(&frame.window);
        let value = execution.slots.pop(&mut frame.window)?;
        match super::bindings::initialize_derived_binding(
            runtime,
            definition,
            Some(execution.slots.local(&frame.window, index)?),
            value,
        ) {
            Ok(replacement) => {
                if let Some(binding) = replacement {
                    let old = execution
                        .slots
                        .replace_local(&frame.window, index, binding)?;
                    if !matches!(old, super::bindings::FrameBinding::Uninitialized) {
                        return Err(Error::internal(
                            "derived initialization replaced an initialized slot",
                        ));
                    }
                }
                frame.resume_pc = frame
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("initialization resume PC overflow"))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(depth);
                return Ok(Some(CallStep::Entered));
            }
            Err(error) => {
                let Some(kind) =
                    crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
                else {
                    return Err(error);
                };
                let value = runtime
                    .new_native_error_from_error(frame.executable.realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?;
                forwarded = Some(Completion::Throw(value));
                exit = RunExit::Complete;
            }
        }
    }
    if let RunExit::ReturnDerived(index) = exit {
        let frame = execution.frames.current_mut(id)?;
        let definition = frame
            .executable
            .frame_layout()
            .locals()
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| Error::internal("local definition index is out of bounds"))?;
        let value = execution.slots.pop(&mut frame.window)?;
        let completion = super::bindings::finish_derived_return(
            runtime,
            frame.cold.caller_realm,
            definition,
            Some(execution.slots.local(&frame.window, index)?),
            value,
        )?;
        #[cfg(feature = "profiling")]
        if matches!(completion, Completion::Return(_)) {
            crate::engine::api::profiling::record_owned_instruction(
                execution.slots.depth(&frame.window) + 1,
            );
        }
        forwarded = Some(completion);
        exit = RunExit::Complete;
    }
    if exit == RunExit::NormalizeThis {
        let frame = execution.frames.current_mut(id)?;
        // This conversion only allocates a primitive wrapper; it cannot
        // call JavaScript. Keep its identity across every later handoff.
        let value = runtime
            .native_to_object(frame.executable.realm, frame.cold.input.this_value.clone())
            .map_err(runtime_error_to_vm_error)?;
        let NativeConversion::Value(object) = value else {
            return Err(Error::internal("non-null primitive this boxing threw"));
        };
        frame.cold.normalized_this = Some(Value::Object(object));
        return Ok(Some(CallStep::Entered));
    }
    Ok(if let Some(completion) = forwarded {
        Some(CallStep::Complete(completion))
    } else if exit == RunExit::Bridge && original != RunExit::Bridge {
        Some(CallStep::Bridge)
    } else {
        None
    })
}
