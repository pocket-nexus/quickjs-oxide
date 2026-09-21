//! One get_proxy_method protocol for all Proxy operations.
//! Method lookup owns its depth guard until the selected method is delivered.
use super::{ProxyMethodStackGuard, RootedProxy};
use crate::engine::api::{
    error::{ErrorKind, NativeErrorKind},
    runtime::Runtime,
    runtime_error::RuntimeError,
};
use crate::engine::heap::ContextId;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::{JsValue, Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(super) enum MethodStep {
    Complete { resume: MethodResume },
    Throw(MethodThrow),
    Read { resume: MethodResume },
}

/// Owns the error thrown by method lookup. Abandoned lookups release the edge
/// through `Drop`; every consumer drains it with [`MethodThrow::take`].
pub(super) struct MethodThrow {
    runtime: Runtime,
    value: Option<JsValue>,
}
impl MethodThrow {
    fn new(runtime: Runtime, value: JsValue) -> Self {
        Self {
            runtime,
            value: Some(value),
        }
    }
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
    // Preserve and reuse the continuation Box allocation across callbacks.
    #[allow(clippy::vec_box)]
    static EMPTY_CONTINUATIONS: std::cell::RefCell<Vec<Box<Option<MethodResumeState>>>> = const { std::cell::RefCell::new(Vec::new()) };
}
impl super::reuse::Reusable for MethodResumeState {
    #[cfg(feature = "profiling")]
    const EVENT: &'static str = "method_resume_allocation";
    fn pool() -> &'static super::reuse::EmptyPool<Self> {
        &EMPTY_CONTINUATIONS
    }
}
const _: () = assert!(std::mem::size_of::<MethodResume>() <= 8);
pub(super) struct MethodResumeState {
    pending_effect: MethodStepPending,
    rooted: Option<RootedProxy>,
    selected: Option<DirectCallTarget>,
    search: Search,
}

struct Search {
    realm: ContextId,
    key: PropertyKey,
    /// Closed trap selector indexing `RuntimeState.proxy_trap_reads`.
    trap: usize,
    limit: Option<usize>,
    depth: usize,
    _guard: ProxyMethodStackGuard,
}

impl MethodStep {
    pub(super) fn start(
        runtime: &Runtime,
        realm: ContextId,
        proxy: ObjectRef,
        name: &'static str,
    ) -> Result<Self, RuntimeError> {
        if runtime.proxy_method_stack_would_overflow() {
            return overflow(runtime, realm);
        }
        let guard = ProxyMethodStackGuard::enter(runtime);
        let (trap, trap_index) = crate::engine::atom::pinned::PinnedAtom::proxy_method(name);
        let key = runtime.pinned_property_key(trap)?;
        Search {
            realm,
            key,
            trap: trap_index,
            limit: runtime.proxy_method_chain_limit(name),
            depth: 0,
            _guard: guard,
        }
        .read(runtime, proxy)
    }
}

fn overflow(runtime: &Runtime, realm: ContextId) -> Result<MethodStep, RuntimeError> {
    Ok(MethodStep::Throw(MethodThrow::new(
        runtime.clone(),
        runtime.new_native_error_jsvalue(realm, NativeErrorKind::Internal, "stack overflow")?,
    )))
}

impl Search {
    fn read(self, runtime: &Runtime, proxy: ObjectRef) -> Result<MethodStep, RuntimeError> {
        if self.limit.is_some_and(|limit| self.depth == limit) {
            return overflow(runtime, self.realm);
        }
        let data = runtime
            .proxy_snapshot_if_any(&proxy)?
            .ok_or(RuntimeError::Invariant(
                "Proxy method dispatch reached an ordinary object",
            ))?;
        if data.is_revoked {
            let NativeConversion::Throw(value) = runtime.proxy_revoked_throw::<()>(self.realm)?
            else {
                unreachable!("revoked proxy throws")
            };
            return Ok(MethodStep::Throw(MethodThrow::new(
                runtime.clone(),
                runtime.unroot_value(&value)?,
            )));
        }
        // A cached data-slot location skips the dynamic `handler[name]` read.
        // The value is always read from today's slot, so a same-shape overwrite
        // of the trap function is observed on the next operation. Accessors,
        // dictionary layouts and Proxy handlers decline and keep the full read.
        if let Some(value) =
            runtime.proxy_trap_read(self.trap, self.realm, data.handler, self.key.atom())?
        {
            let rooted = runtime.root_proxy_snapshot(&proxy, data)?;
            let resume = MethodResume(super::reuse::PooledBox::new(MethodResumeState {
                pending_effect: MethodStepPending::new(runtime.clone()),
                rooted: Some(rooted),
                selected: None,
                search: self,
            }));
            return resume.resume(runtime, Completion::Return(runtime.unroot_value(&value)?));
        }
        let rooted = runtime.root_proxy_snapshot(&proxy, data)?;
        let receiver = runtime.into_jsvalue(Value::Object(rooted.handler.clone()))?;
        Ok(MethodStep::request_read(
            rooted.handler.clone(),
            self.key.clone(),
            receiver,
            MethodResume(super::reuse::PooledBox::new(MethodResumeState {
                pending_effect: MethodStepPending::new(runtime.clone()),
                rooted: Some(rooted),
                selected: None,
                search: self,
            })),
        ))
    }
}

impl MethodResume {
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
        let mut value = match completion {
            Completion::Throw(value) => {
                return Ok(MethodStep::Throw(MethodThrow::new(runtime.clone(), value)));
            }
            Completion::Return(value) => value,
        };
        // Undefined/Null keeps walking the target Proxy chain iteratively; every
        // level first tries the trap cache and otherwise keeps the dynamic read.
        loop {
            if matches!(value, JsValue::Undefined | JsValue::Null) {
                let target = self.0.rooted.as_ref().expect("proxy owner").target.clone();
                let Some(data) = runtime.proxy_snapshot_if_any(&target)? else {
                    return Ok(MethodStep::Complete { resume: self });
                };
                {
                    let state = &mut *self.0;
                    state.search.depth = state.search.depth.saturating_add(1);
                    if state
                        .search
                        .limit
                        .is_some_and(|limit| state.search.depth == limit)
                    {
                        return overflow(runtime, state.search.realm);
                    }
                }
                if data.is_revoked {
                    let realm = self.0.search.realm;
                    let NativeConversion::Throw(value) =
                        runtime.proxy_revoked_throw::<()>(realm)?
                    else {
                        unreachable!("revoked proxy throws")
                    };
                    return Ok(MethodStep::Throw(MethodThrow::new(
                        runtime.clone(),
                        runtime.unroot_value(&value)?,
                    )));
                }
                let next = runtime.root_proxy_snapshot(&target, data)?;
                let cached = runtime.proxy_trap_read(
                    self.0.search.trap,
                    self.0.search.realm,
                    next.handler.object_id(),
                    self.0.search.key.atom(),
                )?;
                let old = self.0.rooted.replace(next);
                match cached {
                    Some(method_value) => {
                        drop(old);
                        value = runtime.unroot_value(&method_value)?;
                        continue;
                    }
                    None => {
                        let (object, receiver, key) = {
                            let rooted = self.0.rooted.as_ref().expect("proxy owner");
                            (
                                rooted.handler.clone(),
                                runtime.into_jsvalue(Value::Object(rooted.handler.clone()))?,
                                self.0.search.key.clone(),
                            )
                        };
                        let step = MethodStep::request_read(object, key, receiver, self);
                        drop(old);
                        return Ok(step);
                    }
                }
            }
            let method = match runtime.direct_call_target_from_jsvalue(value) {
                Ok(method) => method,
                Err(RuntimeError::Engine(error)) if error.kind() == ErrorKind::Type => {
                    let value = runtime.new_native_error_from_error_jsvalue(
                        self.0.search.realm,
                        NativeErrorKind::Type,
                        &error,
                    )?;
                    return Ok(MethodStep::Throw(MethodThrow::new(runtime.clone(), value)));
                }
                Err(error) => return Err(error),
            };
            self.0.selected = Some(method);
            return Ok(MethodStep::Complete { resume: self });
        }
    }
}

struct MethodStepPending {
    runtime: Runtime,
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    read_receiver: Option<JsValue>,
}
impl MethodStepPending {
    fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            read_object: None,
            read_key: None,
            read_receiver: None,
        }
    }
}
impl Drop for MethodStepPending {
    /// Release the internal read edge still held when the request is
    /// abandoned. Consumption goes through `Option::take`.
    fn drop(&mut self) {
        if let Some(value) = self.read_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl MethodStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        receiver: JsValue,
        mut resume: MethodResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        resume.0.pending_effect.read_receiver = Some(receiver);
        Self::Read { resume }
    }
}
impl MethodResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("MethodStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("MethodStep Read key")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("MethodStep Read receiver")
    }
}
const _: () = assert!(std::mem::size_of::<MethodStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<MethodStep>() <= 64);

#[cfg(test)]
mod resident_tests {
    use super::*;
    #[test]
    fn proxy_method_completion_reuses_the_pending_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(proxy) = context.eval("new Proxy({}, {})").unwrap() else {
            panic!("proxy")
        };
        let MethodStep::Read { mut resume } =
            MethodStep::start(&runtime, context.realm, proxy.clone(), "get").unwrap()
        else {
            panic!("read")
        };
        let address = (&*resume.0) as *const MethodResumeState;
        drop(resume.take_read_object());
        drop(resume.take_read_key());
        runtime
            .release_jsvalue(resume.take_read_receiver())
            .unwrap();
        let MethodStep::Complete { mut resume } = resume
            .resume(&runtime, Completion::Return(JsValue::Undefined))
            .unwrap()
        else {
            panic!("complete")
        };
        assert_eq!((&*resume.0) as *const MethodResumeState, address);
        assert_eq!(resume.take_completed_rooted().proxy, proxy);
        assert!(resume.take_completed_target().is_none());
    }
}

#[cfg(test)]
mod trap_cache_tests {
    use super::*;

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
        MethodStep::start(runtime, realm, proxy.clone(), "get").unwrap()
    }

    #[test]
    fn trap_cache_skips_the_dynamic_read_and_follows_same_shape_overwrite() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
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
            matches!(start_get(&runtime, realm, &proxy), MethodStep::Read { .. }),
            "a cold cache still performs the canonical dynamic read"
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
        let mut context = runtime.new_context();
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
        let mut context = runtime.new_context();
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
        let mut context = runtime.new_context();
        drop(context
            .eval(
                "var delHandler={get:function(){return 1}};var delProxy=new Proxy({},delHandler);",
            )
            .unwrap());
        let proxy = object(context.eval("delProxy").unwrap());
        assert!(matches!(
            start_get(&runtime, context.realm, &proxy),
            MethodStep::Read { .. }
        ));
        assert!(matches!(
            start_get(&runtime, context.realm, &proxy),
            MethodStep::Complete { .. }
        ));
        drop(context.eval("delete delHandler.get").unwrap());
        assert!(
            matches!(
                start_get(&runtime, context.realm, &proxy),
                MethodStep::Read { .. }
            ),
            "removing the layout revision invalidates the location"
        );
    }

    #[test]
    fn revoked_proxy_is_rejected_before_the_cache_is_consulted() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(context
            .eval("var revocable=Proxy.revocable({},{get:function(){return 1}});var revoked=revocable.proxy;")
            .unwrap());
        let proxy = object(context.eval("revoked").unwrap());
        assert!(matches!(
            start_get(&runtime, context.realm, &proxy),
            MethodStep::Read { .. }
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
        let mut context = runtime.new_context();
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
