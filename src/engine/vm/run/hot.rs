//! Shared scalar bodies for canonical and QuickOp dispatch.
//! A declined guard has neither consumed operands nor changed a binding.
use super::{
    Error, FrameBinding, JsValue, Number, RunSlots, StoreMode, cold, copy_scalar, immediate, value,
};
use crate::engine::api::Runtime;
use crate::engine::vm::numeric::operation::NumericKind;

/// The successful Number path uses exactly the canonical arithmetic kernels.
/// Non-Number inputs stay resident and unchanged for ordered conversion.
#[inline(always)]
pub(super) fn binary(slots: &mut RunSlots<'_>, kind: NumericKind) -> Result<bool, Error> {
    use NumericKind as N;
    match kind {
        N::Add => slots.binary_number(|a, b| value(a.add(b))),
        N::Sub => slots.binary_number(|a, b| value(a.sub(b))),
        N::Mul => slots.binary_number(|a, b| value(a.mul(b))),
        N::Div => slots.binary_number(|a, b| value(a.div(b))),
        N::Mod => slots.binary_number(|a, b| value(a.rem(b))),
        N::Pow => slots.binary_number(|a, b| value(a.pow(b))),
        N::Shl => {
            slots.binary_number(|a, b| JsValue::Int(a.int32().wrapping_shl(b.int32() as u32 & 31)))
        }
        N::Sar => slots.binary_number(|a, b| JsValue::Int(a.int32() >> (b.int32() as u32 & 31))),
        N::Shr => slots.binary_number(|a, b| {
            value(Number::compact(f64::from(
                (a.int32() as u32) >> (b.int32() as u32 & 31),
            )))
        }),
        N::BitAnd => slots.binary_number(|a, b| JsValue::Int(a.int32() & b.int32())),
        N::BitOr => slots.binary_number(|a, b| JsValue::Int(a.int32() | b.int32())),
        N::BitXor => slots.binary_number(|a, b| JsValue::Int(a.int32() ^ b.int32())),
        N::Eq => slots.binary_number(|a, b| JsValue::Bool(a.float() == b.float())),
        N::Neq => slots.binary_number(|a, b| JsValue::Bool(a.float() != b.float())),
        N::Lt => slots.binary_number(|a, b| JsValue::Bool(a.float() < b.float())),
        N::Lte => slots.binary_number(|a, b| JsValue::Bool(a.float() <= b.float())),
        N::Gt => slots.binary_number(|a, b| JsValue::Bool(a.float() > b.float())),
        N::Gte => slots.binary_number(|a, b| JsValue::Bool(a.float() >= b.float())),
        N::Neg | N::Plus | N::BitNot | N::Inc | N::Dec | N::PostInc | N::PostDec => Ok(false),
    }
}

/// None declines without consuming. Some(usize::MAX) is consumed fallthrough;
/// every other Some is the taken canonical target, matching compare fusion.
#[inline(always)]
pub(super) fn branch(
    slots: &mut RunSlots<'_>,
    target: u32,
    when: bool,
) -> Result<Option<usize>, Error> {
    if !immediate(slots.peek(0)?) {
        return Ok(None);
    }
    let target =
        usize::try_from(target).map_err(|_| cold::internal("branch target does not fit PC"))?;
    let truthy = slots.pop()?.to_boolean_primitive();
    Ok(Some(if truthy == when { target } else { usize::MAX }))
}

#[inline(always)]
pub(super) fn read_scalar(
    _runtime: &Runtime,
    slots: &mut RunSlots<'_>,
    index: u16,
    argument: bool,
) -> Result<bool, Error> {
    let binding = if argument {
        slots.parameter(index)?
    } else {
        slots.local(index)?
    };
    let FrameBinding::Direct(source) = binding else {
        return Ok(false);
    };
    let Some(copied) = copy_scalar(source) else {
        return Ok(false);
    };
    slots.push(copied)?;
    Ok(true)
}

/// The caller authenticates local kind/const semantics. This body accepts only
/// direct scalar source/displaced values, so it needs no publication boundary.
#[inline(always)]
pub(super) fn store_scalar(
    runtime: &Runtime,
    slots: &mut RunSlots<'_>,
    index: u16,
    argument: bool,
    mode: StoreMode,
) -> Result<bool, Error> {
    let binding = if argument {
        slots.parameter(index)?
    } else {
        slots.local(index)?
    };
    if !matches!(binding, FrameBinding::Direct(old) if immediate(old)) || !immediate(slots.peek(0)?)
    {
        return Ok(false);
    }
    let displaced = if argument {
        slots.store_parameter_from_top(runtime, index, mode)?
    } else {
        slots.store_local_from_top(runtime, index, mode)?
    }
    .ok_or_else(|| cold::internal("direct scalar store target changed"))?;
    discard_scalar(displaced);
    Ok(true)
}

/// The store preflight proved the displaced binding is an edge-free scalar.
/// Its move and scalar copy cannot invalidate that proof. Keep the logical
/// release counter without transporting the owner through Runtime's general
/// release path, which rechecks readiness and materializes a wide result.
#[inline(always)]
pub(super) fn discard_scalar(value: JsValue) {
    debug_assert!(immediate(&value));
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_storage(
        crate::engine::api::profiling::OwnedStorageEvent::HotRelease { heap_root: false },
    );
    // Drop the internal value, which has no destructor. Dropping the whole
    // binding would retain dispatch to captured/private owner drop glue.
    drop(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::vm::stack::{FrameStorage, FrameTransaction, FrameWindow, SlotStore};
    use std::rc::Rc;

    fn frame(runtime: &Runtime, local: FrameBinding) -> (SlotStore, FrameWindow) {
        let context = runtime.new_context();
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.local_count = 1;
        layout.metadata.argument_count = 1;
        layout.metadata.max_stack = 4;
        let definitions: Rc<[VariableDefinition]> = Rc::from([VariableDefinition {
            name: None,
            is_lexical: matches!(&local, FrameBinding::Uninitialized),
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        }]);
        layout.local_definitions = Rc::clone(&definitions);
        layout.argument_definitions = definitions;
        let mut store = SlotStore::new(16);
        let window = store
            .push_frame(
                runtime,
                &layout.frame_layout(),
                FrameStorage {
                    original_arguments: vec![JsValue::Int(7)],
                    parameters: vec![FrameBinding::Direct(JsValue::Int(7))],
                    locals: vec![local],
                    operands: vec![],
                },
            )
            .unwrap();
        (store, window)
    }

    fn transaction<'a>(
        store: &'a mut SlotStore,
        window: &'a mut FrameWindow,
        cached: bool,
    ) -> FrameTransaction<'a> {
        if cached {
            store.frame_transaction_with_scalar_tos(window).unwrap()
        } else {
            store.frame_transaction(window).unwrap()
        }
    }

    #[test]
    fn quick_hot_number_kernels_match_canonical_results_and_decline_without_consuming() {
        use NumericKind as N;
        let runtime = Runtime::new();
        for cached in [false, true] {
            let (mut store, mut window) = frame(&runtime, FrameBinding::Direct(JsValue::Int(7)));
            {
                let mut tx = transaction(&mut store, &mut window, cached);
                let mut slots = tx.slots();
                for (kind, expected) in [
                    (N::Add, JsValue::Int(8)),
                    (N::Sub, JsValue::Int(4)),
                    (N::Mul, JsValue::Int(12)),
                    (N::Div, JsValue::Int(3)),
                    (N::Mod, JsValue::Int(0)),
                    (N::Pow, JsValue::Int(36)),
                    (N::Shl, JsValue::Int(24)),
                    (N::Sar, JsValue::Int(1)),
                    (N::Shr, JsValue::Int(1)),
                    (N::BitAnd, JsValue::Int(2)),
                    (N::BitOr, JsValue::Int(6)),
                    (N::BitXor, JsValue::Int(4)),
                    (N::Eq, JsValue::Bool(false)),
                    (N::Neq, JsValue::Bool(true)),
                    (N::Lt, JsValue::Bool(false)),
                    (N::Lte, JsValue::Bool(false)),
                    (N::Gt, JsValue::Bool(true)),
                    (N::Gte, JsValue::Bool(true)),
                ] {
                    slots.push(JsValue::Int(6)).unwrap();
                    slots.push(JsValue::Int(2)).unwrap();
                    assert!(binary(&mut slots, kind).unwrap());
                    assert_eq!(slots.pop().unwrap(), expected, "{kind:?}");
                    assert!(slots.peek(0).is_err());
                }
                slots.push(JsValue::Int(7)).unwrap();
                slots.push(JsValue::Bool(true)).unwrap();
                assert!(!binary(&mut slots, N::Add).unwrap());
                assert_eq!(slots.pop().unwrap(), JsValue::Bool(true));
                assert_eq!(slots.pop().unwrap(), JsValue::Int(7));
                slots.push(JsValue::Float(-0.0)).unwrap();
                slots.push(JsValue::Int(3)).unwrap();
                assert!(binary(&mut slots, N::Pow).unwrap());
                let JsValue::Float(result) = slots.pop().unwrap() else {
                    panic!("negative zero")
                };
                assert_eq!(result.to_bits(), (-0.0_f64).to_bits());
            }
            store.clear_frame(&runtime, window).unwrap();
        }
    }

    #[test]
    fn quick_hot_branch_and_binding_guards_preserve_owner_and_uninitialized_state() {
        let runtime = Runtime::new();
        for cached in [false, true] {
            let owner = runtime.new_object(None).unwrap();
            let id = owner.object_id();
            let (mut store, mut window) = frame(
                &runtime,
                FrameBinding::Direct(JsValue::Object(owner.into_handle())),
            );
            let other = runtime.new_object(None).unwrap();
            let other_id = other.object_id();
            {
                let mut tx = transaction(&mut store, &mut window, cached);
                let mut slots = tx.slots();
                slots.push(JsValue::Int(9)).unwrap();
                assert!(!read_scalar(&runtime, &mut slots, 0, false).unwrap());
                assert!(!store_scalar(&runtime, &mut slots, 0, false, StoreMode::Consume).unwrap());
                assert_eq!(slots.pop().unwrap(), JsValue::Int(9));
                assert!(
                    matches!(slots.local(0).unwrap(), FrameBinding::Direct(JsValue::Object(found)) if *found == id)
                );
                for (source, truthy) in [
                    (JsValue::Undefined, false),
                    (JsValue::Null, false),
                    (JsValue::Bool(true), true),
                    (JsValue::Int(1), true),
                    (JsValue::Float(f64::NAN), false),
                    (JsValue::ShortBigInt(1), true),
                ] {
                    slots.push(source).unwrap();
                    assert_eq!(
                        branch(&mut slots, 17, true).unwrap(),
                        Some(if truthy { 17 } else { usize::MAX })
                    );
                    assert!(slots.peek(0).is_err());
                }
                slots.push(JsValue::Object(other.into_handle())).unwrap();
                assert_eq!(branch(&mut slots, 17, true).unwrap(), None);
                assert!(!store_scalar(&runtime, &mut slots, 0, true, StoreMode::Keep).unwrap());
                assert!(
                    matches!(slots.peek(0).unwrap(), JsValue::Object(found) if *found == other_id)
                );
            }
            store.clear_frame(&runtime, window).unwrap();
            let (mut store, mut window) = frame(&runtime, FrameBinding::Uninitialized);
            {
                let mut tx = transaction(&mut store, &mut window, cached);
                let mut slots = tx.slots();
                slots.push(JsValue::Int(9)).unwrap();
                assert!(!read_scalar(&runtime, &mut slots, 0, false).unwrap());
                assert!(!store_scalar(&runtime, &mut slots, 0, false, StoreMode::Consume).unwrap());
                assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(9));
                assert!(matches!(
                    slots.local(0).unwrap(),
                    FrameBinding::Uninitialized
                ));
            }
            store.clear_frame(&runtime, window).unwrap();
        }
    }

    #[test]
    fn quick_hot_scalar_stores_preserve_keep_consume_and_aliases_with_both_facades() {
        let runtime = Runtime::new();
        for cached in [false, true] {
            let (mut store, mut window) = frame(&runtime, FrameBinding::Direct(JsValue::Int(7)));
            {
                let mut tx = transaction(&mut store, &mut window, cached);
                let mut slots = tx.slots();
                for argument in [false, true] {
                    slots.push(JsValue::ShortBigInt(42)).unwrap();
                    assert!(
                        store_scalar(&runtime, &mut slots, 0, argument, StoreMode::Keep).unwrap()
                    );
                    assert_eq!(slots.pop().unwrap(), JsValue::ShortBigInt(42));
                    assert!(read_scalar(&runtime, &mut slots, 0, argument).unwrap());
                    assert!(
                        store_scalar(&runtime, &mut slots, 0, argument, StoreMode::Consume)
                            .unwrap()
                    );
                    assert!(slots.peek(0).is_err());
                    let target = if argument {
                        slots.parameter(0)
                    } else {
                        slots.local(0)
                    }
                    .unwrap();
                    assert!(matches!(
                        target,
                        FrameBinding::Direct(JsValue::ShortBigInt(42))
                    ));
                }
            }
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}
