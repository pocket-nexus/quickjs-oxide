//! Four inline graph edges cover ordinary objects, properties and wrappers.
//! Larger payloads spill once without an allocation for the common case.
use super::{ObjectId, RawId};

// The experimental projection must fit the original arena budget. Its cold
// parameter descriptor is indirect, so adding QuickOp must not enlarge every
// node kind's stride. A normal M0 build retains its exact original layout.
#[cfg(all(
    target_pointer_width = "64",
    not(any(test, oxide_quick_projection, oxide_quick_dispatch))
))]
const _: () = assert!(std::mem::size_of::<super::ArenaSlot>() == 440);
#[cfg(all(
    target_pointer_width = "64",
    any(test, oxide_quick_projection, oxide_quick_dispatch)
))]
const _: () = assert!(std::mem::size_of::<super::ArenaSlot>() <= 440);

const EMPTY: RawId = RawId::Object(ObjectId {
    index: 0,
    generation: 0,
});

#[derive(Debug)]
pub(super) enum Edges {
    Inline { values: [RawId; 4], len: usize },
    Heap(Vec<RawId>),
}
impl Edges {
    pub(super) const fn new() -> Self {
        Self::Inline {
            values: [EMPTY; 4],
            len: 0,
        }
    }
    pub(super) fn push(&mut self, value: RawId) {
        match self {
            Self::Inline { values, len } if *len < 4 => {
                values[*len] = value;
                *len += 1;
            }
            Self::Inline { values, .. } => {
                let mut heap = Vec::with_capacity(8);
                heap.extend_from_slice(values);
                heap.push(value);
                *self = Self::Heap(heap);
            }
            Self::Heap(values) => values.push(value),
        }
    }
}
impl std::ops::Deref for Edges {
    type Target = [RawId];
    fn deref(&self) -> &[RawId] {
        match self {
            Self::Inline { values, len } => &values[..*len],
            Self::Heap(values) => values,
        }
    }
}
impl Extend<RawId> for Edges {
    fn extend<I: IntoIterator<Item = RawId>>(&mut self, values: I) {
        let mut values = values.into_iter();
        if let Self::Inline {
            values: inline,
            len,
        } = self
        {
            while *len < inline.len() {
                let Some(value) = values.next() else {
                    return;
                };
                inline[*len] = value;
                *len += 1;
            }
            let Some(first) = values.next() else {
                return;
            };
            let mut heap = Vec::with_capacity(8.max(5usize.saturating_add(values.size_hint().0)));
            heap.extend_from_slice(inline);
            heap.push(first);
            *self = Self::Heap(heap);
        }
        if let Self::Heap(heap) = self {
            heap.extend(values);
        }
    }
}
impl FromIterator<RawId> for Edges {
    fn from_iter<I: IntoIterator<Item = RawId>>(values: I) -> Self {
        let mut edges = Self::new();
        edges.extend(values);
        edges
    }
}
impl From<Vec<RawId>> for Edges {
    fn from(values: Vec<RawId>) -> Self {
        if values.len() > 4 {
            Self::Heap(values)
        } else {
            values.into_iter().collect()
        }
    }
}
impl IntoIterator for Edges {
    type Item = RawId;
    type IntoIter = std::iter::Chain<
        std::iter::Take<std::array::IntoIter<RawId, 4>>,
        std::vec::IntoIter<RawId>,
    >;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Inline { values, len } => values.into_iter().take(len).chain(Vec::new()),
            Self::Heap(values) => [EMPTY; 4].into_iter().take(0).chain(values),
        }
    }
}
#[cfg(test)]
impl PartialEq<Vec<RawId>> for Edges {
    fn eq(&self, other: &Vec<RawId>) -> bool {
        &**self == other.as_slice()
    }
}

#[cfg(test)]
impl<const N: usize> PartialEq<[RawId; N]> for Edges {
    fn eq(&self, other: &[RawId; N]) -> bool {
        &**self == other.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_and_spilled_edges_preserve_order() {
        for len in 0..10 {
            let expected = (0..len)
                .map(|index| {
                    RawId::Object(ObjectId {
                        index,
                        generation: 1,
                    })
                })
                .collect::<Vec<_>>();
            let edges: Edges = expected.iter().copied().collect();
            assert_eq!(&*edges, &expected);
            assert_eq!(matches!(edges, Edges::Inline { .. }), len <= 4);
            assert_eq!(edges.into_iter().collect::<Vec<_>>(), expected);
            for split in 0..=expected.len() {
                let mut extended: Edges = expected[..split].iter().copied().collect();
                extended.extend(expected[split..].iter().copied().filter(|_| true));
                assert_eq!(&*extended, &expected);
                assert_eq!(matches!(extended, Edges::Inline { .. }), len <= 4);
            }
        }
    }
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn arena_layout_is_compact() {
        eprintln!(
            "ArenaSlot bytes: {}",
            std::mem::size_of::<super::super::ArenaSlot>()
        );
        // Unit tests use M1's cold parameter-layout indirection. Production
        // M0's exact layout remains independently pinned above.
        assert!(std::mem::size_of::<super::super::ArenaSlot>() <= 440);
    }
}
