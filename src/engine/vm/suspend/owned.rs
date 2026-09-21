//! Move a suspended owned frame across the heap-publication boundary.
use super::{VmActivationResume, VmRunOutcome};
use crate::engine::api::{Error, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::value::JsValue;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{FrameEntry, FrameId};
use crate::engine::vm::{VmResume, VmSuspendKind};

pub(in crate::engine::vm) struct OwnedSuspension {
    entry: Option<FrameEntry>,
    pub(in crate::engine::vm) return_to: Option<crate::engine::vm::frame::ReturnTarget>,
    pc: usize,
    kind: VmSuspendKind,
}

impl Drop for OwnedSuspension {
    /// Release the storage of a suspended frame abandoned before freeze or
    /// resume. `freeze` takes the entry first, so a completed suspension drops
    /// an empty slot. Releases are defer-safe and never run JavaScript.
    fn drop(&mut self) {
        let Some(entry) = self.entry.take() else {
            return;
        };
        let runtime = entry.cold.function.runtime().clone();
        crate::engine::vm::stack::release_frame_storage(&runtime, entry.storage);
    }
}

impl OwnedSuspension {
    pub(in crate::engine::vm) fn detach(
        execution: &mut RunningExecution,
        id: FrameId,
        kind: VmSuspendKind,
    ) -> Result<Self, Error> {
        #[cfg(feature = "profiling")]
        let _profile_phase = crate::engine::api::profiling::PhaseTimer::start_vm("freeze.detach");
        let runtime = execution
            .frames
            .current_mut(id)?
            .cold
            .function
            .runtime()
            .clone();
        execution.frames.materialize(&runtime)?;
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
        let mut frame = execution.frames.pop(id)?;
        let storage = execution.slots.take_frame(&runtime, frame.window.take())?;
        if let Some(guard) = frame.cold.entry_guard.take()
            && let Err(error) = guard.finish()
        {
            crate::engine::vm::stack::release_frame_storage(&runtime, storage);
            return Err(crate::engine::vm::exception::runtime_error_to_vm_error(
                error,
            ));
        }
        let return_to = frame.cold.return_to.take();
        Ok(Self {
            return_to,
            entry: Some(FrameEntry {
                initialize_bindings: false,
                property_generation: frame.property_generation,
                iterator_generation: frame.iterator_generation,
                caller_realm: frame.caller_realm,
                active_frame: frame.active_frame,
                executable: frame.executable.take(),
                cold: frame.cold,
                storage,
            }),
            pc: frame.resume_pc,
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
    entry.cold.release_resume_throw();
    entry.cold.resume_throw = abrupt;
    Ok(())
}

pub(in crate::engine::vm) struct PreparedResume {
    pub entry: FrameEntry,
    pub pc: usize,
}
