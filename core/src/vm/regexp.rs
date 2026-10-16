//! Regular-expression objects (phase 3, section 5.3): the program store and
//! the `RegExp` object kind. A literal allocates a new object on every
//! evaluation, as the specification requires, over one program per distinct
//! `(source, flags)`; `new RegExp(source, flags)` shares the store, so a
//! program built in a loop does not grow it without bound. The store is
//! append-only, like the code store. M4 compiles the programs.

use super::error;
use super::{Cx, Throw};
use crate::heap::hash::FxHashMap;
use crate::heap::object::ObjectKind;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{BytecodeId, StringId, Value};

/// One distinct `(source, flags)` pair. Both are atoms, which are strong
/// roots.
#[derive(Clone, Copy, Debug)]
pub struct RegExpRecord {
    pub source: StringId,
    pub flags: StringId,
}

#[derive(Default)]
pub struct RegExpStore {
    records: Vec<RegExpRecord>,
    index: FxHashMap<(StringId, StringId), u32>,
}

impl RegExpStore {
    /// The record for `(source, flags)`, appended on first use.
    pub fn intern(&mut self, source: StringId, flags: StringId) -> u32 {
        *self.index.entry((source, flags)).or_insert_with(|| {
            self.records.push(RegExpRecord { source, flags });
            (self.records.len() - 1) as u32
        })
    }

    pub fn get(&self, index: u32) -> RegExpRecord {
        self.records[index as usize]
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// `NewRegExp`: a new `RegExp` object over the program for the literal's
/// `(source, flags)`, constants `constant` and `constant + 1` of the running
/// function, with an own writable `lastIndex` of 0.
#[inline(never)]
pub(crate) fn new_regexp(
    cx: &mut Cx<'_>,
    bytecode: BytecodeId,
    constant: u32,
) -> Result<Value, Throw> {
    let source = cx.heap.bytecode_constant(bytecode, constant);
    let flags = cx.heap.bytecode_constant(bytecode, constant + 1);
    let source = source.as_string().expect("a source atom");
    let flags = flags.as_string().expect("a flags atom");
    let program = cx.regexps.intern(source, flags);
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    let names = cx.realm.names;
    let heap = &mut *cx.heap;
    let object = heap
        .alloc_object(ObjectKind::RegExp, Some(prototype), 1)
        .and_then(|object| {
            heap.set_payload(object, program);
            heap.add_property(
                object,
                Names::key(names.last_index),
                PropertyFlags::data(true, false, false),
                Value::int32(0),
            )?;
            Ok(object)
        });
    object
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}
