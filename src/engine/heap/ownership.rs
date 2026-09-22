use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::atom::{Atom, AtomError, AtomIdx};
use crate::engine::heap::runtime::{DeferredRefOp, RuntimeOperation, RuntimeState};
use crate::engine::heap::{
    BigIntId, ContextId, FunctionBytecodeId, HeapError, ObjectId, RawId, RawValue, StringId,
    VarRefId,
};

#[cfg(debug_assertions)]
pub(crate) fn trace_object_matches(id: ObjectId) -> bool {
    static WANTED: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    WANTED
        .get_or_init(|| {
            std::env::var("QJS_TRACE_OBJECT_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
        })
        .is_some_and(|wanted| wanted == id.index)
}

#[cfg(debug_assertions)]
pub(crate) fn trace_string_matches(id: StringId) -> bool {
    static WANTED: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    WANTED
        .get_or_init(|| {
            std::env::var("QJS_TRACE_STRING_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
        })
        .is_some_and(|wanted| wanted == id.index)
}

#[cfg(debug_assertions)]
thread_local! {
    static OUTSTANDING_OBJECT_RETAINS: std::cell::RefCell<
        std::collections::HashMap<u32, Vec<String>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(debug_assertions)]
pub(crate) fn record_object_retain(id: ObjectId) {
    if !trace_object_matches(id) {
        return;
    }
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let compact = backtrace
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let at = line.strip_prefix("at ")?;
            at.split_once(": ")
                .map(|(path, _)| path.to_string())
                .or_else(|| Some(at.to_string()))
        })
        .take(10)
        .collect::<Vec<_>>()
        .join(" <- ");
    OUTSTANDING_OBJECT_RETAINS.with(|slot| {
        slot.borrow_mut().entry(id.index).or_default().push(compact);
    });
}

#[cfg(debug_assertions)]
pub(crate) fn record_object_release(id: ObjectId) {
    if !trace_object_matches(id) {
        return;
    }
    OUTSTANDING_OBJECT_RETAINS.with(|slot| {
        if let Some(entries) = slot.borrow_mut().get_mut(&id.index) {
            entries.pop();
        }
    });
}

#[cfg(debug_assertions)]
pub(crate) fn dump_outstanding_object_retains() {
    let wanted = std::env::var("QJS_TRACE_OBJECT_ID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    let Some(wanted) = wanted else {
        return;
    };
    OUTSTANDING_OBJECT_RETAINS.with(|slot| {
        let mut map = slot.borrow_mut();
        if let Some(entries) = map.remove(&wanted) {
            for (index, entry) in entries.iter().enumerate() {
                eprintln!("[outstanding-retain] {wanted} #{index}: {entry}");
            }
        }
    });
}

/// Whether the debug edge ledger captures creation backtraces.
///
/// Capture is enabled only for explicit leak diagnosis (`QJS_EDGE_LEDGER`,
/// `QJS_TEARDOWN_PROBE`, or `QJS_TRACE_ROOTS`): a backtrace costs far more
/// than the arena operation it annotates, and the acceptance budget is a
/// debug test-time regression under 2×.
#[cfg(debug_assertions)]
pub(crate) fn alloc_site_capture_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        ["QJS_EDGE_LEDGER", "QJS_TEARDOWN_PROBE", "QJS_TRACE_ROOTS"]
            .iter()
            .any(|name| std::env::var_os(name).is_some())
    })
}

/// Compact one captured backtrace into a single diagnostic line.
#[cfg(debug_assertions)]
pub(crate) fn compact_backtrace() -> String {
    std::backtrace::Backtrace::force_capture()
        .to_string()
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let at = line.strip_prefix("at ")?;
            at.split_once(": ")
                .map(|(path, _)| path.to_string())
                .or_else(|| Some(at.to_string()))
        })
        .take(12)
        .collect::<Vec<_>>()
        .join(" <- ")
}

impl Runtime {
    #[inline]
    pub(crate) fn operation(&self) -> RuntimeOperation<'_> {
        let result = self.drain_deferred_references();
        debug_assert!(result.is_ok(), "deferred root release failed: {result:?}");
        RuntimeOperation(self)
    }

    #[inline]
    pub(crate) fn drain_deferred_references(&self) -> Result<(), RuntimeError> {
        if !self.0.deferred_references.has_pending() {
            return Ok(());
        }
        self.drain_deferred_references_slow()
    }

    fn drain_deferred_references_slow(&self) -> Result<(), RuntimeError> {
        let deferred = &self.0.deferred_references;
        let Some(_drain) = deferred.try_start_draining() else {
            return Ok(());
        };
        loop {
            // Do not remove work until it can execute. In particular, blocked
            // drains must leave restoration operations at their original priority.
            let Ok(mut state) = self.0.state.try_borrow_mut() else {
                return Ok(());
            };
            let Some(operation) = deferred.pop_front() else {
                break;
            };
            state.apply_deferred_operation(operation)?;
        }
        Ok(())
    }

    #[inline]
    #[track_caller]
    fn release_or_defer(&self, operation: DeferredRefOp) {
        let result = if let Ok(mut state) = self.0.state.try_borrow_mut() {
            state.apply_deferred_operation(operation)
        } else {
            #[cfg(debug_assertions)]
            if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
                eprintln!(
                    "[defer] {operation:?} at {}",
                    std::panic::Location::caller()
                );
            }
            self.0.deferred_references.push_back(operation);
            // The state is still borrowed. The next existing operation boundary
            // (or a successful release) drains this work after the borrow ends.
            return;
        };
        self.finish_reference_release(operation, result);
    }

    #[inline]
    fn finish_reference_release(&self, operation: DeferredRefOp, result: Result<(), RuntimeError>) {
        // Successful releases are a VM hot path. Consult diagnostic settings
        // only after an error; probing the process environment on every edge
        // release adds a global environment-lock lookup to ordinary value flow.
        if let Err(error) = &result {
            if std::env::var_os("QJS_TEARDOWN_PROBE").is_some() {
                eprintln!("[release] invalid root release {operation:?}: {error:?}");
                if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
                    eprintln!(
                        "[release-invalid-backtrace]\n{}",
                        std::backtrace::Backtrace::force_capture()
                    );
                }
            } else {
                debug_assert!(false, "invalid root release {operation:?}: {error:?}");
            }
        }
        let drain = self.drain_deferred_references();
        if let Err(error) = &drain {
            if std::env::var_os("QJS_TEARDOWN_PROBE").is_some() {
                eprintln!("[release] deferred root release failed: {error:?}");
            } else {
                debug_assert!(false, "deferred root release failed: {error:?}");
            }
        }
    }

    #[track_caller]
    pub(crate) fn retain_object_handle(&self, id: ObjectId) -> Result<(), HeapError> {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!("[retain] {id:?} at {}", std::panic::Location::caller());
            if trace_object_matches(id) {
                let strong = self
                    .0
                    .state
                    .try_borrow()
                    .ok()
                    .and_then(|state| state.heap.object_strong_count(id).ok());
                eprintln!("[o-retain] {id:?} strong_before={strong:?}");
                eprintln!(
                    "[retain-o-backtrace]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            return state.heap.retain_object(id);
        }
        // A nested read may hold a shared state borrow (for example an
        // autoinit property materialization rooting its value).  The counter
        // lives in a `Cell`, so a validated shared-borrow retain is exact.
        let state = self.0.state.try_borrow().map_err(|_| {
            HeapError::Invariant("object root retained during a runtime state borrow")
        })?;
        state.heap.retain_object_shared(id)
    }

    /// Trusted duplicate of an object handle the caller proves live by holding
    /// another owned edge across the whole operation (for example a receiver
    /// argv edge, or a collection record edge owned by a live receiver).
    ///
    /// Skips the redundant slot identity revalidation on the hot path; debug
    /// builds still assert full identity through [`Heap::retain_object_fast`],
    /// and trace diagnostics fall back to the fully validated path. The count
    /// saturates at `u32::MAX` exactly like the established fast retain used
    /// by the inline caches.
    #[inline]
    #[track_caller]
    pub(crate) fn retain_live_object_handle(&self, id: ObjectId) -> Result<(), HeapError> {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            return self.retain_object_handle(id);
        }
        if let Ok(state) = self.0.state.try_borrow() {
            state.heap.retain_object_fast(id);
            return Ok(());
        }
        // The state is mutably borrowed: report the same boundary error the
        // fully validated retain would surface here.
        self.retain_object_handle(id)
    }

    #[track_caller]
    pub(crate) fn release_object_handle(&self, id: ObjectId) {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!("[release] {id:?} at {}", std::panic::Location::caller());
            if trace_object_matches(id) {
                let strong = self
                    .0
                    .state
                    .try_borrow()
                    .ok()
                    .and_then(|state| state.heap.object_strong_count(id).ok());
                eprintln!("[o-release] {id:?} strong_before={strong:?}");
                eprintln!(
                    "[release-o-backtrace]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        self.release_or_defer(DeferredRefOp::Object(id));
    }

    pub(crate) fn retain_atom_handle(&self, atom: Atom) -> Result<(), AtomError> {
        self.0.state.borrow().atoms.retain_shared(atom).map(drop)
    }

    pub(crate) fn retain_function_bytecode_handle(
        &self,
        id: FunctionBytecodeId,
    ) -> Result<(), HeapError> {
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            return state.heap.retain_function_bytecode(id);
        }
        let state = self.0.state.try_borrow().map_err(|_| {
            HeapError::Invariant("function bytecode retained during a runtime state borrow")
        })?;
        state.heap.retain_function_bytecode_shared(id)
    }

    pub(crate) fn retain_context_handle(&self, id: ContextId) -> Result<(), HeapError> {
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            return state.heap.retain_context(id);
        }
        let state =
            self.0.state.try_borrow().map_err(|_| {
                HeapError::Invariant("context retained during a runtime state borrow")
            })?;
        state.heap.retain_context_shared(id)
    }

    pub(crate) fn release_context_handle(&self, id: ContextId) {
        self.release_or_defer(DeferredRefOp::Context(id));
    }

    pub(crate) fn release_function_bytecode_handle(&self, id: FunctionBytecodeId) {
        self.release_or_defer(DeferredRefOp::FunctionBytecode(id));
    }

    pub(crate) fn retain_var_ref_handle(&self, id: VarRefId) -> Result<(), HeapError> {
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            return state.heap.retain_var_ref(id);
        }
        let state =
            self.0.state.try_borrow().map_err(|_| {
                HeapError::Invariant("VarRef retained during a runtime state borrow")
            })?;
        state.heap.retain_var_ref_shared(id)
    }

    pub(crate) fn release_var_ref_handle(&self, id: VarRefId) {
        self.release_or_defer(DeferredRefOp::VarRef(id));
    }

    pub(crate) fn release_atom_index(&self, index: AtomIdx) {
        // The producer owns this index until this release is applied, so the
        // slot cannot be reused while its operation is queued. Branding under
        // a mandatory borrow here would make suspension-owner Drop panic.
        let atom = self
            .0
            .state
            .try_borrow()
            .ok()
            .and_then(|state| state.atoms.brand(index).ok());
        if let Some(atom) = atom {
            // Preserve the Cell decrement fast path under shared borrows.
            self.release_atom_handle(atom);
        } else {
            self.release_or_defer(DeferredRefOp::AtomIndexRelease(index));
        }
    }

    pub(crate) fn release_atom_handle(&self, atom: Atom) {
        // Shared-borrow release: the counter decrement runs immediately; when
        // the last reference drops, slot removal is deferred to the next
        // operation boundary through the existing deferred queue.  An invalid
        // release is re-deferred so the error surfaces at the drain boundary
        // exactly like the historical full-release path.
        let hit_zero = match self.0.state.try_borrow() {
            Ok(state) => match state.atoms.release_shared(atom) {
                Ok(hit_zero) => hit_zero,
                Err(_) => {
                    drop(state);
                    self.0
                        .deferred_references
                        .push_back(DeferredRefOp::AtomRelease(atom));
                    return;
                }
            },
            Err(_) => {
                // The table is mutably borrowed (interning/removal in
                // progress). Defer the whole shared-release pass; it runs at
                // the next safe point.
                self.0
                    .deferred_references
                    .push_back(DeferredRefOp::AtomRelease(atom));
                return;
            }
        };
        if hit_zero {
            self.release_or_defer(DeferredRefOp::AtomRemove(atom));
        }
    }

    #[track_caller]
    pub(crate) fn retain_string_handle(&self, id: StringId) -> Result<(), HeapError> {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!("[retain-s] {id:?} at {}", std::panic::Location::caller());
            if std::env::var("QJS_TRACE_STRING_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                == Some(id.index)
            {
                eprintln!(
                    "[retain-s-backtrace]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        let state = self.0.state.try_borrow().map_err(|_| {
            HeapError::Invariant("string node retained during a runtime state borrow")
        })?;
        state.heap.retain_string_shared(id)
    }

    #[track_caller]
    pub(crate) fn release_string_handle(&self, id: StringId) {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!("[release-s] {id:?} at {}", std::panic::Location::caller());
            if std::env::var("QJS_TRACE_STRING_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                == Some(id.index)
            {
                eprintln!(
                    "[release-s-backtrace]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        self.release_leaf_or_defer(DeferredRefOp::String(id), RawId::String(id));
    }

    #[track_caller]
    pub(crate) fn retain_bigint_handle(&self, id: BigIntId) -> Result<(), HeapError> {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!("[retain-b] {id:?} at {}", std::panic::Location::caller());
            if std::env::var("QJS_TRACE_BIGINT_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                == Some(id.index)
            {
                eprintln!(
                    "[retain-b-backtrace]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        let state = self.0.state.try_borrow().map_err(|_| {
            HeapError::Invariant("bigint node retained during a runtime state borrow")
        })?;
        state.heap.retain_bigint_shared(id)
    }

    #[track_caller]
    pub(crate) fn release_bigint_handle(&self, id: BigIntId) {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!("[release-b] {id:?} at {}", std::panic::Location::caller());
            if std::env::var("QJS_TRACE_BIGINT_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                == Some(id.index)
            {
                eprintln!(
                    "[release-b-backtrace]\n{}",
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        self.release_leaf_or_defer(DeferredRefOp::BigInt(id), RawId::BigInt(id));
    }

    #[inline]
    fn release_leaf_or_defer(&self, operation: DeferredRefOp, id: RawId) {
        let result = if let Ok(mut state) = self.0.state.try_borrow_mut() {
            match state.heap.try_release_leaf_reference(id) {
                Ok(Some(_)) => Ok(()),
                Ok(None) => state.apply_deferred_operation(operation),
                Err(error) => Err(error.into()),
            }
        } else {
            // Keep diagnostics, enqueue order, and blocked-drain behavior in
            // the existing path. The failed borrow did not alter the node.
            self.release_or_defer(operation);
            return;
        };
        self.finish_reference_release(operation, result);
    }

    /// Release one producer-owned string/BigInt conversion edge after the
    /// transactional store has retained its own copy edge.
    #[track_caller]
    pub(crate) fn release_converted_node_edge(&self, edge: RawId) {
        #[cfg(debug_assertions)]
        if std::env::var_os("QJS_TRACE_ROOTS").is_some() {
            eprintln!(
                "[release-converted] {edge:?} at {}",
                std::panic::Location::caller()
            );
        }
        match edge {
            RawId::String(id) => self.release_or_defer(DeferredRefOp::String(id)),
            RawId::BigInt(id) => self.release_or_defer(DeferredRefOp::BigInt(id)),
            _ => unreachable!("conversion edges are only string or bigint node edges"),
        }
    }

    /// Release the conversion edge carried by a boundary-converted value, if
    /// any.  See [`RawValue::conversion_node_edge`].
    pub(crate) fn release_converted_value_edge(&self, value: &RawValue) {
        if let Some(edge) = value.conversion_node_edge() {
            self.release_converted_node_edge(edge);
        }
    }
}

/// Stack-local owner of one boundary conversion's producer node edge.
///
/// The guard borrows the runtime and never enters heap slots or the operand
/// stack.  Dropping it releases the producer edge, so every store-or-decline
/// path balances the conversion automatically once the transactional store
/// retained its own copy edge (or rejected the value).  A caller that hands
/// the value to a by-value owner instead adopts the edge explicitly with
/// [`ConvertedValue::disarm`].
pub(crate) struct ConvertedValue<'a> {
    runtime: &'a Runtime,
    value: Option<RawValue>,
}

impl<'a> ConvertedValue<'a> {
    pub(crate) fn new(runtime: &'a Runtime, value: RawValue) -> Self {
        Self {
            runtime,
            value: Some(value),
        }
    }

    /// Clone the handle without duplicating the producer edge.
    pub(crate) fn raw(&self) -> RawValue {
        self.value.clone().expect("ConvertedValue raw after take")
    }
}

impl Drop for ConvertedValue<'_> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.runtime.release_converted_value_edge(&value);
        }
    }
}
impl RuntimeState {
    #[inline]
    fn release_heap_reference(&mut self, id: RawId) -> Result<(), RuntimeError> {
        // Nonfinal shared decrements are the VM hot path; they cannot produce
        // cleanup or queue work. Possible finalizations, older queued nodes,
        // and invalid handles all decline into the full checked path.
        if self.heap.try_release_nonfinal(id) {
            return Ok(());
        }
        if let Some(cleanup) = self.heap.release_reference(id)? {
            self.apply_cleanup(cleanup)?;
        }
        Ok(())
    }

    /// Apply one raw release or restoration operation at an existing safe point.
    #[inline]
    pub(crate) fn apply_deferred_operation(
        &mut self,
        operation: DeferredRefOp,
    ) -> Result<(), RuntimeError> {
        match operation {
            DeferredRefOp::Object(object) => self.release_heap_reference(RawId::Object(object)),
            DeferredRefOp::Context(context) => self.release_heap_reference(RawId::Context(context)),
            DeferredRefOp::FunctionBytecode(bytecode) => {
                self.release_heap_reference(RawId::FunctionBytecode(bytecode))
            }
            DeferredRefOp::VarRef(var_ref) => self.release_heap_reference(RawId::VarRef(var_ref)),
            DeferredRefOp::String(id) => self.release_heap_reference(RawId::String(id)),
            DeferredRefOp::BigInt(id) => self.release_heap_reference(RawId::BigInt(id)),
            DeferredRefOp::AtomIndexRelease(index) => self
                .atoms
                .release_index(index)
                .map(drop)
                .map_err(Into::into),
            DeferredRefOp::AtomRelease(atom) => {
                if self.atoms.release_shared(atom)? {
                    self.atoms.remove_released(atom)?;
                }
                Ok(())
            }
            DeferredRefOp::AtomRemove(atom) => self.atoms.remove_released(atom).map_err(Into::into),
            DeferredRefOp::ActiveFramePop { token, depth } => {
                self.active_frames.retire(token, depth);
                Ok(())
            }
            DeferredRefOp::ActiveCollectionRecordsTruncate { depth } => {
                self.active_collection_records.truncate(depth);
                Ok(())
            }
            DeferredRefOp::BacktraceBarrierRestore { token, previous } => {
                if let Some(frame) = self
                    .active_frames
                    .iter_mut()
                    .find(|frame| frame.token == token)
                {
                    frame.flags.backtrace_barrier = previous;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod bigint_leaf_tests {
    use super::*;
    use crate::engine::{heap::Heap, value::bigint::JsBigInt};

    fn payload() -> JsBigInt {
        JsBigInt::parse_js_string("170141183460469231731687303715884105729").unwrap()
    }

    #[test]
    fn bigint_shared_retain_and_deferred_release_preserve_owners_and_order() {
        let runtime = Runtime::new();
        let first = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_bigint(payload())
            .unwrap();
        let second = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_bigint(payload())
            .unwrap();
        {
            let state = runtime.0.state.borrow();
            runtime.retain_bigint_handle(first).unwrap();
            runtime.release_bigint_handle(first);
            runtime.release_bigint_handle(second);
            let pending = runtime.0.deferred_references.borrow();
            assert!(matches!(pending.front(), Some(DeferredRefOp::BigInt(id)) if *id == first));
            assert!(matches!(pending.back(), Some(DeferredRefOp::BigInt(id)) if *id == second));
            assert!(state.heap.bigint(first).is_ok());
            assert!(state.heap.bigint(second).is_ok());
        }
        // Immediate release must also drain older deferred work.
        runtime.release_bigint_handle(first);
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(runtime.0.state.borrow().heap.bigint(first).is_err());
        assert!(runtime.0.state.borrow().heap.bigint(second).is_err());
        assert!(runtime.retain_bigint_handle(first).is_err());

        let id = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_bigint(payload())
            .unwrap();
        {
            let _state = runtime.0.state.borrow_mut();
            assert!(runtime.retain_bigint_handle(id).is_err());
            runtime.release_bigint_handle(id);
        }
        runtime.drain_deferred_references().unwrap();
        assert!(runtime.0.state.borrow().heap.bigint(id).is_err());
    }

    #[test]
    fn string_shared_retain_and_deferred_release_preserve_owners() {
        let runtime = Runtime::new();
        let id = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_string(crate::engine::value::JsString::from_static("shared leaf"))
            .unwrap();
        {
            let state = runtime.0.state.borrow();
            runtime.retain_string_handle(id).unwrap();
            runtime.release_string_handle(id);
            assert!(state.heap.string(id).is_ok());
        }
        runtime.release_string_handle(id);
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(runtime.0.state.borrow().heap.string(id).is_err());
    }

    #[test]
    fn bigint_leaf_decline_preserves_zero_queue_and_public_cleanup_counts() {
        let mut heap = Heap::new();
        let first = heap.allocate_bigint(payload()).unwrap();
        let second = heap.allocate_bigint(payload()).unwrap();
        heap.release_raw_no_drain(RawId::BigInt(first)).unwrap();
        assert_eq!(
            heap.try_release_leaf_reference(RawId::BigInt(second))
                .unwrap(),
            None
        );
        assert!(heap.bigint(second).is_ok());
        let cleanup = heap.release_bigint(second).unwrap();
        assert_eq!(cleanup.finalized_bigints, 2);
        assert!(
            heap.try_release_leaf_reference(RawId::BigInt(second))
                .is_err()
        );
        let third = heap.allocate_bigint(payload()).unwrap();
        assert_eq!(heap.release_bigint(third).unwrap().finalized_bigints, 1);
    }
}
