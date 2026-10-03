use super::*;

#[test]
fn slot_reservations_reuse_spare_capacity_and_grow_amortized() {
    let mut slots = Slots::new();
    let mut growths = 0;
    for value in 0..128 {
        let capacity = slots.capacity();
        let address = slots.as_slice().as_ptr();
        let spilled = matches!(slots, Slots::Spilled(_));
        slots.try_reserve(1).unwrap();
        if spilled && slots.len() < capacity {
            assert_eq!(slots.capacity(), capacity);
            assert_eq!(slots.as_slice().as_ptr(), address);
        }
        growths += usize::from(slots.capacity() != capacity);
        slots.push(PropertySlot::Data(RawValue::Int(value)));
    }
    assert!(
        growths < 16,
        "slot growth must not allocate for each append"
    );
    for (index, slot) in slots.iter().enumerate() {
        assert!(matches!(slot, PropertySlot::Data(RawValue::Int(value)) if *value == index as i32));
    }

    let capacity = slots.capacity();
    let address = slots.as_slice().as_ptr();
    slots.clear();
    slots.try_reserve(3).unwrap();
    assert_eq!(slots.capacity(), capacity);
    assert_eq!(slots.as_slice().as_ptr(), address);
}

#[test]
fn overflowing_slot_reservation_preserves_inline_and_spilled_storage() {
    for count in [1, 3] {
        let mut slots = Slots::from_vec(
            (0..count)
                .map(|value| PropertySlot::Data(RawValue::Int(value)))
                .collect(),
        );
        let capacity = slots.capacity();
        let address = slots.as_slice().as_ptr();
        assert!(slots.try_reserve(usize::MAX).is_err());
        assert_eq!(slots.len(), count as usize);
        assert_eq!(slots.capacity(), capacity);
        assert_eq!(slots.as_slice().as_ptr(), address);
        for (index, slot) in slots.iter().enumerate() {
            assert!(
                matches!(slot, PropertySlot::Data(RawValue::Int(value)) if *value == index as i32)
            );
        }
    }
}
