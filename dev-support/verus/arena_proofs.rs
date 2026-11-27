use vstd::prelude::old as previous;

verus! {
impl<T> Arena<T> {
    pub closed spec fn extra_bytes_spec(&self) -> usize { self.extra }

    pub closed spec fn same_except_extra(&self, other: &Self) -> bool {
        *self == (Arena { extra: self.extra, ..*other })
    }
}
}
