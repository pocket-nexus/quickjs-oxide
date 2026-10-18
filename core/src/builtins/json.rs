//! `JSON`: `parse` with a reviver and the JSON.parse source text access
//! proposal's `context.source`, `stringify`, and `rawJSON`/`isRawJSON`. The
//! parser reports QuickJS's messages; the serializer is the spec's
//! `SerializeJSONProperty` over the internal methods, so proxies, getters and
//! `toJSON` are observed in spec order.

use super::{Native, define, methods, to_string_tag};
use crate::ErrorKind;
use crate::PropertyKey;
use crate::heap::HeapError;
use crate::heap::object::{InternalTag, ObjectKind};
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{Kind, ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, number, ops, property};

/// QuickJS's nesting limit for `JSON.parse`.
const MAX_DEPTH: usize = 10_891;

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let json = super::plain_object(cx, Some(object_prototype), 6)?;
    methods(
        cx,
        json,
        &[
            Native::JsonIsRawJson,
            Native::JsonParse,
            Native::JsonRawJson,
            Native::JsonStringify,
        ],
    )?;
    to_string_tag(cx, json, "JSON")?;
    let global = cx.realm.global();
    define(cx, global, "JSON", Value::object(json), super::hidden())
}

// ---- parse ----

/// What a parse records for the reviver: each value with its source span
/// (primitives) or its children (containers).
enum Record {
    Primitive {
        value: Value,
        start: usize,
        end: usize,
    },
    Array {
        value: Value,
        elements: Vec<Record>,
    },
    Object {
        value: Value,
        entries: Vec<(PropertyKey, Record)>,
    },
}

impl Record {
    fn value(&self) -> Value {
        match self {
            Record::Primitive { value, .. }
            | Record::Array { value, .. }
            | Record::Object { value, .. } => *value,
        }
    }

    fn entry(&self, key: PropertyKey) -> Option<&Record> {
        let Record::Object { entries, .. } = self else {
            return None;
        };
        // Of duplicate names the last one's record, whose value the object
        // keeps (QuickJS finds the first one's in records of up to eight
        // members).
        entries
            .iter()
            .rev()
            .find(|(name, _)| *name == key)
            .map(|(_, record)| record)
    }
}

enum Failure {
    /// A syntax error at a unit index.
    Syntax(String, usize),
    Throw(Throw),
}

impl From<Throw> for Failure {
    fn from(throw: Throw) -> Self {
        Failure::Throw(throw)
    }
}

type Parsed<T> = Result<T, Failure>;

/// A recursive-descent parser over the text's UTF-16 units. Every value it
/// makes is left on the root stack; a container's children are popped once
/// defined unless the reviver needs their records.
struct Parser<'a> {
    units: &'a [u16],
    cursor: usize,
    records: bool,
    /// QuickJS's extended JSON (`type: "json5"` modules): comments, single
    /// quotes, identifier keys, trailing commas, `+`, `.5`, hexadecimal,
    /// octal and binary numbers, `NaN` and `Infinity`.
    extended: bool,
    /// Units standing for invalid UTF-8 of a module's bytes, sorted.
    invalid: &'a [u32],
    /// Units QuickJS's byte columns do not count, sorted: among the
    /// invalid ones, the stray continuation bytes.
    no_column: &'a [u32],
}

fn heap_failure(cx: &mut Cx<'_>, error: HeapError) -> Failure {
    Failure::Throw(error::heap_error(cx, error))
}

impl Parser<'_> {
    fn peek(&self) -> Option<u16> {
        self.units.get(self.cursor).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(u16::from(byte)) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek_at(&self, offset: usize) -> Option<u16> {
        self.units.get(self.cursor + offset).copied()
    }

    /// Skips whitespace and, in extended mode, comments; an unterminated
    /// block comment is an error.
    fn skip_space(&mut self) -> Parsed<()> {
        loop {
            while self.peek().is_some_and(|unit| {
                matches!(unit, 0x09 | 0x0a | 0x0d | 0x20)
                    || (self.extended && matches!(unit, 0x0b | 0x0c))
            }) {
                self.cursor += 1;
            }
            if !self.extended || self.peek() != Some(0x2f) {
                return Ok(());
            }
            match self.peek_at(1) {
                Some(0x2f) => {
                    self.cursor += 2;
                    while self
                        .peek()
                        .is_some_and(|unit| !matches!(unit, 0x0a | 0x0d | 0x2028 | 0x2029))
                    {
                        self.cursor += 1;
                    }
                    if matches!(self.peek(), Some(0x2028 | 0x2029)) {
                        self.cursor += 1;
                    }
                }
                Some(0x2a) => {
                    let start = self.cursor;
                    self.cursor += 2;
                    loop {
                        let Some(unit) = self.peek() else {
                            return Err(Failure::Syntax(
                                "unexpected end of comment".to_owned(),
                                start,
                            ));
                        };
                        if unit == 0x2a && self.peek_at(1) == Some(0x2f) {
                            self.cursor += 2;
                            break;
                        }
                        self.cursor += 1;
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// Whether `unit` starts a number here.
    fn starts_number(&self, unit: u16) -> bool {
        matches!(unit, 0x2d | 0x30..=0x39)
            || (self.extended
                && (unit == 0x2b || (unit == 0x2e && self.peek_at(1).is_some_and(is_digit))))
    }

    fn is_invalid(&self, at: usize) -> bool {
        !self.invalid.is_empty() && self.invalid.binary_search(&(at as u32)).is_ok()
    }

    /// Whether the unit at `at` stands for a stray continuation byte.
    fn is_continuation(&self, at: usize) -> bool {
        self.is_invalid(at) && self.no_column.binary_search(&(at as u32)).is_ok()
    }

    /// QuickJS's `Unexpected token '%c'` of the first byte at `at` (or the
    /// end), read back by `JS_NewString`: a NUL ends the text, any other
    /// non-ASCII byte is U+FFFD, and a stray continuation byte swallows the
    /// closing quote.
    fn unexpected_token<T>(&self, at: usize) -> Parsed<T> {
        let message = match self.units.get(at) {
            None | Some(0) => "Unexpected token '".to_owned(),
            Some(_) if self.is_continuation(at) => "Unexpected token '\u{fffd}".to_owned(),
            Some(&unit) if unit < 0x80 && !self.is_invalid(at) => {
                format!("Unexpected token '{}'", unit as u8 as char)
            }
            Some(_) => "Unexpected token '\u{fffd}'".to_owned(),
        };
        Err(Failure::Syntax(message, self.cursor))
    }

    /// An error at unit `at` rather than at the cursor.
    fn syntax_at<T>(&self, message: &str, at: usize) -> Parsed<T> {
        Err(Failure::Syntax(message.to_owned(), at))
    }

    /// QuickJS's byte-level parser meets invalid UTF-8 here.
    fn bad_utf8<T>(&self, at: usize) -> Parsed<T> {
        Err(Failure::Syntax("Bad UTF-8 sequence".to_owned(), at))
    }

    fn syntax<T>(&self, message: &str) -> Parsed<T> {
        Err(Failure::Syntax(message.to_owned(), self.cursor))
    }

    fn document(&mut self, cx: &mut Cx<'_>) -> Parsed<(Value, Option<Record>)> {
        let parsed = self.value(cx, 0)?;
        self.skip_space()?;
        if self.cursor != self.units.len() {
            return self.syntax("unexpected data at the end");
        }
        Ok(parsed)
    }

    fn value(&mut self, cx: &mut Cx<'_>, depth: usize) -> Parsed<(Value, Option<Record>)> {
        self.skip_space()?;
        let Some(unit) = self.peek() else {
            return self.syntax("Unexpected end of JSON input");
        };
        let start = self.cursor;
        // `json_next_token` rejects any byte past ASCII in token context.
        if self.is_invalid(start) {
            return self.syntax("unexpected character");
        }
        // QuickJS's budget runs out at a value nested this deep, a leaf
        // included (an empty innermost container is one value shallower).
        if depth > MAX_DEPTH {
            return self.syntax("stack overflow");
        }
        let value = match unit {
            0x7b | 0x5b => {
                if !cx.heap.vm.enter_native() {
                    return self.syntax("stack overflow");
                }
                let result = if unit == 0x7b {
                    self.object(cx, depth)
                } else {
                    self.array(cx, depth)
                };
                cx.heap.vm.leave_native();
                return result;
            }
            0x22 | 0x27 if unit == 0x22 || self.extended => {
                let units = self.string()?;
                ops::alloc_units(cx, &units)?
            }
            _ if self.starts_number(unit) => self.number()?,
            _ if is_identifier_start(unit) => {
                while self.peek().is_some_and(is_identifier_continue) {
                    self.cursor += 1;
                }
                match &self.units[start..self.cursor] {
                    word if ascii_eq(word, "true") => Value::TRUE,
                    word if ascii_eq(word, "false") => Value::FALSE,
                    word if ascii_eq(word, "null") => Value::NULL,
                    word if self.extended && ascii_eq(word, "NaN") => Value::number(f64::NAN),
                    word if self.extended && ascii_eq(word, "Infinity") => {
                        Value::number(f64::INFINITY)
                    }
                    word => {
                        let word = String::from_utf16_lossy(word);
                        return self.syntax(&format!("unexpected token: '{word}'"));
                    }
                }
            }
            _ if unit >= 0x80 => return self.syntax("unexpected character"),
            0 => return self.syntax("unexpected token: ''"),
            _ => {
                let unit = char::from(unit as u8);
                return self.syntax(&format!("unexpected token: '{unit}'"));
            }
        };
        cx.heap.roots().push(value);
        let record = self.records.then_some(Record::Primitive {
            value,
            start,
            end: self.cursor,
        });
        Ok((value, record))
    }

    fn object(&mut self, cx: &mut Cx<'_>, depth: usize) -> Parsed<(Value, Option<Record>)> {
        self.cursor += 1;
        let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
        let object = cx
            .heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 4)
            .map_err(|error| heap_failure(cx, error))?;
        cx.heap.roots().push(Value::object(object));
        let mark = cx.heap.roots().mark();
        let mut entries = Vec::new();
        self.skip_space()?;
        if !self.eat(b'}') {
            loop {
                self.skip_space()?;
                if self.is_invalid(self.cursor) {
                    return self.syntax("unexpected character");
                }
                let name = match self.peek() {
                    Some(0x22) => self.string()?,
                    Some(0x27) if self.extended => self.string()?,
                    Some(unit) if self.extended && is_identifier_start(unit) => {
                        let start = self.cursor;
                        while self.peek().is_some_and(is_identifier_continue) {
                            self.cursor += 1;
                        }
                        self.units[start..self.cursor].to_vec()
                    }
                    Some(unit) if unit >= 0x80 => return self.syntax("unexpected character"),
                    None => return self.syntax("expecting property name"),
                    _ => return self.syntax("expecting property name"),
                };
                let key = cx
                    .heap
                    .property_key_units(&name)
                    .map_err(|error| heap_failure(cx, error))?;
                self.skip_space()?;
                if !self.eat(b':') {
                    return self.syntax("expecting ':'");
                }
                let (value, record) = self.value(cx, depth + 1)?;
                internal::create_data_property(cx, object, key, value)?;
                match record {
                    Some(record) => entries.push((key, record)),
                    None => cx.heap.roots().truncate(mark),
                }
                self.skip_space()?;
                if self.eat(b',') {
                    self.skip_space()?;
                    if self.extended && self.eat(b'}') {
                        break;
                    }
                    continue;
                }
                if !self.eat(b'}') {
                    return self.syntax("expecting '}'");
                }
                break;
            }
        }
        let value = Value::object(object);
        let record = self.records.then_some(Record::Object { value, entries });
        Ok((value, record))
    }

    fn array(&mut self, cx: &mut Cx<'_>, depth: usize) -> Parsed<(Value, Option<Record>)> {
        self.cursor += 1;
        let array = internal::array_create(cx, 0)?;
        cx.heap.roots().push(Value::object(array));
        let mark = cx.heap.roots().mark();
        let mut elements = Vec::new();
        self.skip_space()?;
        if !self.eat(b']') {
            loop {
                let (value, record) = self.value(cx, depth + 1)?;
                cx.heap
                    .push_element(array, value)
                    .map_err(|error| heap_failure(cx, error))?;
                match record {
                    Some(record) => elements.push(record),
                    None => cx.heap.roots().truncate(mark),
                }
                self.skip_space()?;
                if self.eat(b',') {
                    self.skip_space()?;
                    if self.extended && self.eat(b']') {
                        break;
                    }
                    continue;
                }
                if !self.eat(b']') {
                    return self.syntax("expecting ']'");
                }
                break;
            }
        }
        let value = Value::object(array);
        let record = self.records.then_some(Record::Array { value, elements });
        Ok((value, record))
    }

    fn string(&mut self) -> Parsed<Vec<u16>> {
        let separator = self.peek().unwrap_or(0x22);
        self.cursor += 1;
        let mut output = Vec::new();
        loop {
            let Some(unit) = self.peek() else {
                return self.syntax("Unexpected end of JSON input");
            };
            if self.is_invalid(self.cursor) {
                return self.bad_utf8(self.cursor);
            }
            self.cursor += 1;
            match unit {
                _ if unit == separator => return Ok(output),
                0x00..=0x1f => {
                    return self
                        .syntax_at("Bad control character in string literal", self.cursor - 1);
                }
                0x5c => {
                    let Some(escaped) = self.peek() else {
                        return self.syntax("Unexpected end of JSON input");
                    };
                    self.cursor += 1;
                    output.push(match escaped {
                        0x5c | 0x2f => escaped,
                        _ if escaped == separator => escaped,
                        0x76 if self.extended => 0x0b,
                        0x0a if self.extended => continue,
                        0x62 => 0x08,
                        0x66 => 0x0c,
                        0x6e => 0x0a,
                        0x72 => 0x0d,
                        0x74 => 0x09,
                        0x75 => {
                            let mut value = 0u16;
                            for _ in 0..4 {
                                let Some(digit) = self.peek().and_then(hex_value) else {
                                    return self.syntax("Bad Unicode escape");
                                };
                                self.cursor += 1;
                                value = value << 4 | u16::from(digit);
                            }
                            value
                        }
                        _ => return self.syntax_at("Bad escaped character", self.cursor - 1),
                    });
                }
                _ => output.push(unit),
            }
        }
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(is_digit) {
            self.cursor += 1;
        }
    }

    fn number(&mut self) -> Parsed<Value> {
        let start = self.cursor;
        let negative = self.eat(b'-');
        if !negative && self.extended {
            self.eat(b'+');
        }
        if self.extended {
            let sign = if negative { -1.0 } else { 1.0 };
            let rest = &self.units[self.cursor..];
            if rest.starts_with(&ascii_units("Infinity")) {
                self.cursor += "Infinity".len();
                return Ok(Value::number(sign * f64::INFINITY));
            }
            if rest.starts_with(&ascii_units("NaN")) {
                self.cursor += "NaN".len();
                return Ok(Value::number(f64::NAN));
            }
            let radix = match (self.peek(), self.peek_at(1)) {
                (Some(0x30), Some(0x78 | 0x58)) => Some(16),
                (Some(0x30), Some(0x6f | 0x4f)) => Some(8),
                (Some(0x30), Some(0x62 | 0x42)) => Some(2),
                _ => None,
            };
            if let Some(radix) = radix {
                self.cursor += 2;
                let digit = |unit: u16| hex_value(unit).filter(|&d| u32::from(d) < radix);
                if self.peek().and_then(digit).is_none() {
                    return self.unexpected_token(self.cursor);
                }
                let mut value = 0.0f64;
                while let Some(d) = self.peek().and_then(digit) {
                    value = value * f64::from(radix) + f64::from(d);
                    self.cursor += 1;
                }
                return Ok(number::number(if negative { -value } else { value }));
            }
            if self.peek() == Some(0x2e) {
                self.digits_after_point()?;
                return Ok(number::number(number::parse(
                    &self.units[start..self.cursor],
                )));
            }
        }
        match self.peek() {
            None => return self.syntax("Unexpected token '"),
            Some(0x30) => {
                self.cursor += 1;
                if self.peek().is_some_and(is_digit) {
                    return self.syntax("Unexpected number");
                }
            }
            Some(0x31..=0x39) => self.digits(),
            Some(_) => return self.unexpected_token(self.cursor),
        }
        if self.peek() == Some(0x2e) {
            self.digits_after_point()?;
            return Ok(number::number(number::parse(
                &self.units[start..self.cursor],
            )));
        }
        self.exponent()?;
        Ok(number::number(number::parse(
            &self.units[start..self.cursor],
        )))
    }

    /// A fraction from its point, then an exponent.
    fn digits_after_point(&mut self) -> Parsed<()> {
        self.cursor += 1;
        if !self.peek().is_some_and(is_digit) {
            return self.syntax("Unterminated fractional number");
        }
        self.digits();
        self.exponent()
    }

    fn exponent(&mut self) -> Parsed<()> {
        if matches!(self.peek(), Some(0x65 | 0x45)) {
            self.cursor += 1;
            if matches!(self.peek(), Some(0x2b | 0x2d)) {
                self.cursor += 1;
            }
            if !self.peek().is_some_and(is_digit) {
                return self.syntax("Exponent part is missing a number");
            }
            self.digits();
        }
        Ok(())
    }
}

fn ascii_units(text: &str) -> Vec<u16> {
    text.bytes().map(u16::from).collect()
}

fn is_digit(unit: u16) -> bool {
    (0x30..=0x39).contains(&unit)
}

fn is_identifier_start(unit: u16) -> bool {
    matches!(unit, 0x41..=0x5a | 0x61..=0x7a | 0x24 | 0x5f)
}

fn is_identifier_continue(unit: u16) -> bool {
    is_identifier_start(unit) || is_digit(unit)
}

fn hex_value(unit: u16) -> Option<u8> {
    char::from_u32(u32::from(unit))?
        .to_digit(16)
        .map(|digit| digit as u8)
}

fn ascii_eq(units: &[u16], text: &str) -> bool {
    units.len() == text.len()
        && units
            .iter()
            .zip(text.bytes())
            .all(|(&a, b)| a == u16::from(b))
}

/// Parses `units` as strict JSON; the value is left on the root stack.
/// Parses `units`; a syntax error of the text of `file` (a JSON module)
/// carries its position there.
fn parse_text(
    cx: &mut Cx<'_>,
    units: &[u16],
    records: bool,
    file: Option<&str>,
    extended: bool,
    invalid: &[u32],
) -> Result<(Value, Option<Record>), Throw> {
    let mut parser = Parser {
        units,
        cursor: 0,
        records,
        extended,
        invalid,
        no_column: &[],
    };
    match parser.document(cx) {
        Ok(parsed) => Ok(parsed),
        Err(Failure::Syntax(message, at)) => Err(match file {
            Some(file) => {
                let (line, column) = position(units, at);
                error::throw_at(cx, ErrorKind::Syntax, &message, file, line, column)
            }
            // `JSON.parse` locates its error in `<input>`, the running
            // frames after it, as QuickJS's does.
            None => {
                let (line, column) = position(units, at);
                error::throw_located(cx, ErrorKind::Syntax, &message, "<input>", line, column)
            }
        }),
        Err(Failure::Throw(throw)) => Err(throw),
    }
}

/// The one-based line and column of unit `at`: LF ends a line.
fn position(units: &[u16], at: usize) -> (u32, u32) {
    let before = &units[..at.min(units.len())];
    let line = before.iter().filter(|&&unit| unit == 0x0a).count() + 1;
    let start = before
        .iter()
        .rposition(|&unit| unit == 0x0a)
        .map_or(0, |newline| newline + 1);
    (line as u32, (before.len() - start) as u32 + 1)
}

/// The value of a JSON module `file` (`extended`: a JSON5 module, QuickJS's
/// extended JSON); `invalid` marks the units of invalid UTF-8.
pub(crate) fn json_module_text(
    cx: &mut Cx<'_>,
    text: &crate::vm::module::JsonText,
    file: &str,
    extended: bool,
) -> Result<Value, Throw> {
    let mut parser = Parser {
        units: &text.units,
        cursor: 0,
        records: false,
        extended,
        invalid: &text.invalid,
        no_column: &text.no_column,
    };
    match parser.document(cx) {
        Ok((value, _)) => Ok(value),
        Err(Failure::Syntax(message, at)) => {
            let (line, column) = position_skipping(&text.units, at, &text.no_column);
            Err(error::throw_at(
                cx,
                ErrorKind::Syntax,
                &message,
                file,
                line,
                column,
            ))
        }
        Err(Failure::Throw(throw)) => Err(throw),
    }
}

/// [`position`] where the units of `no_column` add no column.
fn position_skipping(units: &[u16], at: usize, no_column: &[u32]) -> (u32, u32) {
    let (line, column) = position(units, at);
    let line_start = at + 1 - column as usize;
    let skipped = no_column
        .iter()
        .filter(|&&index| (line_start..at).contains(&(index as usize)))
        .count();
    (line, column - skipped as u32)
}

/// `JSON.parse(text, reviver)`.
pub(super) fn parse(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let text = site.arg(cx, 0);
    let text = ops::to_string(cx, text)?;
    let units = ops::units(cx.heap, text.as_string().unwrap());
    let reviver = site.arg(cx, 1);
    let revive = ops::is_callable(cx.heap, reviver);
    let (value, record) = parse_text(cx, &units, revive, None, false, &[])?;
    if !revive {
        return Ok(value);
    }
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let root = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, Some(prototype), 1)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(root));
    let empty = Names::key(cx.realm.names.empty);
    internal::create_data_property(cx, root, empty, value)?;
    let mut reviving = Reviving {
        units: &units,
        reviver,
        depth: 0,
    };
    reviving.internalize(cx, root, empty, record.as_ref())
}

struct Reviving<'a> {
    units: &'a [u16],
    reviver: Value,
    /// Nested `InternalizeJSONProperty` calls.
    depth: usize,
}

/// The most nested `InternalizeJSONProperty` calls: the holder's, then one
/// per value, as deep as QuickJS 2026-06-04's `internalize_json_property`
/// recursion reaches within its stack budget (3,630 nested arrays).
const MAX_REVIVER_DEPTH: usize = 3_631;

impl Reviving<'_> {
    /// `InternalizeJSONProperty(holder, name, reviver)` with the parse
    /// record of the value as parsed, which applies while the value is
    /// unchanged.
    fn internalize(
        &mut self,
        cx: &mut Cx<'_>,
        holder: ObjectId,
        name: PropertyKey,
        record: Option<&Record>,
    ) -> Result<Value, Throw> {
        if self.depth >= MAX_REVIVER_DEPTH || !cx.heap.vm.enter_native() {
            return Err(error::stack_overflow(cx));
        }
        self.depth += 1;
        let mark = cx.heap.roots().mark();
        let result = self.internalize_rooted(cx, holder, name, record);
        cx.heap.roots().truncate(mark);
        cx.heap.vm.leave_native();
        self.depth -= 1;
        result
    }

    fn internalize_rooted(
        &mut self,
        cx: &mut Cx<'_>,
        holder: ObjectId,
        name: PropertyKey,
        record: Option<&Record>,
    ) -> Result<Value, Throw> {
        let value = internal::get(cx, holder, name, Value::object(holder))?;
        cx.heap.roots().push(value);
        let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
        let context = cx
            .heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 1)
            .map_err(|error| error::heap_error(cx, error))?;
        cx.heap.roots().push(Value::object(context));
        let record = record.filter(|record| internal::same_value(cx, record.value(), value));
        if let Some(Record::Primitive { start, end, .. }) = record
            && value.as_object().is_none()
        {
            let source = ops::alloc_units(cx, &self.units[*start..*end])?;
            let key = cx
                .heap
                .property_key("source")
                .map_err(|error| error::heap_error(cx, error))?;
            internal::create_data_property(cx, context, key, source)?;
        }
        if let Some(object) = value.as_object() {
            if internal::is_array(cx, value)? {
                let length = internal::length_of_array_like(cx, object)?;
                let mut index = 0u64;
                while index < length {
                    let key = internal::number_key(cx, index as f64)?;
                    let child = match record {
                        Some(Record::Array { elements, .. }) => elements.get(index as usize),
                        _ => None,
                    };
                    self.revise(cx, object, key, child)?;
                    index += 1;
                }
            } else {
                let keys = internal::enumerable_own_keys(cx, object)?;
                for key in keys {
                    let child = record.and_then(|record| record.entry(key));
                    self.revise(cx, object, key, child)?;
                }
            }
        }
        let name = super::object::key_string(cx, name)?;
        cx.heap.roots().push(name);
        crate::vm::call(
            cx,
            self.reviver,
            Value::object(holder),
            &[name, value, Value::object(context)],
        )
    }

    /// Replaces or deletes `key` of `object` with the reviver's result.
    fn revise(
        &mut self,
        cx: &mut Cx<'_>,
        object: ObjectId,
        key: PropertyKey,
        record: Option<&Record>,
    ) -> Result<(), Throw> {
        let element = self.internalize(cx, object, key, record)?;
        if element.is_undefined() {
            internal::delete(cx, object, key)?;
        } else {
            cx.heap.roots().push(element);
            internal::create_data_property(cx, object, key, element)?;
        }
        Ok(())
    }
}

// ---- rawJSON ----

/// `JSON.rawJSON(text)`: a frozen null-prototype object whose `rawJSON` is
/// the text of a JSON primitive, serialized verbatim.
pub(super) fn raw_json(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let text = site.arg(cx, 0);
    let text = ops::to_string(cx, text)?;
    cx.heap.roots().push(text);
    let units = ops::units(cx.heap, text.as_string().unwrap());
    let boundary = |unit: u16| matches!(unit, 0x61..=0x7a | 0x30..=0x39 | 0x2d | 0x22);
    let valid = units.first().is_some_and(|&unit| boundary(unit))
        && units.last().is_some_and(|&unit| boundary(unit))
        && parse_text(cx, &units, false, None, false, &[]).is_ok();
    if !valid {
        return Err(error::throw(
            cx,
            ErrorKind::Syntax,
            "invalid rawJSON string",
        ));
    }
    let object = cx
        .heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::RawJson,
            None,
            1,
            &[],
            Default::default(),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(object));
    let key = cx
        .heap
        .property_key("rawJSON")
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap
        .add_property(object, key, PropertyFlags::data(false, true, false), text)
        .map_err(|error| error::heap_error(cx, error))?;
    internal::prevent_extensions(cx, object)?;
    Ok(Value::object(object))
}

fn is_raw_json_object(cx: &Cx<'_>, value: Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| cx.heap.internal_tag(object) == InternalTag::RawJson)
}

pub(super) fn is_raw_json(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    Ok(Value::bool(is_raw_json_object(cx, value)))
}

// ---- stringify ----

struct Serializer {
    replacer: Option<Value>,
    keys: Option<Vec<PropertyKey>>,
    gap: Vec<u16>,
    indent: Vec<u16>,
    /// The containers being serialized, for the cycle check.
    stack: crate::heap::hash::FxHashSet<ObjectId>,
    out: Vec<u16>,
}

/// What `SerializeJSONProperty` found before serializing a value.
enum Prepared {
    /// Written out (`true`), or not serializable (`false`).
    Done(bool),
    /// An object or array, still to write; the root stack keeps it from
    /// the mark.
    Container(ObjectId, bool),
}

/// A container being written: its members are serialized one at a time
/// on this explicit stack, so deep nesting uses no host stack.
struct Level {
    object: ObjectId,
    /// The keys of an object; `None` for an array.
    keys: Option<Vec<PropertyKey>>,
    /// An array's length.
    length: u64,
    next: u64,
    any: bool,
    stepback: usize,
    mark: crate::RootMark,
}

/// `JSON.stringify(value, replacer, space)`.
pub(super) fn stringify(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let replacer = site.arg(cx, 1);
    let mut serializer = Serializer {
        replacer: None,
        keys: None,
        gap: Vec::new(),
        indent: Vec::new(),
        stack: Default::default(),
        out: Vec::new(),
    };
    if ops::is_callable(cx.heap, replacer) {
        serializer.replacer = Some(replacer);
    } else if internal::is_array(cx, replacer)? {
        serializer.keys = Some(property_list(cx, replacer.as_object().unwrap())?);
    }
    let space = site.arg(cx, 2);
    serializer.gap = gap(cx, space)?;
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let wrapper = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, Some(prototype), 1)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(wrapper));
    let empty = Names::key(cx.realm.names.empty);
    internal::create_data_property(cx, wrapper, empty, value)?;
    if !serializer.property(cx, wrapper, empty)? {
        return Ok(Value::UNDEFINED);
    }
    ops::alloc_units(cx, &serializer.out)
}

/// The keys a replacer array selects, deduplicated, in order.
fn property_list(cx: &mut Cx<'_>, replacer: ObjectId) -> Result<Vec<PropertyKey>, Throw> {
    let length = internal::length_of_array_like(cx, replacer)?;
    let mut keys = Vec::new();
    let mut index = 0u64;
    while index < length {
        let key = internal::number_key(cx, index as f64)?;
        let value = internal::get(cx, replacer, key, Value::object(replacer))?;
        index += 1;
        let item = match value.kind() {
            Kind::String => value,
            Kind::Int32 | Kind::Number => ops::to_string(cx, value)?,
            Kind::Object
                if matches!(
                    cx.heap.object_kind(value.as_object().unwrap()),
                    ObjectKind::StringWrapper | ObjectKind::NumberWrapper
                ) =>
            {
                ops::to_string(cx, value)?
            }
            _ => continue,
        };
        let units = ops::units(cx.heap, item.as_string().unwrap());
        let key = cx
            .heap
            .property_key_units(&units)
            .map_err(|error| error::heap_error(cx, error))?;
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// The indentation `space` asks for: up to ten spaces or ten units.
fn gap(cx: &mut Cx<'_>, space: Value) -> Result<Vec<u16>, Throw> {
    let mut space = space;
    if let Some(object) = space.as_object() {
        match cx.heap.object_kind(object) {
            ObjectKind::NumberWrapper => space = Value::number(ops::to_number(cx, space)?),
            ObjectKind::StringWrapper => space = ops::to_string(cx, space)?,
            _ => {}
        }
    }
    if let Some(number) = space.as_f64() {
        let count = internal::to_integer_or_infinity(cx, Value::number(number))?.clamp(0.0, 10.0);
        return Ok(vec![0x20; count as usize]);
    }
    if let Some(string) = space.as_string() {
        let mut units = ops::units(cx.heap, string);
        units.truncate(10);
        return Ok(units);
    }
    Ok(Vec::new())
}

impl Serializer {
    fn text(&mut self, text: &str) {
        self.out.extend(text.encode_utf16());
    }

    /// `SerializeJSONProperty(key, holder)`; false when the value is not
    /// serializable (undefined, a symbol, a function). Nested containers are
    /// walked on an explicit stack of [`Level`]s.
    fn property(
        &mut self,
        cx: &mut Cx<'_>,
        holder: ObjectId,
        key: PropertyKey,
    ) -> Result<bool, Throw> {
        let mark = cx.heap.roots().mark();
        let result = self.serialize(cx, holder, key, mark);
        cx.heap.roots().truncate(mark);
        self.stack.clear();
        result
    }

    fn serialize(
        &mut self,
        cx: &mut Cx<'_>,
        holder: ObjectId,
        key: PropertyKey,
        mark: crate::RootMark,
    ) -> Result<bool, Throw> {
        let mut levels: Vec<Level> = Vec::new();
        match self.prepare(cx, holder, key)? {
            Prepared::Done(serialized) => return Ok(serialized),
            Prepared::Container(object, is_array) => {
                levels.push(self.open(cx, object, is_array, mark)?);
            }
        }
        while let Some(level) = levels.last_mut() {
            let object = level.object;
            let member = match &level.keys {
                Some(keys) => keys.get(level.next as usize).copied(),
                None if level.next < level.length => {
                    Some(internal::number_key(cx, level.next as f64)?)
                }
                None => None,
            };
            let Some(member) = member else {
                let level = levels.pop().expect("a level");
                self.close(cx, &level);
                continue;
            };
            let level = levels.last_mut().expect("a level");
            let is_object = level.keys.is_some();
            let rollback = self.out.len();
            if is_object {
                if level.any {
                    self.out.push(0x2c);
                }
            } else if level.next > 0 {
                self.out.push(0x2c);
            }
            level.next += 1;
            self.separator();
            if is_object {
                let name = super::object::key_string(cx, member)?;
                let units = ops::units(cx.heap, name.as_string().unwrap());
                quote(&mut self.out, &units);
                self.out.push(0x3a);
                if !self.gap.is_empty() {
                    self.out.push(0x20);
                }
            }
            let child_mark = cx.heap.roots().mark();
            match self.prepare(cx, object, member)? {
                Prepared::Done(serialized) => {
                    cx.heap.roots().truncate(child_mark);
                    let level = levels.last_mut().expect("a level");
                    if is_object {
                        if serialized {
                            level.any = true;
                        } else {
                            self.out.truncate(rollback);
                        }
                    } else if !serialized {
                        self.text("null");
                    }
                }
                Prepared::Container(child, is_array) => {
                    levels.last_mut().expect("a level").any = true;
                    let level = self.open(cx, child, is_array, child_mark)?;
                    levels.push(level);
                }
            }
        }
        Ok(true)
    }

    /// The part of `SerializeJSONProperty` before a container's members:
    /// `toJSON`, the replacer, unwrapping and primitives.
    fn prepare(
        &mut self,
        cx: &mut Cx<'_>,
        holder: ObjectId,
        key: PropertyKey,
    ) -> Result<Prepared, Throw> {
        let mut value = internal::get(cx, holder, key, Value::object(holder))?;
        cx.heap.roots().push(value);
        if matches!(value.kind(), Kind::Object | Kind::BigInt) {
            let to_json = Names::key(cx.realm.names.to_json);
            let method = property::get(cx, value, to_json)?;
            if ops::is_callable(cx.heap, method) {
                let name = super::object::key_string(cx, key)?;
                cx.heap.roots().push(name);
                value = crate::vm::call(cx, method, value, &[name])?;
                cx.heap.roots().push(value);
            }
        }
        if let Some(replacer) = self.replacer {
            let name = super::object::key_string(cx, key)?;
            cx.heap.roots().push(name);
            value = crate::vm::call(cx, replacer, Value::object(holder), &[name, value])?;
            cx.heap.roots().push(value);
        }
        if let Some(object) = value.as_object() {
            match cx.heap.object_kind(object) {
                ObjectKind::NumberWrapper => value = Value::number(ops::to_number(cx, value)?),
                ObjectKind::StringWrapper => value = ops::to_string(cx, value)?,
                ObjectKind::BooleanWrapper | ObjectKind::BigIntWrapper => {
                    value = cx.heap.internal_slot(object, 0);
                }
                _ if is_raw_json_object(cx, value) => {
                    let key = cx
                        .heap
                        .property_key("rawJSON")
                        .map_err(|error| error::heap_error(cx, error))?;
                    let raw = internal::get(cx, object, key, value)?;
                    let raw = ops::to_string(cx, raw)?;
                    let units = ops::units(cx.heap, raw.as_string().unwrap());
                    self.out.extend(units);
                    return Ok(Prepared::Done(true));
                }
                _ => {}
            }
            cx.heap.roots().push(value);
        }
        match value.kind() {
            Kind::Null => self.text("null"),
            Kind::Bool => self.text(if value == Value::TRUE {
                "true"
            } else {
                "false"
            }),
            Kind::String => {
                let units = ops::units(cx.heap, value.as_string().unwrap());
                quote(&mut self.out, &units);
            }
            Kind::Int32 | Kind::Number => {
                let number = value.as_f64().unwrap();
                if number.is_finite() {
                    let text = number::to_string(number);
                    self.text(&text);
                } else {
                    self.text("null");
                }
            }
            Kind::BigInt => {
                return Err(error::type_error(
                    cx,
                    "Do not know how to serialize a BigInt",
                ));
            }
            Kind::Object if !ops::is_callable(cx.heap, value) => {
                let object = value.as_object().unwrap();
                if self.stack.contains(&object) {
                    return Err(error::type_error(cx, "circular reference"));
                }
                let is_array = internal::is_array(cx, value)?;
                return Ok(Prepared::Container(object, is_array));
            }
            _ => return Ok(Prepared::Done(false)),
        }
        Ok(Prepared::Done(true))
    }

    /// Starts writing a container: its opening bracket, keys or length.
    fn open(
        &mut self,
        cx: &mut Cx<'_>,
        object: ObjectId,
        is_array: bool,
        mark: crate::RootMark,
    ) -> Result<Level, Throw> {
        self.stack.insert(object);
        let stepback = self.indent.len();
        self.indent.extend_from_slice(&self.gap.clone());
        let (keys, length) = if is_array {
            (None, internal::length_of_array_like(cx, object)?)
        } else {
            let keys = match &self.keys {
                Some(keys) => keys.clone(),
                None => internal::enumerable_own_keys(cx, object)?,
            };
            (Some(keys), 0)
        };
        self.out.push(if is_array { 0x5b } else { 0x7b });
        Ok(Level {
            object,
            keys,
            length,
            next: 0,
            any: false,
            stepback,
            mark,
        })
    }

    /// Ends a container: the closing bracket after its last member.
    fn close(&mut self, cx: &mut Cx<'_>, level: &Level) {
        self.indent.truncate(level.stepback);
        let is_array = level.keys.is_none();
        if (is_array && level.length > 0) || (!is_array && level.any) {
            self.separator();
        }
        self.out.push(if is_array { 0x5d } else { 0x7d });
        self.stack.remove(&level.object);
        cx.heap.roots().truncate(level.mark);
    }

    /// The line break and indentation before a member, with a gap.
    fn separator(&mut self) {
        if !self.gap.is_empty() {
            self.out.push(0x0a);
            self.out.extend_from_slice(&self.indent);
        }
    }
}

/// `QuoteJSONString`: lone surrogates and control characters escaped.
fn quote(out: &mut Vec<u16>, units: &[u16]) {
    out.push(0x22);
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        index += 1;
        let escape = match unit {
            0x08 => "\\b",
            0x09 => "\\t",
            0x0a => "\\n",
            0x0c => "\\f",
            0x0d => "\\r",
            0x22 => "\\\"",
            0x5c => "\\\\",
            0xd800..=0xdbff
                if units
                    .get(index)
                    .is_some_and(|next| (0xdc00..=0xdfff).contains(next)) =>
            {
                out.push(unit);
                out.push(units[index]);
                index += 1;
                continue;
            }
            0x00..=0x1f | 0xd800..=0xdfff => {
                out.extend(format!("\\u{unit:04x}").encode_utf16());
                continue;
            }
            _ => {
                out.push(unit);
                continue;
            }
        };
        out.extend(escape.encode_utf16());
    }
    out.push(0x22);
}
