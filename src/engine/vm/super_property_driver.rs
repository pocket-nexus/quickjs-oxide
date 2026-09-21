//! Preserve the pinned super read/call/write ordering around owned key conversion.
use super::{
    Completion, driver::CallStep, exception::runtime_error_to_vm_error,
    execution::RunningExecution, frame::FrameId,
};
use crate::engine::{
    api::{Error, ErrorKind, runtime::Runtime},
    value::{JsValue, Value, conversion::NativeConversion},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Read,
    Call,
    Write,
}
pub(super) struct Input {
    receiver: JsValue,
    base: JsValue,
    pub key: JsValue,
    value: Option<JsValue>,
    kind: Kind,
    depth: usize,
}
pub(super) enum Progress {
    Convert(Box<Input>),
    Call(CallStep),
}
impl Input {
    #[cfg(feature = "profiling")]
    pub(super) fn operand_count(&self) -> usize {
        if self.kind == Kind::Write { 4 } else { 3 }
    }

    /// Release the owned internal edges an abandoned conversion still holds.
    pub(super) fn release_edges(self, runtime: &Runtime) {
        let _ = runtime.release_jsvalue(self.receiver);
        let _ = runtime.release_jsvalue(self.base);
        let _ = runtime.release_jsvalue(self.key);
        if let Some(value) = self.value {
            let _ = runtime.release_jsvalue(value);
        }
    }
}
pub(super) fn start(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: Kind,
) -> Result<Progress, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let value = if kind == Kind::Write {
        Some(execution.slots.pop(&mut frame.window)?)
    } else {
        None
    };
    let key = execution.slots.pop(&mut frame.window)?;
    let base = execution.slots.pop(&mut frame.window)?;
    let receiver = execution.slots.pop(&mut frame.window)?;
    let depth = execution.slots.depth(&frame.window);
    // PutSuperValue rejects the base before converting a raw key, after RHS.
    // At a call site QuickJS uses ordinary GetArrayEl's nullish precheck.
    let error = if kind == Kind::Write && !matches!(base, JsValue::Object(_)) {
        Some("not an object")
    } else if kind == Kind::Call && matches!(base, JsValue::Null | JsValue::Undefined) {
        Some(if matches!(base, JsValue::Null) {
            "cannot read property of null"
        } else {
            "cannot read property of undefined"
        })
    } else {
        None
    };
    if let Some(message) = error {
        if let Some(value) = value {
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
        }
        runtime
            .release_jsvalue(key)
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .release_jsvalue(base)
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .release_jsvalue(receiver)
            .map_err(runtime_error_to_vm_error)?;
        return super::property_driver::throw_error(
            runtime,
            realm,
            Error::new(ErrorKind::Type, message),
        )
        .map(Progress::Call);
    }
    let input = Box::new(Input {
        receiver,
        base,
        key,
        value,
        kind,
        depth,
    });
    if matches!(input.key, JsValue::Object(_)) {
        Ok(Progress::Convert(input))
    } else {
        converted(runtime, execution, id, input).map(Progress::Call)
    }
}
/// Release the owned internal edges of a conversion that ends before the
/// read/write handler takes them over.
fn release_abandoned_operands(
    runtime: &Runtime,
    receiver: JsValue,
    base: JsValue,
    value: Option<JsValue>,
) -> Result<(), Error> {
    if let Some(assigned) = value {
        runtime
            .release_jsvalue(assigned)
            .map_err(runtime_error_to_vm_error)?;
    }
    runtime
        .release_jsvalue(receiver)
        .map_err(runtime_error_to_vm_error)?;
    runtime
        .release_jsvalue(base)
        .map_err(runtime_error_to_vm_error)?;
    Ok(())
}

// The pending super-property conversion transfers its existing box directly to this consuming handler.
#[allow(clippy::boxed_local)]
pub(super) fn converted(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    input: Box<Input>,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let strict = frame.executable.metadata.strict;
    let Input {
        receiver,
        base,
        key,
        value,
        kind,
        depth,
    } = *input;
    if matches!(key, JsValue::Object(_)) {
        return Err(Error::internal("super key conversion returned an object"));
    }
    let conversion = runtime.native_to_property_key_jsvalue(realm, key);
    let key = match conversion {
        Ok(NativeConversion::Value(key)) => key,
        Ok(NativeConversion::Throw(thrown)) => {
            let thrown = runtime
                .into_jsvalue(thrown)
                .map_err(runtime_error_to_vm_error)?;
            release_abandoned_operands(runtime, receiver, base, value)?;
            return Ok(CallStep::Complete(Completion::Throw(thrown)));
        }
        Err(error) => {
            release_abandoned_operands(runtime, receiver, base, value)?;
            return Err(runtime_error_to_vm_error(error));
        }
    };
    if kind == Kind::Write {
        let JsValue::Object(base) = base else {
            return Err(Error::internal("super write lost its validated base"));
        };
        let base = crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), base);
        let value = value.ok_or_else(|| Error::internal("super write lost its value"))?;
        return super::proxy_get_driver::start_write(
            runtime, execution, id, base, key, value, receiver, strict, depth,
        );
    }
    if let JsValue::Object(object) = base {
        let object = crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), object);
        if kind == Kind::Call {
            let frame = execution.frames.current_mut(id)?;
            execution.slots.push(&mut frame.window, receiver)?;
            let getter_receiver = Value::Object(object.clone());
            return super::proxy_get_driver::start_owned_read(
                runtime,
                execution,
                id,
                object,
                key,
                getter_receiver,
                depth,
            );
        }
        let getter_receiver = runtime
            .root_and_release_jsvalue(receiver)
            .map_err(runtime_error_to_vm_error)?;
        return super::proxy_get_driver::start_owned_read(
            runtime,
            execution,
            id,
            object,
            key,
            getter_receiver,
            depth,
        );
    }
    if kind == Kind::Read {
        let suffix = match base {
            JsValue::Null => "' of null",
            JsValue::Undefined => "' of undefined",
            _ => {
                runtime
                    .release_jsvalue(receiver)
                    .map_err(runtime_error_to_vm_error)?;
                runtime
                    .release_jsvalue(base)
                    .map_err(runtime_error_to_vm_error)?;
                return super::property_driver::throw_error(
                    runtime,
                    realm,
                    Error::new(ErrorKind::Type, "not an object"),
                );
            }
        };
        let error = runtime
            .native_atom_error(ErrorKind::Type, "cannot read property '", &key, suffix)
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .release_jsvalue(receiver)
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .release_jsvalue(base)
            .map_err(runtime_error_to_vm_error)?;
        return super::property_driver::throw_error(runtime, realm, error);
    }
    let read = runtime.prepare_value_property_read_borrowed_jsvalue(realm, &base, &key);
    let read = match read {
        Ok(read) => read,
        Err(error) => {
            return super::property_driver::throw_error(
                runtime,
                realm,
                runtime_error_to_vm_error(error),
            );
        }
    };
    super::property_driver::read_prepared(
        runtime,
        execution,
        id,
        receiver,
        key,
        read,
        None,
        kind == Kind::Call,
        0,
        depth,
    )
}
