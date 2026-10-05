//! Descriptor payload ownership belongs to execution, not Runtime roots.
use super::*;
use crate::engine::heap::{ObjectId, runtime::RuntimeState};
use std::cell::Cell;

/// The field presence and already validated accessor identities survive real
/// effects. This owns each present JS edge, has no Clone/Drop, and must be
/// consumed by storage or released through the current State access.
#[must_use]
pub(crate) struct StatePropertyDescriptor {
    pub value: DescriptorField<JsValue>,
    pub writable: DescriptorField<bool>,
    pub get: DescriptorField<Option<ObjectId>>,
    pub set: DescriptorField<Option<ObjectId>>,
    pub enumerable: DescriptorField<bool>,
    pub configurable: DescriptorField<bool>,
}

impl StatePropertyDescriptor {
    pub(crate) fn raw_record(&self) -> PropertyDescriptor<RawValue> {
        PropertyDescriptor {
            value: self.value.as_ref().into_option().map(JsValue::as_raw),
            writable: self.writable.as_ref().into_option().copied(),
            get: self
                .get
                .as_ref()
                .into_option()
                .map(|value| value.map(RawValue::Object)),
            set: self
                .set
                .as_ref()
                .into_option()
                .map(|value| value.map(RawValue::Object)),
            enumerable: self.enumerable.as_ref().into_option().copied(),
            configurable: self.configurable.as_ref().into_option().copied(),
        }
    }

    pub(crate) fn release_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if let DescriptorField::Present(value) =
            std::mem::replace(&mut self.value, DescriptorField::Absent)
        {
            state.release_owned_jsvalue(poisoned, value)?;
        }
        // Keep the previous descriptor Drop order, including aliased get/set
        // owners: each field contributes one independently owned edge.
        for field in [&mut self.get, &mut self.set] {
            if let DescriptorField::Present(Some(id)) =
                std::mem::replace(field, DescriptorField::Absent)
            {
                state.release_owned_jsvalue(poisoned, JsValue::Object(id))?;
            }
        }
        Ok(())
    }

    /// Only an unmigrated exotic boundary uses the rooted ABI. The State
    /// borrow must end before its Runtime-backed cleanup guard can run.
    pub(crate) fn into_legacy(self, runtime: &Runtime) -> OwnedPropertyDescriptor {
        let accessor = |field: DescriptorField<Option<ObjectId>>| {
            field.map(|id| {
                id.map_or(AccessorValue::Undefined, |id| {
                    AccessorValue::Callable(super::super::CallableRef::from_validated_object(
                        super::super::ObjectRef::from_owned_handle(runtime.clone(), id),
                    ))
                })
            })
        };
        OwnedPropertyDescriptor {
            runtime: runtime.clone(),
            value: self.value,
            writable: self.writable,
            get: accessor(self.get),
            set: accessor(self.set),
            enumerable: self.enumerable,
            configurable: self.configurable,
        }
    }
}

impl OwnedPropertyDescriptor {
    /// Transfer existing payload owners without retaining or releasing them.
    /// The receiving guard or execution record becomes their cleanup owner.
    pub(crate) fn into_state_owned(mut self) -> StatePropertyDescriptor {
        let accessor = |field: DescriptorField<AccessorValue>| {
            field.map(|value| match value {
                AccessorValue::Undefined => None,
                AccessorValue::Callable(value) => Some(value.into_object().into_execution_handle()),
            })
        };
        StatePropertyDescriptor {
            value: std::mem::replace(&mut self.value, DescriptorField::Absent),
            writable: self.writable,
            get: accessor(std::mem::replace(&mut self.get, DescriptorField::Absent)),
            set: accessor(std::mem::replace(&mut self.set, DescriptorField::Absent)),
            enumerable: self.enumerable,
            configurable: self.configurable,
        }
    }
}

/// Local State ownership guard. Short reborrows permit canonical publication;
/// taking the descriptor moves ownership to the actual effect record.
#[must_use]
pub(crate) struct StateDescriptorGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    descriptor: Option<StatePropertyDescriptor>,
}
impl<'a> StateDescriptorGuard<'a> {
    pub(crate) fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        descriptor: StatePropertyDescriptor,
    ) -> Self {
        Self {
            state,
            poisoned,
            descriptor: Some(descriptor),
        }
    }
    pub(crate) fn parts(&mut self) -> (&mut RuntimeState, &mut Option<StatePropertyDescriptor>) {
        (self.state, &mut self.descriptor)
    }
    pub(crate) fn finish(&mut self) -> Result<(), RuntimeError> {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        if let Some(descriptor) = self.descriptor.take() {
            descriptor.release_in_state(self.state, self.poisoned)?;
        }
        Ok(())
    }
}
impl Drop for StateDescriptorGuard<'_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::heap::RawId;
    use crate::engine::object::CallableRef;
    use crate::engine::value::Value;

    #[test]
    fn descriptor_transfer_removes_runtime_owners_and_cleans_aliases_in_state() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("(function(){})").unwrap() else {
            panic!()
        };
        let id = function.object_id();
        let baseline = std::rc::Rc::strong_count(&runtime.0);
        let mut descriptor = OwnedPropertyDescriptor::data(
            &runtime,
            runtime.dup_jsvalue(&JsValue::Object(id)).unwrap(),
        );
        descriptor.get = DescriptorField::Present(AccessorValue::Callable(
            CallableRef::from_validated_object(function.try_clone().unwrap()),
        ));
        descriptor.set = DescriptorField::Present(AccessorValue::Callable(
            CallableRef::from_validated_object(function.try_clone().unwrap()),
        ));
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), baseline + 3);
        let raw = descriptor.into_state_owned();
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), baseline);
        assert!(matches!(raw.raw_record().get, Some(Some(RawValue::Object(value))) if value == id));
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(state.heap.object_strong_count(id), Ok(4));
        drop(StateDescriptorGuard::new(
            &mut state,
            &runtime.0.poisoned,
            raw,
        ));
        assert_eq!(state.heap.object_strong_count(id), Ok(1));
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn true_exotic_boundary_round_trip_transfers_edges_without_retains() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("(function(){})").unwrap() else {
            panic!()
        };
        let id = function.object_id();
        let mut descriptor = OwnedPropertyDescriptor::new(&runtime);
        descriptor.get = DescriptorField::Present(AccessorValue::Callable(
            CallableRef::from_validated_object(function.try_clone().unwrap()),
        ));
        descriptor.set = DescriptorField::Present(AccessorValue::Undefined);
        let raw = descriptor.into_state_owned();
        assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(2));
        let descriptor = raw.into_legacy(&runtime);
        assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(2));
        assert!(matches!(
            descriptor.set,
            DescriptorField::Present(AccessorValue::Undefined)
        ));
        drop(descriptor);
        assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(1));
    }

    #[test]
    fn destructive_descriptor_cleanup_quarantines_remaining_accessor_owners() {
        let runtime = Runtime::new();
        let invalid = runtime.new_object(None).unwrap().into_handle();
        let remaining = runtime.new_object(None).unwrap().into_handle();
        let descriptor = StatePropertyDescriptor {
            value: DescriptorField::Present(JsValue::Object(invalid)),
            writable: DescriptorField::Absent,
            get: DescriptorField::Present(Some(remaining)),
            set: DescriptorField::Absent,
            enumerable: DescriptorField::Absent,
            configurable: DescriptorField::Absent,
        };
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(invalid), 0);
        let mut guard = StateDescriptorGuard::new(&mut state, &runtime.0.poisoned, descriptor);
        assert!(guard.finish().is_err());
        drop(guard);
        assert!(runtime.is_poisoned());
        assert_eq!(state.heap.object_strong_count(remaining), Ok(1));
    }

    #[test]
    fn panic_poisoned_descriptor_guard_can_be_destroyed_without_a_second_panic() {
        let runtime = Runtime::new();
        let value = runtime.new_object(None).unwrap().into_handle();
        let descriptor =
            OwnedPropertyDescriptor::data(&runtime, JsValue::Object(value)).into_state_owned();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut state = runtime.0.state.borrow_mut();
            let _guard = StateDescriptorGuard::new(&mut state, &runtime.0.poisoned, descriptor);
            panic!("injected descriptor publication panic");
        }));
        assert!(caught.is_err());
        assert!(runtime.is_poisoned());
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(value),
            Ok(1)
        );
        drop(runtime);
    }
}
