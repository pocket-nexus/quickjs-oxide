//! One executable owner and one exclusive storage window per running frame.

mod storage;
#[cfg(any(test, feature = "profiling"))]
pub(in crate::engine::vm) use storage::FrameBody;
pub(in crate::engine::vm) use storage::Resident;
pub(in crate::engine::vm) use storage::{CallStorage, ColdFrame};

use crate::engine::api::error::Error;
use crate::engine::api::{Runtime, runtime_error::RuntimeError};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::ContextId;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::JsValue;
use crate::engine::vm::CallInput;
use crate::engine::vm::frames::{ActiveFrameRestore, ActiveFrameToken};
use crate::engine::vm::stack::FrameStorage;

#[cfg(test)]
thread_local! {
    static NEXT_PC_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Observe only the supplied completion interval, not interpreter decoding.
#[cfg(test)]
pub(super) fn count_next_pc_calls<T>(run: impl FnOnce() -> T) -> (T, usize) {
    let previous = NEXT_PC_CALLS.replace(0);
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            NEXT_PC_CALLS.set(self.0);
        }
    }
    let _restore = Restore(previous);
    let result = run();
    (result, NEXT_PC_CALLS.get())
}

#[derive(Clone, Copy)]
pub(super) enum ReturnValue {
    Push,
    Discard,
}

#[derive(Clone, Copy)]
pub(super) struct ReturnTarget {
    pub value_use: ReturnValue,
    pub owner: ReturnOwner,
    pub tail: bool,
    pub operation: Option<OperationTarget>,
}

/// A continuation may belong to a bytecode frame or a root native/job request.
#[derive(Clone, Copy)]
pub(super) enum ReturnOwner {
    Frame(FrameId),
    Root,
}
impl ReturnOwner {
    pub(super) fn frame(self) -> Result<FrameId, Error> {
        match self {
            Self::Frame(id) => Ok(id),
            Self::Root => Err(Error::internal(
                "root request used a bytecode-only operation",
            )),
        }
    }
}
impl ReturnTarget {
    pub(super) fn frame(self) -> Result<FrameId, Error> {
        self.owner.frame()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationTarget {
    Conversion(u64),
    PropertyGet(u64),
    Iterator(u64),
    Eval(u16),
}

pub(super) enum ConstructorReturn {
    Base(JsValue),
    Derived,
}

#[derive(Default)]
pub(super) struct FrameRare {
    pub property_keys: std::collections::HashMap<u32, crate::engine::object::PropertyKey>,
    pub normalized_this: Option<JsValue>,
    property_wait: Option<Box<super::proxy_get_driver::PendingProxyGet>>,
    pub iterator_wait: Option<crate::engine::vm::iterator_driver::PendingIterator>,
    pub resume_throw: Option<JsValue>,
    pub regions: Vec<crate::engine::vm::VmUnwindRegion>,
    pub eval_arguments: Option<Vec<crate::engine::value::JsValue>>,
    pub constructor_return: Option<ConstructorReturn>,
    pub conversion: Option<crate::engine::vm::conversion_driver::ConversionWait>,
    pub computed_read: Option<super::property_driver::ComputedReadEffect>,
}

pub(super) struct FrameCold {
    pub rare: std::cell::OnceCell<Box<FrameRare>>,
    pub return_to: Option<ReturnTarget>,
    pub entry_guard: Option<ActiveFrameRestore>,
    pub function: storage::Resident<crate::engine::vm::closure::FrameFunction>,
    pub reusable_captured_locals: Vec<bool>,
    pub input: storage::Resident<CallInput>,
}

/// Owners crossing the driver boundary before installation or after detachment.
#[must_use]
pub(super) struct FrameEntry {
    pub property_generation: u64,
    pub iterator_generation: u64,
    pub caller_realm: ContextId,
    pub active_frame: ActiveFrameToken,

    pub initialize_bindings: bool,
    pub executable: PublishedFunctionSnapshot,
    pub cold: ColdFrame,
    pub storage: FrameStorage,
}

impl FrameEntry {
    pub(super) fn release(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        runtime.check_poison()?;
        self.cold.release_legacy(runtime);
        runtime.check_poison()?;
        {
            let mut state = runtime.0.state.borrow_mut();
            super::stack::release_frame_storage_in_state(&mut state, self.storage)?;
            self.cold.release_owned(&mut state)?;
        }
        Ok(())
    }
}

/// Uninstalled entry stays guarded before the first reservation. This guard
/// borrows Runtime outside state-held execution; successful publication takes
/// the entry into RunningExecution's reachable frame storage.
pub(super) struct FrameEntryGuard<'a> {
    runtime: &'a Runtime,
    entry: Option<FrameEntry>,
}
impl<'a> FrameEntryGuard<'a> {
    pub(super) fn new(runtime: &'a Runtime, entry: FrameEntry) -> Self {
        Self {
            runtime,
            entry: Some(entry),
        }
    }
    pub(super) fn take(&mut self) -> FrameEntry {
        self.entry.take().expect("frame entry transferred once")
    }
}
impl std::ops::Deref for FrameEntryGuard<'_> {
    type Target = FrameEntry;
    fn deref(&self) -> &Self::Target {
        self.entry.as_ref().expect("guarded frame entry")
    }
}
impl std::ops::DerefMut for FrameEntryGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.entry.as_mut().expect("guarded frame entry")
    }
}
impl Drop for FrameEntryGuard<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if let Some(entry) = self.entry.take() {
            if entry.release(self.runtime).is_err() {
                self.runtime.0.poisoned.set(true);
            }
        }
    }
}

/// A popped frame remains responsible for its window and cold owners until
/// both have been explicitly recycled. This borrowed legacy boundary is never
/// constructed while the execution holds a state borrow.
pub(super) struct RetiredFrame<'a> {
    runtime: &'a Runtime,
    slots: &'a mut super::stack::SlotStore,
    frame: Option<Frame>,
}
impl<'a> RetiredFrame<'a> {
    pub(super) fn new(
        runtime: &'a Runtime,
        slots: &'a mut super::stack::SlotStore,
        frame: Frame,
    ) -> Self {
        Self {
            runtime,
            slots,
            frame: Some(frame),
        }
    }
    #[cfg(test)]
    fn take(&mut self) -> Frame {
        self.frame.take().expect("retired frame transferred once")
    }
    pub(super) fn clear_window(&mut self) -> Result<(), Error> {
        let frame = self.frame.as_mut().expect("retired frame owner");
        if let Some(window) = frame.window.take_optional() {
            if let Err(error) = self.slots.clear_frame(self.runtime, window) {
                // Failed window/edge validation means state is not fit for a
                // second teardown attempt. All ordinary allocation failures
                // happen before the frame enters this retirement scope.
                self.runtime.0.poisoned.set(true);
                return Err(error);
            }
        }
        Ok(())
    }
    pub(super) fn recycle(mut self, storage: &mut CallStorage) -> Result<(), RuntimeError> {
        let frame = self.frame.take().expect("retired frame transferred once");
        let result = storage.recycle_legacy(self.runtime, frame.cold);
        if result.is_err() {
            self.runtime.0.poisoned.set(true);
        }
        result
    }
}
impl std::ops::Deref for RetiredFrame<'_> {
    type Target = Frame;
    fn deref(&self) -> &Self::Target {
        self.frame.as_ref().expect("retired frame owner")
    }
}
impl std::ops::DerefMut for RetiredFrame<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.frame.as_mut().expect("retired frame owner")
    }
}
impl Drop for RetiredFrame<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        if self.frame.is_none() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if self.clear_window().is_err() {
            return;
        }
        if let Some(mut frame) = self.frame.take() {
            frame.cold.release_legacy(self.runtime);
            if self.runtime.is_poisoned() {
                return;
            }
            let result = frame
                .cold
                .release_owned(&mut self.runtime.0.state.borrow_mut());
            if result.is_err() {
                self.runtime.0.poisoned.set(true);
            }
        }
    }
}

/// Only the dispatch header moves on frame-stack push/pop. The executable and
/// affine window live in the already pooled cold allocation, whose address is
/// stable across vector growth and reuse. No extra per-call allocation exists.
pub(super) struct Frame {
    pub property_generation: u64,
    pub iterator_generation: u64,
    pub caller_realm: ContextId,
    pub active_frame: ActiveFrameToken,

    pub fault_pc: usize,
    pub resume_pc: usize,
    pub cold: ColdFrame,
}
impl Frame {
    /// The next instruction boundary in the sole published word stream.
    pub(super) fn next_pc(&self) -> Result<usize, Error> {
        #[cfg(test)]
        NEXT_PC_CALLS.set(NEXT_PC_CALLS.get() + 1);
        self.executable
            .exec
            .decode(self.fault_pc as u32)
            .map(|decoded| decoded.next_pc as usize)
            .map_err(|_| Error::internal("frame fault PC is not a verified instruction boundary"))
    }

    /// Resume metadata is always stored at an instruction boundary, including
    /// when a callback or generator enters in the middle of a frame.
    pub(super) fn set_resume_pc(&mut self, pc: usize) -> Result<(), Error> {
        let fault = self.executable.exec.previous_pc(pc).ok_or_else(|| {
            Error::internal("frame resume PC is not a verified instruction boundary")
        })?;
        self.resume_pc = pc;
        self.fault_pc = fault;
        Ok(())
    }
}
impl std::ops::Deref for Frame {
    type Target = storage::FrameBody;
    fn deref(&self) -> &storage::FrameBody {
        &self.cold
    }
}
impl std::ops::DerefMut for Frame {
    fn deref_mut(&mut self) -> &mut storage::FrameBody {
        &mut self.cold
    }
}
const _: () = assert!(size_of::<Frame>() <= 64);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<Frame>() == 56);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameId {
    execution: u64,
    generation: u64,
}

pub(super) struct FrameStore {
    frames: Vec<(FrameId, Frame)>,
    execution: u64,
    next_generation: u64,
    limit: usize,
    // Exact sum: at most usize::MAX frames, each charged at most usize::MAX.
    // Wider accounting preserves overflow recovery without rescanning ancestors.
    installed_wait_depth: u128,
    materialized_watermark: usize,
    unmaterialized_depth: usize,
}

impl FrameStore {
    pub(super) fn new(execution: u64, limit: usize) -> Self {
        Self {
            frames: Vec::new(),
            execution,
            next_generation: 1,
            limit,
            installed_wait_depth: 0,
            materialized_watermark: 0,
            unmaterialized_depth: 0,
        }
    }

    pub(super) fn can_reply_property_directly(&self, target: ReturnTarget) -> bool {
        let Some((id, parent)) = self.frames.get(self.frames.len().saturating_sub(2)) else {
            return false;
        };
        target.frame().ok() == Some(*id)
            && parent
                .cold
                .rare
                .get()
                .and_then(|rare| rare.property_wait.as_ref())
                .is_some_and(|pending| pending.is_direct_property_read(target.operation))
    }

    pub(super) fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Register only the newly observable suffix; ancestors have been frozen
    /// at their call PC since the previous suffix was materialized.
    pub(super) fn materialize(
        &mut self,
        runtime: &crate::engine::api::runtime::Runtime,
    ) -> Result<(), Error> {
        if self.frames.is_empty() {
            self.materialized_watermark = 0;
            return Ok(());
        }
        self.materialize_in_state(&mut runtime.0.state.borrow_mut())
    }

    pub(super) fn materialize_in_state(&mut self, state: &mut RuntimeState) -> Result<(), Error> {
        use super::exception::runtime_error_to_vm_error;
        let depth = self.frames.len();
        let start = self.materialized_watermark.saturating_sub(1);
        for (offset, (_, frame)) in self.frames[start..].iter_mut().enumerate() {
            if frame.active_frame.is_materialized() {
                state
                    .publish_materialized_pc(
                        frame.active_frame,
                        frame
                            .cold
                            .entry_guard
                            .as_ref()
                            .map(|guard| guard.registry_depth()),
                        super::BytecodePc::new(frame.fault_pc),
                    )
                    .map_err(runtime_error_to_vm_error)?;
            } else {
                let restore = state
                    .materialize_owned_frame(frame)
                    .map_err(runtime_error_to_vm_error)?;
                frame.active_frame = restore.token();
                self.unmaterialized_depth -= 1;
                frame.cold.entry_guard = Some(restore);
            }
            self.materialized_watermark = start + offset + 1;
        }
        self.materialized_watermark = depth;
        Ok(())
    }

    pub(super) fn logical_active_depth(
        &self,
        runtime: &crate::engine::api::runtime::Runtime,
    ) -> usize {
        runtime
            .0
            .active_frame_depth
            .get()
            .saturating_add(self.unmaterialized_depth)
    }

    pub(super) fn can_push(&self) -> bool {
        self.frames.len() < self.limit
    }

    /// Domain continuations replace recursive calls and share the existing
    /// execution depth ceiling, including calls that install no bytecode frame.
    pub(super) fn can_push_with_continuations(&self, pending: usize) -> bool {
        self.frames
            .len()
            .checked_add(pending)
            .and_then(|depth| {
                usize::try_from(self.installed_wait_depth)
                    .ok()
                    .and_then(|wait| depth.checked_add(wait))
            })
            .is_some_and(|depth| depth < self.limit)
    }

    fn remove_installed_wait_depth(&mut self, removed: usize) {
        self.installed_wait_depth -= removed as u128;
    }

    pub(super) fn take_pending(
        &mut self,
        id: FrameId,
    ) -> Result<Box<super::proxy_get_driver::PendingProxyGet>, Error> {
        let pending = self
            .current_mut(id)?
            .cold
            .property_wait
            .take()
            .ok_or_else(|| Error::internal("request reply has no pending operation"))?;
        self.remove_installed_wait_depth(pending.continuation_depth());
        Ok(pending)
    }

    pub(super) fn put_pending(
        &mut self,
        runtime: &Runtime,
        id: FrameId,
        pending: Box<super::proxy_get_driver::PendingProxyGet>,
    ) -> Result<(), Error> {
        let frame = match self.current_mut(id) {
            Ok(frame) => frame,
            Err(error) => {
                pending.release(runtime);
                return Err(error);
            }
        };
        if frame.cold.property_wait.is_some() {
            pending.release(runtime);
            return Err(Error::internal("request overwrote a pending reply"));
        }
        let depth = pending.continuation_depth();
        frame.cold.property_wait = Some(pending);
        self.installed_wait_depth += depth as u128;
        Ok(())
    }

    pub(super) fn current_id(&self) -> Option<FrameId> {
        self.frames.last().map(|(id, _)| *id)
    }

    /// Reserve identity and capacity while retaining exclusive access to this
    /// store. Slot publication may then fail, but installing a frame cannot.
    pub(super) fn prepare_push(&mut self) -> Result<FramePush<'_>, Error> {
        if self.frames.len() >= self.limit {
            return Err(Error::internal("execution frame limit exceeded"));
        }
        let next = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| Error::internal("execution frame identity exhausted"))?;
        #[cfg(feature = "profiling")]
        let before = self.frames.capacity();
        self.frames
            .try_reserve(1)
            .map_err(|_| Error::internal("execution frame allocation failed"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_storage(
            crate::engine::api::profiling::OwnedStorageEvent::FrameCapacity {
                before,
                after: self.frames.capacity(),
            },
        );
        Ok(FramePush { store: self, next })
    }

    pub(super) fn pop_current(&mut self) -> Option<Frame> {
        let (_, frame) = self.frames.pop()?;
        self.materialized_watermark = self.materialized_watermark.min(self.frames.len());
        self.unmaterialized_depth -= usize::from(!frame.active_frame.is_materialized());
        self.remove_installed_wait_depth(frame.cold.pending_depth());
        Some(frame)
    }

    /// Lend the actual stack top; no caller-supplied identity can select a
    /// different frame. The exclusive borrow blocks installation/retirement.
    pub(super) fn current_frame_mut(&mut self) -> Option<(FrameId, &mut Frame)> {
        self.frames.last_mut().map(|(id, frame)| (*id, frame))
    }

    pub(super) fn current_mut(&mut self, id: FrameId) -> Result<&mut Frame, Error> {
        match self.frames.last_mut() {
            Some((current, frame)) if *current == id => Ok(frame),
            _ => Err(Error::internal(
                "frame identity is not the current execution frame",
            )),
        }
    }

    pub(super) fn pop(&mut self, id: FrameId) -> Result<Frame, Error> {
        self.current_mut(id)?;
        Ok(self.pop_current().unwrap())
    }
}

/// The borrow prevents another push/pop from invalidating reserved capacity.
pub(super) struct FramePush<'a> {
    store: &'a mut FrameStore,
    next: u64,
}
impl FramePush<'_> {
    /// Reserved publication still lends the actual caller stack top.
    pub(super) fn current_frame_mut(&mut self) -> Option<(FrameId, &mut Frame)> {
        self.store.current_frame_mut()
    }
    pub(super) fn current_mut(&mut self, id: FrameId) -> Result<&mut Frame, Error> {
        self.store.current_mut(id)
    }
    pub(super) fn install(self, frame: Frame) -> FrameId {
        let id = FrameId {
            execution: self.store.execution,
            generation: self.store.next_generation,
        };
        self.store.next_generation = self.next;
        let depth = frame.cold.pending_depth();
        self.store.unmaterialized_depth += usize::from(!frame.active_frame.is_materialized());
        self.store.frames.push((id, frame));
        self.store.installed_wait_depth += depth as u128;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_storage(
            crate::engine::api::profiling::OwnedStorageEvent::FramePush(self.store.frames.len()),
        );
        id
    }
}
impl Drop for FrameStore {
    fn drop(&mut self) {
        // Child query/activation owners must disappear before their parents.
        while let Some(frame) = self.pop_current() {
            drop(frame);
        }
    }
}

impl FrameCold {
    pub(super) fn release_resume_throw(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = self
            .rare
            .get_mut()
            .and_then(|rare| rare.resume_throw.take())
        {
            state.release_jsvalue(value)?;
        }
        Ok(())
    }

    pub(super) fn release_constructor_return(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        if let Some(ConstructorReturn::Base(value)) = self
            .rare
            .get_mut()
            .and_then(|rare| rare.constructor_return.take())
        {
            state.release_jsvalue(value)?;
        }
        Ok(())
    }

    pub(super) fn release_eval_arguments(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        if let Some(values) = self
            .rare
            .get_mut()
            .and_then(|rare| rare.eval_arguments.take())
        {
            for value in values {
                state.release_jsvalue(value)?;
            }
        }
        Ok(())
    }

    pub(super) fn release_normalized_this(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = self
            .rare
            .get_mut()
            .and_then(|rare| rare.normalized_this.take())
        {
            state.release_jsvalue(value)?;
        }
        Ok(())
    }

    /// Temporary public-root continuations still use their existing cleanup
    /// protocol. Call this outside the state borrow until their B migration.
    pub(super) fn release_legacy(&mut self, runtime: &Runtime) {
        if let Some(rare) = self.rare.get_mut() {
            rare.property_keys.clear();
            if let Some(read) = rare.computed_read.take() {
                read.release_owned(runtime);
            }
            if let Some(pending) = rare.property_wait.take() {
                pending.release(runtime);
            }
            rare.iterator_wait = None;
            if let Some(wait) = rare.conversion.take() {
                wait.release_owned(runtime);
            }
        }
    }

    pub(super) fn release_owned(&mut self, state: &mut RuntimeState) -> Result<(), RuntimeError> {
        self.release_normalized_this(state)?;
        self.release_eval_arguments(state)?;
        self.release_resume_throw(state)?;
        self.release_constructor_return(state)?;
        if let Some(guard) = self.entry_guard.take() {
            guard.finish(state)?;
        }
        if let Some(mut function) = self.function.take_optional() {
            function.release(state)?;
        }
        if let Some(mut input) = self.input.take_optional() {
            input.release(state)?;
        }
        Ok(())
    }

    pub(super) fn has_pending_query(&self) -> bool {
        self.rare
            .get()
            .is_some_and(|rare| rare.property_wait.is_some())
    }
    fn pending_depth(&self) -> usize {
        self.rare
            .get()
            .and_then(|rare| rare.property_wait.as_ref())
            .map_or(0, |wait| wait.continuation_depth())
    }
    /// Legacy ordinary returns have no constructor result to normalize.
    pub(super) fn ordinary_return(&self) -> Option<ReturnTarget> {
        let target = self.simple_return_target()?;
        if target.tail
            || self
                .rare
                .get()
                .is_some_and(|rare| rare.constructor_return.is_some())
        {
            return None;
        }
        Some(target)
    }

    /// A Base receiver is an explicit owned edge and can be selected under
    /// current state access. Derived constructors keep their existing semantic
    /// validation and unwinding path until that consumer is migrated.
    pub(super) fn state_return(&self) -> Option<ReturnTarget> {
        let target = self.simple_return_target()?;
        if self
            .rare
            .get()
            .is_some_and(|rare| matches!(rare.constructor_return, Some(ConstructorReturn::Derived)))
        {
            return None;
        }
        Some(target)
    }

    /// Report only the metadata gate that already declined this state return.
    /// This diagnostic reads no heap data and disappears from timing builds.
    #[cfg(feature = "profiling")]
    pub(super) fn record_state_return_decline(&self) {
        let reason = match self.return_to {
            None => "core.return_decline.root",
            Some(target)
                if target.operation.is_some()
                    && !matches!(target.operation, Some(OperationTarget::PropertyGet(_))) =>
            {
                "core.return_decline.operation"
            }
            Some(target) if !matches!(target.owner, ReturnOwner::Frame(_)) => {
                "core.return_decline.root"
            }
            Some(_) => {
                let rare = self.rare.get().expect("declined return owns a rare phase");
                if rare.property_wait.is_some()
                    || rare.iterator_wait.is_some()
                    || rare.conversion.is_some()
                    || rare.computed_read.is_some()
                    || !rare.regions.is_empty()
                    || rare.resume_throw.is_some()
                {
                    "core.return_decline.live_wait"
                } else {
                    debug_assert!(matches!(
                        rare.constructor_return,
                        Some(ConstructorReturn::Derived)
                    ));
                    "core.return_decline.derived"
                }
            }
        };
        crate::engine::api::profiling::record_owned_execution_event(reason);
    }

    /// Keep the incoming result and saved Base receiver registered throughout
    /// selection. Ordinary frames never initialize their rare storage here.
    pub(super) fn normalize_base_return_in_state(
        &mut self,
        state: &mut RuntimeState,
        pending: &mut Option<JsValue>,
    ) -> Result<(), RuntimeError> {
        if !self
            .rare
            .get()
            .is_some_and(|rare| matches!(rare.constructor_return, Some(ConstructorReturn::Base(_))))
        {
            return Ok(());
        }
        if matches!(pending, Some(JsValue::Object(_))) {
            return self.release_constructor_return(state);
        }
        let value = pending.take().expect("registered constructor result");
        state.release_jsvalue(value)?;
        let Some(ConstructorReturn::Base(receiver)) = self
            .rare
            .get_mut()
            .expect("checked Base constructor storage")
            .constructor_return
            .take()
        else {
            unreachable!("checked Base constructor receiver")
        };
        *pending = Some(receiver);
        Ok(())
    }

    fn simple_return_target(&self) -> Option<ReturnTarget> {
        let target = self.return_to?;
        if (target.operation.is_some()
            && !matches!(target.operation, Some(OperationTarget::PropertyGet(_))))
            || !matches!(target.owner, ReturnOwner::Frame(_))
        {
            return None;
        }
        if self.rare.get().is_some_and(|rare| {
            rare.property_wait.is_some()
                || rare.iterator_wait.is_some()
                || rare.conversion.is_some()
                || !rare.regions.is_empty()
                || rare.resume_throw.is_some()
        }) {
            return None;
        }
        Some(target)
    }
}
impl std::ops::Deref for FrameCold {
    type Target = FrameRare;
    fn deref(&self) -> &FrameRare {
        self.rare.get_or_init(Default::default)
    }
}
impl std::ops::DerefMut for FrameCold {
    fn deref_mut(&mut self) -> &mut FrameRare {
        if self.rare.get().is_none() {
            self.rare.set(Default::default()).ok();
        }
        self.rare.get_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;
    use crate::engine::value::JsValue;
    use crate::engine::vm::stack::{FrameStorage, SlotStore};

    fn push(
        runtime: &Runtime,
        slots: &mut SlotStore,
        frames: &mut FrameStore,
        frame: Frame,
    ) -> Result<FrameId, Error> {
        let mut owner = RetiredFrame::new(runtime, slots, frame);
        Ok(frames.prepare_push()?.install(owner.take()))
    }

    fn release(runtime: &Runtime, slots: &mut SlotStore, frame: Frame) {
        let mut owner = RetiredFrame::new(runtime, slots, frame);
        owner.clear_window().unwrap();
        owner.recycle(&mut CallStorage::default()).unwrap();
    }

    fn assert_wait_depth_matches_scan(frames: &FrameStore) {
        let sum = frames.frames.iter().try_fold(0usize, |sum, (_, frame)| {
            sum.checked_add(frame.cold.pending_depth())
        });
        assert_eq!(usize::try_from(frames.installed_wait_depth).ok(), sum);
        for pending in [0, 1, 2, 3, 7, usize::MAX - 1, usize::MAX] {
            let original = frames.frames.len().checked_add(pending).and_then(|depth| {
                frames.frames.iter().try_fold(depth, |depth, (_, frame)| {
                    depth.checked_add(frame.cold.pending_depth())
                })
            });
            assert_eq!(
                frames.can_push_with_continuations(pending),
                original.is_some_and(|depth| depth < frames.limit),
            );
        }
    }

    #[test]
    fn installed_wait_cache_matches_scan_across_install_take_and_both_pops() {
        use super::super::proxy_get_driver::PendingProxyGet;
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut frames = FrameStore::new(1, 9);
        assert_wait_depth_matches_scan(&frames);
        let (first, mut first_slots) = frame(&runtime, context.realm);
        let first_id = push(&runtime, &mut first_slots, &mut frames, first).unwrap();
        frames
            .put_pending(
                &runtime,
                first_id,
                PendingProxyGet::with_parent_depth_for_test(context.realm, 3),
            )
            .unwrap();
        assert_wait_depth_matches_scan(&frames);
        assert!(frames.can_push_with_continuations(4));
        assert!(!frames.can_push_with_continuations(5));
        let (mut second, mut second_slots) = frame(&runtime, context.realm);
        second.cold.property_wait = Some(PendingProxyGet::with_parent_depth_for_test(
            context.realm,
            4,
        ));
        let second_id = frames.prepare_push().unwrap().install(second);
        assert_wait_depth_matches_scan(&frames);
        assert!(!frames.can_push_with_continuations(0));
        let pending = frames.take_pending(second_id).unwrap();
        assert_eq!(pending.continuation_depth(), 4);
        assert_wait_depth_matches_scan(&frames);
        frames.put_pending(&runtime, second_id, pending).unwrap();
        assert_wait_depth_matches_scan(&frames);
        release(&runtime, &mut second_slots, frames.pop(second_id).unwrap());
        assert_wait_depth_matches_scan(&frames);
        release(&runtime, &mut first_slots, frames.pop_current().unwrap());
        assert_wait_depth_matches_scan(&frames);
        assert!(frames.pop_current().is_none());
        assert_wait_depth_matches_scan(&frames);
    }

    #[test]
    fn pending_errors_preserve_cache_and_put_does_not_add_a_budget_rejection() {
        use super::super::proxy_get_driver::PendingProxyGet;
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut frames = FrameStore::new(1, 1);
        let (first, mut slots) = frame(&runtime, context.realm);
        let id = push(&runtime, &mut slots, &mut frames, first).unwrap();
        let invalid = FrameId {
            execution: 2,
            generation: id.generation,
        };
        assert!(frames.take_pending(id).is_err());
        assert!(frames.take_pending(invalid).is_err());
        assert!(
            frames
                .put_pending(
                    &runtime,
                    invalid,
                    PendingProxyGet::with_parent_depth_for_test(context.realm, 2)
                )
                .is_err()
        );
        assert!(frames.pop(invalid).is_err());
        assert_wait_depth_matches_scan(&frames);
        frames
            .put_pending(
                &runtime,
                id,
                PendingProxyGet::with_parent_depth_for_test(context.realm, 3),
            )
            .unwrap();
        assert!(!frames.can_push_with_continuations(0));
        assert!(
            frames
                .put_pending(
                    &runtime,
                    id,
                    PendingProxyGet::with_parent_depth_for_test(context.realm, 5)
                )
                .is_err()
        );
        assert_wait_depth_matches_scan(&frames);
        assert_eq!(frames.take_pending(id).unwrap().continuation_depth(), 3);
        assert_wait_depth_matches_scan(&frames);
        release(&runtime, &mut slots, frames.pop_current().unwrap());
        assert!(frames.take_pending(id).is_err());
        assert_wait_depth_matches_scan(&frames);
    }

    #[test]
    fn numeric_installed_wait_overflow_recovers_without_scanning() {
        let mut frames = FrameStore::new(1, usize::MAX);
        frames.installed_wait_depth = usize::MAX as u128 + 1;
        assert!(!frames.can_push_with_continuations(0));
        frames.remove_installed_wait_depth(1);
        assert_eq!(frames.installed_wait_depth, usize::MAX as u128);
        frames.remove_installed_wait_depth(usize::MAX);
        assert!(frames.can_push_with_continuations(0));
    }

    #[test]
    fn cached_cold_storage_reuses_empty_capacity_without_retaining_runtime() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut cache = CallStorage::default();
        cache.reserve().unwrap();
        let (mut first, mut first_slots) = frame(&runtime, context.realm);
        first.cold.reusable_captured_locals = vec![true; 23];
        let address = &*first.cold as *const FrameBody;
        first_slots
            .clear_frame(&runtime, first.window.take())
            .unwrap();
        cache.recycle_legacy(&runtime, first.cold).unwrap();
        let (flags, grown) = cache.capture_flags(23).unwrap();
        assert_eq!(grown, 0);
        assert_eq!(flags, vec![false; 23]);
        let (mut second, mut second_slots) = frame(&runtime, context.realm);
        second_slots
            .clear_frame(&runtime, second.window.take())
            .unwrap();
        let mut contents = second.cold.into_inner();
        contents.reusable_captured_locals = flags;
        let (cold, allocated) = cache.install(contents);
        assert_eq!(allocated, 0);
        assert_eq!(&*cold as *const FrameBody, address);
        cache.recycle_legacy(&runtime, cold).unwrap();
        drop((first_slots, second_slots, context, runtime));
        assert!(weak.upgrade().is_none());
        drop(cache);
    }

    fn frame(runtime: &Runtime, realm: ContextId) -> (Frame, SlotStore) {
        let executable = PublishedFunctionSnapshot::empty_for_test(realm);
        let mut slots = SlotStore::new(0);
        let window = slots
            .push_frame(
                runtime,
                &executable.frame_layout(),
                FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: Vec::new(),
                    locals: Vec::new(),
                    operands: Vec::new(),
                },
            )
            .unwrap();
        let function = runtime.new_object(None).unwrap();
        let mut cold = super::ColdFrame::new(FrameCold {
            rare: std::cell::OnceCell::new(),
            return_to: None,
            entry_guard: None,
            input: (CallInput::new(
                runtime,
                JsValue::Undefined,
                JsValue::Undefined,
                Some(function.try_clone().expect("duplicate root")),
            ))
            .into(),
            function: crate::engine::vm::closure::FrameFunction::new(function, Default::default())
                .unwrap()
                .into(),
            reusable_captured_locals: Vec::new(),
        });
        cold.executable = executable.into();
        cold.window = window.into();
        (
            Frame {
                property_generation: 0,
                iterator_generation: 0,
                caller_realm: realm,
                active_frame: ActiveFrameToken(0),

                fault_pc: 0,
                resume_pc: 0,
                cold,
            },
            slots,
        )
    }

    #[test]
    fn limits_and_stale_ids_preserve_the_active_frame_and_release_rejected_owners() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut frames = FrameStore::new(1, 1);
        let (first, mut first_slots) = frame(&runtime, context.realm);
        let first_id = push(&runtime, &mut first_slots, &mut frames, first).unwrap();
        let capacity = frames.frames.capacity();
        let (rejected, mut rejected_slots) = frame(&runtime, context.realm);
        let rejected_object = rejected.cold.function.object_id();
        assert!(push(&runtime, &mut rejected_slots, &mut frames, rejected).is_err());
        assert!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object(rejected_object)
                .is_err()
        );
        assert!(frames.current_mut(first_id).is_ok());
        assert_eq!(frames.frames.capacity(), capacity);
        release(&runtime, &mut first_slots, frames.pop(first_id).unwrap());
        let (replacement, mut replacement_slots) = frame(&runtime, context.realm);
        let replacement_id =
            push(&runtime, &mut replacement_slots, &mut frames, replacement).unwrap();
        assert_ne!(first_id, replacement_id);
        assert!(frames.current_mut(first_id).is_err());
        assert!(
            frames
                .current_mut(FrameId {
                    execution: 2,
                    generation: replacement_id.generation
                })
                .is_err()
        );
        assert!(frames.current_mut(replacement_id).is_ok());
        assert_eq!(frames.frames.capacity(), capacity);
        release(
            &runtime,
            &mut replacement_slots,
            frames.pop(replacement_id).unwrap(),
        );
    }

    #[test]
    fn exhausted_frame_identity_rejects_before_installing_ownership() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut frames = FrameStore::new(1, 1);
        frames.next_generation = u64::MAX;
        let (rejected, mut rejected_slots) = frame(&runtime, context.realm);
        let object = rejected.cold.function.object_id();
        assert!(push(&runtime, &mut rejected_slots, &mut frames, rejected).is_err());
        assert!(frames.frames.is_empty());
        assert_eq!(frames.frames.capacity(), 0);
        assert!(runtime.0.state.borrow().heap.object(object).is_err());
    }
    fn entry(runtime: &Runtime, realm: ContextId) -> FrameEntry {
        let (mut frame, _slots) = frame(runtime, realm);
        FrameEntry {
            initialize_bindings: false,
            property_generation: frame.property_generation,
            iterator_generation: frame.iterator_generation,
            caller_realm: frame.caller_realm,
            active_frame: frame.active_frame,
            executable: frame.executable.take(),
            cold: frame.cold,
            storage: FrameStorage {
                original_arguments: Vec::new(),
                parameters: Vec::new(),
                locals: Vec::new(),
                operands: Vec::new(),
            },
        }
    }

    #[test]
    fn rejected_retirement_preserves_forwarded_completion_until_execution_cleanup() {
        use crate::engine::vm::{
            Completion,
            driver::push_frame,
            execute::VmAction,
            execution::{ExecutionLimits, RunningExecution},
        };
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 2,
                slots: 16,
            },
        )
        .unwrap();
        let active = push_frame(&runtime, &mut execution, entry(&runtime, context.realm)).unwrap();
        let rejected = FrameId {
            execution: active.execution,
            generation: active.generation + 1,
        };
        let value = runtime.new_object(None).unwrap().into_handle();
        assert!(
            crate::engine::vm::frame_exit::finish(
                &runtime,
                &mut execution,
                rejected,
                VmAction::Complete,
                Some(Completion::Throw(JsValue::Object(value)))
            )
            .is_err()
        );
        assert!(
            matches!(&execution.pending_completion, Some(Completion::Throw(JsValue::Object(id))) if *id==value)
        );
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(value),
            Ok(1)
        );
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(value).is_err());
    }

    #[test]
    fn rejected_child_push_preserves_parent_window_and_releases_child_owners() {
        use crate::engine::vm::{
            driver::push_frame,
            execution::{ExecutionLimits, RunningExecution},
        };
        for exhausted_identity in [false, true] {
            let runtime = Runtime::new();
            let context = runtime.new_context().expect("create context");
            let mut execution = RunningExecution::new(
                &runtime,
                ExecutionLimits {
                    frames: 1,
                    slots: 16,
                },
            )
            .unwrap();
            let parent =
                push_frame(&runtime, &mut execution, entry(&runtime, context.realm)).unwrap();
            let generation = execution.frames.next_generation;
            if exhausted_identity {
                execution.frames.limit = 2;
                execution.frames.next_generation = u64::MAX;
            }
            let mut child = entry(&runtime, context.realm);
            let child_object = child.cold.function.object_id();
            let argument = runtime.new_object(None).unwrap().into_handle();
            child
                .storage
                .original_arguments
                .push(JsValue::Object(argument));
            let this_value = runtime.new_object(None).unwrap().into_handle();
            let new_target = runtime.new_object(None).unwrap().into_handle();
            child.cold.input.this_value = JsValue::Object(this_value);
            child.cold.input.new_target = JsValue::Object(new_target);
            child
                .storage
                .parameters
                .push(super::super::bindings::FrameBinding::Direct(JsValue::Int(
                    42,
                )));
            let error = push_frame(&runtime, &mut execution, child).unwrap_err();
            assert!(error.to_string().contains(if exhausted_identity {
                "identity exhausted"
            } else {
                "frame limit"
            }));
            assert_eq!(execution.frames.current_id(), Some(parent));
            let frame = execution.frames.current_mut(parent).unwrap();
            assert_eq!(
                execution.slots.binding_counts(&frame.window).unwrap(),
                (0, 0)
            );
            for id in [child_object, argument, this_value, new_target] {
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
            }
            execution.frames.limit = 2;
            execution.frames.next_generation = generation;
            let replacement =
                push_frame(&runtime, &mut execution, entry(&runtime, context.realm)).unwrap();
            let frame = execution.frames.pop(replacement).unwrap();
            release(&runtime, &mut execution.slots, frame);
            let parent = execution.frames.current_mut(parent).unwrap();
            assert_eq!(
                execution.slots.binding_counts(&parent.window).unwrap(),
                (0, 0)
            );
        }
    }

    struct DropLog(
        &'static str,
        std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    );
    impl Drop for DropLog {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    fn tracked_runtime(
        name: &'static str,
        events: &std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    ) -> Runtime {
        let runtime = Runtime::new();
        let log = DropLog(name, events.clone());
        runtime
            .set_host_promise_rejection_tracker(move |_| {
                let _ = &log;
            })
            .expect("configure test runtime");
        runtime
    }
    #[test]
    fn populated_execution_does_not_own_runtime() {
        use crate::engine::vm::{
            driver::push_frame,
            execution::{ExecutionLimits, RunningExecution},
        };
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let runtime = tracked_runtime("runtime", &events);
        let context = runtime.new_context().unwrap();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let before = std::rc::Rc::strong_count(&runtime.0);
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 2,
                slots: 16,
            },
        )
        .unwrap();
        push_frame(&runtime, &mut execution, entry(&runtime, context.realm)).unwrap();
        push_frame(&runtime, &mut execution, entry(&runtime, context.realm)).unwrap();
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), before);
        assert_eq!(runtime.0.raw_execution_owners.get(), 1);
        drop(context);
        drop(runtime);
        assert_eq!(*events.borrow(), ["runtime"]);
        assert!(weak.upgrade().is_none());
        // The populated raw frame record cannot traverse the destroyed heap.
        drop(execution);
    }

    #[test]
    fn execution_abandon_releases_child_and_parent_owners() {
        use crate::engine::vm::{
            driver::push_frame,
            execution::{ExecutionLimits, RunningExecution},
        };
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 2,
                slots: 16,
            },
        )
        .unwrap();
        let parent = entry(&runtime, context.realm);
        let parent_function = parent.cold.function.object_id();
        push_frame(&runtime, &mut execution, parent).unwrap();
        let mut child = entry(&runtime, context.realm);
        let child_function = child.cold.function.object_id();
        let marker = runtime.new_object(None).unwrap().into_handle();
        child
            .storage
            .original_arguments
            .push(JsValue::Object(marker));
        child
            .storage
            .parameters
            .push(super::super::bindings::FrameBinding::Direct(
                JsValue::Undefined,
            ));
        push_frame(&runtime, &mut execution, child).unwrap();
        drop(execution);
        assert_eq!(runtime.0.raw_execution_owners.get(), 0);
        let state = runtime.0.state.borrow();
        for id in [parent_function, child_function, marker] {
            assert!(state.heap.object(id).is_err());
        }
    }
}
