//! A base class constructor calls its instance initializer on entry. Classes
//! without instance fields, accessors or private methods have none, and the
//! call only drops the active function; complete that case in the segment.
use super::FrameCursor;
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::heap::ObjectPayload;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn};

/// Pop the constructor operand when it has no instance initializer and the
/// receiver is an Object. Returns `false` with both operands in place.
#[inline(never)]
pub(super) fn try_skip_absent(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
) -> Result<bool, Error> {
    let FrameTurn {
        executable,
        transaction,
        fault_pc,
        resume_pc,
        ..
    } = segment.frame();
    let next = executable
        .exec
        .decode_published(*fault_pc as u32)
        .map_err(|_| Error::internal("class initializer has no instruction"))?
        .next_pc as usize;
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    let absent = cursor.with_slots(|slots| {
        let (JsValue::Object(constructor), JsValue::Object(_)) = (slots.peek(0)?, slots.peek(1)?)
        else {
            return Ok(false);
        };
        let object = state
            .heap
            .object(*constructor)
            .map_err(|error| runtime_error_to_vm_error(error.into()))?;
        Ok(matches!(
            object.payload,
            ObjectPayload::BytecodeFunction {
                class_instance_initializer: None,
                ..
            }
        ))
    })?;
    if !absent {
        return Ok(false);
    }
    let constructor = cursor.move_owned()?;
    state
        .release_owned_jsvalue(&runtime.0.poisoned, constructor)
        .map_err(runtime_error_to_vm_error)?;
    cursor.advance(next);
    profile!(record_owned_execution_event(
        "class_initializer_absent_in_segment"
    ));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn constructors_with_and_without_instance_initializers_agree() {
        let (_runtime, mut context) = test_context();
        let source = r#"
            class Plain { constructor(x) { this.x = x; } }
            class Field { y = 2; constructor(x) { this.x = x; } }
            class Private { #p = 3; get p() { return this.#p; } }
            class Accessor { get a() { return 4; } }
            class Derived extends Plain { z = 6; constructor() { super(5); } }
            class Empty {}
            var total = 0;
            for (var i = 0; i < 50; i++) {
                total += new Plain(i).x + new Field(1).y + new Private().p + new Accessor().a
                    + new Derived().x + new Derived().z + (new Empty() instanceof Empty ? 1 : 0);
            }
            var errors = [];
            try { Plain(); } catch (e) { errors.push(e.name); }
            var proxied = Reflect.construct(Plain, [8], Field);
            String(total) + ':' + errors + ':' + proxied.x + ':' + ('y' in proxied)
                + ':' + (Object.getPrototypeOf(proxied) === Field.prototype)
        "#;
        assert_eq!(
            eval_string(&mut context, source),
            "2275:TypeError:8:false:true"
        );
    }
}
