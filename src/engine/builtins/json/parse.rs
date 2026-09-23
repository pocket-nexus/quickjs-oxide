//! Strict, allocation-direct JSON parser from pinned QuickJS.
//!
//! This is intentionally not routed through the JavaScript lexer or an
//! external serialization crate. `JSON.parse` has a smaller lexical grammar,
//! preserves arbitrary UTF-16 code units, allocates realm-correct objects as
//! input is consumed, and records exact source spans for the reviver.

use std::rc::Rc;

use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::ContextId;
use crate::source::{LineColumn, QuickJsSourceLocator};

use crate::engine::object::{DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsStringError, Value};
use crate::engine::vm::frames::ExplicitBacktraceLocation;

/// Maximum nesting depth for `JSON.parse` and `JSON.rawJSON`: the value-entry
/// check passes at this depth and fails at the next one.
///
/// Pinned QuickJS has no explicit nesting constant. `json_next_token` polls
/// the platform stack pointer against the one-MiB `JS_DEFAULT_STACK_SIZE`
/// budget while advancing every token (`quickjs.c` `json_next_token`), and
/// the recursive `json_parse_value` consumes one C frame per open container.
/// On the pinned 2026-06-04 x86-64 artifact, a try-wrapped top-level call
/// bottoms out with 10,894 live value frames; the value entered at nesting
/// depth 10,894 is the first one whose token advance raises the catchable
/// `SyntaxError("stack overflow")`. A leaf nested in `n` arrays therefore
/// fails at `n = 10_894`, while a chain of `n` *empty* arrays reaches only
/// depth `n - 1` and survives to `n = 10_895`. Arrays and objects share one
/// frame shape, so both obey the same count in that calibration shape. Pinned
/// QuickJS's exact cutoff shifts with the active native call stack; the fixed
/// logical-budget differences are recorded in `docs/deviations.md`.
///
/// The descent in `parse_document` is iterative: each open container lives in
/// a heap-allocated [`JsonContainerFrame`], so this number is a logical parity
/// budget reproducing the pinned cutoff independently of the Rust thread
/// stack or build profile. A pathological payload can never consume the host
/// call stack and abort the process.
const MAX_JSON_PARSE_DEPTH: usize = 10_893;

/// A direct static JSON import from the entry module reaches `JS_ParseJSON`
/// through a shallower pinned C call path and therefore retains eighteen more
/// nested values than the try-wrapped `JSON.parse` calibration within the same
/// one-MiB stack budget. Nested and dynamic imports have different pinned
/// cutoffs; the fixed logical-budget differences are recorded in
/// `docs/deviations.md`. Strict JSON and host-selected extended JSON use the
/// same Oxide parser entry path.
const MAX_JSON_MODULE_PARSE_DEPTH: usize = 10_911;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JsonContainerKind {
    Array,
    Object,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JsonObjectPhase {
    /// The opener or a comma has been consumed; the next token is a member
    /// name (or the closing brace, already handled before the frame push).
    MemberName,
    /// A member name and its ':' have been consumed; the next token is the
    /// member value.
    MemberValue,
}

/// Iterative counterpart of one live recursive `json_parse_value` C frame for
/// an open `{` or `[` container.
struct JsonContainerFrame {
    kind: JsonContainerKind,
    object: ObjectRef,
    /// Parse records of array elements (only populated while retaining).
    elements: Vec<Rc<JsonParseRecord>>,
    /// Parse records of object members (only populated while retaining).
    entries: Vec<JsonObjectParseRecordEntry>,
    /// Object member name awaiting its value; the phase is always
    /// [`JsonObjectPhase::MemberValue`] while this is present.
    pending_key: Option<PropertyKey>,
    phase: JsonObjectPhase,
    /// Monotonic array element count, retained purely as the pinned
    /// `uint32_t` overflow guard. The heap appends elements positionally.
    array_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JsonParseMode {
    Strict,
    QuickJsExtended,
}

impl JsonParseMode {
    const fn is_extended(self) -> bool {
        matches!(self, Self::QuickJsExtended)
    }
}

pub(crate) struct JsonParseRecord {
    original: Value,
    kind: JsonParseRecordKind,
}

enum JsonParseRecordKind {
    Primitive { start: usize, end: usize },
    Array(Vec<Rc<JsonParseRecord>>),
    Object(JsonObjectParseRecord),
}

struct JsonObjectParseRecord {
    entries: Vec<JsonObjectParseRecordEntry>,
    /// Pinned QuickJS starts its record hash table while adding member nine.
    /// Linear lookup returns the first duplicate; hashed lookup returns the
    /// newest duplicate because new entries are linked at the bucket head.
    hashed: bool,
}

struct JsonObjectParseRecordEntry {
    key: PropertyKey,
    record: Rc<JsonParseRecord>,
}

impl JsonParseRecord {
    pub(crate) fn matches(&self, value: &Value) -> bool {
        self.original.same_value(value)
    }

    pub(crate) fn primitive_span(&self) -> Option<(usize, usize)> {
        match self.kind {
            JsonParseRecordKind::Primitive { start, end } => Some((start, end)),
            JsonParseRecordKind::Array(_) | JsonParseRecordKind::Object(_) => None,
        }
    }

    pub(crate) fn array_child(&self, index: usize) -> Option<Rc<Self>> {
        let JsonParseRecordKind::Array(elements) = &self.kind else {
            return None;
        };
        elements.get(index).cloned()
    }
    pub(crate) fn object_child(&self, key: &PropertyKey) -> Option<Rc<Self>> {
        let JsonParseRecordKind::Object(object) = &self.kind else {
            return None;
        };
        let mut entries = object.entries.iter();
        let entry = if object.hashed {
            entries.rev().find(|entry| &entry.key == key)
        } else {
            entries.find(|entry| &entry.key == key)
        };
        entry.map(|entry| entry.record.clone())
    }
}

struct JsonSyntaxFailure {
    message: String,
    /// UTF-16 source-unit offset used as QuickJS's `token.ptr` equivalent.
    offset: usize,
}

enum JsonParseFailure {
    Syntax(JsonSyntaxFailure),
    Runtime(RuntimeError),
}

impl From<RuntimeError> for JsonParseFailure {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}

type JsonParseResult<T> = Result<T, JsonParseFailure>;

struct JsonParser<'a> {
    runtime: &'a Runtime,
    realm: ContextId,
    units: Vec<u16>,
    /// UTF-16-unit offsets whose DEL carrier represents one malformed source
    /// byte. Genuine U+007F input has no entry and therefore keeps its normal
    /// JSON string semantics.
    invalid_unit_offsets: Vec<usize>,
    /// The explicitly sized host buffer used by JSON module parsing. Keeping
    /// this borrowed rather than copying it lets diagnostics recover the
    /// exact QuickJS byte column, including CESU-8 and malformed sequences.
    raw_source: Option<&'a [u8]>,
    cursor: usize,
    retain_record: bool,
    mode: JsonParseMode,
    max_depth: usize,
}

#[derive(Clone, Copy)]
enum JsonModuleSource<'source> {
    Text(&'source JsString),
    Bytes(&'source [u8]),
}

impl Runtime {
    pub(crate) fn parse_json_text(
        &self,
        realm: ContextId,
        source: &JsString,
        retain_record: bool,
    ) -> Result<NativeConversion<(Value, Option<JsonParseRecord>)>, RuntimeError> {
        self.0.state.borrow().heap.context(realm)?;
        let mut parser = JsonParser {
            runtime: self,
            realm,
            units: source.utf16_units().collect(),
            invalid_unit_offsets: Vec::new(),
            raw_source: None,
            cursor: 0,
            retain_record,
            mode: JsonParseMode::Strict,
            max_depth: MAX_JSON_PARSE_DEPTH,
        };
        match parser.parse_document() {
            Ok(value) => Ok(NativeConversion::Value(value)),
            Err(JsonParseFailure::Syntax(failure)) => {
                // `js_json_parse` passes the synthetic filename `<input>` to
                // `JS_ParseJSON3`.  Its `js_parse_error_v` path constructs the
                // SyntaxError without a backtrace, then prepends that exact
                // token location before the active native/bytecode frames.
                let position = parser.source_location(failure.offset)?;
                let exception = self.new_native_error_without_backtrace_from_error(
                    realm,
                    NativeErrorKind::Syntax,
                    &Error::new(ErrorKind::Syntax, failure.message),
                )?;
                self.ensure_error_backtrace(
                    &exception,
                    false,
                    Some(ExplicitBacktraceLocation {
                        filename: JsString::from_static("<input>"),
                        position,
                    }),
                )?;
                Ok(NativeConversion::Throw(exception))
            }
            Err(JsonParseFailure::Runtime(error)) => Err(error),
        }
    }

    /// Parse one strict JSON module payload while retaining the pinned
    /// QuickJS filename and token-start diagnostic location.
    pub(crate) fn parse_json_module_text(
        &self,
        realm: ContextId,
        source: &JsString,
        filename: &JsString,
    ) -> Result<NativeConversion<Value>, RuntimeError> {
        self.parse_json_module_source_with_mode(
            realm,
            JsonModuleSource::Text(source),
            filename,
            JsonParseMode::Strict,
        )
    }

    /// Parse one strict JSON module from an explicitly sized byte buffer.
    ///
    /// Unlike an intermediate [`JsString`], this preserves malformed UTF-8,
    /// surrogate encodings, and byte-oriented QuickJS diagnostics.
    pub(crate) fn parse_json_module_bytes(
        &self,
        realm: ContextId,
        source: &[u8],
        filename: &JsString,
    ) -> Result<NativeConversion<Value>, RuntimeError> {
        self.parse_json_module_source_with_mode(
            realm,
            JsonModuleSource::Bytes(source),
            filename,
            JsonParseMode::Strict,
        )
    }

    /// Parse one host-selected QuickJS extended-JSON module payload.
    ///
    /// This is deliberately separate from `JSON.parse` and strict JSON
    /// modules: only an attributes-aware host loader may select this mode.
    pub(crate) fn parse_json5_module_text(
        &self,
        realm: ContextId,
        source: &JsString,
        filename: &JsString,
    ) -> Result<NativeConversion<Value>, RuntimeError> {
        self.parse_json_module_source_with_mode(
            realm,
            JsonModuleSource::Text(source),
            filename,
            JsonParseMode::QuickJsExtended,
        )
    }

    /// Parse one host-selected QuickJS extended-JSON module from an explicitly
    /// sized byte buffer.
    pub(crate) fn parse_json5_module_bytes(
        &self,
        realm: ContextId,
        source: &[u8],
        filename: &JsString,
    ) -> Result<NativeConversion<Value>, RuntimeError> {
        self.parse_json_module_source_with_mode(
            realm,
            JsonModuleSource::Bytes(source),
            filename,
            JsonParseMode::QuickJsExtended,
        )
    }

    fn parse_json_module_source_with_mode<'source>(
        &self,
        realm: ContextId,
        source: JsonModuleSource<'source>,
        filename: &JsString,
        mode: JsonParseMode,
    ) -> Result<NativeConversion<Value>, RuntimeError> {
        self.0.state.borrow().heap.context(realm)?;
        let mut parser = match source {
            JsonModuleSource::Text(source) => JsonParser {
                runtime: self,
                realm,
                units: source.utf16_units().collect(),
                invalid_unit_offsets: Vec::new(),
                raw_source: None,
                cursor: 0,
                retain_record: false,
                mode,
                max_depth: MAX_JSON_MODULE_PARSE_DEPTH,
            },
            JsonModuleSource::Bytes(source) => JsonParser::try_from_raw_bytes(
                self,
                realm,
                source,
                mode,
                MAX_JSON_MODULE_PARSE_DEPTH,
            )?,
        };
        match parser.parse_document() {
            Ok((value, None)) => Ok(NativeConversion::Value(value)),
            Ok((_, Some(_))) => Err(RuntimeError::Invariant(
                "JSON module parsing unexpectedly retained a parse record",
            )),
            Err(JsonParseFailure::Syntax(failure)) => {
                let position = parser.source_location(failure.offset)?;
                let exception = self.new_native_error_without_backtrace_from_error(
                    realm,
                    NativeErrorKind::Syntax,
                    &Error::new(ErrorKind::Syntax, failure.message),
                )?;
                self.ensure_error_backtrace(
                    &exception,
                    false,
                    Some(ExplicitBacktraceLocation {
                        filename: filename.clone(),
                        position,
                    }),
                )?;
                Ok(NativeConversion::Throw(exception))
            }
            Err(JsonParseFailure::Runtime(error)) => Err(error),
        }
    }
}

impl<'a> JsonParser<'a> {
    fn try_from_raw_bytes(
        runtime: &'a Runtime,
        realm: ContextId,
        source: &'a [u8],
        mode: JsonParseMode,
        max_depth: usize,
    ) -> Result<Self, RuntimeError> {
        let mut units = Vec::new();
        units
            .try_reserve_exact(source.len())
            .map_err(|_| JsStringError::OutOfMemory)?;
        let mut invalid_unit_offsets = Vec::new();
        let mut byte_offset = 0;
        while byte_offset < source.len() {
            let byte = source[byte_offset];
            if byte < 0x80 {
                units.push(u16::from(byte));
                byte_offset += 1;
                continue;
            }

            match crate::engine::value::decode_quickjs_utf8(&source[byte_offset..]) {
                Some((code_point, consumed)) if code_point <= 0x10_ffff => {
                    if code_point <= 0xffff {
                        units.push(code_point as u16);
                    } else {
                        let scalar = code_point - 0x1_0000;
                        units.push(0xd800 | ((scalar >> 10) as u16));
                        units.push(0xdc00 | ((scalar & 0x3ff) as u16));
                    }
                    byte_offset += consumed;
                }
                Some(_) | None => {
                    invalid_unit_offsets
                        .try_reserve(1)
                        .map_err(|_| JsStringError::OutOfMemory)?;
                    invalid_unit_offsets.push(units.len());
                    // DEL occupies one unit but is semantically inert while
                    // its offset is present in `invalid_unit_offsets`.
                    units.push(u16::from(b'\x7f'));
                    byte_offset += 1;
                }
            }
        }

        Ok(Self {
            runtime,
            realm,
            units,
            invalid_unit_offsets,
            raw_source: Some(source),
            cursor: 0,
            retain_record: false,
            mode,
            max_depth,
        })
    }

    fn parse_document(&mut self) -> JsonParseResult<(Value, Option<JsonParseRecord>)> {
        // Explicit heap stack of open `{` and `[` containers. Pinned QuickJS
        // keeps one recursive `json_parse_value` C frame per open container
        // and polls its one-MiB platform stack pointer while advancing every
        // token. This descent is iterative: each open container lives in a
        // [`JsonContainerFrame`], so the depth cutoff reproduced below is
        // independent of the host thread stack or build profile and a
        // pathological payload can never abort the process.
        let mut frames: Vec<JsonContainerFrame> = Vec::new();
        // A value that just finished (leaf or freshly closed container) and
        // is waiting to be attached to its parent container.
        let mut pending: Option<(Value, Option<JsonParseRecord>)> = None;

        let root = loop {
            // Attach a finished child to the container on top of the stack.
            if let Some((value, record)) = pending.take() {
                let Some(parent_index) = frames.len().checked_sub(1) else {
                    // No open container: the root value is complete.
                    break (value, record);
                };
                let closed =
                    match frames[parent_index].kind {
                        JsonContainerKind::Array => {
                            let array = frames[parent_index].object.clone();
                            self.runtime.append_fresh_array_value(&array, value)?;
                            if let Some(child) = record {
                                frames[parent_index].elements.push(Rc::new(child));
                            }
                            let next = frames[parent_index].array_index.checked_add(1).ok_or_else(
                                || {
                                    JsonParseFailure::Runtime(RuntimeError::Engine(Error::new(
                                        ErrorKind::Range,
                                        "invalid array length",
                                    )))
                                },
                            )?;
                            frames[parent_index].array_index = next;
                            self.finish_container_element(&frames[parent_index])?
                        }
                        JsonContainerKind::Object => {
                            let key = frames[parent_index]
                                .pending_key
                                .take()
                                .expect("object member value has no pending key");
                            let object = frames[parent_index].object.clone();
                            self.define_json_property(&object, &key, value)?;
                            if let Some(child) = record {
                                frames[parent_index]
                                    .entries
                                    .push(JsonObjectParseRecordEntry {
                                        key,
                                        record: Rc::new(child),
                                    });
                            }
                            let closed = self.finish_container_element(&frames[parent_index])?;
                            if !closed {
                                frames[parent_index].phase = JsonObjectPhase::MemberName;
                            }
                            closed
                        }
                    };
                if closed {
                    let frame = frames.pop().expect("closed container has no frame");
                    pending = Some(self.complete_container_frame(frame));
                }
                continue;
            }

            // An open object whose opener or comma was consumed is waiting
            // for its member name and ':' before the value descent.
            if frames.last().is_some_and(|frame| {
                frame.kind == JsonContainerKind::Object
                    && frame.phase == JsonObjectPhase::MemberName
            }) {
                self.parse_object_member_name(&mut frames)?;
                continue;
            }

            // Parse one value nested beneath every open container. Pinned
            // QuickJS has already lexed this token before recursively entering
            // `json_parse_value`. A container opener immediately advances to
            // its first child, so an over-budget opener reports stack overflow.
            // A leaf is different: its lexer error (including EOF or an
            // unexpected token) has already won before the recursive call.
            // Only a successfully lexed leaf advances again and observes the
            // exhausted stack. Preserve that diagnostic ordering while keeping
            // the descent itself iterative.
            let depth = frames.len();
            self.skip_whitespace()?;
            if self.current_unit_is_invalid() {
                return self.syntax("unexpected character");
            }
            let Some(unit) = self.peek() else {
                return self.syntax("Unexpected end of JSON input");
            };
            let over_depth_budget = depth > self.max_depth;
            match unit {
                unit if unit == u16::from(b'{') => {
                    if over_depth_budget {
                        return self.syntax("stack overflow");
                    }
                    if self.open_object_frame(&mut frames)? {
                        let frame = frames.pop().expect("closed container has no frame");
                        pending = Some(self.complete_container_frame(frame));
                    }
                }
                unit if unit == u16::from(b'[') => {
                    if over_depth_budget {
                        return self.syntax("stack overflow");
                    }
                    if self.open_array_frame(&mut frames)? {
                        let frame = frames.pop().expect("closed container has no frame");
                        pending = Some(self.complete_container_frame(frame));
                    }
                }
                _ => {
                    let token_start = self.cursor;
                    let leaf = self.parse_leaf_value(unit)?;
                    if over_depth_budget {
                        return self.syntax_at(token_start, "stack overflow");
                    }
                    pending = Some(leaf);
                }
            }
        };

        self.skip_whitespace()?;
        if self.cursor != self.units.len() {
            // QuickJS lexes the next token before reporting trailing data, so
            // malformed trailing strings/numbers retain their lexical error.
            let trailing_start = self.cursor;
            self.validate_current_token_lexically()?;
            return self.syntax_at(trailing_start, "unexpected data at the end");
        }
        Ok(root)
    }

    fn parse_leaf_value(&mut self, unit: u16) -> JsonParseResult<(Value, Option<JsonParseRecord>)> {
        match unit {
            unit if unit == u16::from(b'"')
                || (self.mode.is_extended() && unit == u16::from(b'\'')) =>
            {
                let start = self.cursor;
                let string = self.parse_string(unit)?;
                let end = self.cursor;
                let value = Value::String(string);
                let record = self.primitive_record(value.clone(), start, end);
                Ok((value, record))
            }
            unit if unit == u16::from(b'-')
                || is_ascii_digit(unit)
                || (self.mode.is_extended()
                    && (unit == u16::from(b'+')
                        || (unit == u16::from(b'.')
                            && self.peek_at(1).is_some_and(is_ascii_digit)))) =>
            {
                self.parse_number_value()
            }
            unit if is_ascii_identifier_start(unit) => self.parse_identifier_value(),
            unit if unit >= 0x80 => self.syntax("unexpected character"),
            0 => self.syntax("unexpected token: ''"),
            _ => self.syntax(&format!("unexpected token: '{}'", display_unit(unit))),
        }
    }

    fn parse_object_member_name(
        &mut self,
        frames: &mut [JsonContainerFrame],
    ) -> JsonParseResult<()> {
        let frame = frames
            .last_mut()
            .expect("member name requested without an object frame");
        self.skip_whitespace()?;
        if self.current_unit_is_invalid() {
            return self.syntax("unexpected character");
        }
        let name = match self.peek() {
            Some(unit)
                if unit == u16::from(b'"')
                    || (self.mode.is_extended() && unit == u16::from(b'\'')) =>
            {
                self.parse_string(unit)?
            }
            Some(unit) if self.mode.is_extended() && is_ascii_identifier_start(unit) => {
                self.parse_identifier_name()
            }
            Some(unit) if unit >= 0x80 => return self.syntax("unexpected character"),
            _ => {
                self.validate_current_token_lexically()?;
                return self.syntax("expecting property name");
            }
        };
        let key = self
            .runtime
            .intern_property_key_js_string(&name)
            .map_err(RuntimeError::from)?;
        self.skip_whitespace()?;
        self.validate_current_token_lexically()?;
        if !self.consume_ascii(b':') {
            return self.syntax("expecting ':'");
        }
        frame.pending_key = Some(key);
        frame.phase = JsonObjectPhase::MemberValue;
        Ok(())
    }

    /// Consume the punctuation following a container element or member.
    /// Returns `true` when it closed the container, `false` when a comma
    /// introduces another value (object) or element (array).
    fn finish_container_element(&mut self, frame: &JsonContainerFrame) -> JsonParseResult<bool> {
        let close = match frame.kind {
            JsonContainerKind::Array => b']',
            JsonContainerKind::Object => b'}',
        };
        self.skip_whitespace()?;
        self.validate_current_token_lexically()?;
        if self.consume_ascii(b',') {
            self.skip_whitespace()?;
            // QuickJS extended JSON permits a trailing comma before close.
            if self.mode.is_extended() && self.consume_ascii(close) {
                return Ok(true);
            }
            return Ok(false);
        }
        if !self.consume_ascii(close) {
            let message = match frame.kind {
                JsonContainerKind::Array => "expecting ']'",
                JsonContainerKind::Object => "expecting '}'",
            };
            return self.syntax(message);
        }
        Ok(true)
    }

    /// Open an object container after consuming its `{`. Returns `true` when
    /// the object was immediately closed (`{}`) and is therefore complete.
    fn open_object_frame(&mut self, frames: &mut Vec<JsonContainerFrame>) -> JsonParseResult<bool> {
        self.cursor += 1;
        let object = self.runtime.new_ordinary_object_in_realm(self.realm)?;
        self.skip_whitespace()?;
        let immediate_close = self.consume_ascii(b'}');
        frames.push(JsonContainerFrame {
            kind: JsonContainerKind::Object,
            object,
            elements: Vec::new(),
            entries: Vec::new(),
            pending_key: None,
            phase: JsonObjectPhase::MemberName,
            array_index: 0,
        });
        Ok(immediate_close)
    }

    /// Open an array container after consuming its `[`. Returns `true` when
    /// the array was immediately closed (`[]`) and is therefore complete.
    fn open_array_frame(&mut self, frames: &mut Vec<JsonContainerFrame>) -> JsonParseResult<bool> {
        self.cursor += 1;
        let array = self.runtime.new_array(self.realm)?;
        self.skip_whitespace()?;
        let immediate_close = self.consume_ascii(b']');
        frames.push(JsonContainerFrame {
            kind: JsonContainerKind::Array,
            object: array,
            elements: Vec::new(),
            entries: Vec::new(),
            pending_key: None,
            phase: JsonObjectPhase::MemberValue,
            array_index: 0,
        });
        Ok(immediate_close)
    }

    fn complete_container_frame(
        &self,
        frame: JsonContainerFrame,
    ) -> (Value, Option<JsonParseRecord>) {
        let value = Value::Object(frame.object);
        let record = self.retain_record.then(|| {
            let kind = match frame.kind {
                JsonContainerKind::Array => JsonParseRecordKind::Array(frame.elements),
                JsonContainerKind::Object => JsonParseRecordKind::Object(JsonObjectParseRecord {
                    // Pinned QuickJS starts its record hash table while
                    // adding member nine.
                    hashed: frame.entries.len() >= 9,
                    entries: frame.entries,
                }),
            };
            JsonParseRecord {
                original: value.clone(),
                kind,
            }
        });
        (value, record)
    }

    fn parse_string(&mut self, separator: u16) -> JsonParseResult<JsString> {
        debug_assert_eq!(self.peek(), Some(separator));
        let token_start = self.cursor;
        self.cursor += 1;
        let mut output = Vec::new();
        loop {
            let Some(unit) = self.peek() else {
                return self.syntax_at(token_start, "Unexpected end of JSON input");
            };
            let unit_offset = self.cursor;
            self.cursor += 1;
            if self.invalid_unit_at(unit_offset) {
                return self.syntax_at(unit_offset, "Bad UTF-8 sequence");
            }
            match unit {
                unit if unit == separator => break,
                unit if unit < 0x20 => {
                    return self.syntax_at(unit_offset, "Bad control character in string literal");
                }
                unit if unit == u16::from(b'\\') => {
                    let Some(escaped) = self.peek() else {
                        return self.syntax_at(token_start, "Unexpected end of JSON input");
                    };
                    let escaped_offset = self.cursor;
                    self.cursor += 1;
                    match escaped {
                        unit if unit == separator
                            || unit == u16::from(b'\\')
                            || unit == u16::from(b'/') =>
                        {
                            output.push(unit)
                        }
                        unit if unit == u16::from(b'b') => output.push(0x08),
                        unit if unit == u16::from(b'f') => output.push(0x0c),
                        unit if unit == u16::from(b'n') => output.push(0x0a),
                        unit if unit == u16::from(b'r') => output.push(0x0d),
                        unit if unit == u16::from(b't') => output.push(0x09),
                        unit if unit == u16::from(b'v') && self.mode.is_extended() => {
                            output.push(0x0b)
                        }
                        unit if unit == u16::from(b'\n') && self.mode.is_extended() => continue,
                        unit if unit == u16::from(b'u') => {
                            let mut value = 0_u16;
                            for _ in 0..4 {
                                let Some(hex) = self.peek().and_then(hex_value) else {
                                    return self.syntax("Bad Unicode escape");
                                };
                                self.cursor += 1;
                                value = (value << 4) | u16::from(hex);
                            }
                            output.push(value);
                        }
                        _ => return self.syntax_at(escaped_offset, "Bad escaped character"),
                    }
                }
                _ => output.push(unit),
            }
        }
        Ok(JsString::from_owned_utf16(output))
    }

    fn parse_number_value(&mut self) -> JsonParseResult<(Value, Option<JsonParseRecord>)> {
        let start = self.cursor;
        let negative = if self.consume_ascii(b'-') {
            true
        } else {
            if self.mode.is_extended() {
                self.consume_ascii(b'+');
            }
            false
        };
        if self.cursor != start && self.peek().is_none() {
            return self.syntax("Unexpected token '");
        }

        if self.mode.is_extended() {
            if ascii_starts_with(&self.units[self.cursor..], b"Infinity") {
                self.cursor += b"Infinity".len();
                let value = Value::number(if negative {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                });
                let record = self.primitive_record(value.clone(), start, self.cursor);
                return Ok((value, record));
            }
            if ascii_starts_with(&self.units[self.cursor..], b"NaN") {
                self.cursor += b"NaN".len();
                let value = Value::number(f64::NAN);
                let record = self.primitive_record(value.clone(), start, self.cursor);
                return Ok((value, record));
            }

            if self.peek() == Some(u16::from(b'0')) {
                let radix = match self.peek_at(1) {
                    Some(unit) if unit == u16::from(b'x') || unit == u16::from(b'X') => Some(16),
                    Some(unit) if unit == u16::from(b'o') || unit == u16::from(b'O') => Some(8),
                    Some(unit) if unit == u16::from(b'b') || unit == u16::from(b'B') => Some(2),
                    _ => None,
                };
                if let Some(radix) = radix {
                    self.cursor += 2;
                    let digits_start = self.cursor;
                    if self
                        .peek()
                        .and_then(hex_value)
                        .is_none_or(|digit| u32::from(digit) >= radix)
                    {
                        let Some(unit) = self.peek() else {
                            return self.syntax("Unexpected token '");
                        };
                        let message = self.unexpected_percent_c_message_at(self.cursor, unit)?;
                        return self.syntax(&message);
                    }
                    while self
                        .peek()
                        .and_then(hex_value)
                        .is_some_and(|digit| u32::from(digit) < radix)
                    {
                        self.cursor += 1;
                    }
                    let digits =
                        JsString::from_owned_utf16(self.units[digits_start..self.cursor].to_vec());
                    let mut number = crate::engine::value::number_parse::parse_int(
                        &digits,
                        i32::try_from(radix).expect("JSON radix fits i32"),
                    );
                    if negative {
                        number = -number;
                    }
                    let value = Value::number(number);
                    let record = self.primitive_record(value.clone(), start, self.cursor);
                    return Ok((value, record));
                }
            }
        }

        let digits_start = self.cursor;
        let Some(first) = self.peek() else {
            return self.syntax("Unexpected end of JSON input");
        };
        if self.mode.is_extended() && first == u16::from(b'.') {
            // The fractional scanner below consumes the leading decimal point.
        } else if first == u16::from(b'0') {
            self.cursor += 1;
            if self.peek().is_some_and(is_ascii_digit) {
                return self.syntax_at(digits_start, "Unexpected number");
            }
        } else if (u16::from(b'1')..=u16::from(b'9')).contains(&first) {
            self.cursor += 1;
            while self.peek().is_some_and(is_ascii_digit) {
                self.cursor += 1;
            }
        } else {
            let message = self.unexpected_percent_c_message_at(self.cursor, first)?;
            return self.syntax(&message);
        }

        if self.consume_ascii(b'.') {
            if !self.peek().is_some_and(is_ascii_digit) {
                return self.syntax("Unterminated fractional number");
            }
            while self.peek().is_some_and(is_ascii_digit) {
                self.cursor += 1;
            }
        }
        if self
            .peek()
            .is_some_and(|unit| unit == u16::from(b'e') || unit == u16::from(b'E'))
        {
            self.cursor += 1;
            if self
                .peek()
                .is_some_and(|unit| unit == u16::from(b'+') || unit == u16::from(b'-'))
            {
                self.cursor += 1;
            }
            if !self.peek().is_some_and(is_ascii_digit) {
                return self.syntax("Exponent part is missing a number");
            }
            while self.peek().is_some_and(is_ascii_digit) {
                self.cursor += 1;
            }
        }

        let end = self.cursor;
        let spelling = JsString::from_owned_utf16(self.units[start..end].to_vec());
        let value = Value::number(crate::engine::value::number_parse::parse_float(&spelling));
        let record = self.primitive_record(value.clone(), start, end);
        Ok((value, record))
    }

    fn parse_identifier_name(&mut self) -> JsString {
        let start = self.cursor;
        debug_assert!(self.peek().is_some_and(is_ascii_identifier_start));
        self.cursor += 1;
        while self.peek().is_some_and(is_ascii_identifier_continue) {
            self.cursor += 1;
        }
        JsString::from_owned_utf16(self.units[start..self.cursor].to_vec())
    }

    fn parse_identifier_value(&mut self) -> JsonParseResult<(Value, Option<JsonParseRecord>)> {
        let start = self.cursor;
        self.cursor += 1;
        while self.peek().is_some_and(is_ascii_identifier_continue) {
            self.cursor += 1;
        }
        let end = self.cursor;
        let spelling = &self.units[start..end];
        let value = if ascii_eq(spelling, b"true") {
            Value::Bool(true)
        } else if ascii_eq(spelling, b"false") {
            Value::Bool(false)
        } else if ascii_eq(spelling, b"null") {
            Value::Null
        } else if self.mode.is_extended() && ascii_eq(spelling, b"NaN") {
            Value::number(f64::NAN)
        } else if self.mode.is_extended() && ascii_eq(spelling, b"Infinity") {
            Value::number(f64::INFINITY)
        } else {
            let token = spelling
                .iter()
                .map(|unit| char::from_u32(u32::from(*unit)).unwrap_or('\u{fffd}'))
                .collect::<String>();
            return self.syntax_at(start, &format!("unexpected token: '{token}'"));
        };
        let record = self.primitive_record(value.clone(), start, end);
        Ok((value, record))
    }

    fn primitive_record(
        &self,
        original: Value,
        start: usize,
        end: usize,
    ) -> Option<JsonParseRecord> {
        self.retain_record.then(|| JsonParseRecord {
            original,
            kind: JsonParseRecordKind::Primitive { start, end },
        })
    }

    fn define_json_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> JsonParseResult<()> {
        if !self.runtime.define_own_property(
            object,
            key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(value),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(true),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )? {
            return Err(JsonParseFailure::Runtime(RuntimeError::Invariant(
                "fresh JSON property definition was rejected",
            )));
        }
        Ok(())
    }

    /// QuickJS keeps one token of lookahead. Container punctuation errors are
    /// therefore reported only after the intervening token has been lexed;
    /// malformed strings and numbers retain their more specific diagnostics.
    fn validate_current_token_lexically(&mut self) -> JsonParseResult<()> {
        let saved_cursor = self.cursor;
        let Some(unit) = self.peek() else {
            return Ok(());
        };
        let result = if self.current_unit_is_invalid() || unit >= 0x80 {
            self.syntax("unexpected character")
        } else if unit == u16::from(b'"') || (self.mode.is_extended() && unit == u16::from(b'\'')) {
            self.parse_string(unit).map(|_| ())
        } else if unit == u16::from(b'-')
            || is_ascii_digit(unit)
            || (self.mode.is_extended()
                && (unit == u16::from(b'+')
                    || (unit == u16::from(b'.') && self.peek_at(1).is_some_and(is_ascii_digit))))
        {
            self.parse_number_value().map(|_| ())
        } else {
            Ok(())
        };
        if result.is_ok() {
            self.cursor = saved_cursor;
        }
        result
    }

    fn skip_whitespace(&mut self) -> JsonParseResult<()> {
        loop {
            while self.peek().is_some_and(|unit| {
                matches!(unit, 0x09 | 0x0a | 0x0d | 0x20)
                    || (self.mode.is_extended() && matches!(unit, 0x0b | 0x0c))
            }) {
                self.cursor += 1;
            }
            if !self.mode.is_extended() || self.peek() != Some(u16::from(b'/')) {
                return Ok(());
            }
            match self.peek_at(1) {
                Some(unit) if unit == u16::from(b'/') => {
                    self.cursor += 2;
                    while self
                        .peek()
                        .is_some_and(|unit| !matches!(unit, 0x0a | 0x0d | 0x2028 | 0x2029))
                    {
                        self.cursor += 1;
                    }
                    if self
                        .peek()
                        .is_some_and(|unit| matches!(unit, 0x2028 | 0x2029))
                    {
                        self.cursor += 1;
                    }
                }
                Some(unit) if unit == u16::from(b'*') => {
                    let comment_start = self.cursor;
                    self.cursor += 2;
                    loop {
                        let Some(unit) = self.peek() else {
                            return self.syntax_at(comment_start, "unexpected end of comment");
                        };
                        if unit == u16::from(b'*') && self.peek_at(1) == Some(u16::from(b'/')) {
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

    fn consume_ascii(&mut self, byte: u8) -> bool {
        if self.peek() == Some(u16::from(byte)) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u16> {
        self.units.get(self.cursor).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u16> {
        self.units.get(self.cursor.checked_add(offset)?).copied()
    }

    fn current_unit_is_invalid(&self) -> bool {
        self.invalid_unit_at(self.cursor)
    }

    fn invalid_unit_at(&self, offset: usize) -> bool {
        self.invalid_unit_offsets.binary_search(&offset).is_ok()
    }

    fn display_percent_c_unit_at(&self, offset: usize, unit: u16) -> char {
        if self.invalid_unit_at(offset) {
            '\u{fffd}'
        } else {
            display_percent_c_unit(unit)
        }
    }

    fn unexpected_percent_c_message_at(&self, offset: usize, unit: u16) -> JsonParseResult<String> {
        if unit == 0 {
            // `vsnprintf("...%c...", 0)` terminates the C string before the
            // format's closing quote, including for an embedded source NUL.
            return Ok("Unexpected token '".to_owned());
        }
        let omit_closing_quote = if self.invalid_unit_at(offset) {
            let raw_source =
                self.raw_source
                    .ok_or(JsonParseFailure::Runtime(RuntimeError::Invariant(
                        "JSON invalid-unit marker has no raw source",
                    )))?;
            let byte_offset = json_raw_byte_offset(raw_source, offset)?;
            let byte = *raw_source
                .get(byte_offset)
                .ok_or(JsonParseFailure::Runtime(RuntimeError::Invariant(
                    "JSON invalid-unit byte offset is invalid",
                )))?;
            // QuickJS materializes the `vsnprintf` bytes through its malformed
            // UTF-8 decoder. A lone continuation consumes the following ASCII
            // quote as part of the replacement span; invalid lead bytes do not.
            (0x80..=0xbf).contains(&byte)
        } else {
            false
        };
        let token = self.display_percent_c_unit_at(offset, unit);
        Ok(if omit_closing_quote {
            format!("Unexpected token '{token}")
        } else {
            format!("Unexpected token '{token}'")
        })
    }

    fn source_location(&self, offset: usize) -> Result<LineColumn, RuntimeError> {
        let Some(raw_source) = self.raw_source else {
            return json_source_location(&self.units, offset);
        };
        let byte_offset = json_raw_byte_offset(raw_source, offset)?;
        QuickJsSourceLocator::from_bytes(raw_source)
            .locate_byte_offset(byte_offset)
            .map_err(|_| RuntimeError::Invariant("JSON diagnostic byte offset is invalid"))
    }

    fn syntax<T>(&self, message: &str) -> JsonParseResult<T> {
        self.syntax_at(self.cursor, message)
    }

    fn syntax_at<T>(&self, offset: usize, message: &str) -> JsonParseResult<T> {
        debug_assert!(offset <= self.units.len());
        Err(JsonParseFailure::Syntax(JsonSyntaxFailure {
            message: message.to_owned(),
            offset,
        }))
    }
}

fn json_source_location(units: &[u16], offset: usize) -> Result<LineColumn, RuntimeError> {
    if offset > units.len() {
        return Err(RuntimeError::Invariant(
            "JSON diagnostic offset is outside its source",
        ));
    }

    let mut line = 0_u32;
    let mut column = 0_u32;
    let mut cursor = 0;
    while cursor < offset {
        let unit = units[cursor];
        if unit == u16::from(b'\n') {
            line = line
                .checked_add(1)
                .ok_or(RuntimeError::Invariant("JSON diagnostic line overflowed"))?;
            column = 0;
            cursor += 1;
            continue;
        }

        column = column
            .checked_add(1)
            .ok_or(RuntimeError::Invariant("JSON diagnostic column overflowed"))?;
        cursor += if (0xd800..=0xdbff).contains(&unit)
            && cursor + 1 < offset
            && (0xdc00..=0xdfff).contains(&units[cursor + 1])
        {
            2
        } else {
            1
        };
    }
    Ok(LineColumn::new(line, column))
}

/// Translate the parser's decoded UTF-16 cursor back to the exact byte
/// boundary used by QuickJS's `JSParseState`. Valid non-BMP scalars contribute
/// two parser units but one raw source column; CESU-8 surrogate pairs remain
/// two separately encoded units and therefore two columns.
fn json_raw_byte_offset(source: &[u8], unit_offset: usize) -> Result<usize, RuntimeError> {
    let mut byte_cursor = 0;
    let mut unit_cursor = 0;
    while byte_cursor < source.len() {
        if unit_cursor == unit_offset {
            return Ok(byte_cursor);
        }

        let byte = source[byte_cursor];
        let (consumed, produced_units) = if byte < 0x80 {
            (1, 1)
        } else {
            match crate::engine::value::decode_quickjs_utf8(&source[byte_cursor..]) {
                Some((code_point, consumed)) if code_point <= 0xffff => (consumed, 1),
                Some((code_point, consumed)) if code_point <= 0x10_ffff => (consumed, 2),
                Some(_) | None => (1, 1),
            }
        };
        if unit_offset < unit_cursor + produced_units {
            // No JSON grammar error can point between the two UTF-16 units of
            // one valid non-BMP scalar. Retain a deterministic source start if
            // a future caller nevertheless requests that interior position.
            return Ok(byte_cursor);
        }
        unit_cursor += produced_units;
        byte_cursor += consumed;
    }

    if unit_cursor == unit_offset {
        Ok(source.len())
    } else {
        Err(RuntimeError::Invariant(
            "JSON diagnostic offset is outside its raw source",
        ))
    }
}

fn is_ascii_digit(unit: u16) -> bool {
    (u16::from(b'0')..=u16::from(b'9')).contains(&unit)
}

fn is_ascii_identifier_start(unit: u16) -> bool {
    unit == u16::from(b'_')
        || unit == u16::from(b'$')
        || (u16::from(b'a')..=u16::from(b'z')).contains(&unit)
        || (u16::from(b'A')..=u16::from(b'Z')).contains(&unit)
}

fn is_ascii_identifier_continue(unit: u16) -> bool {
    is_ascii_identifier_start(unit) || is_ascii_digit(unit)
}

fn hex_value(unit: u16) -> Option<u8> {
    match unit {
        unit if (u16::from(b'0')..=u16::from(b'9')).contains(&unit) => {
            Some((unit - u16::from(b'0')) as u8)
        }
        unit if (u16::from(b'a')..=u16::from(b'f')).contains(&unit) => {
            Some((unit - u16::from(b'a') + 10) as u8)
        }
        unit if (u16::from(b'A')..=u16::from(b'F')).contains(&unit) => {
            Some((unit - u16::from(b'A') + 10) as u8)
        }
        _ => None,
    }
}

fn ascii_eq(units: &[u16], bytes: &[u8]) -> bool {
    units.len() == bytes.len()
        && units
            .iter()
            .zip(bytes)
            .all(|(unit, byte)| *unit == u16::from(*byte))
}

fn ascii_starts_with(units: &[u16], bytes: &[u8]) -> bool {
    units.len() >= bytes.len()
        && units
            .iter()
            .zip(bytes)
            .take(bytes.len())
            .all(|(unit, byte)| *unit == u16::from(*byte))
}

fn display_unit(unit: u16) -> char {
    char::from_u32(u32::from(unit)).unwrap_or('\u{fffd}')
}

/// QuickJS's numeric diagnostics format one raw UTF-8 byte with `%c`.
/// A non-ASCII leading byte is invalid UTF-8 on its own and becomes U+FFFD
/// when the resulting error string is materialized.
fn display_percent_c_unit(unit: u16) -> char {
    if unit >= 0x80 {
        '\u{fffd}'
    } else {
        display_unit(unit)
    }
}
