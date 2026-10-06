#[cfg(test)]
mod tests {
    use super::super::{DirectSlot, FrameBinding, FrameWindow, SlotStore};
    use crate::engine::api::Runtime;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::value::JsValue;
    use crate::engine::vm::stack::FrameStorage;
    use std::rc::Rc;

    fn frame(runtime: &Runtime, capacity: u16) -> (SlotStore, FrameWindow) {
        let context = runtime.new_context().expect("create context");
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.argument_count = 1;
        owner.metadata.local_count = 1;
        owner.metadata.max_stack = capacity;
        let definition = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        owner.argument_definitions = Rc::from([definition]);
        owner.local_definitions = Rc::from([definition]);
        let mut slots = SlotStore::new(20);
        let window = slots
            .push_frame(
                runtime,
                &owner.frame_layout(),
                FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: vec![FrameBinding::Direct(JsValue::Int(11))],
                    locals: vec![FrameBinding::Direct(JsValue::Int(7))],
                    operands: Vec::new(),
                },
            )
            .unwrap();
        (slots, window)
    }

    #[test]
    fn direct_slot_read_is_short_borrow_of_direct_local_or_argument() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        {
            let run = slots.borrow_frame_slots(&mut window).unwrap();
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );
            assert_eq!(
                run.direct_value(DirectSlot::Argument(0)),
                Some(&JsValue::Int(11))
            );
            assert!(run.direct_value(DirectSlot::Local(1)).is_none());
            assert!(run.direct_value(DirectSlot::Argument(1)).is_none());
        }
        slots.slots[window.parameters().start] = Some(FrameBinding::Uninitialized);
        assert!(
            slots
                .borrow_frame_slots(&mut window)
                .unwrap()
                .direct_value(DirectSlot::Argument(0))
                .is_none()
        );
        slots.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn ordinary_number_write_preserves_put_set_and_declines_without_mutation() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        {
            let mut run = slots.borrow_frame_slots(&mut window).unwrap();
            assert!(!run.store_proven_number_operand(DirectSlot::Local(0), false));
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );

            run.push(JsValue::Bool(true)).unwrap();
            assert!(!run.store_proven_number_operand(DirectSlot::Local(0), false));
            assert_eq!(run.peek(0).unwrap(), &JsValue::Bool(true));
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );
            assert_eq!(run.pop().unwrap(), JsValue::Bool(true));

            let nan_bits = 0x7ff8_0000_0000_0042;
            run.push(JsValue::Float(f64::from_bits(nan_bits))).unwrap();
            assert!(run.store_proven_number_operand(DirectSlot::Local(0), true));
            let JsValue::Float(top) = run.peek(0).unwrap() else {
                panic!("SetLocal lost the Number operand");
            };
            assert_eq!(top.to_bits(), nan_bits);
            let Some(JsValue::Float(local)) = run.direct_value(DirectSlot::Local(0)) else {
                panic!("SetLocal changed the Number representation");
            };
            assert_eq!(local.to_bits(), nan_bits);

            assert!(run.store_proven_number_operand(DirectSlot::Argument(0), false));
            assert!(run.peek(0).is_err());
            let Some(JsValue::Float(argument)) = run.direct_value(DirectSlot::Argument(0)) else {
                panic!("PutArg changed the Number representation");
            };
            assert_eq!(argument.to_bits(), nan_bits);
        }
        assert_eq!(window.depth, 0);
        slots.clear_frame(&runtime, window).unwrap();
    }
}
