//! QuickJS's `JS_PrintValue`: what `print`, `console.log` and an
//! uncaught-exception dump show for a value. It runs no JavaScript: no
//! accessor, proxy trap, coercion or lazy intrinsic. It matches the pinned
//! QuickJS.

use super::Cx;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{InternalData, InternalTag, NONE, ObjectKind, flags};
use crate::runtime::{Names, Runtime};
use crate::shape::PropertyFlags;
use crate::value::{Kind, ObjectId, StringId, Value};

const MAX_DEPTH: usize = 2;
const MAX_STRING_LENGTH: usize = 1_000;
const MAX_ITEM_COUNT: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Comma {
    /// Nothing printed yet.
    Empty,
    /// The next item needs `, `.
    Between,
    /// A `[Function f]`-style prefix: the next item opens ` { `.
    Suffix,
}

enum Body {
    Ordinary,
    /// A dense array: its leading elements, then empty items to `length`.
    /// A sparse one (`sparse`) prints its elements as properties, inside the
    /// brackets.
    Array {
        dense: Vec<Value>,
        length: u32,
        sparse: bool,
    },
    TypedArray,
    Function,
    /// The entries in order, `None` for a zombie (`Table::is_zombie`).
    Map(Vec<Option<(Value, Value)>>, usize),
    Set(Vec<Option<Value>>, usize),
    RegExp,
    Date(f64),
    Error,
}

enum Slot {
    Data(Value),
    Accessor { get: bool, set: bool },
}

struct Printer<'a, 'c> {
    cx: &'a mut Cx<'c>,
    out: Out,
    stack: Vec<ObjectId>,
}

/// The printer's output: UTF-8 text, with the C strings QuickJS writes raw
/// (names, messages, stacks) as WTF-8.
struct Out(Vec<u8>);

impl Out {
    fn push_str(&mut self, text: &str) {
        self.0.extend_from_slice(text.as_bytes());
    }

    fn push(&mut self, character: char) {
        let mut buffer = [0; 4];
        self.push_str(character.encode_utf8(&mut buffer));
    }

    fn extend(&mut self, characters: impl Iterator<Item = char>) {
        for character in characters {
            self.push(character);
        }
    }
}

/// UTF-16 code units as QuickJS's `JS_ToCStringLen` encodes them: UTF-8,
/// a lone surrogate as its own three bytes (WTF-8).
pub(crate) fn wtf8(units: impl Iterator<Item = u16>, out: &mut Vec<u8>) {
    for point in char::decode_utf16(units) {
        match point {
            Ok(point) => {
                let mut buffer = [0; 4];
                out.extend_from_slice(point.encode_utf8(&mut buffer).as_bytes());
            }
            Err(error) => {
                let unit = error.unpaired_surrogate();
                out.extend_from_slice(&[
                    0xE0 | (unit >> 12) as u8,
                    0x80 | ((unit >> 6) & 0x3F) as u8,
                    0x80 | (unit & 0x3F) as u8,
                ]);
            }
        }
    }
}

impl Runtime {
    /// The value as QuickJS's `JS_PrintValue` prints it; a string is quoted
    /// and escaped (`print` writes a top-level string raw instead). A lone
    /// surrogate in a raw C string shows as U+FFFD; [`Self::print_value_bytes`]
    /// keeps it.
    pub fn print_value(&mut self, value: Value) -> String {
        String::from_utf8_lossy(&print_value(&mut self.cx(), value)).into_owned()
    }

    /// [`Self::print_value`] as the bytes QuickJS writes (WTF-8).
    pub fn print_value_bytes(&mut self, value: Value) -> Vec<u8> {
        print_value(&mut self.cx(), value)
    }
}

/// [`Runtime::print_value_bytes`] from inside the engine (`print`).
pub(crate) fn print_value(cx: &mut Cx<'_>, value: Value) -> Vec<u8> {
    let mut printer = Printer {
        cx,
        out: Out(Vec::new()),
        stack: Vec::with_capacity(MAX_DEPTH),
    };
    printer.value(value);
    printer.out.0
}

impl Printer<'_, '_> {
    fn value(&mut self, value: Value) {
        match value.kind() {
            Kind::Undefined => self.out.push_str("undefined"),
            Kind::Null => self.out.push_str("null"),
            Kind::Bool => self.out.push_str(if value.as_bool() == Some(true) {
                "true"
            } else {
                "false"
            }),
            Kind::Int32 | Kind::Number => {
                let number = value.as_f64().unwrap_or(f64::NAN);
                if number == 0.0 && number.is_sign_negative() {
                    self.out.push_str("-0");
                } else {
                    self.out.push_str(&super::number::to_string(number));
                }
            }
            Kind::BigInt => {
                let text = super::ops::big(self.cx.heap, value).to_string_radix10();
                self.out.push_str(&text);
                self.out.push('n');
            }
            Kind::String => self.string(value.as_string().expect("a string")),
            Kind::Symbol => {
                self.out.push_str("Symbol(");
                let record = self.cx.heap.symbol(value.as_symbol().expect("a symbol"));
                let units = if record.description == NONE {
                    Vec::new()
                } else {
                    self.units(StringId(record.description))
                };
                self.atom_units(&units);
                self.out.push(')');
            }
            Kind::Object => self.object(value.as_object().expect("an object")),
            _ => self.out.push_str("uninitialized"),
        }
    }

    fn units(&self, id: StringId) -> Vec<u16> {
        self.cx.heap.string(id).repr().units().collect()
    }

    /// The leaves of `id` left to right without flattening it: QuickJS
    /// prints a rope leaf by leaf, so a surrogate pair split across two
    /// leaves prints as two escapes.
    fn leaves(&self, id: StringId) -> Vec<Vec<u16>> {
        let heap = &*self.cx.heap;
        let mut leaves = Vec::new();
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            let record = heap.strings.get(id.0);
            match (record.flat(), heap.ropes.get(&id.0)) {
                (Some(flat), _) => leaves.push(flat.units().collect()),
                (None, Some(&(left, right))) => {
                    stack.push(right);
                    stack.push(left);
                }
                (None, None) => {}
            }
        }
        leaves
    }

    fn string(&mut self, id: StringId) {
        let leaves = self.leaves(id);
        let total: usize = leaves.iter().map(Vec::len).sum();
        self.out.push('"');
        let mut position = 0;
        for leaf in &leaves {
            if position < MAX_STRING_LENGTH {
                let shown = leaf.len().min(MAX_STRING_LENGTH - position);
                self.escaped(&leaf[..shown], '"');
            }
            position += leaf.len();
        }
        self.out.push('"');
        if total > MAX_STRING_LENGTH {
            let remaining = total - MAX_STRING_LENGTH;
            self.out
                .push_str(&format!("... {remaining} more character"));
            if remaining > 1 {
                self.out.push('s');
            }
        }
    }

    fn escaped(&mut self, units: &[u16], separator: char) {
        let mut index = 0;
        while index < units.len() {
            let unit = units[index];
            index += 1;
            match unit {
                0x09 => self.out.push_str("\\t"),
                0x0d => self.out.push_str("\\r"),
                0x0a => self.out.push_str("\\n"),
                0x08 => self.out.push_str("\\b"),
                0x0c => self.out.push_str("\\f"),
                0x5c => self.out.push_str("\\\\"),
                _ if unit == separator as u16 => {
                    self.out.push('\\');
                    self.out.push(separator);
                }
                0x20..=0x7e => self.out.push(unit as u8 as char),
                0x00..=0x1f | 0x7f..=0x9f => self.unicode_escape(unit),
                0xd800..=0xdbff => match units.get(index) {
                    Some(&low) if (0xdc00..=0xdfff).contains(&low) => {
                        index += 1;
                        let code = 0x1_0000
                            + ((u32::from(unit) - 0xd800) << 10)
                            + (u32::from(low) - 0xdc00);
                        self.out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    _ => self.unicode_escape(unit),
                },
                0xdc00..=0xdfff => self.unicode_escape(unit),
                _ => self
                    .out
                    .push(char::from_u32(u32::from(unit)).unwrap_or('\u{fffd}')),
            }
        }
    }

    fn unicode_escape(&mut self, unit: u16) {
        self.out.push_str(&format!("\\u{unit:04x}"));
    }

    /// A property name or symbol description: bare when it is an
    /// identifier of ASCII letters, `$`, `_` and digits, else quoted.
    fn atom_units(&mut self, units: &[u16]) {
        let identifier = !units.is_empty()
            && units.iter().enumerate().all(|(index, &unit)| {
                matches!(unit, 0x61..=0x7a | 0x41..=0x5a | 0x5f | 0x24)
                    || (index > 0 && (0x30..=0x39).contains(&unit))
            });
        if identifier {
            self.out
                .extend(units.iter().map(|&unit| unit as u8 as char));
        } else {
            self.out.push('"');
            self.escaped(units, '"');
            self.out.push('"');
        }
    }

    fn key(&mut self, key: PropertyKey) {
        match key.kind() {
            KeyKind::Index(index) => self.out.push_str(&index.to_string()),
            KeyKind::String(id) => {
                let units = self.units(id);
                match self.cx.heap.string(id).array_index {
                    NONE => self.atom_units(&units),
                    index => self.out.push_str(&index.to_string()),
                }
            }
            KeyKind::Symbol(id) => {
                let record = self.cx.heap.symbol(id);
                let units = if record.description == NONE {
                    Vec::new()
                } else {
                    self.units(StringId(record.description))
                };
                self.atom_units(&units);
            }
        }
    }

    fn class_name(&self, object: ObjectId) -> &'static str {
        let heap = &self.cx.heap;
        match heap.object_kind(object) {
            ObjectKind::Array => "Array",
            ObjectKind::Function | ObjectKind::BoundFunction => self.function_class(object),
            ObjectKind::Arguments => "Arguments",
            ObjectKind::RegExp => "RegExp",
            ObjectKind::ForInIterator => "ForInIterator",
            ObjectKind::StringWrapper => "String",
            ObjectKind::NumberWrapper => "Number",
            ObjectKind::BooleanWrapper => "Boolean",
            ObjectKind::SymbolWrapper => "Symbol",
            ObjectKind::BigIntWrapper => "BigInt",
            ObjectKind::Date => "Date",
            ObjectKind::TypedArray => super::typed::view(heap, object).0.kind.name(),
            ObjectKind::ModuleNamespace => "Module",
            ObjectKind::Builtin => match heap.internal_tag(object) {
                InternalTag::ArrayIterator => "Array Iterator",
                InternalTag::StringIterator => "String Iterator",
                InternalTag::MapIterator => "Map Iterator",
                InternalTag::SetIterator => "Set Iterator",
                InternalTag::RegExpStringIterator => "RegExp String Iterator",
                InternalTag::IteratorHelper => "Iterator Helper",
                InternalTag::Iterator => "Iterator",
                InternalTag::WrapForValidIterator => "Iterator Wrap",
                InternalTag::AsyncFromSyncIterator => "",
                InternalTag::Promise => "Promise",
                InternalTag::Map => "Map",
                InternalTag::Set => "Set",
                InternalTag::WeakMap => "WeakMap",
                InternalTag::WeakSet => "WeakSet",
                InternalTag::WeakRef => "WeakRef",
                InternalTag::FinalizationRegistry => "FinalizationRegistry",
                InternalTag::Generator => "Generator",
                InternalTag::AsyncGenerator => "AsyncGenerator",
                InternalTag::AsyncFunction => "AsyncFunction",
                InternalTag::ArrayBuffer => "ArrayBuffer",
                InternalTag::SharedArrayBuffer => "SharedArrayBuffer",
                InternalTag::DataView => "DataView",
                _ => "Object",
            },
            _ if heap.header(object).has(flags::ERROR) => "Error",
            _ => "Object",
        }
    }

    fn function_class(&self, object: ObjectId) -> &'static str {
        let heap = &self.cx.heap;
        if heap.object_kind(object) != ObjectKind::Function {
            return "Function";
        }
        let Some(bytecode) = heap.function_bytecode(object) else {
            // QuickJS's resolving functions have their own classes.
            let native = heap.function_record(object).native;
            return if native == crate::builtins::Native::PromiseResolveFunction.index() {
                "PromiseResolveFunction"
            } else if native == crate::builtins::Native::PromiseRejectFunction.index() {
                "PromiseRejectFunction"
            } else {
                "Function"
            };
        };
        let unit = self.cx.unit(heap.bytecode_unit(bytecode));
        use crate::bytecode::flags as unit_flags;
        match (unit.has(unit_flags::ASYNC), unit.has(unit_flags::GENERATOR)) {
            (false, false) => "Function",
            (false, true) => "GeneratorFunction",
            (true, false) => "AsyncFunction",
            (true, true) => "AsyncGeneratorFunction",
        }
    }

    fn object(&mut self, object: ObjectId) {
        if let Some(index) = self.stack.iter().position(|&seen| seen == object) {
            self.out.push_str(&format!("[circular {index}]"));
            return;
        }
        if self.stack.len() >= MAX_DEPTH {
            let name = self.class_name(object);
            self.out.push('[');
            let units: Vec<u16> = name.encode_utf16().collect();
            self.atom_units(&units);
            self.out.push(']');
            return;
        }
        let body = self.body(object);
        let properties = self.properties(object, &body);
        self.stack.push(object);
        self.print_object(object, body, properties);
        self.stack.pop();
    }

    fn body(&mut self, object: ObjectId) -> Body {
        let heap = &self.cx.heap;
        if heap.header(object).has(flags::ERROR) {
            return Body::Error;
        }
        match heap.object_kind(object) {
            ObjectKind::Array => {
                let length = heap.array_length(object);
                if heap.header(object).has(flags::SPARSE) || heap.holey.contains(&object.0) {
                    return Body::Array {
                        dense: Vec::new(),
                        length,
                        sparse: true,
                    };
                }
                let mut dense = Vec::new();
                let mut index = 0;
                while index < length {
                    let value = heap.get_element(object, index);
                    if value.is_empty() {
                        break;
                    }
                    dense.push(value);
                    index += 1;
                }
                // A hole followed by an element: a sparse array, printed by
                // its index properties as QuickJS prints a slow array.
                if (index..length).any(|later| !heap.get_element(object, later).is_empty()) {
                    return Body::Array {
                        dense: Vec::new(),
                        length,
                        sparse: true,
                    };
                }
                Body::Array {
                    dense,
                    length,
                    sparse: false,
                }
            }
            ObjectKind::TypedArray => Body::TypedArray,
            ObjectKind::Function | ObjectKind::BoundFunction => Body::Function,
            ObjectKind::RegExp => Body::RegExp,
            ObjectKind::Date => {
                Body::Date(heap.internal_slot(object, 0).as_f64().unwrap_or(f64::NAN))
            }
            ObjectKind::Builtin => match heap.internal_tag(object) {
                InternalTag::Map | InternalTag::Set => {
                    let map = heap.internal_tag(object) == InternalTag::Map;
                    let internal = heap.internal(object);
                    let InternalData::Table(table) = &internal.data else {
                        return Body::Ordinary;
                    };
                    let width = table.width();
                    let size = table.len();
                    let mut entries = Vec::new();
                    let mut live = 0;
                    for (index, entry) in internal.slots.chunks(width).enumerate() {
                        if live >= MAX_ITEM_COUNT {
                            break;
                        }
                        if entry[0].is_empty() {
                            if table.is_zombie(index) {
                                entries.push(None);
                            }
                            continue;
                        }
                        live += 1;
                        entries.push(Some((
                            entry[0],
                            if map { entry[1] } else { Value::UNDEFINED },
                        )));
                    }
                    if map {
                        Body::Map(entries, size)
                    } else {
                        Body::Set(
                            entries
                                .into_iter()
                                .map(|entry| entry.map(|(key, _)| key))
                                .collect(),
                            size,
                        )
                    }
                }
                _ => Body::Ordinary,
            },
            _ => Body::Ordinary,
        }
    }

    /// The enumerable own properties QuickJS's printer visits: an array's
    /// dense elements are its body, a sparse array's and an ordinary
    /// object's elements are properties, then the shape's in order.
    fn properties(&self, object: ObjectId, body: &Body) -> (Vec<(PropertyKey, Slot)>, usize) {
        let heap = &self.cx.heap;
        let mut properties = Vec::new();
        let mut count = 0;
        let elements_are_properties =
            !matches!(body, Body::Array { sparse: false, .. } | Body::TypedArray)
                && heap.object_kind(object).has_elements()
                && heap.object_kind(object) != ObjectKind::Arguments;
        if elements_are_properties {
            let header = heap.header(object);
            let length = header.elements().len;
            let limit = if header.kind == ObjectKind::Array {
                length.min(header.payload)
            } else {
                length
            };
            for index in 0..limit {
                let value = heap.get_element(object, index);
                if value.is_empty() {
                    continue;
                }
                count += 1;
                if properties.len() < MAX_ITEM_COUNT
                    && let Some(key) = PropertyKey::index(index)
                {
                    properties.push((key, Slot::Data(value)));
                }
            }
        }
        for (key, flags, slot) in heap.shape_entries(heap.shape_of(object)) {
            if !flags.has(PropertyFlags::ENUMERABLE) {
                continue;
            }
            if let KeyKind::Symbol(id) = key.kind()
                && heap.symbol(id).private
            {
                continue;
            }
            count += 1;
            if properties.len() >= MAX_ITEM_COUNT {
                continue;
            }
            let value = if flags.has(PropertyFlags::ACCESSOR) {
                Slot::Accessor {
                    get: !heap.get_slot(object, slot).is_undefined(),
                    set: !heap.get_slot(object, slot + 1).is_undefined(),
                }
            } else {
                Slot::Data(heap.get_slot(object, slot))
            };
            properties.push((key, value));
        }
        if heap.object_kind(object) == ObjectKind::Arguments
            && heap.header(object).has(crate::heap::object::flags::SPARSE)
        {
            self.slow_arguments(object, &mut properties, &mut count);
        }
        (properties, count)
    }

    /// A converted `arguments` object's index properties, QuickJS's
    /// ordinary properties once its fast array is gone: the still mapped
    /// indices join the named ones, indices first.
    fn slow_arguments(
        &self,
        object: ObjectId,
        properties: &mut Vec<(PropertyKey, Slot)>,
        count: &mut usize,
    ) {
        let heap = &self.cx.heap;
        for index in 0..crate::vm::arguments::mapped_count(heap, object) {
            let (Some(cell), Some(key)) = (
                crate::vm::arguments::mapped_cell(heap, object, index),
                PropertyKey::index(index),
            ) else {
                continue;
            };
            if heap.lookup_own(object, key).is_none() {
                *count += 1;
                if properties.len() < MAX_ITEM_COUNT {
                    properties.push((key, Slot::Data(heap.scope_get(cell, 0, 0))));
                }
            }
        }
        let rank = |key: &PropertyKey| match key.kind() {
            KeyKind::Index(index) => u64::from(index),
            _ => u64::MAX,
        };
        properties.sort_by_key(|(key, _)| rank(key));
    }

    fn comma(&mut self, state: &mut Comma) {
        match *state {
            Comma::Empty => {}
            Comma::Between => self.out.push_str(", "),
            Comma::Suffix => self.out.push_str(" { "),
        }
        *state = Comma::Between;
    }

    fn more_items(&mut self, state: &mut Comma, count: usize) {
        self.comma(state);
        self.out.push_str(&format!("... {count} more item"));
        if count > 1 {
            self.out.push('s');
        }
    }

    fn print_object(
        &mut self,
        object: ObjectId,
        body: Body,
        (properties, count): (Vec<(PropertyKey, Slot)>, usize),
    ) {
        let mut state = Comma::Empty;
        let mut is_array = false;
        match body {
            Body::Array {
                dense,
                length,
                sparse,
            } => {
                is_array = true;
                self.out.push_str("[ ");
                for &value in dense.iter().take(MAX_ITEM_COUNT) {
                    self.comma(&mut state);
                    self.value(value);
                }
                if dense.len() > MAX_ITEM_COUNT {
                    self.more_items(&mut state, dense.len() - MAX_ITEM_COUNT);
                }
                if !sparse && (dense.len() as u32) < length {
                    let empty = length as usize - dense.len();
                    self.comma(&mut state);
                    self.out.push_str(&format!("<{empty} empty item"));
                    if empty > 1 {
                        self.out.push('s');
                    }
                    self.out.push('>');
                }
            }
            Body::TypedArray => {
                is_array = true;
                let name = self.class_name(object);
                let length = super::typed::length(self.cx.heap, object).unwrap_or(0);
                self.atom_units(&name.encode_utf16().collect::<Vec<_>>());
                self.out.push_str(&format!("({length}) [ "));
                let shown = length.min(MAX_ITEM_COUNT);
                for index in 0..shown {
                    self.comma(&mut state);
                    match super::typed::get_element(self.cx, object, index as f64) {
                        // QuickJS prints a 64-bit element as a C integer,
                        // without the `n`.
                        Ok(Some(value)) if value.kind() == Kind::BigInt => {
                            let text = super::ops::big(self.cx.heap, value).to_string_radix10();
                            self.out.push_str(&text);
                        }
                        Ok(Some(value)) => self.value(value),
                        _ => self.out.push_str("undefined"),
                    }
                }
                if shown < length {
                    self.more_items(&mut state, length - shown);
                }
            }
            Body::Function => {
                self.out.push_str("[Function ");
                match self.raw_string(object, self.cx.realm.names.name) {
                    Some(name) if !name.is_empty() => self.c_string(&name),
                    _ => self.out.push_str("(anonymous)"),
                }
                self.out.push(']');
                state = Comma::Suffix;
            }
            Body::Map(entries, size) => {
                self.out.push_str(&format!("Map({size}) {{ "));
                let mut live = 0;
                for &entry in &entries {
                    self.comma(&mut state);
                    if let Some((key, value)) = entry {
                        self.value(key);
                        self.out.push_str(" => ");
                        self.value(value);
                        live += 1;
                    }
                }
                if live < size {
                    self.more_items(&mut state, size - live);
                }
            }
            Body::Set(entries, size) => {
                self.out.push_str(&format!("Set({size}) {{ "));
                let mut live = 0;
                for &entry in &entries {
                    self.comma(&mut state);
                    if let Some(key) = entry {
                        self.value(key);
                        live += 1;
                    }
                }
                if live < size {
                    self.more_items(&mut state, size - live);
                }
            }
            Body::RegExp => {
                self.regexp(object);
                state = Comma::Suffix;
            }
            Body::Date(time) => match crate::builtins::date_iso_string(self.cx, time) {
                Some(text) => {
                    self.out.push_str(&text);
                    state = Comma::Suffix;
                }
                None => self.out.push_str("Date { "),
            },
            Body::Error => {
                self.error(object);
                state = Comma::Suffix;
            }
            Body::Ordinary => {
                let name = self.class_name(object);
                let plain =
                    self.cx.heap.object_kind(object) == ObjectKind::Ordinary && name == "Object";
                if !plain {
                    self.atom_units(&name.encode_utf16().collect::<Vec<_>>());
                    self.out.push(' ');
                }
                self.out.push_str("{ ");
            }
        }
        let shown = properties.len();
        for (key, slot) in properties {
            self.comma(&mut state);
            self.key(key);
            self.out.push_str(": ");
            match slot {
                Slot::Data(value) => self.value(value),
                Slot::Accessor { get, set } => self.out.push_str(match (get, set) {
                    (true, true) => "[Getter/Setter]",
                    (false, true) => "[Setter]",
                    _ => "[Getter]",
                }),
            }
        }
        if shown < count {
            self.more_items(&mut state, count - shown);
        }
        if is_array {
            self.out.push_str(" ]");
        } else if state != Comma::Suffix {
            self.out.push_str(" }");
        }
    }

    /// An own data property's string, else the prototype's (one level, as
    /// the pinned printer reads `name` and `message`).
    fn raw_string(&self, object: ObjectId, name: StringId) -> Option<Vec<u16>> {
        let heap = &self.cx.heap;
        let key = Names::key(name);
        let read = |object: ObjectId| -> Option<Option<Vec<u16>>> {
            let (slot, flags) = heap.lookup_own(object, key)?;
            if flags.has(PropertyFlags::ACCESSOR) {
                return Some(None);
            }
            // QuickJS's `get_prop_string` takes only `JS_TAG_STRING`: a rope
            // (`JS_TAG_STRING_ROPE`) reads as absent.
            Some(
                heap.get_slot(object, slot)
                    .as_string()
                    .filter(|id| heap.strings.get(id.0).flat().is_some())
                    .map(|id| heap.string(id).repr().units().collect()),
            )
        };
        match read(object) {
            Some(found) => found,
            None => heap.prototype_of(object).and_then(read).flatten(),
        }
    }

    /// A string as QuickJS's C-string conversion prints it: UTF-8 with lone
    /// surrogates encoded as is (WTF-8), cut at the first NUL.
    fn c_string(&mut self, units: &[u16]) {
        let end = units
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(units.len());
        wtf8(units[..end].iter().copied(), &mut self.out.0);
    }

    fn error(&mut self, object: ObjectId) {
        let names = &self.cx.realm.names;
        let (name, message, stack) = (names.name, names.message, names.stack);
        match self.raw_string(object, name) {
            Some(name) => self.c_string(&name),
            None => self.out.push_str("Error"),
        }
        if let Some(message) = self.raw_string(object, message)
            && message.first() != Some(&0)
            && !message.is_empty()
        {
            self.out.push_str(": ");
            self.c_string(&message);
        }
        if let Some(mut units) = self.raw_string(object, stack) {
            if let Some(nul) = units.iter().position(|&unit| unit == 0) {
                units.truncate(nul);
            }
            if units.last() == Some(&0x0a) {
                units.pop();
            }
            self.out.push('\n');
            wtf8(units.into_iter(), &mut self.out.0);
        }
    }

    fn regexp(&mut self, object: ObjectId) {
        let index = self.cx.heap.header(object).payload;
        let record = self.cx.heap.regexps.get(index);
        let pattern = self.units(record.source);
        let flags = self.units(record.flags);
        self.out.push('/');
        if pattern.is_empty() {
            self.out.push_str("(?:)");
        } else {
            let mut in_class = false;
            let mut index = 0;
            while index < pattern.len() {
                let mut first = pattern[index];
                index += 1;
                let mut second = None;
                match first {
                    0x5c => {
                        if let Some(&next) = pattern.get(index) {
                            second = Some(next);
                            index += 1;
                        }
                    }
                    0x5d => in_class = false,
                    0x5b if !in_class => {
                        if pattern.get(index) == Some(&0x5d) {
                            second = Some(0x5d);
                            index += 1;
                        }
                        in_class = true;
                    }
                    0x0a => {
                        first = 0x5c;
                        second = Some(u16::from(b'n'));
                    }
                    0x0d => {
                        first = 0x5c;
                        second = Some(u16::from(b'r'));
                    }
                    0x2f if !in_class => {
                        first = 0x5c;
                        second = Some(0x2f);
                    }
                    _ => {}
                }
                // The pinned printer narrows each unit to a byte.
                self.out.push(first as u8 as char);
                if let Some(second) = second {
                    self.out.push(second as u8 as char);
                }
            }
        }
        self.out.push('/');
        // QuickJS prints `lre_get_flags` bits 0 to 7 as `gimsuydv`: bit 7 is
        // in fact `LRE_FLAG_NAMED_GROUPS`, so a pattern with named groups
        // shows `v` and the `v` flag (bit 8) never shows.
        for flag in "gimsuyd".chars() {
            if flags.contains(&(flag as u16)) {
                self.out.push(flag);
            }
        }
        if self.cx.heap.regexps.program(index).has_named_captures() {
            self.out.push('v');
        }
    }
}
