use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Item {
    value: u8,
    extra: u8,
}

impl Record for Item {
    fn vacant() -> Self {
        Self { value: 0, extra: 0 }
    }
    fn extra_bytes(&self) -> usize {
        self.extra as usize
    }
}

// Symbolic slot-array model for allocate/free/reuse and content accounting.
// Separate schedules keep CBMC's heap-alias formula small; these are bounded
// sequences, not an arbitrary-operation induction theorem.
#[kani::proof]
#[kani::unwind(8)]
fn arena_free_and_reuse() {
    let mut arena = Arena::<Item>::new();
    let mut room = Room::unbounded();
    let a = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let b = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let replacement = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let mut model = [a, b];
    assert_eq!(arena.alloc(a, &mut room), 0);
    assert_eq!(arena.alloc(b, &mut room), 1);
    let index: usize = kani::any();
    kani::assume(index < 2);
    assert_eq!(arena.free(index as u32), model[index]);
    assert!(!arena.is_live(index as u32));
    assert_eq!(arena.alloc(replacement, &mut room), index as u32);
    model[index] = replacement;
    for i in 0..2 {
        assert!(arena.is_live(i as u32));
        assert_eq!(*arena.get(i as u32), model[i]);
    }
    assert_eq!(arena.live_count(), 2);
    assert_eq!(
        arena.extra,
        model[0].extra as usize + model[1].extra as usize
    );
    assert!(arena.free_count_matches());
}

#[kani::proof]
#[kani::unwind(8)]
fn arena_write_frame_and_accounting() {
    let mut arena = Arena::<Item>::new();
    let mut room = Room::unbounded();
    let a = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let b = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let replacement = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let mut model = [a, b];
    arena.alloc(a, &mut room);
    arena.alloc(b, &mut room);
    let index: usize = kani::any();
    kani::assume(index < 2);
    arena.adjust_extra(model[index].extra as usize, replacement.extra as usize);
    *arena.get_mut(index as u32) = replacement;
    model[index] = replacement;
    for i in 0..2 {
        assert_eq!(*arena.get(i as u32), model[i]);
    }
    assert_eq!(
        arena.extra,
        model[0].extra as usize + model[1].extra as usize
    );
    assert_eq!(arena.live_count(), 2);
    assert!(arena.free_count_matches());
}

#[kani::proof]
#[kani::unwind(8)]
fn arena_mark_and_sweep() {
    let mut arena = Arena::<Item>::new();
    let mut room = Room::unbounded();
    let a = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let b = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let model = [a, b];
    arena.alloc(a, &mut room);
    arena.alloc(b, &mut room);
    let index: usize = kani::any();
    kani::assume(index < 2);
    arena.clear_marks();
    assert!(arena.mark(index as u32));
    assert!(!arena.mark(index as u32));
    let mut seen = [false; 2];
    assert_eq!(
        arena.sweep(|id, item| {
            assert_eq!(id as usize, 1 - index);
            assert_eq!(item, model[id as usize]);
            assert!(!seen[id as usize]);
            seen[id as usize] = true;
        }),
        1
    );
    assert!(seen[1 - index]);
    assert!(!seen[index]);
    assert_eq!(*arena.get(index as u32), model[index]);
    assert!(!arena.is_live((1 - index) as u32));
    assert_eq!(arena.live_count(), 1);
    assert_eq!(arena.extra, model[index].extra as usize);
    assert!(arena.free_count_matches());
}

#[kani::proof]
#[kani::unwind(8)]
fn arena_sweep_all_and_empty() {
    let mut arena = Arena::<Item>::new();
    let mut room = Room::unbounded();
    let item = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    assert_eq!(arena.live_count(), 0);
    assert!(arena.free_count_matches());
    arena.alloc(item, &mut room);
    arena.alloc(item, &mut room);
    let mut order = 2;
    assert_eq!(
        arena.sweep(|index, record| {
            order -= 1;
            assert_eq!(index, order);
            assert_eq!(record, item);
        }),
        2
    );
    assert_eq!(order, 0);
    assert_eq!(arena.live_count(), 0);
    assert_eq!(arena.extra, 0);
    assert!(arena.free_count_matches());
    let mut callbacks = 0;
    assert_eq!(arena.sweep(|_, _| callbacks += 1), 0);
    assert_eq!(callbacks, 0);
    assert_eq!(arena.slot_count(), 0);
    assert!(arena.free_count_matches());
}

#[kani::proof]
#[kani::unwind(12)]
fn arena_sweep_quarantine_cycle() {
    let mut arena = Arena::<Item>::new();
    let mut room = Room::unbounded();
    let a = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let b = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    let first = arena.alloc(a, &mut room);
    let second = arena.alloc(b, &mut room);
    arena.mark(second);
    assert_eq!(
        arena.sweep(|i, item| {
            assert_eq!(i, first);
            assert_eq!(item, a);
        }),
        1
    );
    assert_eq!(*arena.get(second), b);
    let next = arena.alloc(a, &mut room);
    assert_eq!(
        next,
        if cfg!(feature = "gc-stress") {
            2
        } else {
            first
        }
    );
    arena.clear_marks();
    arena.mark(second);
    arena.mark(next);
    let mut callbacks = 0;
    assert_eq!(arena.sweep(|_, _| callbacks += 1), 0);
    assert_eq!(callbacks, 0);
    if cfg!(feature = "gc-stress") {
        assert_eq!(arena.alloc(a, &mut room), first);
    }
    assert!(arena.free_count_matches());
}

#[kani::proof]
#[kani::unwind(140)]
fn arena_multiword_reuse() {
    let item = Item {
        value: kani::any(),
        extra: 0,
    };
    // Fully occupied three-word fixture. Constructor/append are checked above;
    // this fixture isolates cursor movement across both 63/64 and 127/128.
    let mut arena = Arena {
        records: vec![item; 130],
        live: vec![u64::MAX, u64::MAX, 3],
        marks: vec![0; 3],
        free_count: 0,
        cursor: 0,
        #[cfg(feature = "gc-stress")]
        quarantine: vec![0; 3],
        live_count: 130,
        extra: 0,
    };
    let index: u32 = kani::any();
    kani::assume(index < 130);
    assert_eq!(arena.free(index), item);
    assert_eq!(arena.alloc(item, &mut Room::unbounded()), index);
    assert_eq!(arena.live, vec![u64::MAX, u64::MAX, 3]);
    assert_eq!(arena.live_count(), 130);
    assert!(arena.free_count_matches());
}

// Inductive state space: at most two Item slots, with capacity 0 (new) or 4
// (the first production growth). Marks may outlive a slot; quarantined slots
// are not reusable. Payload and external-byte counts cover every u8 value.
// Each step starts from ANY state in this space, not a bounded history.
fn inductive_state() -> Arena<Item> {
    let len: usize = kani::any();
    kani::assume(len <= 2);
    let grown: bool = kani::any();
    kani::assume(grown || len == 0);
    let live: u8 = kani::any();
    let marks: u8 = kani::any();
    kani::assume(live < (1 << len) && marks < 4);
    #[cfg(feature = "gc-stress")]
    let quarantine: u8 = kani::any();
    #[cfg(not(feature = "gc-stress"))]
    let quarantine: u8 = 0;
    kani::assume(quarantine < (1 << len) && live & quarantine == 0);
    let mut records = if grown {
        Vec::with_capacity(4)
    } else {
        Vec::new()
    };
    let mut count = 0;
    let mut extra = 0;
    let mut free = 0;
    for i in 0..len {
        let item = if live & (1 << i) != 0 {
            count += 1;
            Item {
                value: kani::any(),
                extra: kani::any(),
            }
        } else {
            if quarantine & (1 << i) == 0 {
                free += 1;
            }
            Item::vacant()
        };
        extra += item.extra as usize;
        records.push(item);
    }
    let word = |bits: u8| {
        let mut words = Vec::with_capacity(usize::from(grown));
        if len != 0 {
            words.push(bits as u64);
        }
        words
    };
    Arena {
        records,
        live: word(live),
        marks: word(marks),
        #[cfg(feature = "gc-stress")]
        quarantine: word(quarantine),
        free_count: free,
        cursor: 0,
        live_count: count,
        extra,
    }
}

fn assert_inductive_invariant(arena: &Arena<Item>) {
    let len = arena.records.len();
    assert!(len <= 2);
    assert!(arena.records.capacity() == 0 || arena.records.capacity() == 4);
    assert_eq!(arena.live.len(), usize::from(len != 0));
    assert_eq!(arena.marks.len(), arena.live.len());
    let words_capacity = usize::from(arena.records.capacity() != 0);
    assert_eq!(arena.live.capacity(), words_capacity);
    assert_eq!(arena.marks.capacity(), words_capacity);
    assert_eq!(arena.cursor, 0);
    #[cfg(feature = "gc-stress")]
    {
        assert_eq!(arena.quarantine.len(), arena.live.len());
        assert_eq!(arena.quarantine.capacity(), words_capacity);
    }
    let mut live_count = 0;
    let mut free_count = 0;
    let mut extra = 0;
    if len != 0 {
        assert!(arena.live[0] < (1 << len));
        assert!(arena.marks[0] < 4);
        #[cfg(feature = "gc-stress")]
        {
            assert!(arena.quarantine[0] < (1 << len));
            assert_eq!(arena.live[0] & arena.quarantine[0], 0);
        }
    }
    for i in 0..len {
        let live = arena.live[0] & (1 << i) != 0;
        #[cfg(feature = "gc-stress")]
        let quarantined = arena.quarantine[0] & (1 << i) != 0;
        #[cfg(not(feature = "gc-stress"))]
        let quarantined = false;
        if live {
            live_count += 1;
            extra += arena.records[i].extra as usize;
        } else {
            assert_eq!(arena.records[i], Item::vacant());
            if !quarantined {
                free_count += 1;
            }
        }
    }
    assert_eq!(arena.live_count, live_count);
    assert_eq!(arena.free_count, free_count);
    assert_eq!(arena.extra, extra);
}

#[kani::proof]
#[kani::unwind(8)]
fn inductive_constructor() {
    assert_inductive_invariant(&Arena::<Item>::new());
}

#[kani::proof]
#[kani::unwind(8)]
fn inductive_alloc() {
    let mut arena = inductive_state();
    kani::assume(arena.free_count != 0 || arena.records.len() < 2);
    arena.alloc(
        Item {
            value: kani::any(),
            extra: kani::any(),
        },
        &mut Room::unbounded(),
    );
    assert_inductive_invariant(&arena);
}

#[kani::proof]
#[kani::unwind(8)]
fn inductive_free() {
    let mut arena = inductive_state();
    let index: u32 = kani::any();
    kani::assume(index < arena.records.len() as u32);
    kani::assume(arena.live[0] & (1 << index) != 0);
    arena.free(index);
    assert_inductive_invariant(&arena);
}

#[kani::proof]
#[kani::unwind(8)]
fn inductive_mark_and_clear() {
    let mut arena = inductive_state();
    let index: u32 = kani::any();
    kani::assume(index < arena.records.len() as u32);
    kani::assume(arena.live[0] & (1 << index) != 0);
    arena.mark(index);
    assert_inductive_invariant(&arena);
    arena.clear_marks();
    assert_inductive_invariant(&arena);
}

#[kani::proof]
#[kani::unwind(8)]
fn inductive_write() {
    let mut arena = inductive_state();
    let index: u32 = kani::any();
    kani::assume(index < arena.records.len() as u32);
    kani::assume(arena.live[0] & (1 << index) != 0);
    let old = arena.get(index).extra as usize;
    let item = Item {
        value: kani::any(),
        extra: kani::any(),
    };
    *arena.get_mut(index) = item;
    arena.adjust_extra(old, item.extra as usize);
    assert_inductive_invariant(&arena);
}

#[kani::proof]
#[kani::unwind(8)]
fn inductive_sweep_and_shrink() {
    let mut arena = inductive_state();
    arena.sweep(|_, _| {});
    assert_inductive_invariant(&arena);
    arena.shrink();
    assert_inductive_invariant(&arena);
}
