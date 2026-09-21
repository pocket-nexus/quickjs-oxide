//! Transfer assignment inputs once, then let the object protocol own the write.
use super::{
    Completion, driver::CallStep, exception::runtime_error_to_vm_error,
    execution::RunningExecution, frame::FrameId, property_driver::PropertyProgress,
};
use crate::engine::{
    api::{Error, ErrorKind, runtime::Runtime},
    object::PropertyKey,
    value::{JsValue, conversion::NativeConversion},
};

pub(super) struct ConvertedWrite {
    pub base: Option<JsValue>,
    pub key: Option<JsValue>,
    pub value: Option<JsValue>,
    pub runtime: Runtime,
}

impl Drop for ConvertedWrite {
    fn drop(&mut self) {
        for value in [self.base.take(), self.key.take(), self.value.take()]
            .into_iter()
            .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}

pub(super) fn write(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    static_key: Option<u32>,
) -> Result<CallStep, Error> {
    write_progress(runtime, execution, frame, static_key).map(PropertyProgress::into_call_step)
}

pub(super) fn write_progress(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    static_key: Option<u32>,
) -> Result<PropertyProgress, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let depth = execution.slots.depth(&parent.window);
    let key = if let Some(index) = static_key {
        let atom = parent
            .executable
            .property_key_atoms
            .as_ref()
            .and_then(|atoms| atoms.get(index as usize))
            .copied()
            .filter(|atom| !atom.is_null())
            .ok_or_else(|| Error::internal("property write has no linked key"))?;
        PropertyKey::from_borrowed_atom(runtime.clone(), atom)
            .map_err(|error| Error::internal(error.to_string()))?
    } else {
        let value = runtime
            .dup_jsvalue(execution.slots.peek(&parent.window, 1)?)
            .map_err(runtime_error_to_vm_error)?;
        if matches!(value, JsValue::Object(_)) {
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            return Err(Error::internal("object write key did not enter conversion"));
        }
        match runtime
            .native_to_property_key_jsvalue(realm, value)
            .map_err(runtime_error_to_vm_error)?
        {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(PropertyProgress::Deferred(CallStep::Complete(
                    Completion::Throw(value),
                )));
            }
        }
    };
    let (base, value, discarded_key) = {
        // Key conversion has completed; authenticate this no-callback owner
        // transfer once and end the borrow before entering object storage.
        let mut slots = execution.slots.run_window(&mut parent.window)?;
        slots.peek(if static_key.is_none() { 2 } else { 1 })?;
        let value = slots.pop()?;
        let discarded_key = if static_key.is_none() {
            Some(slots.pop()?)
        } else {
            None
        };
        (slots.pop()?, value, discarded_key)
    };
    if let Some(discarded_key) = discarded_key {
        if let Err(error) = runtime.release_jsvalue(discarded_key) {
            let _ = runtime.release_jsvalue(value);
            let _ = runtime.release_jsvalue(base);
            return Err(runtime_error_to_vm_error(error));
        }
    }
    dispatch(runtime, execution, frame, base, key, value, depth)
}

// Keep the conversion operand bundle boxed until its consuming write handler, rather than copying it through dispatch.
#[allow(clippy::boxed_local)]
pub(super) fn converted(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    mut input: Box<ConvertedWrite>,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let depth = execution.slots.depth(&parent.window) + 3;
    let key = input.key.take().expect("converted write key");
    if matches!(key, JsValue::Object(_)) {
        runtime
            .release_jsvalue(key)
            .map_err(runtime_error_to_vm_error)?;
        return Err(Error::internal("write key conversion returned an object"));
    }
    let key = match runtime
        .native_to_property_key_jsvalue(realm, key)
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(key) => key,
        NativeConversion::Throw(value) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
    };
    let base = input.base.take().expect("converted write base");
    let value = input.value.take().expect("converted write owns its value");
    dispatch(runtime, execution, frame, base, key, value, depth)
        .map(PropertyProgress::into_call_step)
}

fn dispatch(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    base: JsValue,
    key: PropertyKey,
    value: JsValue,
    depth: usize,
) -> Result<PropertyProgress, Error> {
    let parent = match execution.frames.current_mut(frame) {
        Ok(parent) => parent,
        Err(error) => {
            let _ = runtime.release_jsvalue(value);
            let _ = runtime.release_jsvalue(base);
            return Err(error);
        }
    };
    let realm = parent.executable.realm;
    let strict = parent.executable.metadata.strict;
    let object = match &base {
        JsValue::Object(_) => {
            return super::proxy_get_driver::start_receiver_write_progress(
                runtime, execution, frame, key, value, base, strict, depth,
            );
        }
        JsValue::Null | JsValue::Undefined => {
            let suffix = if matches!(base, JsValue::Null) {
                "' of null"
            } else {
                "' of undefined"
            };
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            runtime
                .release_jsvalue(base)
                .map_err(runtime_error_to_vm_error)?;
            let error = runtime
                .native_atom_error(ErrorKind::Type, "cannot set property '", &key, suffix)
                .map_err(runtime_error_to_vm_error)?;
            return super::property_driver::throw_error(runtime, realm, error)
                .map(PropertyProgress::Deferred);
        }
        primitive => {
            use crate::engine::builtins::native::PrimitiveKind;
            let kind = match primitive {
                JsValue::Bool(_) => PrimitiveKind::Boolean,
                JsValue::Int(_) | JsValue::Float(_) => PrimitiveKind::Number,
                JsValue::String(_) => PrimitiveKind::String,
                JsValue::BigInt(_) | JsValue::ShortBigInt(_) => PrimitiveKind::BigInt,
                JsValue::Symbol(_) => PrimitiveKind::Symbol,
                _ => unreachable!(),
            };
            match runtime.primitive_prototype_for_realm(realm, kind) {
                Ok(object) => object,
                Err(error) => {
                    let _ = runtime.release_jsvalue(value);
                    let _ = runtime.release_jsvalue(base);
                    return Err(runtime_error_to_vm_error(error));
                }
            }
        }
    };
    super::proxy_get_driver::start_write_progress(
        runtime, execution, frame, object, key, value, base, strict, depth,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::value::Value;

    #[test]
    fn borrowed_set_vm_keeps_selected_callbacks_and_strict_rejection() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                let trace = '';
                const target = Object.create({set x(v) { trace += 's' + v; }});
                target[{toString() { trace += 'k'; return 'x'; }}] = 7;
                const proxy = new Proxy({}, {
                    set(t, k, v, receiver) {
                        trace += 'p' + v;
                        return Reflect.set(t, k, v, receiver);
                    }
                });
                proxy.x = 9;
                const frozen = Object.freeze({x: 1});
                frozen.x = 2;
                try { (function() { 'use strict'; frozen.x = 3; })(); }
                catch (e) { trace += e instanceof TypeError ? 't' : '?'; }
                return trace === 'ks7p9t' && proxy.x === 9 && frozen.x === 1;
            })()
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn borrowed_set_vm_preserves_typed_conversion_reentry_and_throw() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                const target = new Uint8Array(1), marker = {};
                let calls = 0;
                target[0] = {valueOf() { calls++; target[0] = 8; return 257; }};
                try { target[0] = {valueOf() { calls++; throw marker; }}; }
                catch (e) { if (e !== marker) return false; }
                return calls === 2 && target[0] === 1;
            })()
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}
