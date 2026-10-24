//! Regular-expression objects: the program store and the `RegExp` object kind.
//! A literal allocates a new object on every evaluation, as the specification
//! requires, over one compiled program per distinct `(source, flags)`, compiled
//! on first use; `new RegExp(source, flags)` shares the store, so a program
//! built in a loop does not grow it without bound. The store lives in the heap:
//! a `RegExp` object's payload names its program, a collection marks the
//! programs of the live objects (and their two atoms), and sweeps the rest.

use oxide_regexp::CompiledRegExp;

use super::error::{self, ErrorKind};
use super::{Cx, Throw};
use crate::heap::hash::FxHashMap;
use crate::heap::object::ObjectKind;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{BytecodeId, StringId, Value};

/// One distinct `(source, flags)` pair, both atoms, which a live program
/// keeps alive.
#[derive(Clone, Copy, Debug)]
pub struct RegExpRecord {
    pub source: StringId,
    pub flags: StringId,
}

#[derive(Default)]
pub struct RegExpStore {
    /// By program index; `None` for a free slot.
    programs: Vec<Option<(RegExpRecord, CompiledRegExp)>>,
    index: FxHashMap<(StringId, StringId), u32>,
    /// Free slots, reused lowest first.
    free: Vec<u32>,
    /// Under `gc-stress`, slots freed by a sweep are reusable only after the
    /// next one.
    #[cfg(feature = "gc-stress")]
    quarantine: Vec<u32>,
    marks: Vec<u64>,
    live: usize,
    bytes: usize,
}

/// Bytes a program holds.
fn program_bytes(program: &CompiledRegExp) -> usize {
    size_of::<Option<(RegExpRecord, CompiledRegExp)>>()
        + std::mem::size_of_val(program.instructions())
        + program.group_names().map_or(0, |names| {
            std::mem::size_of_val(names)
                + names
                    .iter()
                    .flatten()
                    .map(|name| std::mem::size_of_val(&**name))
                    .sum::<usize>()
        })
}

impl RegExpStore {
    /// The program for `(source, flags)`, compiled and stored on first use;
    /// the compiler's diagnostic when the pattern or the flags are invalid.
    /// It allocates nothing on the heap: the caller stores the index in a
    /// `RegExp` object before its next allocation.
    pub(crate) fn intern(
        &mut self,
        strings: &crate::heap::arena::Arena<crate::heap::string::StringRecord>,
        source: StringId,
        flags: StringId,
    ) -> Result<u32, String> {
        if let Some(&index) = self.index.get(&(source, flags)) {
            return Ok(index);
        }
        let units = |id: StringId| -> Vec<u16> {
            strings
                .get(id.index())
                .flat()
                .expect("an atom is flat")
                .units()
                .collect()
        };
        let program = oxide_regexp::compile(&units(source), &units(flags))
            .map_err(|error| oxide_regexp::javascript_compile_error_message(&error).to_owned())?;
        self.bytes += program_bytes(&program);
        self.live += 1;
        let entry = Some((RegExpRecord { source, flags }, program));
        let index = match self.free.pop() {
            Some(index) => {
                self.programs[index as usize] = entry;
                index
            }
            None => {
                self.programs.push(entry);
                let index = (self.programs.len() - 1) as u32;
                if index as usize / 64 == self.marks.len() {
                    self.marks.push(0);
                }
                index
            }
        };
        self.index.insert((source, flags), index);
        Ok(index)
    }

    #[track_caller]
    fn entry(&self, index: u32) -> &(RegExpRecord, CompiledRegExp) {
        self.programs[index as usize]
            .as_ref()
            .expect("a live regular-expression program")
    }

    pub fn get(&self, index: u32) -> RegExpRecord {
        self.entry(index).0
    }

    pub fn program(&self, index: u32) -> &CompiledRegExp {
        &self.entry(index).1
    }

    /// Live programs.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    #[cfg(any(test, feature = "gc-stress"))]
    pub(crate) fn audit_records(&self) -> impl Iterator<Item = (u32, RegExpRecord, bool)> + '_ {
        self.programs
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|(record, _)| (index as u32, *record, self.is_marked(index as u32)))
            })
    }

    /// Bytes the live programs hold.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn clear_marks(&mut self) {
        self.marks.fill(0);
    }

    /// Marks program `index`; its record if it was not marked yet.
    #[inline]
    pub(crate) fn mark(&mut self, index: u32) -> Option<RegExpRecord> {
        let (word, mask) = ((index / 64) as usize, 1u64 << (index % 64));
        if self.marks[word] & mask != 0 {
            return None;
        }
        self.marks[word] |= mask;
        Some(self.get(index))
    }

    fn is_marked(&self, index: u32) -> bool {
        self.marks[(index / 64) as usize] & (1u64 << (index % 64)) != 0
    }

    /// Frees every unmarked program.
    pub(crate) fn sweep(&mut self) {
        #[cfg(feature = "gc-stress")]
        let held = {
            let released = std::mem::take(&mut self.quarantine);
            self.free.extend_from_slice(&released);
            self.free.len()
        };
        for index in (0..self.programs.len() as u32).rev() {
            if self.is_marked(index) {
                continue;
            }
            let Some((record, program)) = self.programs[index as usize].take() else {
                continue;
            };
            self.index.remove(&(record.source, record.flags));
            self.bytes -= program_bytes(&program);
            self.live -= 1;
            self.free.push(index);
        }
        #[cfg(feature = "gc-stress")]
        self.quarantine.extend(self.free.drain(held..));
    }

    /// Debug check after marking: exactly the live programs are marked.
    pub(crate) fn verify(&self) {
        for (index, program) in self.programs.iter().enumerate() {
            assert_eq!(
                program.is_some(),
                self.is_marked(index as u32),
                "gc verify: regular-expression program {index} is {}",
                if program.is_some() {
                    "live but unreachable"
                } else {
                    "reachable but freed"
                }
            );
        }
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
    regexp_object(cx, source, flags)
}

/// A new `RegExp` object over `(source, flags)` (atoms), compiling them on
/// first use: a `SyntaxError` for an invalid pattern or flags.
pub(crate) fn regexp_object(
    cx: &mut Cx<'_>,
    source: StringId,
    flags: StringId,
) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    regexp_object_with(cx, source, flags, prototype)
}

/// Gives `object` the program for `(source, flags)`: a `SyntaxError` when
/// invalid. The program is compiled without allocating, so the object names
/// it before any collection.
pub(crate) fn set_program(
    cx: &mut Cx<'_>,
    object: crate::value::ObjectId,
    source: StringId,
    flags: StringId,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    match heap.regexps.intern(&heap.strings, source, flags) {
        Ok(program) => {
            heap.set_payload(object, program);
            Ok(())
        }
        Err(message) => Err(error::throw(cx, ErrorKind::Syntax, &message)),
    }
}

/// [`regexp_object`] with `prototype` (a subclass's, from `new.target`).
pub(crate) fn regexp_object_with(
    cx: &mut Cx<'_>,
    source: StringId,
    flags: StringId,
    prototype: crate::value::ObjectId,
) -> Result<Value, Throw> {
    let names = cx.realm.names;
    let heap = &mut *cx.heap;
    let mark = heap.roots().mark();
    heap.roots().push(Value::string(source));
    heap.roots().push(Value::string(flags));
    let object = heap.alloc_object(ObjectKind::RegExp, Some(prototype), 1);
    heap.roots().truncate(mark);
    let object = object.map_err(|error| error::heap_error(cx, error))?;
    set_program(cx, object, source, flags)?;
    let heap = &mut *cx.heap;
    heap.add_property(
        object,
        Names::key(names.last_index),
        PropertyFlags::data(true, false, false),
        Value::int32(0),
    )
    .map_err(|error| error::heap_error(cx, error))?;
    if prototype == cx.realm.intrinsic(Intrinsic::RegExpPrototype)
        && cx.realm.regexp_instance_shape.get().is_none()
    {
        let (slot, _) = cx
            .heap
            .lookup_own(object, Names::key(names.last_index))
            .expect("lastIndex");
        cx.realm
            .regexp_instance_shape
            .set(Some((cx.heap.shape_of(object), slot)));
    }
    Ok(Value::object(object))
}

#[cfg(test)]
mod tests {
    #[test]
    fn unmarked_programs_are_freed_and_their_keys_forgotten() {
        let mut heap = crate::Heap::default();
        let source = heap.intern("a+b").unwrap();
        let other = heap.intern("c").unwrap();
        let flags = heap.intern("g").unwrap();
        let store = &mut heap.regexps;
        let kept = store.intern(&heap.strings, source, flags).unwrap();
        store.intern(&heap.strings, other, flags).unwrap();
        assert_eq!(store.intern(&heap.strings, source, flags), Ok(kept));
        assert_eq!(store.len(), 2);
        let bytes = store.bytes();
        store.clear_marks();
        assert!(store.mark(kept).is_some());
        assert!(store.mark(kept).is_none(), "marked once");
        store.sweep();
        assert_eq!(store.len(), 1);
        assert!(store.bytes() < bytes);
        assert_eq!(store.get(kept).source, source);
        // The dropped pair compiles again on its next use.
        let again = store.intern(&heap.strings, other, flags).unwrap();
        assert_eq!(store.get(again).source, other);
        assert_ne!(again, kept);
        assert!(
            store.intern(&heap.strings, flags, source).is_err(),
            "invalid flags"
        );
    }
}
