//! Compatibility adapter for remaining owning Proxy families. GetMethod's
//! semantic prefix and selected read effects live in the current-State kernel.
mod state;
use super::RootedProxy;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    atom::Atom,
    heap::ContextId,
    object::{ObjectRef, StateReadEffect},
    value::JsValue,
    vm::{Completion, call::DirectCallTarget},
};
pub(crate) use state::{
    StateMethodResume, StateMethodSelection, StateMethodStep, StateMethodTarget, StateRootedProxy,
};

pub(super) enum MethodStep {
    Complete { resume: MethodResume },
    Throw(MethodThrow),
    Read { resume: MethodResume },
}
pub(super) struct MethodThrow {
    runtime: Runtime,
    value: Option<JsValue>,
}
impl MethodThrow {
    pub(super) fn take(mut self) -> JsValue {
        self.value.take().expect("MethodStep Throw value")
    }
}
impl Drop for MethodThrow {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}

pub(super) struct MethodResume(super::reuse::PooledBox<MethodResumeState>);
impl std::ops::Deref for MethodResume {
    type Target = MethodResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for MethodResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
thread_local! {
    #[allow(clippy::vec_box)]
    static EMPTY_CONTINUATIONS: std::cell::RefCell<Vec<Box<Option<MethodResumeState>>>> = const { std::cell::RefCell::new(Vec::new()) };
}
impl super::reuse::Reusable for MethodResumeState {
    #[cfg(feature = "profiling")]
    const EVENT: &'static str = "legacy_method_resume_allocation";
    fn pool() -> &'static super::reuse::EmptyPool<Self> {
        &EMPTY_CONTINUATIONS
    }
}
const _: () = assert!(std::mem::size_of::<MethodResume>() <= 8);
pub(super) struct MethodResumeState {
    runtime: Runtime,
    rooted: Option<RootedProxy>,
    selected: Option<DirectCallTarget>,
    waiting: Option<StateMethodResume>,
    read: Option<StateReadEffect>,
    atom: Option<Atom>,
}
impl Drop for MethodResumeState {
    fn drop(&mut self) {
        if let Some(effect) = self.read.take() {
            state::release_read_legacy(
                &self.runtime,
                effect,
                self.atom.take().expect("method effect atom"),
            );
        }
        if let Some(waiting) = self.waiting.take() {
            waiting.release_legacy(&self.runtime);
        }
    }
}
impl MethodStep {
    pub(super) fn start(
        runtime: &Runtime,
        realm: ContextId,
        proxy: ObjectRef,
        name: &'static str,
    ) -> Result<Self, RuntimeError> {
        if !proxy.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("Proxy"));
        }
        let step = StateMethodStep::start(
            runtime,
            &mut runtime.0.state.borrow_mut(),
            realm,
            proxy.object_id(),
            name,
        )?;
        Self::from_state(runtime, step)
    }
    fn from_state(runtime: &Runtime, step: StateMethodStep) -> Result<Self, RuntimeError> {
        Ok(match step {
            StateMethodStep::Complete(StateMethodSelection { rooted, target }) => Self::Complete {
                resume: MethodResume(super::reuse::PooledBox::new(MethodResumeState {
                    runtime: runtime.clone(),
                    rooted: Some(rooted.into_legacy(runtime)),
                    selected: target.map(|target| target.into_legacy(runtime)),
                    waiting: None,
                    read: None,
                    atom: None,
                })),
            },
            StateMethodStep::Throw(value) => Self::Throw(MethodThrow {
                runtime: runtime.clone(),
                value: Some(value),
            }),
            StateMethodStep::Read {
                effect,
                atom,
                resume,
            } => Self::Read {
                resume: MethodResume(super::reuse::PooledBox::new(MethodResumeState {
                    runtime: runtime.clone(),
                    rooted: None,
                    selected: None,
                    waiting: Some(resume),
                    read: Some(effect),
                    atom: Some(atom),
                })),
            },
        })
    }
}
impl MethodResume {
    pub(super) fn take_state_read(&mut self) -> (StateReadEffect, Atom) {
        (
            self.0.read.take().expect("method read effect"),
            self.0.atom.take().expect("method read atom"),
        )
    }
    pub(super) fn take_completed_rooted(&mut self) -> RootedProxy {
        self.0.rooted.take().expect("completed proxy owner")
    }
    pub(super) fn take_completed_target(&mut self) -> Option<DirectCallTarget> {
        self.0.selected.take()
    }
    pub(super) fn resume(
        mut self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<MethodStep, RuntimeError> {
        let waiting = self.0.waiting.take().ok_or(RuntimeError::Invariant(
            "completed Method has no waiting effect",
        ))?;
        let step = waiting.resume(runtime, &mut runtime.0.state.borrow_mut(), completion)?;
        MethodStep::from_state(runtime, step)
    }
}
const _: () = assert!(std::mem::size_of::<MethodStep>() <= 64);

#[cfg(test)]
mod trap_cache_tests {
    use super::*;
    use crate::engine::value::Value;

    fn object(value: Value) -> ObjectRef {
        let Value::Object(object) = value else {
            panic!("expected object")
        };
        object
    }

    fn callable_id(target: &DirectCallTarget) -> crate::engine::heap::ObjectId {
        match target {
            DirectCallTarget::Callable(callable) => callable.as_object().object_id(),
            DirectCallTarget::NonCallableProxy(object) => object.object_id(),
        }
    }

    fn start_get(runtime: &Runtime, realm: ContextId, proxy: &ObjectRef) -> MethodStep {
        MethodStep::start(
            runtime,
            realm,
            proxy.try_clone().expect("duplicate root"),
            "get",
        )
        .unwrap()
    }

    #[test]
    fn trap_cache_skips_the_dynamic_read_and_follows_same_shape_overwrite() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(
            context
                .eval(
                    "var first=function(){return 1};var second=function(){return 2};\
                 var trapHandler={get:first};var trapProxy=new Proxy({},trapHandler);",
                )
                .unwrap(),
        );
        let proxy = object(context.eval("trapProxy").unwrap());
        let realm = context.realm;

        assert!(
            matches!(
                start_get(&runtime, realm, &proxy),
                MethodStep::Complete { .. }
            ),
            "a cold data lookup completes through the canonical State read"
        );
        let MethodStep::Complete { mut resume } = start_get(&runtime, realm, &proxy) else {
            panic!("the trained location must skip the dynamic read")
        };
        assert_eq!(
            callable_id(&resume.take_completed_target().unwrap()),
            object(context.eval("first").unwrap()).object_id()
        );

        // Overwriting a data property keeps the shape and revision; the cache
        // stores a location, so the next operation observes the new function.
        drop(context.eval("trapHandler.get=second").unwrap());
        let MethodStep::Complete { mut resume } = start_get(&runtime, realm, &proxy) else {
            panic!("same-shape overwrite keeps the cache location")
        };
        assert_eq!(
            callable_id(&resume.take_completed_target().unwrap()),
            object(context.eval("second").unwrap()).object_id()
        );
    }

    #[test]
    fn accessor_proxy_handler_trap_always_uses_the_dynamic_read() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(context
            .eval(
                "var accessorReads=0;var accessorHandler={};\
                 Object.defineProperty(accessorHandler,'get',{get(){accessorReads++;return function(){return 5}}});\
                 var accessorProxy=new Proxy({},accessorHandler);",
            )
            .unwrap());
        let proxy = object(context.eval("accessorProxy").unwrap());
        for _ in 0..3 {
            assert!(
                matches!(
                    start_get(&runtime, context.realm, &proxy),
                    MethodStep::Read { .. }
                ),
                "an accessor trap may run observable code on every read"
            );
        }
    }

    #[test]
    fn proxy_handler_trap_declines_the_cache() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(
            context
                .eval(
                    "var innerHandler={get:function(){return 1}};\
                 var proxyHandler=new Proxy(innerHandler,{});\
                 var chainedProxy=new Proxy({},proxyHandler);",
                )
                .unwrap(),
        );
        let proxy = object(context.eval("chainedProxy").unwrap());
        for _ in 0..3 {
            assert!(matches!(
                start_get(&runtime, context.realm, &proxy),
                MethodStep::Read { .. }
            ));
        }
    }

    #[test]
    fn deleted_trap_location_falls_back_to_the_dynamic_read() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(context
            .eval(
                "var delHandler={get:function(){return 1}};var delProxy=new Proxy({},delHandler);",
            )
            .unwrap());
        let proxy = object(context.eval("delProxy").unwrap());
        assert!(matches!(
            start_get(&runtime, context.realm, &proxy),
            MethodStep::Complete { .. }
        ));
        assert!(matches!(
            start_get(&runtime, context.realm, &proxy),
            MethodStep::Complete { .. }
        ));
        drop(context.eval("delete delHandler.get").unwrap());
        assert!(
            matches!(
                start_get(&runtime, context.realm, &proxy),
                MethodStep::Complete { .. }
            ),
            "a deleted trap is absent through canonical synchronous lookup"
        );
    }

    #[test]
    fn revoked_proxy_is_rejected_before_the_cache_is_consulted() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(context
            .eval("var revocable=Proxy.revocable({},{get:function(){return 1}});var revoked=revocable.proxy;")
            .unwrap());
        let proxy = object(context.eval("revoked").unwrap());
        assert!(matches!(
            start_get(&runtime, context.realm, &proxy),
            MethodStep::Complete { .. }
        ));
        drop(context.eval("revocable.revoke()").unwrap());
        assert!(
            matches!(
                start_get(&runtime, context.realm, &proxy),
                MethodStep::Throw(_)
            ),
            "revocation is checked before any cached hit"
        );
    }

    #[test]
    fn proxy_method_chain_limit_still_bounds_cached_descent() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        // Empty handlers forward by returning a non-method, so an end-to-end
        // lookup descends the whole chain and trips the closed logical budget.
        assert_eq!(
            context
                .eval(
                    "(()=>{let chain=new Proxy({},{});\
                     for(let i=0;i<3000;i++)chain=new Proxy(chain,{});\
                     try{chain.x;return false}catch(e){return String(e).includes('stack overflow')}})()"
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}
