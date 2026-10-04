//! Reuse only empty query containers. Live callback owners stay in their Query.
use super::{Finish, NativeScope, Parents, Query, Resume};
use crate::engine::heap::ContextId;

/// Observe only an actual successful Vec reserve; unchanged capacity is not
/// an allocation. This wrapper adds no work when profiling is disabled.
#[inline]
pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    _name: &'static str,
) -> Result<(), std::collections::TryReserveError> {
    if values.capacity().saturating_sub(values.len()) >= additional {
        return Ok(());
    }
    #[cfg(feature = "profiling")]
    let before = values.capacity();
    values.try_reserve(additional)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_call_buffer_capacity(
        _name,
        before,
        values.capacity(),
        size_of::<T>(),
    );
    Ok(())
}

#[derive(Default)]
pub(super) struct Buffers {
    parents: Parents,
    natives: Vec<NativeScope>,
    spare_parents: Vec<Parents>,
}

#[derive(Default)]
pub(in crate::engine::vm) struct QueryStorage {
    free: Vec<Buffers>,
    native_waits: Vec<Vec<super::native::NativeWaitRecord>>,
    // Reuse the pending allocations themselves; all cached query fields are empty.
    #[allow(clippy::vec_box)]
    pending: Vec<Box<super::PendingProxyGet>>,
}

impl QueryStorage {
    pub(in crate::engine::vm) fn pending(
        &mut self,
        identity: u64,
        query: Query,
        resume: Resume,
    ) -> Box<super::PendingProxyGet> {
        if let Some(mut pending) = self.pending.pop() {
            pending.identity = identity;
            pending.query = query;
            // release_pending caches only the inert replacement record.
            debug_assert!(matches!(pending.resume, Resume::Identity));
            pending.resume = resume;
            return pending;
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_call_buffer_capacity(
            "query.pending_box",
            0,
            1,
            size_of::<super::PendingProxyGet>(),
        );
        Box::new(super::PendingProxyGet {
            identity,
            query,
            resume,
        })
    }

    pub(in crate::engine::vm) fn release_pending(
        &mut self,
        mut pending: Box<super::PendingProxyGet>,
    ) -> (u64, Query, Resume) {
        let empty = Query {
            native_runtime: std::rc::Weak::new(),
            #[cfg(feature = "profiling")]
            had_callback: false,
            realm: pending.query.realm,
            parents: Parents::default(),
            natives: Vec::new(),
            saved_native_depth: 0,
            spare_parents: Vec::new(),
            finish: None,
        };
        let query = std::mem::replace(&mut pending.query, empty);
        let resume = std::mem::replace(&mut pending.resume, Resume::Identity);
        let identity = pending.identity;
        if self.pending.len() < 16 && reserve(&mut self.pending, 1, "query.pending_pool").is_ok() {
            self.pending.push(pending);
        }
        (identity, query, resume)
    }

    pub(super) fn take_native_wait<'a>(
        &mut self,
        runtime: &'a crate::engine::api::runtime::Runtime,
    ) -> Result<super::native::NativeWaitGuard<'a>, crate::engine::api::Error> {
        let mut waiting = self.native_waits.pop().unwrap_or_default();
        if waiting.is_empty() {
            reserve(&mut waiting, 1, "query.native_wait_payload").map_err(|_| {
                crate::engine::api::Error::internal("native waiting payload allocation failed")
            })?;
            waiting.push(super::native::NativeWaitRecord {
                call: None,
                step: super::Step::Complete(Some(crate::engine::vm::Completion::Return(
                    crate::engine::value::JsValue::Undefined,
                ))),
                parents: Vec::new(),
            });
        }
        debug_assert_eq!(waiting.len(), 1);
        debug_assert!(waiting[0].call.is_none() && waiting[0].parents.is_empty());
        Ok(super::native::NativeWaitGuard::new(runtime, waiting))
    }

    pub(super) fn recycle_native_wait(&mut self, waiting: super::native::NativeWaitGuard<'_>) {
        debug_assert_eq!(waiting.len(), 1);
        debug_assert!(waiting[0].call.is_none() && waiting[0].parents.is_empty());
        let waiting = waiting.into_empty_vec();
        if reserve(&mut self.native_waits, 1, "query.native_wait_pool").is_ok() {
            self.native_waits.push(waiting);
        }
    }

    pub(super) fn has_cached_entry(&self) -> bool {
        !self.free.is_empty()
    }

    /// Reserve the same empty native containers without constructing a Query.
    /// A cold cache declines so its original allocation path remains unchanged.
    pub(super) fn reserve_cached_native_entry(
        &mut self,
    ) -> Result<bool, crate::engine::api::Error> {
        let Some(buffers) = self.free.last_mut() else {
            return Ok(false);
        };
        debug_assert!(buffers.parents.is_empty() && buffers.natives.is_empty());
        reserve(&mut buffers.natives, 1, "query.native_scopes").map_err(|_| {
            crate::engine::api::Error::internal("native continuation allocation failed")
        })?;
        reserve(&mut buffers.spare_parents, 1, "query.spare_parents").map_err(|_| {
            crate::engine::api::Error::internal("native parent storage allocation failed")
        })?;
        Ok(true)
    }

    pub(super) fn acquire(
        &mut self,
        realm: ContextId,
        parents: Vec<Resume>,
        finish: Finish,
    ) -> Query {
        let reused = !self.free.is_empty();
        let mut buffers = self.free.pop().unwrap_or_default();
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(if reused {
            "query_storage_reused"
        } else {
            "query_storage_new"
        });
        #[cfg(not(feature = "profiling"))]
        let _ = reused;
        if !parents.is_empty() {
            // The caller already owns and populated this container. Do not
            // introduce another fallible copy just to use a cached allocation.
            if buffers.parents.0.capacity() >= parents.len() {
                buffers.parents.0.extend(parents);
            } else {
                buffers.parents = Parents(parents);
            }
        }
        Query {
            native_runtime: std::rc::Weak::new(),
            #[cfg(feature = "profiling")]
            had_callback: false,
            realm,
            parents: buffers.parents,
            natives: buffers.natives,
            saved_native_depth: 0,
            spare_parents: buffers.spare_parents,
            finish: Some(finish),
        }
    }
}

impl Query {
    pub(super) fn recycle(mut self, storage: &mut QueryStorage) {
        self.release_native_members(true);
        self.recycle_empty(storage);
    }
    pub(in crate::engine::vm) fn recycle_in_state(
        mut self,
        runtime: &crate::engine::api::runtime::Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        storage: &mut QueryStorage,
    ) -> Result<(), crate::engine::api::RuntimeError> {
        let retired = self.retire_raw_in_state(runtime, state);
        // Keep registration through all direct retirement or quarantine, then
        // disarm the boundary fallback before this Query can drop under State.
        let capability = std::mem::take(&mut self.native_runtime);
        if capability.strong_count() != 0 {
            runtime.unregister_raw_execution_owner();
        }
        retired?;
        self.parents.0.clear();
        self.natives.clear();
        self.saved_native_depth = 0;
        self.recycle_empty(storage);
        Ok(())
    }
    fn recycle_empty(mut self, storage: &mut QueryStorage) {
        debug_assert!(self.spare_parents.iter().all(Parents::is_empty));
        let buffers = Buffers {
            parents: std::mem::take(&mut self.parents),
            natives: std::mem::take(&mut self.natives),
            spare_parents: std::mem::take(&mut self.spare_parents),
        };
        // The final continuation may still own roots on an error. Release it
        // before making the empty buffers available to another query.
        drop(self);
        if reserve(&mut storage.free, 1, "query.free_pool").is_ok() {
            storage.free.push(buffers);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;

    fn prepared_scope(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
    ) -> (
        crate::engine::vm::call::PreparedNativeCall,
        crate::engine::heap::ObjectId,
    ) {
        use crate::engine::{value::JsValue, vm::call::*};
        let callable = runtime
            .callable_from_value(context.eval("Number.isFinite").unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("native predicate")
        };
        let marker = runtime.new_object(None).unwrap().into_handle();
        let alias = runtime.dup_jsvalue(&JsValue::Object(marker)).unwrap();
        let call = runtime
            .prepare_native_invocation_jsvalue(
                &callable,
                realm,
                target,
                min_readable_args,
                NativeInvocation::Call {
                    this_value: JsValue::Object(marker),
                },
                vec![alias],
                NativeInvokeMode::Ordinary,
            )
            .unwrap()
            .into_inner();
        (call, marker)
    }

    #[test]
    fn resident_outer_scopes_reuse_empty_parents_after_return_and_throw() {
        use crate::engine::{
            value::JsValue,
            vm::{
                Completion,
                proxy_get_driver::{RawNativeQuery, Step},
                stack::SlotStore,
            },
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let mut storage = QueryStorage::default();
        let mut slots = SlotStore::new(1024);
        let mut warmed_capacity = None;
        for throwing in [false, true] {
            for _ in 0..128 {
                let (call, marker) = prepared_scope(&runtime, &mut context);
                let query = storage.acquire(context.realm, Vec::new(), Finish::Root);
                let completion = if throwing {
                    Completion::Throw(JsValue::Int(42))
                } else {
                    Completion::Return(JsValue::Int(42))
                };
                let mut state = runtime.0.state.borrow_mut();
                let mut owner = RawNativeQuery::new(
                    &runtime,
                    &mut state,
                    call,
                    query,
                    Step::Complete(Some(completion)),
                );
                // Actual outer activation publication, including an active
                // semantic parent that must not be moved into the empty scope.
                owner.query.as_mut().unwrap().parents.push(Resume::Identity);
                owner.publish_outer_scope().unwrap();
                assert_eq!(owner.query.as_ref().unwrap().parents.len(), 1);
                assert!(owner.query.as_ref().unwrap().natives[0].parents.is_empty());
                owner.finish_native_scope(&mut slots).unwrap();
                let completion = owner.finish_outer(&mut slots).unwrap();
                assert!(matches!(completion, Completion::Throw(JsValue::Int(42))) == throwing);
                let query = owner.take_query();
                assert!(query.parents.is_empty());
                assert_eq!(query.spare_parents.len(), 1);
                assert!(query.spare_parents[0].is_empty());
                drop(owner);
                query
                    .recycle_in_state(&runtime, &mut state, &mut storage)
                    .unwrap();
                assert!(state.heap.object(marker).is_err());
                assert!(state.active_frames.is_empty());
                let buffers = &storage.free[0];
                let capacity = (buffers.natives.capacity(), buffers.spare_parents.capacity());
                assert_eq!(*warmed_capacity.get_or_insert(capacity), capacity);
                assert_eq!(storage.free.len(), 1);
            }
        }
        assert_eq!(runtime.0.raw_execution_owners.get(), 0);
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn resident_nested_scopes_reserve_all_return_buffers_and_retire_each_owner() {
        use crate::engine::{
            value::JsValue,
            vm::{
                Completion,
                proxy_get_driver::{RawNativeQuery, Step},
                stack::SlotStore,
            },
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let mut storage = QueryStorage::default();
        let mut slots = SlotStore::new(1024);
        let (call, marker) = prepared_scope(&runtime, &mut context);
        let mut nested = Vec::new();
        let mut markers = vec![marker];
        for _ in 0..8 {
            let (call, marker) = prepared_scope(&runtime, &mut context);
            nested.push(call);
            markers.push(marker);
        }
        let query = storage.acquire(context.realm, Vec::new(), Finish::Root);
        let mut state = runtime.0.state.borrow_mut();
        let mut owner =
            RawNativeQuery::new(&runtime, &mut state, call, query, Step::Complete(None));
        owner.publish_outer_scope().unwrap();
        for call in nested {
            owner.step = Step::RawCall {
                inputs: None,
                resume: Some(Resume::Identity),
            };
            owner.pending_call = Some(call);
            owner.pending_step = Some(Step::Complete(Some(Completion::Return(JsValue::Int(42)))));
            owner.install_pending_native_scope().unwrap();
        }
        let query = owner.query.as_ref().unwrap();
        assert_eq!(query.natives.len(), 9);
        assert!(query.spare_parents.capacity() >= query.natives.len());
        let capacity = query.spare_parents.capacity();
        for marker in markers.iter().rev() {
            owner.query.as_mut().unwrap().parents.push(Resume::Identity);
            owner.finish_native_scope(&mut slots).unwrap();
            assert_eq!(
                owner.query.as_ref().unwrap().spare_parents.capacity(),
                capacity
            );
            assert!(owner.state.heap.object(*marker).is_err());
        }
        assert!(matches!(
            owner.finish_outer(&mut slots).unwrap(),
            Completion::Return(JsValue::Int(42))
        ));
        assert!(owner.state.active_frames.is_empty());
        let query = owner.take_query();
        drop(owner);
        query
            .recycle_in_state(&runtime, &mut state, &mut storage)
            .unwrap();
        assert!(storage.free[0].spare_parents.iter().all(Parents::is_empty));
        assert_eq!(runtime.0.raw_execution_owners.get(), 0);
        assert!(!runtime.is_poisoned());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn query_reserve_ledger_counts_growth_only_after_success() {
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut values = Vec::<u64>::new();
        reserve(&mut values, 3, "query.test_capacity").unwrap();
        let capacity = values.capacity();
        reserve(&mut values, 1, "query.test_capacity").unwrap();
        assert!(reserve(&mut values, usize::MAX, "query.test_capacity").is_err());
        let costs = profile.snapshot();
        let cost = &costs.call_buffers["query.test_capacity"];
        assert_eq!(cost.capacity_growths, 1);
        assert_eq!(
            cost.capacity_growth_bytes,
            (capacity * size_of::<u64>()) as u64
        );
        assert_eq!(cost.allocated_capacity_bytes, cost.capacity_growth_bytes);
    }

    #[test]
    fn cached_native_buffers_are_isolated_during_reentry_and_reused_after_wait() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut storage = QueryStorage::default();
        storage
            .acquire(context.realm, Vec::new(), Finish::Root)
            .recycle(&mut storage);
        assert!(storage.reserve_cached_native_entry().unwrap());
        let query = storage.acquire(context.realm, Vec::new(), Finish::Root);
        assert!(!storage.has_cached_entry());
        let nested = storage.acquire(context.realm, Vec::new(), Finish::Root);
        assert_eq!(nested.natives.capacity(), 0);
        nested.recycle(&mut storage);
        assert!(query.natives.capacity() >= 1);
        assert!(query.spare_parents.capacity() >= 1);
        query.recycle(&mut storage);
        assert_eq!(storage.free.len(), 2);
    }

    #[test]
    fn completed_queries_reuse_capacity_and_never_share_live_parents() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut storage = QueryStorage::default();
        let mut outer = storage.acquire(context.realm, Vec::new(), Finish::Root);
        outer.parents.try_reserve(12).unwrap();
        outer.parents.push(Resume::Identity);
        let capacity = outer.parents.0.capacity();
        let inner = storage.acquire(context.realm, Vec::new(), Finish::Root);
        assert!(inner.parents.is_empty());
        assert_eq!(outer.parents.len(), 1);
        inner.recycle(&mut storage);
        outer.recycle(&mut storage);
        let reused = storage.acquire(context.realm, Vec::new(), Finish::Root);
        assert!(reused.parents.is_empty());
        assert_eq!(reused.parents.0.capacity(), capacity);
        assert!(reused.natives.is_empty());
        assert!(reused.spare_parents.iter().all(Parents::is_empty));
    }
    #[test]
    fn cached_native_inplace_preserves_capacity_until_real_wait() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut storage = QueryStorage::default();
        assert!(!storage.reserve_cached_native_entry().unwrap());
        storage
            .acquire(context.realm, Vec::new(), Finish::Root)
            .recycle(&mut storage);
        let pool_capacity = storage.free.capacity();
        let entry_address = storage.free.as_ptr();
        for _ in 0..8 {
            assert!(storage.reserve_cached_native_entry().unwrap());
            let buffers = storage.free.last().unwrap();
            assert!(buffers.parents.is_empty() && buffers.natives.is_empty());
            assert_eq!(storage.free.len(), 1);
            assert_eq!(storage.free.capacity(), pool_capacity);
            assert_eq!(storage.free.as_ptr(), entry_address);
        }
        let reserved = storage.free.last().unwrap().natives.capacity();
        let query = storage.acquire(context.realm, Vec::new(), Finish::Root);
        assert_eq!(query.natives.capacity(), reserved);
        assert!(!storage.has_cached_entry());
        query.recycle(&mut storage);
        assert_eq!(storage.free.len(), 1);
    }

    #[test]
    fn cached_native_inplace_host_execution_has_independent_storage() {
        use crate::engine::vm::execution::{ExecutionLimits, HostBoundaryGuard, RunningExecution};
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut outer = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        outer
            .query_storage
            .acquire(context.realm, Vec::new(), Finish::Root)
            .recycle(&mut outer.query_storage);
        assert!(outer.query_storage.reserve_cached_native_entry().unwrap());
        let borrowed = outer.query_storage.free.last().unwrap();
        let reserved = borrowed.natives.capacity();
        let boundary = HostBoundaryGuard::enter(&runtime).unwrap();
        let mut inner = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        assert!(!inner.query_storage.has_cached_entry());
        let query = inner
            .query_storage
            .acquire(context.realm, Vec::new(), Finish::Root);
        assert_eq!(query.natives.capacity(), 0);
        query.recycle(&mut inner.query_storage);
        drop(inner);
        boundary.finish(&runtime).unwrap();
        assert_eq!(borrowed.natives.capacity(), reserved);
        assert!(borrowed.natives.is_empty());
        assert_eq!(outer.query_storage.free.len(), 1);
    }

    #[test]
    fn caller_supplied_parents_do_not_accumulate_idle_buffers() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut storage = QueryStorage::default();
        for _ in 0..2000 {
            let query = storage.acquire(context.realm, vec![Resume::Identity], Finish::Root);
            assert_eq!(query.parents.len(), 1);
            assert!(query.spare_parents.is_empty());
            query.recycle(&mut storage);
            assert_eq!(storage.free.len(), 1);
            assert!(storage.free[0].spare_parents.is_empty());
        }
    }
}
