//! Local query ownership borrows its cleanup context. Published records carry
//! only algorithm state; their execution releases them explicitly.
use super::{Query, QueryStorage, Resume, Runtime};

/// The innermost resume stays guarded while reply ownership is authenticated.
#[must_use]
pub(super) struct ResumeScope<'a> {
    runtime: &'a Runtime,
    resume: Option<Resume>,
}

impl<'a> ResumeScope<'a> {
    pub(super) fn new(runtime: &'a Runtime, resume: Resume) -> Self {
        Self {
            runtime,
            resume: Some(resume),
        }
    }

    pub(super) fn take(mut self) -> Resume {
        self.resume.take().expect("resume transferred once")
    }
}

impl Drop for ResumeScope<'_> {
    fn drop(&mut self) {
        if let Some(resume) = self.resume.take() {
            let _unwind = self.runtime.unwind_guard();
            resume.release_owned(self.runtime);
        }
    }
}

#[must_use]
pub(super) struct QueryScope<'a> {
    runtime: &'a Runtime,
    query: Option<Query>,
}

impl<'a> QueryScope<'a> {
    pub(super) fn new(runtime: &'a Runtime, query: Query) -> Self {
        Self {
            runtime,
            query: Some(query),
        }
    }

    pub(super) fn take(mut self) -> Query {
        self.query.take().expect("query transferred once")
    }

    pub(super) fn runtime(&self) -> &'a Runtime {
        self.runtime
    }

    pub(super) fn recycle(mut self, storage: &mut QueryStorage) {
        // Completed queries remain in their resident scope. Moving the whole
        // record just to drain three empty buffers needlessly spills its finish
        // and callback state; only publication needs an ownership transfer.
        self.query
            .as_mut()
            .expect("query owner")
            .recycle(self.runtime, storage);
    }
}

impl std::ops::Deref for QueryScope<'_> {
    type Target = Query;
    fn deref(&self) -> &Query {
        self.query.as_ref().expect("query owner")
    }
}

impl std::ops::DerefMut for QueryScope<'_> {
    fn deref_mut(&mut self) -> &mut Query {
        self.query.as_mut().expect("query owner")
    }
}

impl Drop for QueryScope<'_> {
    fn drop(&mut self) {
        if let Some(query) = self.query.as_mut() {
            let _unwind = self.runtime.unwind_guard();
            query.release_owned(self.runtime);
        }
    }
}

pub(super) struct StepScope<'a> {
    runtime: &'a Runtime,
    step: Option<super::Step>,
}
impl<'a> StepScope<'a> {
    pub(super) fn new(runtime: &'a Runtime, step: super::Step) -> Self {
        Self {
            runtime,
            step: Some(step),
        }
    }
    pub(super) fn take(&mut self) -> super::Step {
        self.step.take().expect("selected step owner")
    }
}
impl std::ops::Deref for StepScope<'_> {
    type Target = super::Step;
    fn deref(&self) -> &Self::Target {
        self.step.as_ref().expect("selected step owner")
    }
}
impl std::ops::DerefMut for StepScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.step.as_mut().expect("selected step owner")
    }
}
impl Drop for StepScope<'_> {
    fn drop(&mut self) {
        if let Some(step) = self.step.as_mut() {
            let _unwind = self.runtime.unwind_guard();
            step.release_owned(self.runtime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::Value,
        vm::{
            execution::{ExecutionLimits, RunningExecution},
            frame::ReturnOwner,
            proxy_get_driver::{Finish, Resume, put_pending},
        },
    };

    #[test]
    fn rejected_pending_publication_releases_the_new_owner_only() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context.eval("({})").unwrap() else {
            panic!("object")
        };
        let object_id = object.object_id();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let first =
            execution
                .query_storage
                .acquire(&runtime, context.realm, Vec::new(), Finish::Root);
        let first = execution.query_storage.pending(1, first, Resume::Identity);
        put_pending(&runtime, &mut execution, ReturnOwner::Root, first).unwrap();
        let second =
            execution
                .query_storage
                .acquire(&runtime, context.realm, Vec::new(), Finish::Root);
        let second = execution
            .query_storage
            .pending(2, second, Resume::ReadOwner(object));
        assert!(put_pending(&runtime, &mut execution, ReturnOwner::Root, second).is_err());
        assert!(runtime.0.state.borrow().heap.object(object_id).is_err());
        assert_eq!(execution.root_query.as_ref().unwrap().identity, 1);
        drop(execution);
        assert!(!runtime.is_poisoned());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn local_query_unwind_poisons_before_cleanup_with_a_state_borrow() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context.eval("({})").unwrap() else {
            panic!("object")
        };
        let mut storage = QueryStorage::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = runtime.0.state.borrow_mut();
            let mut query = storage.acquire(&runtime, context.realm, Vec::new(), Finish::Root);
            query.parents.push(Resume::ReadOwner(object));
            panic!("query unwind witness");
        }));
        assert!(result.is_err());
        assert!(runtime.is_poisoned());
        assert!(runtime.check_poison().is_err());
        drop(storage);
        drop(context);
        drop(runtime);
    }
}
