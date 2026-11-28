verus! {
impl ValueHeap {
    pub closed spec fn slots(&self) -> Seq<Value> { self.values@ }

    pub open spec fn contains(&self, r: Range) -> bool {
        &&& r.start + r.len <= self.slots().len()
        &&& r.start + r.len <= u32::MAX
    }

    pub closed spec fn space(&self) -> ([Vec<u32>; 64], u64, Seq<(u32,u32,u32)>, (u32,u32), usize, Seq<u64>) {
        (self.free, self.listed, self.runs@, self.current, self.free_slots, self.bits@)
    }
}

// A write through a relative range is visible by both absolute and relative
// addressing. Every other slot, including those in disjoint ranges, is preserved.
#[allow(dead_code)]
fn write_and_read(heap: &mut ValueHeap, r: Range, slot: u32, value: Value) -> (result: (Value, Value))
    requires old(heap).contains(r), slot < r.len,
    ensures result == (value, value),
        final(heap).space() == old(heap).space(),
        final(heap).slots() == old(heap).slots().update(r.start + slot, value),
{
    heap.set(r, slot, value);
    (heap.get_or_empty(r, slot), heap.get_at(r.start + slot))
}

}
