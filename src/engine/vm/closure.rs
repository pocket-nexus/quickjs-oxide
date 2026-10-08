//! Immutable closure indices. The callee, not this Rc slice, owns the cells.
use crate::engine::api::{Runtime, runtime_error::RuntimeError};
use crate::engine::heap::{ObjectId, runtime::RuntimeState};
use crate::engine::heap::{
    VarRefId,
    roots::{VarRefRoot, VarRefView},
};
use crate::engine::object::ObjectRef;
use std::rc::Rc;

pub(crate) struct ClosureSlots(Environment);

enum Environment {
    Shared {
        owner: ObjectRef,
        ids: Rc<[VarRefId]>,
    },
    // Materialized/synthetic external entry keeps its independent ownership.
    Rooted(Vec<VarRefRoot>),
    /// The frame's callee owns these cells. An unrelated synthetic environment
    /// keeps its distinct supporting object edge until the frame is released.
    ResidentShared {
        supporting_owner: Option<ObjectId>,
        ids: Rc<[VarRefId]>,
    },
    ResidentOwned(Vec<VarRefId>),
}
impl Default for ClosureSlots {
    fn default() -> Self {
        Self(Environment::Rooted(Vec::new()))
    }
}
impl From<Vec<VarRefRoot>> for ClosureSlots {
    fn from(roots: Vec<VarRefRoot>) -> Self {
        Self(Environment::Rooted(roots))
    }
}
impl ClosureSlots {
    pub(super) fn shared(owner: ObjectRef, ids: Rc<[VarRefId]>) -> Self {
        Self(Environment::Shared { owner, ids })
    }
    pub(crate) fn len(&self) -> usize {
        match &self.0 {
            Environment::Shared { ids, .. } => ids.len(),
            Environment::Rooted(roots) => roots.len(),
            Environment::ResidentShared { ids, .. } => ids.len(),
            Environment::ResidentOwned(ids) => ids.len(),
        }
    }

    pub(crate) fn borrowed_cell<'a>(
        &'a self,
        runtime: &'a Runtime,
        index: usize,
    ) -> Option<(&'a Runtime, VarRefId)> {
        match &self.0 {
            Environment::Shared { owner, ids } => ids.get(index).map(|id| (owner.runtime(), *id)),
            Environment::Rooted(roots) => roots.get(index).map(|root| (&root.runtime, root.id())),
            Environment::ResidentShared { ids, .. } => ids.get(index).map(|id| (runtime, *id)),
            Environment::ResidentOwned(ids) => ids.get(index).map(|id| (runtime, *id)),
        }
    }
    pub(crate) fn get<'a>(&'a self, runtime: &'a Runtime, index: usize) -> Option<VarRefView<'a>> {
        VarRefView::from_closure(self, runtime, index)
    }

    fn into_resident(self, runtime: &Runtime, callee: ObjectId) -> Result<Self, RuntimeError> {
        let environment = match self.0 {
            Environment::Shared { owner, ids } => {
                if !owner.belongs_to(runtime) {
                    return Err(RuntimeError::WrongRuntime("closure environment"));
                }
                let supporting_owner = if owner.object_id() == callee {
                    drop(owner);
                    None
                } else {
                    Some(owner.into_execution_handle())
                };
                Environment::ResidentShared {
                    supporting_owner,
                    ids,
                }
            }
            Environment::Rooted(roots) => {
                if roots.iter().any(|root| !root.belongs_to(runtime)) {
                    return Err(RuntimeError::WrongRuntime("closure variable"));
                }
                Environment::ResidentOwned(
                    roots
                        .into_iter()
                        .map(VarRefRoot::into_execution_handle)
                        .collect(),
                )
            }
            resident @ (Environment::ResidentShared { .. } | Environment::ResidentOwned(_)) => {
                resident
            }
        };
        Ok(Self(environment))
    }

    fn release(&mut self, state: &mut RuntimeState) -> Result<(), RuntimeError> {
        match &mut self.0 {
            Environment::ResidentShared {
                supporting_owner, ..
            } => {
                if let Some(owner) = supporting_owner.take() {
                    state.release_object_handle(owner)?;
                }
            }
            Environment::ResidentOwned(ids) => {
                for id in ids.drain(..) {
                    state.release_var_ref_handle(id)?;
                }
            }
            Environment::Shared { .. } | Environment::Rooted(_) => {
                return Err(RuntimeError::Invariant(
                    "frame retained an external closure owner",
                ));
            }
        }
        Ok(())
    }
}
/// A frame's function and environment share one callee root. A borrowed cell
/// view cannot outlive this owner; detached environments remain independently
/// rooted through ClosureSlots.
#[must_use]
pub(in crate::engine::vm) struct FrameFunction {
    owner: Option<ObjectId>,
    domain: u64,
    slots: ClosureSlots,
}
impl FrameFunction {
    pub(in crate::engine::vm) fn new(
        owner: ObjectRef,
        slots: ClosureSlots,
    ) -> Result<Self, RuntimeError> {
        let domain = owner.domain_id();
        let slots = slots.into_resident(owner.runtime(), owner.object_id())?;
        Ok(Self {
            owner: Some(owner.into_execution_handle()),
            domain,
            slots,
        })
    }
    pub(in crate::engine::vm) fn shared(domain: u64, owner: ObjectId, ids: Rc<[VarRefId]>) -> Self {
        Self {
            owner: Some(owner),
            domain,
            slots: ClosureSlots(Environment::ResidentShared {
                supporting_owner: None,
                ids,
            }),
        }
    }
    pub(in crate::engine::vm) fn closures(&self) -> &ClosureSlots {
        &self.slots
    }
    pub(in crate::engine::vm) fn object_id(&self) -> ObjectId {
        self.owner.expect("resident frame owns its callee")
    }
    pub(in crate::engine::vm) fn belongs_to(&self, runtime: &Runtime) -> bool {
        self.domain == runtime.domain_id()
    }
    /// Temporary legacy boundary root; internal storage itself owns only IDs.
    pub(in crate::engine::vm) fn to_root(
        &self,
        runtime: &Runtime,
    ) -> Result<ObjectRef, RuntimeError> {
        if !self.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("frame function"));
        }
        Ok(ObjectRef::from_borrowed_handle(
            runtime.clone(),
            self.object_id(),
        )?)
    }
    pub(in crate::engine::vm) fn release(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        self.slots.release(state)?;
        if let Some(owner) = self.owner.take() {
            state.release_object_handle(owner)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        api::{Runtime, Value},
        object::CallableRef,
        value::JsValue,
        vm::call::CallableExecution,
    };
    #[test]
    fn shared_environment_does_not_retain_each_cell_and_outlives_external_callee() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(function) = context
            .eval("(()=>{let a=1,b=2;return ()=>a+b})()")
            .unwrap()
        else {
            panic!()
        };
        let callable = CallableRef::from_validated_object(function);
        let ids = {
            let state = runtime.0.state.borrow();
            let crate::engine::heap::ObjectPayload::BytecodeFunction { closure_slots, .. } = &state
                .heap
                .object(callable.as_object().object_id())
                .unwrap()
                .payload
            else {
                panic!()
            };
            closure_slots.clone()
        };
        assert_eq!(ids.len(), 2);
        let counts: Vec<_> = ids
            .iter()
            .map(|id| {
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .var_ref_strong_count(*id)
                    .unwrap()
            })
            .collect();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!()
        };
        for (id, count) in ids.iter().zip(&counts) {
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .var_ref_strong_count(*id)
                    .unwrap(),
                *count
            );
        }
        drop(callable);
        drop(bytecode);
        assert_eq!(
            runtime
                .read_var_ref(&closure_slots.get(&runtime, 0).unwrap())
                .unwrap(),
            JsValue::Int(1)
        );
        runtime
            .write_var_ref(&closure_slots.get(&runtime, 0).unwrap(), JsValue::Int(9))
            .unwrap();
        assert_eq!(
            runtime
                .read_var_ref(&closure_slots.get(&runtime, 0).unwrap())
                .unwrap(),
            JsValue::Int(9)
        );
        let escaped = closure_slots.get(&runtime, 0).unwrap().try_clone().unwrap();
        drop(closure_slots);
        assert_eq!(runtime.read_var_ref(&escaped).unwrap(), JsValue::Int(9));
        assert!(runtime.0.state.borrow().heap.var_ref(ids[1]).is_err());
        drop(escaped);
        assert!(runtime.0.state.borrow().heap.var_ref(ids[0]).is_err());
    }

    #[test]
    fn detached_capture_edges_become_raw_frame_owners_and_release_with_current_state() {
        use crate::engine::{
            code::function::metadata::ClosureVariableKind, vm::closure::FrameFunction,
        };
        let runtime = Runtime::new();
        let before = std::rc::Rc::strong_count(&runtime.0);
        let value = runtime.new_object(None).unwrap().into_handle();
        let capture = runtime
            .new_var_ref(
                JsValue::Object(value),
                false,
                false,
                ClosureVariableKind::Normal,
            )
            .unwrap();
        let cell = capture.id();
        let function = runtime.new_object(None).unwrap();
        let callee = function.object_id();
        let mut owner = FrameFunction::new(function, vec![capture].into()).unwrap();
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), before);
        assert_eq!(owner.closures().get(&runtime, 0).unwrap().id(), cell);
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
        owner.release(&mut state).unwrap();
        assert!(state.heap.var_ref(cell).is_err());
        assert!(state.heap.object(value).is_err());
        assert!(state.heap.object(callee).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn foreign_capture_admission_releases_both_external_owners() {
        use crate::engine::{
            code::function::metadata::ClosureVariableKind, vm::closure::FrameFunction,
        };
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let function = runtime.new_object(None).unwrap();
        let callee = function.object_id();
        let capture = foreign
            .new_var_ref(JsValue::Int(7), false, false, ClosureVariableKind::Normal)
            .unwrap();
        let cell = capture.id();
        assert!(matches!(
            FrameFunction::new(function, vec![capture].into()),
            Err(crate::engine::api::RuntimeError::WrongRuntime(
                "closure variable"
            ))
        ));
        assert!(runtime.0.state.borrow().heap.object(callee).is_err());
        assert!(foreign.0.state.borrow().heap.var_ref(cell).is_err());
    }
}
