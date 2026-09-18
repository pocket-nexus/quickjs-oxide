use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::atom::{Atom, AtomError};
use crate::engine::heap::runtime::{DeferredRefOp, RuntimeOperation, RuntimeState};
use crate::engine::heap::{
    BigIntId, ContextId, FunctionBytecodeId, HeapError, ObjectId, RawId, StringId, VarRefId,
};

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
    fn release_or_defer(&self, operation: DeferredRefOp) {
        let result = if let Ok(mut state) = self.0.state.try_borrow_mut() {
            state.apply_deferred_operation(operation)
        } else {
            self.0.deferred_references.push_back(operation);
            // The state is still borrowed. The next existing operation boundary
            // (or a successful release) drains this work after the borrow ends.
            return;
        };
        debug_assert!(
            result.is_ok(),
            "invalid root release {operation:?}: {result:?}"
        );
        let drain = self.drain_deferred_references();
        debug_assert!(drain.is_ok(), "deferred root release failed: {drain:?}");
    }

    pub(crate) fn retain_object_handle(&self, id: ObjectId) -> Result<(), HeapError> {
        let mut state = self.0.state.try_borrow_mut().map_err(|_| {
            HeapError::Invariant("object root retained during a runtime state borrow")
        })?;
        state.heap.retain_object(id)
    }

    pub(crate) fn release_object_handle(&self, id: ObjectId) {
        self.release_or_defer(DeferredRefOp::Object(id));
    }

    pub(crate) fn retain_atom_handle(&self, atom: Atom) -> Result<(), AtomError> {
        // The atom counter is a `Cell`, so a proven-live atom can be retained
        // under a shared state borrow (S1b).
        self.0.state.borrow().atoms.retain(atom).map(drop)
    }

    pub(crate) fn retain_function_bytecode_handle(
        &self,
        id: FunctionBytecodeId,
    ) -> Result<(), HeapError> {
        let mut state = self.0.state.try_borrow_mut().map_err(|_| {
            HeapError::Invariant("function bytecode retained during a runtime state borrow")
        })?;
        state.heap.retain_function_bytecode(id)
    }

    pub(crate) fn retain_context_handle(&self, id: ContextId) -> Result<(), HeapError> {
        let mut state =
            self.0.state.try_borrow_mut().map_err(|_| {
                HeapError::Invariant("context retained during a runtime state borrow")
            })?;
        state.heap.retain_context(id)
    }

    pub(crate) fn release_context_handle(&self, id: ContextId) {
        self.release_or_defer(DeferredRefOp::Context(id));
    }

    pub(crate) fn release_function_bytecode_handle(&self, id: FunctionBytecodeId) {
        self.release_or_defer(DeferredRefOp::FunctionBytecode(id));
    }

    pub(crate) fn retain_var_ref_handle(&self, id: VarRefId) -> Result<(), HeapError> {
        let mut state =
            self.0.state.try_borrow_mut().map_err(|_| {
                HeapError::Invariant("VarRef retained during a runtime state borrow")
            })?;
        state.heap.retain_var_ref(id)
    }

    pub(crate) fn release_var_ref_handle(&self, id: VarRefId) {
        self.release_or_defer(DeferredRefOp::VarRef(id));
    }

    pub(crate) fn release_atom_handle(&self, atom: Atom) {
        self.release_or_defer(DeferredRefOp::Atom(atom));
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "S3-A1.2 scaffolding; wired in A1.2b")
    )]
    pub(crate) fn retain_string_handle(&self, id: StringId) -> Result<(), HeapError> {
        let mut state = self.0.state.try_borrow_mut().map_err(|_| {
            HeapError::Invariant("string root retained during a runtime state borrow")
        })?;
        state.heap.retain_string(id)
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "S3-A1.2 scaffolding; wired in A1.2b")
    )]
    pub(crate) fn retain_bigint_handle(&self, id: BigIntId) -> Result<(), HeapError> {
        let mut state = self.0.state.try_borrow_mut().map_err(|_| {
            HeapError::Invariant("bigint root retained during a runtime state borrow")
        })?;
        state.heap.retain_bigint(id)
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "S3-A1.2 scaffolding; wired in A1.2b")
    )]
    pub(crate) fn release_string_handle(&self, id: StringId) {
        self.release_or_defer(DeferredRefOp::String(id));
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "S3-A1.2 scaffolding; wired in A1.2b")
    )]
    pub(crate) fn release_bigint_handle(&self, id: BigIntId) {
        self.release_or_defer(DeferredRefOp::BigInt(id));
    }
}
impl RuntimeState {
    #[inline]
    fn release_heap_reference(&mut self, id: RawId) -> Result<(), RuntimeError> {
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
            DeferredRefOp::String(string) => self.release_heap_reference(RawId::String(string)),
            DeferredRefOp::BigInt(bigint) => self.release_heap_reference(RawId::BigInt(bigint)),
            DeferredRefOp::Atom(atom) => self.atoms.release(atom).map(drop).map_err(Into::into),
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
