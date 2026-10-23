//! Finite exhaustive storage schedules complement, but do not replace, proofs.
use super::*;

#[test]
fn coalescing_growth_and_sweep_preserve_independent_live_ranges() {
    for left in 1..=4 {
        for right in 1..=4 {
            let mut heap = ValueHeap::new();
            let mut room = Room::unbounded();
            let a = heap.alloc(left, &mut room);
            let b = heap.alloc(right, &mut room);
            for i in 0..b.len {
                heap.set(b, i, Value::int32(100 + i as i32));
            }
            heap.free(a);
            heap.coalesce();
            for i in 0..b.len {
                assert_eq!(heap.get(b, i), Value::int32(100 + i as i32));
            }
            let c = heap.alloc(left, &mut room);
            assert!(c.start + c.len <= b.start || b.start + b.len <= c.start);
            for i in 0..c.len {
                assert_eq!(heap.get(c, i), Value::EMPTY);
            }
            let grown = heap.grow(b, b.len + 5, &mut room);
            assert!(c.start + c.len <= grown.start || grown.start + grown.len <= c.start);
            for i in 0..grown.len {
                let expected = if i < b.len {
                    Value::int32(100 + i as i32)
                } else {
                    Value::EMPTY
                };
                assert_eq!(heap.get(grown, i), expected);
            }
            assert_eq!(heap.used_slots(), (c.len + grown.len) as usize);
            heap.begin_sweep();
            heap.release(c);
            heap.finish_sweep();
            assert_eq!(heap.used_slots(), grown.len as usize);
            for i in 0..b.len {
                assert_eq!(heap.get(grown, i), Value::int32(100 + i as i32));
            }
            heap.begin_sweep();
            heap.release(grown);
            heap.finish_sweep();
            assert_eq!(heap.used_slots(), 0);
            assert_eq!(heap.total_slots(), 0);
        }
    }
}
