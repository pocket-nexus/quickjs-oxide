//! `in` validates its RHS before key conversion; `delete` checks its base after it.
use super::{
    Completion, driver::CallStep, exception::runtime_error_to_vm_error,
    execution::RunningExecution, frame::FrameId,
};
use crate::engine::{
    api::{Error, ErrorKind, runtime::Runtime},
    object::ProxyBooleanKind,
    value::{JsValue, conversion::NativeConversion},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Instance,
    Has,
    Delete,
}
pub(super) struct Input {
    runtime: Runtime,
    base: JsValue,
    pub key: JsValue,
    kind: Kind,
    depth: usize,
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.base, JsValue::Undefined));
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.key, JsValue::Undefined));
    }
}
pub(super) enum Progress {
    Convert(Box<Input>),
    Call(CallStep),
}
pub(super) fn start(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: Kind,
) -> Result<Progress, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    for offset in 0..2 {
        execution.slots.peek(&frame.window, offset)?;
    }
    let depth = execution.slots.depth(&frame.window);
    let right = execution.slots.pop(&mut frame.window)?;
    let left = execution.slots.pop(&mut frame.window)?;
    if kind == Kind::Instance {
        let target = match right {
            JsValue::Object(target) => {
                crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), target)
            }
            value => {
                runtime
                    .release_jsvalue(value)
                    .map_err(runtime_error_to_vm_error)?;
                runtime
                    .release_jsvalue(left)
                    .map_err(runtime_error_to_vm_error)?;
                return super::property_driver::throw_error(
                    runtime,
                    realm,
                    Error::new(ErrorKind::Type, "invalid 'instanceof' right operand"),
                )
                .map(Progress::Call);
            }
        };
        return super::proxy_get_driver::start_instance(
            runtime, execution, id, left, target, depth,
        )
        .map(Progress::Call);
    }
    let (base, key) = if kind == Kind::Has {
        (right, left)
    } else {
        (left, right)
    };
    if kind == Kind::Has && !matches!(base, JsValue::Object(_)) {
        runtime
            .release_jsvalue(key)
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .release_jsvalue(base)
            .map_err(runtime_error_to_vm_error)?;
        return super::property_driver::throw_error(
            runtime,
            realm,
            Error::new(ErrorKind::Type, "invalid 'in' operand"),
        )
        .map(Progress::Call);
    }
    let mut input = Input {
        runtime: runtime.clone(),
        base,
        key,
        kind,
        depth,
    };
    if matches!(input.key, JsValue::Object(_)) {
        Ok(Progress::Convert(Box::new(input)))
    } else {
        complete(runtime, execution, id, &mut input).map(Progress::Call)
    }
}
// The pending predicate conversion transfers its existing box directly to this consuming handler.
#[allow(clippy::boxed_local)]
pub(super) fn converted(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mut input: Box<Input>,
) -> Result<CallStep, Error> {
    complete(runtime, execution, id, &mut input)
}

// Both immediate and resumed predicates keep the same input owner live through
// completion; only an object key that can suspend needs heap-resident storage.
fn complete(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    input: &mut Input,
) -> Result<CallStep, Error> {
    let kind = input.kind;
    let depth = input.depth;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let strict = frame.executable.metadata.strict;
    if matches!(input.key, JsValue::Object(_)) {
        return Err(Error::internal(
            "predicate key conversion returned an object",
        ));
    }
    let key = std::mem::replace(&mut input.key, JsValue::Undefined);
    let key = match runtime
        .native_to_property_key_jsvalue(realm, key)
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(key) => key,
        NativeConversion::Throw(value) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
    };
    if let JsValue::Object(object) = &input.base {
        let object = crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), *object);
        input.base = JsValue::Undefined;
        let op = if kind == Kind::Has {
            ProxyBooleanKind::Has(key)
        } else {
            ProxyBooleanKind::Delete(key)
        };
        return super::proxy_get_driver::start_boolean(
            runtime,
            execution,
            id,
            object,
            op,
            kind == Kind::Delete && strict,
            depth,
        );
    }
    if kind != Kind::Delete {
        return Err(Error::internal("in lost its validated object"));
    }
    let result = runtime
        .primitive_delete_property_jsvalue(&input.base, &key)
        .and_then(|value| runtime.finish_property_delete(NativeConversion::Value(value), strict));
    let completion = match result {
        Ok(result) => result,
        Err(error) => {
            return super::property_driver::throw_error(
                runtime,
                realm,
                runtime_error_to_vm_error(error),
            );
        }
    };
    if let Completion::Return(value) = completion {
        let frame = execution.frames.current_mut(id)?;
        execution.slots.push(&mut frame.window, value)?;
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("predicate resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        Ok(CallStep::Entered)
    } else {
        Ok(CallStep::Complete(completion))
    }
}
