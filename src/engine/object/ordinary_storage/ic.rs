//! Promote a location-cache hit without draining runtime cleanup or invoking JS.
use super::{LinkedNativeSelection, linked_field_atom};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{ObjectPayload, RawValue, SlotReleaseReadiness};
use crate::engine::value::Value;

impl Runtime {
    /// A miss only records a location and leaves the canonical read untouched.
    /// Native classification, when requested, describes this retained result;
    /// it never caches a value or outlives the result's ordinary slot owner.
    pub(crate) fn try_property_ic_read_owned(
        &self,
        base: &Value,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Result<Option<Value>, RuntimeError> {
        let Some(atom) = linked_field_atom(self, executable, key_index) else {
            return Ok(None);
        };
        let Some(cache) = executable.property_read_ic.site(pc) else {
            return Ok(None);
        };
        if !keep_receiver && self.0.deferred_references.has_pending() {
            return Ok(None);
        }
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return Ok(None);
        };
        if !keep_receiver && state.heap.has_pending_zero_cleanup() {
            return Ok(None);
        }
        let receiver = match base {
            Value::Object(object) if object.belongs_to(self) => object.object_id(),
            Value::Object(_) => return Ok(None),
            _ => {
                cache.miss(
                    &state.heap,
                    &state.atoms,
                    self.domain_id(),
                    executable.realm,
                    None,
                    atom,
                );
                return Ok(None);
            }
        };
        // Prove replacement cannot release the last receiver owner BEFORE
        // promoting a result. The proof and retain share this state borrow.
        if !keep_receiver
            && state.heap.slot_object_release_readiness(receiver)? != SlotReleaseReadiness::Ready
        {
            return Ok(None);
        }
        let Some(raw) = cache.read(&state.heap, self.domain_id(), executable.realm, receiver)
        else {
            cache.miss(
                &state.heap,
                &state.atoms,
                self.domain_id(),
                executable.realm,
                Some(receiver),
                atom,
            );
            return Ok(None);
        };
        if matches!(
            raw,
            RawValue::Private(_) | RawValue::Uninitialized | RawValue::Exception
        ) {
            return Ok(None);
        }
        let raw = raw.clone();
        let selected = if keep_receiver {
            if let RawValue::Object(function) = &raw {
                state.heap.object(*function).ok().and_then(|object| {
                    let ObjectPayload::NativeFunction { data, .. } = &object.payload else {
                        return None;
                    };
                    let realm = data.realm?;
                    (data.operation().is_some() && state.heap.context(realm).is_ok())
                        .then_some((*function, *data))
                })
            } else {
                None
            }
        } else {
            None
        };
        // String/BigInt clone their backing owner; Object/Symbol retain checks
        // overflow before changing the count. No public value conversion below
        // can fail after the sentinel exclusion above.
        state.retain_raw_root(&raw)?;
        drop(state);
        let value = self.take_owned_raw_value(raw)?;
        *native = selected.map(|(function, data)| LinkedNativeSelection {
            runtime: self.clone(),
            function,
            data,
        });
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_ic.hit");
        Ok(Some(value))
    }

    /// Trusted shared-borrow data-property read.
    ///
    /// Covers the location-cache hit for a live receiver without a mutable
    /// state borrow or fallible plumbing. Symbols need an atom-table retain
    /// (S1b) and every non-data or non-cached case declines with `None`, so the
    /// caller keeps its canonical `try_property_ic_read_owned` fallback. A
    /// declined read claims no owner.
    #[inline]
    pub(crate) fn property_ic_read_fast(
        &self,
        base: &Value,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Option<Value> {
        let atom = linked_field_atom(self, executable, key_index)?;
        let cache = executable.property_read_ic.site(pc)?;
        if !keep_receiver && self.0.deferred_references.has_pending() {
            return None;
        }
        let state = self.0.state.try_borrow().ok()?;
        if !keep_receiver && state.heap.has_pending_zero_cleanup() {
            return None;
        }
        let receiver = match base {
            Value::Object(object) if object.belongs_to(self) => object.object_id(),
            Value::Object(_) => return None,
            _ => {
                cache.miss(
                    &state.heap,
                    &state.atoms,
                    self.domain_id(),
                    executable.realm,
                    None,
                    atom,
                );
                return None;
            }
        };
        if !keep_receiver
            && state.heap.slot_object_release_readiness_fast(receiver)
                != SlotReleaseReadiness::Ready
        {
            return None;
        }
        let Some(raw) = cache.read(&state.heap, self.domain_id(), executable.realm, receiver)
        else {
            cache.miss(
                &state.heap,
                &state.atoms,
                self.domain_id(),
                executable.realm,
                Some(receiver),
                atom,
            );
            return None;
        };
        match raw {
            RawValue::Object(function) => {
                let selected = if keep_receiver {
                    let object = state.heap.object_fast(*function);
                    match &object.payload {
                        ObjectPayload::NativeFunction { data, .. } => {
                            data.realm.and_then(|realm| {
                                (data.operation().is_some() && state.heap.context(realm).is_ok())
                                    .then_some((*function, *data))
                            })
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                state.heap.retain_object_fast(*function);
                *native = selected.map(|(function, data)| LinkedNativeSelection {
                    runtime: self.clone(),
                    function,
                    data,
                });
                Some(self.take_owned_raw_value_fast(RawValue::Object(*function)))
            }
            RawValue::String(value) => {
                Some(self.take_owned_raw_value_fast(RawValue::String(value.clone())))
            }
            RawValue::BigInt(value) => {
                Some(self.take_owned_raw_value_fast(RawValue::BigInt(value.clone())))
            }
            RawValue::Symbol(atom) => {
                state.atoms.retain(*atom).ok()?;
                Some(self.take_owned_raw_value_fast(RawValue::Symbol(*atom)))
            }
            RawValue::Undefined
            | RawValue::Null
            | RawValue::Bool(_)
            | RawValue::Int(_)
            | RawValue::Float(_) => Some(self.take_owned_raw_value_fast(raw.clone())),
            RawValue::Private(_) | RawValue::Uninitialized | RawValue::Exception => None,
        }
    }
}

impl Runtime {
    pub(crate) fn try_property_ic_write_owned(
        &self,
        base: &Value,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key: u32,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        let Some(atom) = linked_field_atom(self, executable, key) else {
            return Ok(false);
        };
        let Value::Object(object) = base else {
            return Ok(false);
        };
        if !object.belongs_to(self) {
            return Ok(false);
        }
        let Some(cache) = executable.property_read_ic.write_site(pc) else {
            return Ok(false);
        };
        let raw = self.raw_property_value(value)?;
        let mut state = self.0.state.borrow_mut();
        let id = object.object_id();
        let slot = match cache.slot(&state.heap, self.domain_id(), executable.realm, id) {
            Some(slot) => slot,
            None => {
                cache.miss(
                    &state.heap,
                    &state.atoms,
                    self.domain_id(),
                    executable.realm,
                    id,
                    atom,
                );
                let Some(slot) = cache.slot(&state.heap, self.domain_id(), executable.realm, id)
                else {
                    return Ok(false);
                };
                slot
            }
        };
        // Input owners remain rooted; retain the new value before releasing the
        // old edge. The caller has ended RunSlots and published the current PC.
        state.replace_property_slot(id, slot, crate::engine::heap::PropertySlot::Data(raw))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_write_ic.hit");
        Ok(true)
    }

    pub(crate) fn try_dense_array_write_owned(
        &self,
        base: &Value,
        index: u32,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        let Value::Object(object) = base else {
            return Ok(false);
        };
        if !object.belongs_to(self) {
            return Ok(false);
        }
        let raw = self.raw_property_value(value)?;
        let mut state = self.0.state.borrow_mut();
        let data = state.heap.object(object.object_id())?;
        if data.kind != crate::engine::heap::ObjectKind::Array
            || data.dense_array_value(index).is_none()
        {
            return Ok(false);
        }
        let atoms = state.retain_raw_value_atoms([&raw])?;
        match state
            .heap
            .replace_array_dense_value(object.object_id(), index, raw)
        {
            Ok(cleanup) => state.apply_cleanup(cleanup)?,
            Err(error) => {
                state.release_atoms(atoms)?;
                return Err(error.into());
            }
        }
        Ok(true)
    }

    pub(crate) fn try_define_field_owned(
        &self,
        base: &Value,
        executable: &PublishedFunctionSnapshot,
        key: u32,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        let Some(atom) = linked_field_atom(self, executable, key) else {
            return Ok(false);
        };
        let Value::Object(object) = base else {
            return Ok(false);
        };
        if !object.belongs_to(self) {
            return Ok(false);
        }
        let raw = self.raw_property_value(value)?;
        let mut state = self.0.state.borrow_mut();
        let data = state.heap.object(object.object_id())?;
        if !super::is_ordinary(data)
            || !data.extensible
            || state.heap.shape(data.shape)?.find(atom).is_some()
        {
            return Ok(false);
        }
        state.store_selected_property_slot(
            object.object_id(),
            atom,
            crate::engine::object::shape::PropertyFlags::data(true, true, true),
            crate::engine::heap::PropertySlot::Data(raw),
            None,
        )?;
        Ok(true)
    }

    pub(crate) fn try_delete_own_data(
        &self,
        base: &Value,
        key: &crate::engine::object::PropertyKey,
    ) -> Result<Option<bool>, RuntimeError> {
        let Value::Object(object) = base else {
            return Ok(None);
        };
        if !object.belongs_to(self) {
            return Ok(None);
        }
        {
            let state = self.0.state.borrow();
            let data = state.heap.object(object.object_id())?;
            if !super::is_ordinary(data) {
                return Ok(None);
            }
            let shape = state.heap.shape(data.shape)?;
            let Some(slot) = shape.find(key.atom()) else {
                return Ok(Some(true));
            };
            if !shape.entries()[slot as usize].flags.configurable
                || !matches!(
                    data.slots[slot as usize],
                    crate::engine::heap::PropertySlot::Data(_)
                )
            {
                return Ok(None);
            }
        }
        self.delete_property(object, key).map(Some)
    }
}

impl Runtime {
    pub(crate) fn try_dense_array_kept_read(&self, base: &Value, index: u32) -> Option<Value> {
        let Value::Object(object) = base else {
            return None;
        };
        if !object.belongs_to(self) {
            return None;
        }
        let state = self.0.state.borrow();
        let data = state.heap.object(object.object_id()).ok()?;
        if data.kind != crate::engine::heap::ObjectKind::Array {
            return None;
        }
        super::immediate_value(data.dense_array_value(index)?)
    }
}

impl Runtime {
    pub(crate) fn try_property_ic_write_scalar(
        &self,
        base: &Value,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key: u32,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        if !matches!(
            value,
            Value::Undefined | Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_)
        ) || self.slot_value_release_readiness(base)? != SlotReleaseReadiness::Ready
        {
            return Ok(false);
        }
        let Some(atom) = linked_field_atom(self, executable, key) else {
            return Ok(false);
        };
        let Value::Object(object) = base else {
            return Ok(false);
        };
        if !object.belongs_to(self) {
            return Ok(false);
        }
        let Some(cache) = executable.property_read_ic.write_site(pc) else {
            return Ok(false);
        };
        let mut state = self.0.state.borrow_mut();
        let id = object.object_id();
        let slot = match cache.slot(&state.heap, self.domain_id(), executable.realm, id) {
            Some(slot) => slot,
            None => {
                cache.miss(
                    &state.heap,
                    &state.atoms,
                    self.domain_id(),
                    executable.realm,
                    id,
                    atom,
                );
                let Some(slot) = cache.slot(&state.heap, self.domain_id(), executable.realm, id)
                else {
                    return Ok(false);
                };
                slot
            }
        };
        let crate::engine::heap::PropertySlot::Data(old) = &state.heap.object(id)?.slots[slot]
        else {
            return Ok(false);
        };
        if super::immediate_value(old).is_none() {
            return Ok(false);
        }
        let raw = self.raw_property_value(value)?;
        state.replace_property_slot(id, slot, crate::engine::heap::PropertySlot::Data(raw))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_write_ic.hit");
        Ok(true)
    }
}

impl Runtime {
    pub(crate) fn try_dense_array_write_scalar(
        &self,
        base: &Value,
        index: u32,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        if !matches!(
            value,
            Value::Undefined | Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_)
        ) || self.slot_value_release_readiness(base)? != SlotReleaseReadiness::Ready
        {
            return Ok(false);
        }
        if self.try_dense_array_kept_read(base, index).is_none() {
            return Ok(false);
        }
        self.try_dense_array_write_owned(base, index, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::bytecode::Instruction;

    fn object(value: &Value) -> &crate::engine::object::ObjectRef {
        let Value::Object(object) = value else {
            panic!("object")
        };
        object
    }

    fn site(runtime: &Runtime) -> (PublishedFunctionSnapshot, usize, u32) {
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("(function(o){return o.x})").unwrap())
            .unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode")
        };
        let executable = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        let (pc, key) = executable
            .code
            .iter()
            .enumerate()
            .find_map(|(pc, op)| match op {
                Instruction::GetField(key) => Some((pc, *key)),
                _ => None,
            })
            .unwrap();
        (executable, pc, key)
    }

    #[test]
    fn owned_ic_promotes_every_public_value_and_reads_current_slot() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for expression in [
            "undefined",
            "null",
            "true",
            "123",
            "1.25",
            "'wide λ text'",
            "123456789012345678901234567890n",
            "Symbol('ic')",
            "({nested:7})",
        ] {
            let (code, pc, key) = site(&runtime);
            let base=context.eval(&format!("globalThis.icExpected={expression};globalThis.icHolder={{x:icExpected}};icHolder")).unwrap();
            let expected = context.eval("icExpected").unwrap();
            let mut native = None;
            assert!(
                runtime
                    .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                    .unwrap()
                    .is_none()
            );
            let actual = runtime
                .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                .unwrap()
                .unwrap();
            assert_eq!(actual, expected, "{expression}");
            context.eval("icHolder.x=99").unwrap();
            assert_eq!(
                runtime
                    .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                    .unwrap(),
                Some(Value::Int(99))
            );
            assert!(native.is_none());
        }
    }

    #[test]
    fn owned_ic_guards_borrow_deferred_work_and_final_receiver_before_promotion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let base = context.eval("({x:{marker:1}})").unwrap();
        let mut native = None;
        assert!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                .unwrap()
                .is_none()
        );
        // A cache hit must still leave the last receiver owner untouched.
        assert!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                .unwrap()
                .is_none()
        );
        let receiver = object(&base);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver.object_id())
                .unwrap(),
            1
        );
        {
            let _borrow = runtime.0.state.borrow();
            assert!(
                runtime
                    .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                    .unwrap()
                    .is_none()
            );
        }
        let released = runtime.new_object(None).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        // A kept receiver hit only retains under the exclusive heap borrow;
        // pending unrelated releases cannot mutate its guarded layout.
        let retained_hit = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap();
        assert!(matches!(retained_hit, Some(Value::Object(_))));
        assert!(runtime.0.deferred_references.has_pending());
        runtime.drain_deferred_references().unwrap();
        assert!(matches!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                .unwrap(),
            Some(Value::Object(_))
        ));
    }

    #[test]
    fn owned_ic_native_hint_is_bound_to_current_retained_function() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let base = context
            .eval("globalThis.icNative={x:Math.min};icNative")
            .unwrap();
        let mut native = None;
        assert!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                .unwrap()
                .is_none()
        );
        let first = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap()
            .unwrap();
        let hint = native.take().unwrap();
        context.eval("icNative.x=Math.max").unwrap();
        let data = hint.into_parts(object(&first)).unwrap();
        assert_eq!(
            data.target,
            crate::engine::builtins::native::NativeFunctionId::MathMinMax(
                crate::engine::builtins::native::MathMinMaxKind::Min
            )
        );
        let second = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap()
            .unwrap();
        let hint = native.take().unwrap();
        assert!(hint.into_parts(object(&first)).is_none());
        assert_ne!(first, second);
    }
}
