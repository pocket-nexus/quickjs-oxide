// Observe a single slot through two allocations, an explicit free, and sweep.
// The bit is attached to the slot until clear_marks starts the next epoch.
#[kani::proof]
#[kani::unwind(8)]
fn reused_slot_mark_epoch() {
    #[derive(Debug, PartialEq)]
    struct Entry(u32);
    impl Record for Entry {
        fn vacant() -> Self { Entry(0) }
    }
    let mut arena = Arena::new();
    let mut room = Room::unbounded();
    let old: u32 = kani::any();
    let new: u32 = kani::any();
    let clear: bool = kani::any();
    let i = arena.alloc(Entry(old), &mut room);
    assert!(arena.mark(i));
    assert_eq!(arena.free(i).0, old);
    assert_eq!(arena.alloc(Entry(new), &mut room), i);
    if clear { arena.clear_marks(); }
    assert_eq!(arena.is_marked(i), !clear);
    let mut called = false;
    let freed = arena.sweep(|id, entry| {
        assert!(!called && id == i && entry.0 == new);
        called = true;
    });
    assert_eq!(called, clear);
    assert_eq!(freed, usize::from(clear));
    assert_eq!(arena.is_live(i), !clear);
}
