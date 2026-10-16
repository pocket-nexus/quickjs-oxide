//! Object headers and the kind-specific payload arenas.

use super::arena::{Arena, Record};
use super::values::Range;
use crate::value::ShapeId;

/// Sentinel for an absent index in a `u32` payload field.
pub const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjectKind {
    Ordinary,
    Array,
    Function,
    /// A closure environment: `slots` holds the variables, `payload` the
    /// parent scope (or [`NONE`]).
    Scope,
    GlobalObject,
    /// Placeholder for the builtin kinds ported in phase 4.
    Native,
    /// Poison written over a freed header.
    Free,
}

pub mod flags {
    pub const EXTENSIBLE: u8 = 1 << 0;
    pub const USED_AS_PROTOTYPE: u8 = 1 << 1;
    pub const CALLABLE: u8 = 1 << 2;
    pub const CONSTRUCTOR: u8 = 1 << 3;
    pub const DICTIONARY: u8 = 1 << 4;
}

/// One fixed-size record per object (20 bytes).
#[derive(Clone, Copy, Debug)]
pub struct ObjectHeader {
    pub shape: ShapeId,
    pub slots_start: u32,
    pub elements_start: u32,
    /// Index into the kind's side arena, or an inline payload: the array
    /// length, the parent scope.
    pub payload: u32,
    pub slots_log: u8,
    pub elements_log: u8,
    pub kind: ObjectKind,
    pub flags: u8,
}

const _: () = assert!(size_of::<ObjectHeader>() <= 24);

impl ObjectHeader {
    pub fn new(
        kind: ObjectKind,
        shape: ShapeId,
        slots: Range,
        elements: Range,
        payload: u32,
        flags: u8,
    ) -> Self {
        ObjectHeader {
            shape,
            slots_start: slots.start,
            elements_start: elements.start,
            payload,
            slots_log: slots.log(),
            elements_log: elements.log(),
            kind,
            flags,
        }
    }

    #[inline]
    pub fn slots(&self) -> Range {
        Range::from_log(self.slots_start, self.slots_log)
    }

    #[inline]
    pub fn set_slots(&mut self, range: Range) {
        self.slots_start = range.start;
        self.slots_log = range.log();
    }

    #[inline]
    pub fn elements(&self) -> Range {
        Range::from_log(self.elements_start, self.elements_log)
    }

    #[inline]
    pub fn set_elements(&mut self, range: Range) {
        self.elements_start = range.start;
        self.elements_log = range.log();
    }

    #[inline]
    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

impl Record for ObjectHeader {
    fn vacant() -> Self {
        ObjectHeader::new(
            ObjectKind::Free,
            ShapeId(NONE),
            Range::EMPTY,
            Range::EMPTY,
            NONE,
            0,
        )
    }
}

/// The payload of a `Function` object.
#[derive(Clone, Copy, Debug)]
pub struct FunctionRecord {
    /// The captured scope object, or [`NONE`].
    pub scope: u32,
    /// A [`FunctionBytecode`] index, or [`NONE`].
    pub bytecode: u32,
}

impl Record for FunctionRecord {
    fn vacant() -> Self {
        FunctionRecord {
            scope: NONE,
            bytecode: NONE,
        }
    }
}

/// The heap side of one published code unit, shared by every closure over
/// it: the constant table, and the records of the unit's children (as
/// `Int32` record indices) so that a live function keeps every function it
/// can create alive. Both are value-heap ranges, traced like everything else.
#[derive(Clone, Copy, Debug)]
pub struct FunctionBytecode {
    /// The [`CodeUnitId`](crate::bytecode::CodeUnitId) index.
    pub unit: u32,
    pub constants: Range,
    pub constant_count: u32,
    pub children: Range,
    pub child_count: u32,
}

impl Record for FunctionBytecode {
    fn vacant() -> Self {
        FunctionBytecode {
            unit: NONE,
            constants: Range::EMPTY,
            constant_count: 0,
            children: Range::EMPTY,
            child_count: 0,
        }
    }
}

#[derive(Default)]
pub struct PayloadArenas {
    pub functions: Arena<FunctionRecord>,
    pub bytecode: Arena<FunctionBytecode>,
}
