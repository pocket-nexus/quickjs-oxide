"""Exact weak-table tombstoning with full table/variant/slot framing."""

from records import record, tuple_enum
from source import item, method


def proof(table_source, object_source, value_code, roots_code, index_code):
    parts = [item(roots_code, r"const SPECIAL_EMPTY\b"),
             item(roots_code, r"spec fn boxed_spec\b"),
             item(roots_code, r"impl Value\b")]
    prefix = "use vstd::prelude::*;\nverus! {\n"
    suffix = "}\nfn main() {}\n"
    if not index_code.startswith(prefix) or not index_code.endswith(suffix):
        raise ValueError("Table index proof module boundary changed")
    parts.append(index_code.removeprefix(prefix).removesuffix(suffix))
    data, dp, _ = tuple_enum(object_source, "InternalData", "D", {
        "Table": ("Box<super::table::Table>", "Box<Table<C>>"),
    })
    parameters = ["C"] + dp
    data_type = "InternalData<" + ", ".join(parameters) + ">"
    parts.append(data.replace("enum InternalData", "enum " + data_type, 1))
    internal, ip, _ = record(object_source, "Internal", "I", {
        "slots": ("Vec<Value>", "Vec<Value>"), "data": ("InternalData", data_type),
    })
    generics = ", ".join(parameters + ip)
    parts.append(internal.replace("struct Internal", "struct Internal<" + generics + ">", 1))
    contract = '''
        requires old(internal).data is Table,
            (old(internal).data->Table_0).index_capacity(),
            entry < DELETED as usize - 1,
            entry < (old(internal).data->Table_0).hashes@.len(),
            (old(internal).data->Table_0).live > 0,
            0 < (old(internal).data->Table_0).width,
            entry * (old(internal).data->Table_0).width + (old(internal).data->Table_0).width <= old(internal).slots@.len(),
            exists|i: int| 0 <= i < (old(internal).data->Table_0).index@.len()
                && (old(internal).data->Table_0).index@[i] == entry + 1,
        ensures final(internal).data is Table,
            *final(internal) == (Internal { data: final(internal).data, slots: final(internal).slots, ..*old(internal) }),
            *(final(internal).data->Table_0) == (Table { index: (final(internal).data->Table_0).index,
                live: (final(internal).data->Table_0).live, ..*(old(internal).data->Table_0) }),
            (final(internal).data->Table_0).live == (old(internal).data->Table_0).live - 1,
            exists|i: int| 0 <= i < (old(internal).data->Table_0).index@.len()
                && (old(internal).data->Table_0).index@[i] == entry + 1
                && (final(internal).data->Table_0).index@ == (old(internal).data->Table_0).index@.update(i, DELETED),
            final(internal).slots@.len() == old(internal).slots@.len(),
            forall|i: int| 0 <= i < final(internal).slots@.len() ==> final(internal).slots@[i] ==
                if entry * (old(internal).data->Table_0).width <= i < entry * (old(internal).data->Table_0).width + (old(internal).data->Table_0).width
                    { Value::EMPTY } else { old(internal).slots@[i] },
    '''
    parts.append('''impl<C> Table<C> {
        spec fn found(&self, entry: usize) -> int {
            choose|i: int| 0 <= i < self.index@.len() && self.index@[i] == entry + 1
        }
    }''')
    delete = method(table_source, "tombstone", contract, loops={
        "while table.index[bucket] != entry as u32 + 1": '''
     invariant old(internal).data is Table,
         **table == *(old(internal).data->Table_0), table.index_capacity(),
         width == table.width, width > 0,
         internal.slots@ == old(internal).slots@,
         entry * width + width <= internal.slots@.len(), internal.slots@.len() <= usize::MAX,
         0 <= table.found(entry) < table.index@.len(), table.index@[table.found(entry)] == entry + 1,
         mask + 1 == table.index@.len(), bucket <= mask, ((mask + 1) as usize) & mask == 0,
     decreases if bucket <= table.found(entry) { table.found(entry) - bucket }
         else { table.index@.len() - bucket + table.found(entry) },
    ''',
        "while position < slots.len()": '''
     invariant position <= slots@.len(), slots@.len() == width,
         forall|i: int| 0 <= i < position ==> slots@[i] == Value::EMPTY,
     decreases slots@.len() - position,
    '''
    }, before={
        "while table.index[bucket] != entry as u32 + 1": '''
     assert(internal.slots@.len() == internal.slots.len());
     assert(bucket <= mask) by(bit_vector) requires bucket == hash as usize & mask;
    ''',
        "bucket = (bucket + 1) & mask;": '''
     assert(bucket != table.found(entry));
     probe_cycle(mask, bucket);
    '''
    })
    delete = delete.replace("fn tombstone(", "fn tombstone<" + generics + ">(", 1)
    delete = delete.replace("super::object::Internal,", "super::object::Internal<" + generics + ">,", 1)
    parts.append(delete)
    marker = "pub fn main() {}"
    if value_code.count(marker) != 1:
        raise ValueError("Deletion proof lost its unique Value composition point")
    code = value_code.replace(marker, "\n".join(parts) + "\n" + marker)
    return ('mod object { pub(crate) use crate::heap_proof::Internal; }\nmod heap_proof {\n'
            + code + '\n}\nfn main() {}\n')
