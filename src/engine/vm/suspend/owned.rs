//! Move a suspended owned frame across the heap-publication boundary.
use super::{VmActivationResume, VmRunOutcome};
use crate::engine::api::{Error, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::value::JsValue;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{FrameEntry, FrameId};
use crate::engine::vm::{VmResume, VmSuspendKind};

pub(in crate::engine::vm) struct OwnedSuspension {
    runtime: std::rc::Weak<crate::engine::heap::runtime::RuntimeInner>,
    entry: Option<FrameEntry>,
    pub(in crate::engine::vm) return_to: Option<crate::engine::vm::frame::ReturnTarget>,
    pc: usize,
    kind: VmSuspendKind,
}

impl Drop for OwnedSuspension {
    /// Release the storage of a suspended frame abandoned before freeze or
    /// resume. `freeze` takes the entry first, so a completed suspension drops
    /// an empty slot. Abandonment acquires state outside a running segment;
    /// this weak record never keeps its runtime alive.
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.upgrade().map(Runtime) {
            runtime.unregister_raw_execution_owner();
            if runtime.skip_cleanup() {
                return;
            }
            let _unwind = runtime.unwind_guard();
            if let Some(entry) = self.entry.take() {
                if entry.release(&runtime).is_err() {
                    runtime.0.poisoned.set(true);
                }
            }
        }
    }
}

impl OwnedSuspension {
    pub(in crate::engine::vm) fn detach(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        id: FrameId,
        kind: VmSuspendKind,
    ) -> Result<Self, Error> {
        #[cfg(feature = "profiling")]
        let _profile_phase = crate::engine::api::profiling::PhaseTimer::start_vm("freeze.detach");
        execution.frames.materialize(runtime)?;
        let frame = execution.frames.current_mut(id)?;
        if frame.cold.has_pending_query()
            || frame.cold.iterator_wait.is_some()
            || frame.cold.conversion.is_some()
            || frame.cold.eval_arguments.is_some()
            || frame.cold.constructor_return.is_some()
        {
            return Err(Error::internal(
                "suspension has an unresolved frame continuation",
            ));
        }
        let storage = execution
            .slots
            .take_frame_resident(runtime, &mut frame.window)?;
        let mut storage = crate::engine::vm::stack::FrameStorageGuard::new(runtime, storage);
        let mut frame = execution.frames.pop(id)?;
        let return_to = frame.cold.return_to.take();
        let pc = frame.resume_pc;
        let executable = frame.executable.take();
        let mut entry = crate::engine::vm::frame::FrameEntryGuard::new(
            runtime,
            FrameEntry {
                initialize_bindings: false,
                property_generation: frame.property_generation,
                iterator_generation: frame.iterator_generation,
                caller_realm: frame.caller_realm,
                active_frame: frame.active_frame,
                executable,
                cold: frame.cold,
                storage: storage.take(),
            },
        );
        if let Some(guard) = entry.cold.entry_guard.take() {
            guard
                .finish(&mut runtime.0.state.borrow_mut())
                .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        }
        Ok(Self {
            runtime: runtime
                .register_raw_execution_owner()
                .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?,
            return_to,
            entry: Some(entry.take()),
            pc,
            kind,
        })
    }

    pub(in crate::engine::vm) fn freeze(
        self: Box<Self>,
        runtime: Runtime,
    ) -> Result<VmRunOutcome, RuntimeError> {
        #[cfg(feature = "profiling")]
        let _profile_phase =
            crate::engine::api::profiling::PhaseTimer::start_vm("freeze.owned_export");
        let mut this = *self;
        let pc = this.pc;
        let kind = this.kind;
        let entry = this.entry.as_mut().ok_or(RuntimeError::Invariant(
            "owned suspension lost its frame entry",
        ))?;
        let value = if kind == VmSuspendKind::Initial {
            JsValue::Undefined
        } else {
            std::mem::replace(
                entry
                    .storage
                    .operands
                    .last_mut()
                    .ok_or(RuntimeError::Invariant("suspension has no output operand"))?,
                JsValue::Undefined,
            )
        };
        let entry = this.entry.take().expect("validated suspension entry");
        let activation = match super::freeze_entry(&runtime, entry, kind, pc) {
            Ok(activation) => activation,
            Err(error) => {
                let _ = runtime.release_jsvalue(value);
                return Err(error);
            }
        };
        Ok(VmRunOutcome::Suspend {
            value,
            activation: Box::new(activation),
        })
    }
}

pub(super) fn prepare(
    runtime: &Runtime,
    entry: &mut FrameEntry,
    kind: VmSuspendKind,
    pending: &mut Option<VmActivationResume>,
) -> Result<(), RuntimeError> {
    let resume = pending.as_ref().expect("owned resume input");
    let needs_magic = match (kind, resume) {
        (VmSuspendKind::Initial, VmActivationResume::Initial)
        | (
            VmSuspendKind::Await,
            VmActivationResume::AwaitFulfill(_) | VmActivationResume::AwaitReject(_),
        )
        | (VmSuspendKind::Yield, VmActivationResume::Generator(VmResume::Throw(_))) => false,
        (
            VmSuspendKind::Yield | VmSuspendKind::YieldStar | VmSuspendKind::AsyncYieldStar,
            VmActivationResume::Generator(_),
        ) => true,
        _ => {
            return Err(RuntimeError::Invariant(
                "resume operation disagrees with the suspended VM state",
            ));
        }
    };
    if kind != VmSuspendKind::Initial
        && !matches!(entry.storage.operands.last(), Some(JsValue::Undefined))
    {
        return Err(RuntimeError::Invariant(
            "suspension resume operand was not cleared",
        ));
    }
    if needs_magic {
        entry
            .storage
            .operands
            .try_reserve(1)
            .map_err(|_| RuntimeError::Invariant("resume operand allocation failed"))?;
    }
    // The original frame and pending input still own every edge above. Once
    // moved below, publication into the reserved slots is infallible.
    let resume = pending.take().expect("validated resume input");
    let mut abrupt = None;
    let injection = match (kind, resume) {
        (VmSuspendKind::Initial, VmActivationResume::Initial) => None,
        (VmSuspendKind::Await, VmActivationResume::AwaitFulfill(value)) => Some((value, None)),
        (VmSuspendKind::Await, VmActivationResume::AwaitReject(value))
        | (VmSuspendKind::Yield, VmActivationResume::Generator(VmResume::Throw(value))) => {
            abrupt = Some(value);
            None
        }
        (_, VmActivationResume::Generator(resume)) => {
            let (value, magic) = match resume {
                VmResume::Next(value) => (value, 0),
                VmResume::Return(value) => (value, 1),
                VmResume::Throw(value) => (value, 2),
            };
            Some((value, Some(magic)))
        }
        _ => unreachable!("resume input validated before transfer"),
    };
    if let Some((value, magic)) = injection {
        *entry
            .storage
            .operands
            .last_mut()
            .expect("validated resume operand") = value;
        if let Some(magic) = magic {
            entry.storage.operands.push(JsValue::Int(magic));
        }
    }
    entry
        .cold
        .release_resume_throw(&mut runtime.0.state.borrow_mut())?;
    entry.cold.resume_throw = abrupt;
    Ok(())
}

pub(in crate::engine::vm) struct PreparedResume {
    pub entry: FrameEntry,
    pub pc: usize,
}
