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
    pub(super) fn pending(
        &mut self,
        identity: u64,
        query: Query,
        resume: Resume,
    ) -> Box<super::PendingProxyGet> {
        if let Some(mut pending) = self.pending.pop() {
            pending.identity = identity;
            pending.query = query;
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

    pub(super) fn release_pending(
        &mut self,
        mut pending: Box<super::PendingProxyGet>,
    ) -> (u64, Query, Resume) {
        let empty = Query {
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

    pub(super) fn take_native_wait(
        &mut self,
    ) -> Result<Vec<super::native::NativeWaitRecord>, crate::engine::api::Error> {
        let mut waiting = self.native_waits.pop().unwrap_or_default();
        if waiting.is_empty() {
            reserve(&mut waiting, 1, "query.native_wait_payload").map_err(|_| {
                crate::engine::api::Error::internal("native waiting payload allocation failed")
            })?;
            waiting.push(super::native::NativeWaitRecord {
                call: None,
                step: super::Step::Complete(Some(crate::engine::vm::Completion::Return(
                    crate::engine::value::Value::Undefined,
                ))),
                parents: Vec::new(),
            });
        }
        debug_assert_eq!(waiting.len(), 1);
        debug_assert!(waiting[0].call.is_none() && waiting[0].parents.is_empty());
        Ok(waiting)
    }

    pub(super) fn recycle_native_wait(&mut self, waiting: Vec<super::native::NativeWaitRecord>) {
        debug_assert_eq!(waiting.len(), 1);
        debug_assert!(waiting[0].call.is_none() && waiting[0].parents.is_empty());
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
        // Preserve Query::drop's native/root release order. On abnormal exits
        // this includes every still-waiting native and its enclosing parents.
        while self.parents.pop().is_some() {}
        while let Some(mut scope) = self.natives.pop() {
            drop(scope.call);
            drop(scope.resume);
            while scope.parents.pop().is_some() {}
            // Caching must not turn successful cleanup into an allocation error.
            if reserve(&mut self.spare_parents, 1, "query.spare_parents").is_ok() {
                self.spare_parents.push(scope.parents);
            }
        }
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
        let context = runtime.new_context();
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
        let context = runtime.new_context();
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
        let context = runtime.new_context();
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
        let context = runtime.new_context();
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
        let context = runtime.new_context();
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
