"""Source proof of lowest-slot arena reuse, including the stress quarantine."""

from source import item, method


def proof(source, arena_code):
    scope = item(source, r"impl<T: Record> Arena<T>")
    specs = '''
        #[cfg(feature = "gc-stress")]
        spec fn reuse_layout(&self) -> bool { self.quarantine@.len() == self.live@.len() }
        #[cfg(not(feature = "gc-stress"))]
        spec fn reuse_layout(&self) -> bool { true }
        #[cfg(feature = "gc-stress")]
        spec fn occupied(&self, w: int) -> u64 { self.live@[w] | self.quarantine@[w] }
        #[cfg(not(feature = "gc-stress"))]
        spec fn occupied(&self, w: int) -> u64 { self.live@[w] }
        spec fn reusable(&self, i: u32) -> bool {
            i < self.records@.len() && i / 64 < self.live@.len()
            && self.occupied((i / 64) as int) & (1u64 << (i % 64)) == 0
        }
        spec fn reusable_indices(&self) -> Set<int> {
            Set::<int>::range(0, self.records@.len() as int).filter(
                |i: int| i <= u32::MAX && self.reusable(i as u32))
        }
        spec fn cursor_valid(&self) -> bool {
            forall|i: u32| self.reusable(i) ==> i / 64 >= self.cursor
        }
    '''
    taken = method(scope, 'taken', '''
        requires self.reuse_layout(), word < self.live@.len(),
        ensures result == self.occupied(word as int),
    ''', 'result')
    take = method(scope, 'take_free', '''
        requires old(self).valid_marks(), old(self).reuse_layout(),
            old(self).live@.len() <= 67108864,
            old(self).free_count > 0, old(self).cursor_valid(),
            old(self).free_count == old(self).reusable_indices().len(),
        ensures old(self).reusable(result), final(self).live_at(result),
            forall|i: u32| old(self).reusable(i) ==> result <= i,
            forall|i: u32| final(self).live_at(i) == (old(self).live_at(i) || i == result),
            forall|i: u32| final(self).reusable(i) == (old(self).reusable(i) && i != result),
            final(self).reusable_indices() == old(self).reusable_indices().remove(result as int),
            final(self).free_count == final(self).reusable_indices().len(),
            *final(self) == (Self { live: final(self).live, cursor: final(self).cursor,
                free_count: final(self).free_count, ..*old(self) }),
            final(self).valid_marks(), final(self).reuse_layout(), final(self).cursor_valid(),
            final(self).free_count + 1 == old(self).free_count,
            final(self).cursor == result / 64,
            final(self).records@ == old(self).records@,
            final(self).marks@ == old(self).marks@,
            final(self).live@.len() == old(self).live@.len(),
            final(self).quarantine_unchanged(old(self)),
            final(self).extra == old(self).extra,
            final(self).live_count == old(self).live_count,
    ''', 'result', loops={'loop': '''
        invariant *self == *old(self), self.valid_marks(), self.reuse_layout(),
            self.live@.len() <= 67108864, self.free_count > 0,
            self.free_count == self.reusable_indices().len(),
            exists|i: u32| self.reusable(i),
            forall|i: u32| self.reusable(i) ==> i / 64 >= word,
        decreases self.live@.len() - word,
    '''}, before={
    'let mut word = self.cursor;': """
        broadcast use vstd::set_lib::range_set_properties;
        let slots = self.reusable_indices();
        assert(slots.len() > 0);
        let witness = slots.choose();
        assert(slots.contains(witness));
        assert(self.reusable(witness as u32));
        assert(exists|i: u32| self.reusable(i));
    """,
    'let taken = self.taken(word);': '''
        let witness = choose|i: u32| self.reusable(i);
        assert(word <= witness / 64 < self.live@.len());
    ''',
    'self.live[word] |= 1 << low;': '''
        vstd::std_specs::bits::axiom_u64_trailing_zeros(!taken);
        assert(taken != u64::MAX ==> !taken != 0) by(bit_vector);
        assert(low < 64);
        assert(taken & (1u64 << low) == 0) by(bit_vector)
            requires low < 64, ((!taken) >> (low as u64)) & 1u64 == 1u64;
        assert forall|j: u32| j < low implies taken & (1u64 << j) != 0 by {
            assert(((!taken) >> (j as u64)) & 1u64 == 0u64);
            assert(taken & (1u64 << j) != 0) by(bit_vector)
                requires j < low < 64, ((!taken) >> (j as u64)) & 1u64 == 0u64;
        }
        let witness = choose|i: u32| self.reusable(i);
        if word == witness / 64 {
            assert(low <= witness % 64);
        }
        assert(word * 64 + low <= witness);
        assert(word * 64 + low < self.records@.len());
    ''',
    'word += 1;': '''
        assert forall|i: u32| self.reusable(i) implies i / 64 > word by {
            if i / 64 == word {
                assert(u64::MAX & (1u64 << (i % 64)) != 0) by(bit_vector);
            }
        }
    ''',
    'return index;': '''
        assert(index / 64 == word && index % 64 == low);
        assert(old(self).reusable(index));
        assert forall|i: u32| old(self).reusable(i) implies index <= i by {
            if i / 64 == word {
                assert(i % 64 >= low);
            }
        }
        assert forall|i: u32| self.live_at(i) == (old(self).live_at(i) || i == index) by {
            if i / 64 == word {
                if i == index {
                    let original = old(self).live@[word as int];
                    assert((original | (1u64 << low)) & (1u64 << low) != 0)
                        by(bit_vector) requires low < 64;
                } else {
                    preserve_other_bit(old(self).live@[word as int], index, i);
                }
            }
        }
        assert forall|i: u32| self.live_at(i) implies i < self.records@.len() by {
            if i != index { assert(old(self).live_at(i)); }
        }
        assert forall|i: u32| self.reusable(i) == (old(self).reusable(i) && i != index) by {
            if i / 64 == word {
                let original = old(self).live@[word as int];
                #[cfg(feature = "gc-stress")]
                let q = self.quarantine@[word as int];
                #[cfg(not(feature = "gc-stress"))]
                let q = 0u64;
                assert(original | 0u64 == original) by(bit_vector);
                assert((original | (1u64 << low)) | q == (original | q) | (1u64 << low)) by(bit_vector);
                if i == index {
                    assert(((original | q) | (1u64 << low)) & (1u64 << low) != 0)
                        by(bit_vector) requires low < 64;
                } else {
                    preserve_other_bit(original | q, index, i);
                }
            }
        }
        broadcast use vstd::set_lib::range_set_properties;
        assert(self.reusable_indices() =~= old(self).reusable_indices().remove(index as int)) by {
            assert forall|i: int| self.reusable_indices().contains(i)
                == old(self).reusable_indices().remove(index as int).contains(i) by {
                if 0 <= i <= u32::MAX { assert((i as u32) as int == i); }
            }
        }
        assert(old(self).reusable_indices().contains(index as int));
        assert(self.free_count == self.reusable_indices().len());
        assert forall|i: u32| self.reusable(i) implies i / 64 >= self.cursor by {
            if i / 64 < word { assert(old(self).reusable(i)); }
        }
    '''
    })
    marker = "impl<T> Arena<T> {"
    if arena_code.count(marker) != 1:
        raise ValueError("arena reuse proof lost its unique composition point")
    code = arena_code.replace(marker, marker + "\n" + specs + taken + take, 1)
    witness = r'''

fn initialized_arena<T>() -> (result: Arena<T>)
    ensures result.valid_marks(), result.reuse_layout(), result.cursor_valid(),
        result.reusable_indices().is_empty(), result.free_count == result.reusable_indices().len(),
        result.records@.len() == 0, result.live_count == 0,
{
    let arena = Arena::new();
    proof {
        broadcast use vstd::set_lib::range_set_properties;
        assert(arena.reusable_indices() =~= Set::<int>::empty());
    }
    arena
}

fn marking_preserves_reuse<T>(arena: &mut Arena<T>, index: u32)
    requires old(arena).valid_marks(), old(arena).reuse_layout(),
        old(arena).cursor_valid(), old(arena).live_at(index),
        old(arena).free_count == old(arena).reusable_indices().len(),
    ensures final(arena).valid_marks(), final(arena).reuse_layout(), final(arena).cursor_valid(),
        final(arena).free_count == final(arena).reusable_indices().len(),
        final(arena).reusable_indices() == old(arena).reusable_indices(),
        *final(arena) == (Arena { marks: final(arena).marks, ..*old(arena) }),
        forall|i: u32| !final(arena).marked_at(i),
{
    arena.mark(index);
    arena.clear_marks();
    assert(arena.reusable_indices() =~= old(arena).reusable_indices());
    assert forall|i: u32| arena.live_at(i) implies i < arena.records@.len() by {
        assert(old(arena).live_at(i));
    }
    assert forall|i: u32| arena.reusable(i) implies i / 64 >= arena.cursor by {
        assert(old(arena).reusable(i));
    }
}

fn reuse_lifecycle() {
    let mut arena = Arena {
        records: vec![10u8, 20u8, 30u8, 40u8], live: vec![5u64], marks: vec![5u64],
        #[cfg(feature = "gc-stress")]
        quarantine: vec![2u64],
        #[cfg(feature = "gc-stress")]
        free_count: 1usize,
        #[cfg(not(feature = "gc-stress"))]
        free_count: 2usize,
        cursor: 0usize, live_count: 2usize, extra: 0usize,
    };
    proof {
        broadcast use vstd::set_lib::range_set_properties;
        assert forall|i: u32| arena.live_at(i) implies i < 4 by {
            assert(5u64 & (1u64 << (i % 64)) != 0 && i / 64 == 0 ==> i == 0 || i == 2) by(bit_vector);
        }
        #[cfg(feature = "gc-stress")]
        assert(arena.reusable_indices() =~= set![3int]) by {
            assert forall|i: int| 0 <= i < 4 implies
                #[trigger] arena.reusable(i as u32) == (i == 3) by {
                let j = i as u32;
                assert(5u64 | 2u64 == 7u64) by(bit_vector);
                assert(arena.occupied(0) == 7u64);
                assert(j / 64 == 0);
                assert(j < 4 ==> (7u64 & (1u64 << (j % 64)) == 0) == (j == 3)) by(bit_vector);
            }
        }
        #[cfg(not(feature = "gc-stress"))]
        assert(arena.reusable_indices() =~= set![1int, 3int]) by {
            assert forall|i: int| 0 <= i < 4 implies
                #[trigger] arena.reusable(i as u32) == (i == 1 || i == 3) by {
                let j = i as u32;
                assert(j < 4 ==> (5u64 & (1u64 << (j % 64)) == 0) == (j == 1 || j == 3)) by(bit_vector);
            }
        }
    }
    let ghost original = arena;
    let index = arena.take_free();
    #[cfg(feature = "gc-stress")]
    { proof {
        broadcast use vstd::set_lib::range_set_properties;
        assert(original.reusable_indices().contains(index as int));
        assert(index == 3);
        assert(5u64 & (1u64 << 1u32) == 0) by(bit_vector);
        assert(!original.live_at(1));
        assert(!arena.live_at(1));
    } }
    #[cfg(not(feature = "gc-stress"))]
    {
        proof {
            assert(original.reusable_indices().contains(1));
            assert(original.reusable(1));
            assert(index <= 1);
            assert(index == 1);
        }
        let second = arena.take_free();
        proof { assert(second == 3); }
    }
    proof { assert(arena.free_count == 0); assert(arena.marks@ == seq![5u64]); }
}

'''
    closing = "\n}\n}\nfn main() {}"
    if code.count(closing) != 1:
        raise ValueError("arena reuse witness lost its unique composition point")
    return code.replace(closing, "\n" + witness + closing)
