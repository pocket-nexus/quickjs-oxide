use super::*;

#[kani::proof]
#[kani::unwind(12)]
fn live_ranges_one_and_three() {
    live_ranges_do_not_overlap(1, 3);
}

#[kani::proof]
#[kani::unwind(12)]
fn live_ranges_two_and_four() {
    live_ranges_do_not_overlap(2, 4);
}

fn live_ranges_do_not_overlap(left: u32, right: u32) {
    let mut heap = ValueHeap::new();
    let mut room = Room::unbounded();
    let a = heap.alloc(left, &mut room);
    let b = heap.alloc(right, &mut room);
    assert!(a.start + a.len <= b.start || b.start + b.len <= a.start);
    let value = Value::int32(kani::any());
    heap.set(b, 0, value);
    heap.free(a);
    assert_eq!(heap.get(b, 0), value);
    let c = heap.alloc(left, &mut room);
    assert!(c.start + c.len <= b.start || b.start + b.len <= c.start);
    for slot in 0..c.len {
        assert_eq!(heap.get(c, slot), Value::EMPTY);
    }
    assert_eq!(heap.get(b, 0), value);
    assert_eq!(heap.used_slots(), (b.len + c.len) as usize);
}

#[kani::proof]
#[kani::unwind(24)]
fn growth_preserves_contents() {
    let mut heap = ValueHeap::new();
    let mut room = Room::unbounded();
    let a = heap.alloc(2, &mut room);
    let b = heap.alloc_slots(3, &mut room);
    let x = Value::int32(kani::any());
    let y = Value::int32(kani::any());
    heap.set(a, 0, x);
    heap.set(a, 1, y);
    heap.set(b, 0, y);
    let a = heap.grow(a, 5, &mut room);
    assert_eq!(a.len, 6);
    assert_eq!(heap.get(a, 0), x);
    assert_eq!(heap.get(a, 1), y);
    for i in 2..a.len {
        assert_eq!(heap.get(a, i), Value::EMPTY);
    }
    assert_eq!(heap.get(b, 0), y);
    let b = heap.grow_slots(b, 5, &mut room);
    assert_eq!(b.len, 8);
    assert_eq!(heap.get(b, 0), y);
    assert_eq!(heap.get(a, 0), x);
    assert_eq!(heap.used_slots(), (a.len + b.len) as usize);
}
