"""Exact collection-index insertion: bounded cyclic probing and precise update."""

from records import record
from source import item, method


def proof(source):
    declaration, parameters, _ = record(source, "Table", "T", {
        "index": ("Vec<u32>", "Vec<u32>"),
        "hashes": ("Vec<u32>", "Vec<u32>"),
        "cursors": ("Vec<Weak<Cell<u32>>>", "Vec<C>"),
    })
    parameters = ["C"] + parameters
    generic = ", ".join(parameters)
    declaration = declaration.replace("struct Table", "struct Table<" + generic + ">", 1)
    scope = item(source, r"impl Table\b")
    cycle = '''
    spec fn distance(start: int, position: int, size: nat) -> int {
        if start <= position { position - start } else { size - start + position }
    }

    proof fn probe_cycle(mask: usize, bucket: usize)
        requires bucket <= mask, ((mask + 1) as usize) & mask == 0,
        ensures (((bucket + 1) as usize) & mask) == if bucket == mask { 0usize } else { (bucket + 1) as usize },
    {
        assert((((bucket + 1) as usize) & mask) == if bucket == mask { 0usize } else { (bucket + 1) as usize }) by(bit_vector)
            requires bucket <= mask, ((mask + 1) as usize) & mask == 0;
    }
    '''
    specs = '''
        spec fn available(&self, i: int) -> bool {
            0 <= i < self.index@.len() && (self.index@[i] == VACANT || self.index@[i] == DELETED)
        }
        spec fn destination(&self) -> int {
            choose|i: int| self.available(i)
        }
        spec fn index_capacity(&self) -> bool {
            let n = self.index@.len();
            n > 0 && n <= usize::MAX && (n as usize) & ((n - 1) as usize) == 0
        }
    '''
    insert = method(scope, "insert_index", '''
        requires old(self).index_capacity(), entry < DELETED as usize - 1,
            exists|i: int| old(self).available(i),
        ensures *final(self) == (Self { index: final(self).index, ..*old(self) }),
            final(self).index_capacity(), final(self).index@.len() == old(self).index@.len(),
            exists|i: int| old(self).available(i)
                && final(self).index@ == old(self).index@.update(i, (entry + 1) as u32)
                && (forall|j: int| old(self).available(j) ==>
                    distance((hash as usize & ((old(self).index@.len() - 1) as usize)) as int, i, old(self).index@.len())
                    <= distance((hash as usize & ((old(self).index@.len() - 1) as usize)) as int, j, old(self).index@.len())),
            0 < entry + 1 < DELETED,
    ''', loops={"while self.index[bucket] != VACANT && self.index[bucket] != DELETED": '''
        invariant *self == *old(self), self.index_capacity(), self.available(self.destination()),
            mask + 1 == self.index@.len(), bucket <= mask,
            ((mask + 1) as usize) & mask == 0,
            hash as usize & mask <= mask,
            forall|i: int| self.available(i) ==>
                distance((hash as usize & mask) as int, bucket as int, self.index@.len())
                <= distance((hash as usize & mask) as int, i, self.index@.len()),
        decreases if bucket <= self.destination() { self.destination() - bucket }
            else { self.index@.len() - bucket + self.destination() },
    '''}, before={"while self.index[bucket] != VACANT && self.index[bucket] != DELETED": '''
        assert(bucket <= mask) by(bit_vector) requires bucket == hash as usize & mask;
    ''', "bucket = (bucket + 1) & mask;": '''
        assert(bucket != self.destination());
        let start = (hash as usize & mask) as int;
        assert forall|i: int| self.available(i) implies
            distance(start, bucket as int, self.index@.len()) < distance(start, i, self.index@.len()) by {
            assert(i != bucket);
        }
        assert(distance(start, bucket as int, self.index@.len()) < self.index@.len() - 1);
        probe_cycle(mask, bucket);
    '''}, after={"self.index[bucket] = entry as u32 + 1;": '''
        assert(old(self).available(bucket as int));
        assert(self.index@ == old(self).index@.update(bucket as int, (entry + 1) as u32));
    '''})
    methods = []
    for name, contract in (
        ("new", "ensures result.width == width, result.live == 0, result.index@.len() == 0, result.hashes@.len() == 0, result.cursors@.len() == 0,"),
        ("len", "ensures result == self.live,"),
        ("is_empty", "ensures result == (self.live == 0),"),
        ("width", "ensures result == self.width,"),
        ("entries", "ensures result == self.hashes@.len(),"),
    ):
        methods.append(method(scope, name, contract, "result"))
    return ('use vstd::prelude::*;\nverus! {\n' + declaration + '\n'
        + item(source, r"const VACANT\b") + '\n' + item(source, r"const DELETED\b")
        + cycle + '\nimpl<' + generic + '> Table<' + generic + '> {\n'
        + specs + insert + '\n' + '\n'.join(methods) + '\n}\n}\nfn main() {}\n')
