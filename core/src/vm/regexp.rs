//! Regular-expression objects: the program store and the `RegExp` object kind.
//! A literal allocates a new object on every evaluation, as the specification
//! requires, over one compiled program per distinct `(source, flags)`, compiled
//! on first use; `new RegExp(source, flags)` shares the store, so a program
//! built in a loop does not grow it without bound. The store is append-only,
//! like the code store.

use oxide_regexp::CompiledRegExp;

use super::error::{self, ErrorKind};
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
    programs: Vec<CompiledRegExp>,
    index: FxHashMap<(StringId, StringId), u32>,
}

impl RegExpStore {
    /// The record for `(source, flags)`, compiled and appended on first use;
    /// the compiler's diagnostic when the pattern or the flags are invalid.
    pub(crate) fn intern(
        &mut self,
        heap: &crate::Heap,
        source: StringId,
        flags: StringId,
    ) -> Result<u32, String> {
        if let Some(&index) = self.index.get(&(source, flags)) {
            return Ok(index);
        }
        let pattern: Vec<u16> = heap.string(source).repr().units().collect();
        let flag_units: Vec<u16> = heap.string(flags).repr().units().collect();
        let program = oxide_regexp::compile(&pattern, &flag_units)
            .map_err(|error| oxide_regexp::javascript_compile_error_message(&error).to_owned())?;
        self.records.push(RegExpRecord { source, flags });
        self.programs.push(program);
        let index = (self.records.len() - 1) as u32;
        self.index.insert((source, flags), index);
        Ok(index)
    }

    pub fn get(&self, index: u32) -> RegExpRecord {
        self.records[index as usize]
    }

    pub fn program(&self, index: u32) -> &CompiledRegExp {
        &self.programs[index as usize]
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

/// The program for `(source, flags)`: a `SyntaxError` when invalid.
pub(crate) fn program(cx: &mut Cx<'_>, source: StringId, flags: StringId) -> Result<u32, Throw> {
    match cx.regexps.intern(cx.heap, source, flags) {
        Ok(program) => Ok(program),
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
    let program = program(cx, source, flags)?;
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
    let object = object.map_err(|error| error::heap_error(cx, error))?;
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
