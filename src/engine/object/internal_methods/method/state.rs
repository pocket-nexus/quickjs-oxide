//! Canonical GetMethod prefix. Synchronous lookup and empty-handler forwarding
//! use the current State; only selected read effects allocate a resume.
use super::super::RootedProxy;
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    atom::{Atom, pinned::PinnedAtom},
    heap::{
        ContextId, ObjectId, ObjectPayload, ProxyData,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::{CallableRef, ObjectRef, ReadBoundary, StateReadEffect},
    value::JsValue,
    vm::{Completion, call::DirectCallTarget},
};

#[must_use]
pub(crate) struct StateRootedProxy {
    pub(crate) proxy: ObjectId,
    pub(crate) data: ProxyData,
    pub(crate) target: ObjectId,
    pub(crate) handler: ObjectId,
}
impl StateRootedProxy {
    fn retain(
        state: &mut RuntimeState,
        runtime: &Runtime,
        proxy: ObjectId,
        data: ProxyData,
    ) -> Result<Self, RuntimeError> {
        let target = state.dup_jsvalue(&JsValue::Object(data.target))?;
        let mut target = OwnedValueGuard::new(state, &runtime.0.poisoned, target);
        let (state, target_edge) = target.parts();
        let handler = state.dup_jsvalue(&JsValue::Object(data.handler))?;
        let mut handler = OwnedValueGuard::new(state, &runtime.0.poisoned, handler);
        let (state, handler_edge) = handler.parts();
        let _owned_proxy = state.dup_jsvalue(&JsValue::Object(proxy))?;
        target_edge.take();
        handler_edge.take();
        Ok(Self {
            proxy,
            data,
            target: data.target,
            handler: data.handler,
        })
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_in_state(
        self,
        state: &mut RuntimeState,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        for object in [self.proxy, self.target, self.handler] {
            state.release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))?;
        }
        Ok(())
    }
    pub(super) fn into_legacy(self, runtime: &Runtime) -> RootedProxy {
        RootedProxy {
            proxy: ObjectRef::from_owned_handle(runtime.clone(), self.proxy),
            data: self.data,
            target: ObjectRef::from_owned_handle(runtime.clone(), self.target),
            handler: ObjectRef::from_owned_handle(runtime.clone(), self.handler),
        }
    }
}

#[must_use]
pub(crate) enum StateMethodTarget {
    Callable(ObjectId),
    NonCallableProxy(ObjectId),
}
impl StateMethodTarget {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn object(&self) -> ObjectId {
        match self {
            Self::Callable(id) | Self::NonCallableProxy(id) => *id,
        }
    }
    pub(in crate::engine::object::internal_methods) fn into_legacy(
        self,
        runtime: &Runtime,
    ) -> DirectCallTarget {
        match self {
            Self::Callable(id) => DirectCallTarget::Callable(CallableRef::from_validated_object(
                ObjectRef::from_owned_handle(runtime.clone(), id),
            )),
            Self::NonCallableProxy(id) => DirectCallTarget::NonCallableProxy(
                ObjectRef::from_owned_handle(runtime.clone(), id),
            ),
        }
    }
}

#[must_use]
pub(crate) struct StateMethodSelection {
    pub(crate) rooted: StateRootedProxy,
    pub(crate) target: Option<StateMethodTarget>,
}
impl StateMethodSelection {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_in_state(
        self,
        state: &mut RuntimeState,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        if let Some(target) = self.target {
            state.release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(target.object()))?;
        }
        self.rooted.release_in_state(state, runtime)
    }
}

#[must_use]
pub(crate) enum StateMethodStep {
    Complete(StateMethodSelection),
    Throw(JsValue),
    Read {
        effect: StateReadEffect,
        atom: Atom,
        resume: StateMethodResume,
    },
}
impl StateMethodStep {
    pub(crate) fn start(
        runtime: &Runtime,
        state: &mut RuntimeState,
        realm: ContextId,
        proxy: ObjectId,
        name: &'static str,
    ) -> Result<Self, RuntimeError> {
        if runtime.proxy_method_stack_would_overflow() {
            return native_throw(
                state,
                runtime,
                realm,
                NativeErrorKind::Internal,
                "stack overflow",
            );
        }
        let (pinned, trap) = PinnedAtom::proxy_method(name);
        let atom = state.pinned_atoms.get(pinned);
        runtime
            .0
            .proxy_method_depth
            .set(runtime.0.proxy_method_depth.get().saturating_add(1));
        let search = Search {
            realm,
            atom,
            trap,
            limit: runtime.proxy_method_chain_limit(name),
            depth: 0,
            entered: true,
            rooted: None,
        };
        let mut scope = Scope {
            state,
            runtime,
            search: Some(search),
            value: None,
        };
        scope.select_proxy(proxy)?;
        scope.drive()
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_in_state(
        self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(selection) => selection.release_in_state(state, runtime),
            Self::Throw(value) => state.release_owned_jsvalue(&runtime.0.poisoned, value),
            Self::Read {
                effect,
                atom,
                resume,
            } => {
                let mut scope = Scope {
                    state,
                    runtime,
                    search: Some(resume.0.into_inner()),
                    value: None,
                };
                effect.release_in_state(scope.state, runtime)?;
                scope
                    .state
                    .release_atoms([atom])
                    .inspect_err(|_| runtime.0.poisoned.set(true))?;
                scope.cleanup()
            }
        }
    }
}

pub(crate) struct StateMethodResume(super::super::reuse::PooledBox<Search>);
thread_local! {
    #[allow(clippy::vec_box)]
    static EMPTY_CONTINUATIONS: std::cell::RefCell<Vec<Box<Option<Search>>>> = const { std::cell::RefCell::new(Vec::new()) };
}
impl super::super::reuse::Reusable for Search {
    #[cfg(feature = "profiling")]
    const EVENT: &'static str = "method_resume_allocation";
    fn pool() -> &'static super::super::reuse::EmptyPool<Self> {
        &EMPTY_CONTINUATIONS
    }
}
const _: () = assert!(std::mem::size_of::<StateMethodResume>() <= 8);
struct Search {
    realm: ContextId,
    // A pinned atom is live for this Runtime's lifetime; only the selected
    // read effect acquires an Atom edge to hand to its consumer.
    atom: Atom,
    trap: usize,
    limit: Option<usize>,
    depth: usize,
    entered: bool,
    rooted: Option<StateRootedProxy>,
}
impl StateMethodResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        completion: Completion,
    ) -> Result<StateMethodStep, RuntimeError> {
        let mut scope = Scope {
            state,
            runtime,
            search: Some(self.0.into_inner()),
            value: None,
        };
        match completion {
            Completion::Throw(value) => Ok(StateMethodStep::Throw(value)),
            Completion::Return(value) => {
                scope.value = Some(value);
                scope.drive()
            }
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_in_state(
        self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        Scope {
            state,
            runtime,
            search: Some(self.0.into_inner()),
            value: None,
        }
        .cleanup()
    }
    // Remaining B3 families own this adapter outside State. It supplies the
    // context on cancellation; the raw search still owns no Runtime header.
    pub(super) fn release_legacy(self, runtime: &Runtime) {
        let mut search = self.0.into_inner();
        if search.entered {
            runtime
                .0
                .proxy_method_depth
                .set(runtime.0.proxy_method_depth.get().saturating_sub(1));
            search.entered = false;
        }
        if std::thread::panicking() {
            runtime.0.poisoned.set(true);
        }
        if runtime.0.poisoned.get() {
            return;
        }
        if let Some(rooted) = search.rooted.take() {
            for id in [rooted.proxy, rooted.target, rooted.handler] {
                let _ = runtime.release_jsvalue(JsValue::Object(id));
            }
        }
    }
}

struct Scope<'a> {
    state: &'a mut RuntimeState,
    runtime: &'a Runtime,
    search: Option<Search>,
    value: Option<JsValue>,
}
impl Scope<'_> {
    fn select_proxy(&mut self, proxy: ObjectId) -> Result<(), RuntimeError> {
        let ObjectPayload::Proxy(data) = self.state.heap.object(proxy)?.payload else {
            return Err(RuntimeError::Invariant(
                "Proxy method dispatch reached an ordinary object",
            ));
        };
        let roots = StateRootedProxy::retain(self.state, self.runtime, proxy, data)?;
        if let Some(old) = self.search.as_mut().unwrap().rooted.replace(roots) {
            old.release_in_state(self.state, self.runtime)?;
        }
        Ok(())
    }
    fn drive(&mut self) -> Result<StateMethodStep, RuntimeError> {
        loop {
            let search = self.search.as_ref().unwrap();
            if search.limit.is_some_and(|limit| search.depth == limit) {
                return native_throw(
                    self.state,
                    self.runtime,
                    search.realm,
                    NativeErrorKind::Internal,
                    "stack overflow",
                );
            }
            if self.value.is_none() {
                let rooted = search.rooted.as_ref().unwrap();
                if rooted.data.is_revoked {
                    return native_throw(
                        self.state,
                        self.runtime,
                        search.realm,
                        NativeErrorKind::Type,
                        "revoked proxy",
                    );
                }
                let (realm, trap, atom, handler) =
                    (search.realm, search.trap, search.atom, rooted.handler);
                let mut boundary = None;
                let value = match self.state.proxy_trap_read_in_state(
                    self.runtime.domain_id(),
                    trap,
                    realm,
                    handler,
                    atom,
                )? {
                    Some(value) => Some(value),
                    None => self.state.select_ordinary_read_in_state(
                        &self.runtime.0.poisoned,
                        handler,
                        atom,
                        self.runtime.domain_id(),
                        &mut boundary,
                        None,
                    )?,
                };
                if let Some(value) = value {
                    self.value = Some(value);
                } else if matches!(boundary, Some(ReadBoundary::Absent)) {
                    self.value = Some(JsValue::Undefined);
                } else {
                    let boundary = boundary
                        .ok_or(RuntimeError::Invariant("method read omitted its selection"))?;
                    match super::super::resolve_read_boundary_in_state(
                        self.runtime,
                        self.state,
                        realm,
                        atom,
                        &JsValue::Object(handler),
                        boundary,
                    )? {
                        super::super::get::ProxyGetStep::Complete(Completion::Return(value)) => {
                            self.value = Some(value);
                        }
                        super::super::get::ProxyGetStep::Complete(Completion::Throw(value)) => {
                            return Ok(StateMethodStep::Throw(value));
                        }
                        super::super::get::ProxyGetStep::Effect(effect) => {
                            if let Err(error) = self.state.atoms.retain(atom) {
                                effect.release_in_state(self.state, self.runtime)?;
                                return Err(error.into());
                            }
                            return Ok(StateMethodStep::Read {
                                effect,
                                atom,
                                resume: StateMethodResume(super::super::reuse::PooledBox::new(
                                    self.search.take().unwrap(),
                                )),
                            });
                        }
                    }
                }
            }
            if matches!(self.value, Some(JsValue::Undefined | JsValue::Null)) {
                self.value.take();
                let search = self.search.as_mut().unwrap();
                let rooted = search.rooted.as_ref().unwrap();
                let next = rooted.target;
                if !matches!(
                    self.state.heap.object(next)?.payload,
                    ObjectPayload::Proxy(_)
                ) {
                    let rooted = search.rooted.take().unwrap();
                    return Ok(StateMethodStep::Complete(StateMethodSelection {
                        rooted,
                        target: None,
                    }));
                }
                search.depth = search.depth.saturating_add(1);
                self.select_proxy(next)?;
                continue;
            }
            let search = self.search.as_mut().unwrap();
            let target = match self.value.as_ref().unwrap() {
                JsValue::Object(id) => match self.state.heap.object(*id)?.payload {
                    ObjectPayload::NativeFunction { .. }
                    | ObjectPayload::BoundFunction { .. }
                    | ObjectPayload::BytecodeFunction { .. }
                    | ObjectPayload::Proxy(ProxyData {
                        is_callable: true, ..
                    }) => Some(StateMethodTarget::Callable(*id)),
                    ObjectPayload::Proxy(_) => Some(StateMethodTarget::NonCallableProxy(*id)),
                    _ => None,
                },
                _ => None,
            };
            if let Some(target) = target {
                self.value.take();
                let rooted = search.rooted.take().unwrap();
                return Ok(StateMethodStep::Complete(StateMethodSelection {
                    rooted,
                    target: Some(target),
                }));
            }
            return native_throw(
                self.state,
                self.runtime,
                search.realm,
                NativeErrorKind::Type,
                "not a function",
            );
        }
    }
    fn cleanup(&mut self) -> Result<(), RuntimeError> {
        if let Some(mut search) = self.search.take() {
            if search.entered {
                self.runtime
                    .0
                    .proxy_method_depth
                    .set(self.runtime.0.proxy_method_depth.get().saturating_sub(1));
                search.entered = false;
            }
            if let Some(rooted) = search.rooted.take() {
                rooted.release_in_state(self.state, self.runtime)?;
            }
        }
        if let Some(value) = self.value.take() {
            self.state
                .release_owned_jsvalue(&self.runtime.0.poisoned, value)?;
        }
        Ok(())
    }
}
impl Drop for Scope<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.runtime.0.poisoned.set(true);
        }
        if self.runtime.0.poisoned.get() {
            if self
                .search
                .as_mut()
                .is_some_and(|s| std::mem::replace(&mut s.entered, false))
            {
                self.runtime
                    .0
                    .proxy_method_depth
                    .set(self.runtime.0.proxy_method_depth.get().saturating_sub(1));
            }
            return;
        }
        let _unwind =
            crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(&self.runtime.0.poisoned);
        if self.cleanup().is_err() {
            self.runtime.0.poisoned.set(true);
        }
    }
}
fn native_throw(
    state: &mut RuntimeState,
    runtime: &Runtime,
    realm: ContextId,
    kind: NativeErrorKind,
    message: &'static str,
) -> Result<StateMethodStep, RuntimeError> {
    Ok(StateMethodStep::Throw(JsValue::Object(
        state.new_native_error_from_message(
            &runtime.0.poisoned,
            realm,
            kind,
            NativeErrorMessage::from_utf8(message),
        )?,
    )))
}

pub(super) fn release_read_legacy(runtime: &Runtime, effect: StateReadEffect, atom: Atom) {
    runtime.release_atom_handle(atom);
    match effect {
        StateReadEffect::Getter {
            callee: object,
            receiver,
        }
        | StateReadEffect::Proxy { object, receiver } => {
            let _ = runtime.release_jsvalue(JsValue::Object(object));
            let _ = runtime.release_jsvalue(receiver);
        }
        StateReadEffect::Shared(_) => {}
        StateReadEffect::Get(effect) => {
            if !runtime.skip_cleanup() {
                let _ = effect.release_in_state(&mut runtime.0.state.borrow_mut(), runtime);
            }
        }
    }
}

impl Runtime {
    /// The owning legacy boundary consumes a selected effect directly. No
    /// getter or handler property is searched again between selection and call.
    pub(in crate::engine::object::internal_methods) fn finish_selected_method_read(
        &self,
        realm: ContextId,
        effect: StateReadEffect,
        atom: Atom,
    ) -> Result<Completion, RuntimeError> {
        match effect {
            StateReadEffect::Getter { callee, receiver } => {
                self.release_atom_handle(atom);
                let callable = CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                    self.clone(),
                    callee,
                ));
                self.call_internal_jsvalue(realm, &callable, receiver, Vec::new())
            }
            StateReadEffect::Proxy { object, receiver } => {
                let object = ObjectRef::from_owned_handle(self.clone(), object);
                let key = crate::engine::object::PropertyKey::from_owned_atom(self.clone(), atom);
                self.proxy_get_jsvalue(realm, &object, &key, receiver)
            }
            StateReadEffect::Get(effect) => {
                self.release_atom_handle(atom);
                self.finish_proxy_get_effect(realm, effect)
            }
            StateReadEffect::Shared(read) => {
                self.release_atom_handle(atom);
                let (element, bytes) = read.read()?;
                Ok(Completion::Return(
                    self.0
                        .state
                        .borrow_mut()
                        .decode_typed_index(element, bytes)?,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{api::Value, heap::RawId};

    #[test]
    fn synchronous_proxy_handler_read_does_not_publish_method_or_get_resume() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(proxy) = context
            .eval("new Proxy({x:1},new Proxy(new Proxy({set:undefined},{}),{}))")
            .unwrap()
        else {
            panic!()
        };
        let strong = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut state = runtime.0.state.borrow_mut();
        let StateMethodStep::Complete(selection) = StateMethodStep::start(
            &runtime,
            &mut state,
            context.realm,
            proxy.object_id(),
            "set",
        )
        .unwrap() else {
            panic!("synchronous handler forwarding")
        };
        assert!(selection.target.is_none());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        selection.release_in_state(&mut state, &runtime).unwrap();
        #[cfg(feature = "profiling")]
        for event in ["method_resume_allocation", "get_resume_allocation"] {
            assert!(
                !profile
                    .snapshot()
                    .owned_execution_events
                    .contains_key(event)
            );
        }
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn synchronous_method_prefix_has_no_runtime_owner_or_wait_record() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(proxy) = context.eval("new Proxy(new Proxy({x:1},{}),{})").unwrap()
        else {
            panic!()
        };
        let strong = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut state = runtime.0.state.borrow_mut();
        let step = StateMethodStep::start(
            &runtime,
            &mut state,
            context.realm,
            proxy.object_id(),
            "get",
        )
        .unwrap();
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
        let StateMethodStep::Complete(selection) = step else {
            panic!("synchronous forwarding")
        };
        assert!(selection.target.is_none());
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        selection.release_in_state(&mut state, &runtime).unwrap();
        #[cfg(feature = "profiling")]
        assert!(
            !profile
                .snapshot()
                .owned_execution_events
                .contains_key("method_resume_allocation")
        );
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn selected_method_getter_survives_reconfiguration_without_relookup() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(proxy) = context.eval("globalThis.methodTrace='';globalThis.oldTrap=function(){};globalThis.methodHandler={get get(){methodTrace+='old';return oldTrap}};new Proxy({},methodHandler)").unwrap() else { panic!() };
        let step = StateMethodStep::start(
            &runtime,
            &mut runtime.0.state.borrow_mut(),
            context.realm,
            proxy.object_id(),
            "get",
        )
        .unwrap();
        let StateMethodStep::Read {
            effect,
            atom,
            resume,
        } = step
        else {
            panic!("real getter")
        };
        assert_eq!(runtime.0.proxy_method_depth.get(), 1);
        drop(context.eval("Object.defineProperty(methodHandler,'get',{get(){methodTrace+='new';return function(){}}})").unwrap());
        let completion = runtime
            .finish_selected_method_read(context.realm, effect, atom)
            .unwrap();
        let Value::Object(expected) = context.eval("oldTrap").unwrap() else {
            panic!()
        };
        let mut state = runtime.0.state.borrow_mut();
        let step = resume.resume(&runtime, &mut state, completion).unwrap();
        let StateMethodStep::Complete(selection) = step else {
            panic!("selected method")
        };
        assert!(
            matches!(selection.target.as_ref(),Some(StateMethodTarget::Callable(id)) if *id==expected.object_id())
        );
        selection.release_in_state(&mut state, &runtime).unwrap();
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        drop(state);
        assert_eq!(
            context.eval("methodTrace").unwrap(),
            Value::String(crate::engine::value::JsString::from_static("old"))
        );
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn method_cancellation_and_failed_retain_restore_edges_and_depth() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(proxy) = context
            .eval("new Proxy({}, {get get(){return undefined}})")
            .unwrap()
        else {
            panic!()
        };
        let mut state = runtime.0.state.borrow_mut();
        let ObjectPayload::Proxy(data) = state.heap.object(proxy.object_id()).unwrap().payload
        else {
            panic!()
        };
        let before = state.heap.object_strong_count(data.target).unwrap();
        let step = StateMethodStep::start(
            &runtime,
            &mut state,
            context.realm,
            proxy.object_id(),
            "get",
        )
        .unwrap();
        assert_eq!(runtime.0.proxy_method_depth.get(), 1);
        step.release_in_state(&runtime, &mut state).unwrap();
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        assert_eq!(state.heap.object_strong_count(data.target).unwrap(), before);
        state
            .heap
            .set_strong_count_for_test(RawId::Object(data.handler), u32::MAX);
        let result = StateMethodStep::start(
            &runtime,
            &mut state,
            context.realm,
            proxy.object_id(),
            "get",
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(data.handler), 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(
                crate::engine::heap::HeapError::Overflow { .. }
            ))
        ));
        assert_eq!(state.heap.object_strong_count(data.target).unwrap(), before);
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        assert!(!runtime.is_poisoned());
        assert!(!runtime.0.deferred_references.has_pending());
    }
}
