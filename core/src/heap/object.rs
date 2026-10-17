//! Object headers and the kind-specific payload arenas.

use super::arena::{Arena, Record};
use super::values::Range;
use crate::value::{ShapeId, Value};

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
    /// An `arguments` object (phase 3, section 5.1). Its elements hold the
    /// values of unmapped indices; a mapped one's `payload` is a scope object
    /// whose slot 0 is the mapped count and slots 1.. the parameters' cells,
    /// `Value::EMPTY` once an index is unmapped. An unmapped one's payload is
    /// [`NONE`].
    Arguments,
    /// A regular expression: `payload` indexes the runtime's program store;
    /// `lastIndex` is an own data property.
    RegExp,
    /// A `for-in` enumeration (section 5.4), never exposed to JavaScript: the
    /// elements are the key snapshot, slot 0 the object, slot 1 the key
    /// count, `payload` the cursor.
    ForInIterator,
    /// Placeholder for the builtin kinds ported in phase 4.
    Native,
    /// `new String(s)` (phase 4, section 3.2): internal slot 0 the string.
    /// Its indices are exotic; `length` is an own data property.
    StringWrapper,
    /// `new Number(n)`: internal slot 0 the number.
    NumberWrapper,
    /// `new Boolean(b)`: internal slot 0 the boolean.
    BooleanWrapper,
    /// `Object(symbol)`: internal slot 0 the symbol.
    SymbolWrapper,
    /// `Object(bigint)`: internal slot 0 the BigInt.
    BigIntWrapper,
    /// A `Date`: internal slot 0 the time value.
    Date,
    /// A bound function: internal slots the target, the bound `this`, then
    /// the bound arguments.
    BoundFunction,
    /// A proxy: internal slot 0 the target, slot 1 the handler (`null` once
    /// revoked).
    Proxy,
    /// A typed array (section 3.1): internal slot 0 the buffer, the data
    /// its view. Canonical numeric keys are its elements, never properties.
    TypedArray,
    /// A module namespace (section 5.6): internal slot 0 the module, then
    /// per export (the data's sorted keys) its cell or, for a namespace
    /// export, its module.
    ModuleNamespace,
    /// An object whose internal slots hold builtin state the natives read:
    /// iterators, promises, collections and the like. `Internal::tag` says
    /// which.
    Builtin,
    /// Poison written over a freed header.
    Free,
}

impl ObjectKind {
    /// Kinds whose `payload` indexes an [`Internal`] record.
    #[inline]
    pub const fn has_internal(self) -> bool {
        matches!(
            self,
            ObjectKind::StringWrapper
                | ObjectKind::NumberWrapper
                | ObjectKind::BooleanWrapper
                | ObjectKind::SymbolWrapper
                | ObjectKind::BigIntWrapper
                | ObjectKind::Date
                | ObjectKind::BoundFunction
                | ObjectKind::Proxy
                | ObjectKind::TypedArray
                | ObjectKind::ModuleNamespace
                | ObjectKind::Builtin
        )
    }

    /// Kinds whose index properties may live in the element range.
    #[inline]
    pub const fn has_elements(self) -> bool {
        matches!(
            self,
            ObjectKind::Ordinary
                | ObjectKind::Array
                | ObjectKind::Function
                | ObjectKind::Arguments
                | ObjectKind::RegExp
                | ObjectKind::NumberWrapper
                | ObjectKind::BooleanWrapper
                | ObjectKind::SymbolWrapper
                | ObjectKind::BigIntWrapper
                | ObjectKind::Date
                | ObjectKind::BoundFunction
                | ObjectKind::Builtin
                | ObjectKind::GlobalObject
        )
    }

    /// Kinds whose internal methods are not the ordinary ones for every key
    /// (section 3.1): property lookups that meet one leave the heap-only
    /// fast paths for `vm::property`'s dispatch.
    #[inline]
    pub const fn is_exotic(self) -> bool {
        matches!(
            self,
            ObjectKind::StringWrapper
                | ObjectKind::Proxy
                | ObjectKind::TypedArray
                | ObjectKind::ModuleNamespace
        )
    }
}

pub mod flags {
    pub const EXTENSIBLE: u8 = 1 << 0;
    pub const USED_AS_PROTOTYPE: u8 = 1 << 1;
    pub const CALLABLE: u8 = 1 << 2;
    pub const CONSTRUCTOR: u8 = 1 << 3;
    pub const DICTIONARY: u8 = 1 << 4;
    /// An error object (`new_error` sets it), so `Object.prototype.toString`
    /// and the error checks need no prototype walk.
    pub const ERROR: u8 = 1 << 5;
    /// Index properties live as named properties only (phase 4, section
    /// 3.1): the element range is empty and stays so. Set when an element
    /// gets attributes other than writable, enumerable, configurable, or the
    /// object stops being extensible.
    pub const SPARSE: u8 = 1 << 6;
    /// An array whose `length` is not writable.
    pub const LENGTH_READONLY: u8 = 1 << 7;
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
    /// A [`Native`](crate::vm::native::Native) index for a host function, or
    /// [`NONE`].
    pub native: u32,
    /// The home object of a method that uses `super` (phase 4, section 5.1),
    /// or [`NONE`].
    pub home_object: u32,
    /// A class constructor's instance-field initializer function, or
    /// [`NONE`].
    pub fields: u32,
}

impl Record for FunctionRecord {
    fn vacant() -> Self {
        FunctionRecord {
            scope: NONE,
            bytecode: NONE,
            native: NONE,
            home_object: NONE,
            fields: NONE,
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

/// The internal state of an object of a kind with [`ObjectKind::has_internal`]:
/// internal slots, traced like properties, and data that holds no values.
#[derive(Debug)]
pub struct Internal {
    /// What the slots mean, for [`ObjectKind::Builtin`] objects.
    pub tag: InternalTag,
    pub slots: Vec<Value>,
    pub data: InternalData,
}

/// Which builtin an [`ObjectKind::Builtin`] object belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InternalTag {
    None,
    ArrayIterator,
    StringIterator,
    MapIterator,
    SetIterator,
    RegExpStringIterator,
    IteratorHelper,
    IteratorConcat,
    WrapForValidIterator,
    AsyncFromSyncIterator,
    Promise,
    /// A `then` record (section 5.4): the capability's promise, resolve and
    /// reject, the two handlers, and the async frame an `await` resumes.
    PromiseReaction,
    Map,
    Set,
    WeakMap,
    WeakSet,
    WeakRef,
    FinalizationRegistry,
    Generator,
    AsyncGenerator,
    AsyncFunction,
    ArrayBuffer,
    SharedArrayBuffer,
    TypedArray,
    DataView,
    ModuleNamespace,
    /// A closure environment for a native function (`FunctionRecord::scope`
    /// of a native): its slots are the native's captured values.
    NativeData,
    /// Test262's `$262.IsHTMLDDA`: falsy, `typeof` "undefined", loosely equal
    /// to `null` and `undefined`, and callable (returning `null`).
    HtmlDda,
    /// A `JSON.rawJSON` object (its brand).
    RawJson,
    /// A module record (section 5.6).
    Module,
}

/// Non-value state of an internal record.
#[derive(Debug, Default)]
pub enum InternalData {
    #[default]
    None,
    Bytes(Vec<u8>),
    /// A suspended generator or async frame (phase 4, section 4.3); its
    /// registers are the record's slots from [`FRAME_SLOTS`].
    Frame(SavedFrame),
    /// A collection's index; its entries are the record's slots.
    Table(Box<super::table::Table>),
    /// A collection iterator's position, moved by compactions.
    Cursor(std::rc::Rc<std::cell::Cell<u32>>),
    /// An `ArrayBuffer` or `SharedArrayBuffer`.
    Buffer(Box<super::buffer::Buffer>),
    /// A typed array's or `DataView`'s window; slot 0 is the buffer.
    View(super::buffer::View),
    /// A module record's state.
    Module(Box<crate::vm::module::ModuleData>),
    /// A module namespace's export names, sorted.
    Exports(Box<[super::atom::PropertyKey]>),
}

/// The internal slots of a suspended activation before its registers: the
/// function, then the kind's own (an async function's promise, an async
/// generator's request queue).
pub const FRAME_SLOTS: usize = 2;

/// The header of a suspended frame: everything `Frame` holds that is not a
/// heap reference (the function is internal slot 0) or a position in the
/// register file.
#[derive(Clone, Copy, Debug)]
pub struct SavedFrame {
    pub unit: u32,
    pub bytecode: u32,
    /// Where it resumes: the instruction after its suspension point.
    pub pc: u32,
    pub argc: u32,
    /// The activation's state (`vm::suspend`'s constants).
    pub state: u8,
}

impl Record for Internal {
    fn vacant() -> Self {
        Internal {
            tag: InternalTag::None,
            slots: Vec::new(),
            data: InternalData::None,
        }
    }
}

impl Internal {
    /// Bytes owned outside the arena: charged to the trigger by the
    /// allocating calls that grow them, not tracked by the arena (the slots
    /// change size after allocation).
    pub fn owned_bytes(&self) -> usize {
        self.slots.capacity() * size_of::<Value>()
            + match &self.data {
                InternalData::None
                | InternalData::Frame(_)
                | InternalData::Cursor(_)
                | InternalData::View(_)
                | InternalData::Module(_) => 0,
                InternalData::Exports(keys) => keys.len() * 8,
                InternalData::Buffer(buffer) => buffer.owned_bytes(),
                InternalData::Bytes(bytes) => bytes.capacity(),
                InternalData::Table(table) => table.owned_bytes(),
            }
    }
}

#[derive(Default)]
pub struct PayloadArenas {
    pub functions: Arena<FunctionRecord>,
    pub bytecode: Arena<FunctionBytecode>,
    pub internals: Arena<Internal>,
}
