use super::*;

#[derive(Debug, PartialEq)]
struct Observation {
    completed: bool,
    operands: Vec<(u8, u64)>,
    local: (u8, u64),
    parameter: (u8, u64),
}

fn observe(slots: &RunSlots<'_>, completed: bool) -> Observation {
    let FrameBinding::Direct(local) = slots.local(0).unwrap() else {
        panic!("direct local");
    };
    let FrameBinding::Direct(parameter) = slots.parameter(0).unwrap() else {
        panic!("direct parameter");
    };
    #[cfg(feature = "profiling")]
    assert_eq!(slots.store.live_slots, 3 + slots.window.depth);
    Observation {
        completed,
        operands: (0..slots.window.depth)
            .map(|offset| scalar_bits(slots.peek(offset).unwrap()))
            .collect(),
        local: scalar_bits(local),
        parameter: scalar_bits(parameter),
    }
}

fn run_sequence(cached: bool) -> Vec<Observation> {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, 4);
    let mut observations = Vec::new();
    let mut state = 0x5eed_cafe_u32;
    {
        let mut tx = transaction(&mut store, &mut window, cached);
        for step in 0..256 {
            // Fixed seed makes every capacity/underflow/decline transition
            // reproducible without introducing a test-only random dependency.
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let mut slots = tx.slots();
            let completed = match (state >> 16) % 12 {
                0 | 1 => slots.push(JsValue::Int(step)).is_ok(),
                2 => slots.push(JsValue::Float(-0.0)).is_ok(),
                3 => slots.push(JsValue::ShortBigInt(i64::MIN)).is_ok(),
                4 => slots.pop().is_ok(),
                5 => slots
                    .binary_number(|left, right| left.add(right).into())
                    .unwrap_or(false),
                6 => slots
                    .consume_number_pair(|left, right| left.float() < right.float())
                    .is_ok_and(|result| result.is_some()),
                7 | 8 => {
                    let mode = if state & 1 == 0 {
                        StoreMode::Consume
                    } else {
                        StoreMode::Keep
                    };
                    let stored = if state & 2 == 0 {
                        slots.store_local_from_top(&runtime, 0, mode)
                    } else {
                        slots.store_parameter_from_top(&runtime, 0, mode)
                    };
                    matches!(stored, Ok(Some(_)))
                }
                9 => slots
                    .update_number_local(0, |previous| (previous.update(true), Some(previous)))
                    .unwrap_or(false),
                10 => slots.rotate_operands(0, 2, true).is_ok(),
                _ => {
                    slots.canonicalize("tos.spill.test_sequence");
                    true
                }
            };
            observations.push(observe(&slots, completed));
        }
    }
    // Drop must leave exactly the final logical prefix in canonical backing.
    {
        let slots = store.run_window(&mut window).unwrap();
        observations.push(observe(&slots, true));
    }
    store.clear_frame(&runtime, window).unwrap();
    observations
}

#[test]
fn scalar_tos_seeded_state_machine_matches_canonical_after_each_operation() {
    assert_eq!(run_sequence(false), run_sequence(true));
}
