// Included in the production heap::gc module during verification.
verus! {
impl View for RootMark {
    type V = usize;
    closed spec fn view(&self) -> usize { self.0 }
}

impl View for RootStack {
    type V = Seq<Value>;
    closed spec fn view(&self) -> Seq<Value> { self.values@ }
}

fn restore_mark(stack: &mut RootStack, first: Value, second: Value)
    ensures final(stack)@ == old(stack)@,
{
    let mark = stack.mark();
    stack.push(first);
    stack.push(second);
    stack.truncate(mark);
}

proof fn pushed_members(free: Seq<u32>, x: u32)
    ensures forall|v: u32| #[trigger] free.push(x).contains(v) == (v == x || free.contains(v)),
{
    let pushed = free.push(x);
    assert forall|v: u32| #[trigger] pushed.contains(v) == (v == x || free.contains(v)) by {
        if pushed.contains(v) {
            let i = choose|i: int| 0 <= i < pushed.len() && pushed[i] == v;
            if i < free.len() { assert(free[i] == v); }
        }
        if free.contains(v) {
            let i = choose|i: int| 0 <= i < free.len() && free[i] == v;
            assert(pushed[i] == v);
        }
        assert(pushed[free.len() as int] == x);
    }
}

pub const EMPTY_BITS: u64 = 0xfffa_0000_0000_0003;
impl View for Handle {
    type V = u32;
    closed spec fn view(&self) -> u32 { self.0 }
}

impl Handles {
    pub closed spec fn entries(&self) -> Seq<Value> { self.slots@ }
    pub closed spec fn free_ids(&self) -> Seq<u32> { self.free@ }

    pub closed spec fn active(&self, h: Handle) -> bool {
        h@ < self.entries().len() && self.entries()[h@ as int]@ != EMPTY_BITS
    }

    pub closed spec fn well_formed(&self) -> bool {
        let slots = self.entries();
        let free = self.free_ids();
        &&& slots.len() <= u32::MAX
        &&& free.no_duplicates()
        &&& forall|i: int| 0 <= i < free.len() ==> #[trigger] free[i] < slots.len()
        &&& forall|i: int| 0 <= i < slots.len() ==>
            (#[trigger] slots[i]@ == EMPTY_BITS) == free.contains(i as u32)
    }
}
}

verus! {
impl Handles {
    proof fn free_count_bounded(&self)
        requires self.well_formed(),
        ensures self.free_ids().len() <= self.entries().len(),
    {
        broadcast use vstd::set_lib::range_set_properties;
        let free = self.free_ids();
        let slots = Set::<u32>::range(0, self.entries().len() as u32);
        free.unique_seq_to_set();
        free.to_set_ensures();
        assert(free.to_set().subset_of(slots));
        vstd::set_lib::lemma_len_subset(free.to_set(), slots);
    }
}
}

verus! {
// An inhabited starting state and a client of all four state/access contracts.
// Releasing one handle must leave the other one's value available across reuse.
fn reuse_preserves_other_handle(first: Value, second: Value, replacement: Value)
    requires first@ != EMPTY_BITS, second@ != EMPTY_BITS, replacement@ != EMPTY_BITS,
{
    let mut handles = Handles { slots: Vec::new(), free: Vec::new() };
    assert(handles.well_formed());
    let a = handles.create(first);
    let b = handles.create(second);
    assert(a != b);
    handles.set(a, replacement);
    handles.release(b);
    let reused = handles.create(first);
    let kept = handles.get(a);
    let fresh = handles.get(reused);
    let count = handles.live_count();
    assert(reused == b && kept == replacement && fresh == first && count == 2);
}
}
