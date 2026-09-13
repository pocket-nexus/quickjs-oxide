//! `RegExp.prototype[Symbol.replace]`.

use crate::engine::builtins::native::{RegExpFlagKind, RegExpNativeKind};
use crate::engine::heap::RegExpObjectData;
use crate::engine::value::ReplacementStringBuffer;

use crate::regexp::{CompiledRegExp, ExecError, RegExpFlags, execute_with_interrupt};
use std::rc::Rc;

use super::super::replacement::{
    SubstitutionCaptures, SubstitutionInput, SubstitutionMatch, SubstitutionStatus,
};
use super::match_protocol::advance_string_index;
use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::builtins::native::NativeFunctionId;

use super::super::replacement::{
    NamedSubstitutionCapture, SubstitutionAction, named_substitution_capture,
};
use crate::engine::heap::{
    ContextId, Heap, ObjectData, ObjectId, ObjectPayload, PrimitiveObjectData, PropertySlot,
    RawValue,
};
use crate::engine::object::operations::InternalSetResult;
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, Value};
use crate::engine::vm::call::DirectCallTarget;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};
use crate::engine::vm::{Completion, ToPrimitiveHint};

const MAX_REPLACER_ARGUMENTS: usize = 65_534;

struct StandardRegExpReplace {
    program: Rc<CompiledRegExp>,
    last_index: Value,
}

impl Runtime {
    /// Rust port of pinned QuickJS `js_regexp_Symbol_replace`, including its
    /// raw standard-RegExp predicate and direct matcher.
    pub(crate) fn call_regexp_symbol_replace(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        finish_replace(
            self,
            realm,
            RegExpReplaceStep::start(self, realm, &invocation, arguments)?,
        )
    }

    fn standard_regexp_replace(
        &self,
        regexp: &ObjectRef,
    ) -> Result<Option<StandardRegExpReplace>, RuntimeError> {
        let last_index = self.intern_property_key("lastIndex")?;
        let exec = self.intern_property_key("exec")?;
        let flags = self.intern_property_key("flags")?;
        let global = self.intern_property_key("global")?;
        let unicode = self.intern_property_key("unicode")?;
        let state = self.0.state.borrow();
        let object = state.heap.object(regexp.object_id())?;
        let ObjectPayload::RegExp(RegExpObjectData::Compiled { program, .. }) = &object.payload
        else {
            return Ok(None);
        };
        // Pinned QuickJS's direct replacement helper cannot publish named
        // captures to GetSubstitution. Returning to the generic path makes
        // builtin exec create `groups` and preserves `$<name>` semantics.
        if program.has_named_captures() {
            return Ok(None);
        }
        let shape = state.heap.shape(object.shape)?;
        let Some(last_index_slot) = shape.find(last_index.atom()) else {
            return Ok(None);
        };
        let last_index_slot = usize::try_from(last_index_slot)
            .map_err(|_| RuntimeError::Invariant("shape index does not fit usize"))?;
        let last_index = match object.slots.get(last_index_slot) {
            Some(PropertySlot::Data(RawValue::Int(value))) => Value::Int(*value),
            Some(PropertySlot::Data(RawValue::Float(value))) => Value::Float(*value),
            Some(
                PropertySlot::Data(_)
                | PropertySlot::VarRef(_)
                | PropertySlot::Accessor { .. }
                | PropertySlot::AutoInit(_),
            )
            | None => return Ok(None),
        };

        if !raw_regexp_data_property_matches(
            &state.heap,
            regexp.object_id(),
            exec.atom(),
            NativeFunctionId::RegExp(RegExpNativeKind::Exec),
        )? || !raw_regexp_getter_matches(
            &state.heap,
            regexp.object_id(),
            flags.atom(),
            NativeFunctionId::RegExp(RegExpNativeKind::Flags),
        )? || !raw_regexp_getter_matches(
            &state.heap,
            regexp.object_id(),
            global.atom(),
            NativeFunctionId::RegExp(RegExpNativeKind::Flag(RegExpFlagKind::Global)),
        )? || !raw_regexp_getter_matches(
            &state.heap,
            regexp.object_id(),
            unicode.atom(),
            NativeFunctionId::RegExp(RegExpNativeKind::Flag(RegExpFlagKind::Unicode)),
        )? {
            return Ok(None);
        }

        Ok(Some(StandardRegExpReplace {
            program: program.clone(),
            last_index,
        }))
    }

    #[inline(never)]
    fn call_standard_regexp_replace(
        &self,
        realm: ContextId,
        regexp: &ObjectRef,
        input: &JsString,
        replacement: &JsString,
        standard: StandardRegExpReplace,
    ) -> Result<Completion, RuntimeError> {
        // The outer @@replace buffer was initialized before input conversion.
        // QuickJS's direct helper owns a second buffer and discards the outer
        // one when this path completes.
        let mut output = ReplacementStringBuffer::new(0);
        let program = standard.program;
        let flags = program.flags();
        let global = flags.contains(RegExpFlags::GLOBAL);
        let sticky = flags.contains(RegExpFlags::STICKY);
        let mut last_index = if global {
            if let Some(value) = self.set_regexp_last_index(realm, regexp, 0)? {
                return Ok(Completion::Throw(value));
            }
            0
        } else if sticky {
            match self.native_to_length(realm, &standard.last_index)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => return Ok(Completion::Throw(value)),
            }
        } else {
            0
        };
        let input_units = input.utf16_units().collect::<Vec<_>>();
        let mut next_source_position = 0_usize;

        loop {
            let matched = if last_index > input_units.len() as u64 {
                None
            } else {
                match execute_with_interrupt(
                    program.as_ref(),
                    &input_units,
                    usize::try_from(last_index).expect("RegExp start was bounded by String length"),
                    || false,
                ) {
                    Ok(value) => value,
                    Err(ExecError::OutOfMemory) => {
                        return Ok(Completion::Throw(self.new_native_error(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory in regexp execution",
                        )?));
                    }
                    Err(ExecError::Interrupted) => {
                        return Ok(Completion::Throw(self.new_native_error(
                            realm,
                            NativeErrorKind::Internal,
                            "interrupted",
                        )?));
                    }
                    Err(ExecError::InvalidProgram(_)) => {
                        return Err(RuntimeError::Invariant(
                            "compiled RegExp program failed executor validation",
                        ));
                    }
                    Err(ExecError::StartOutOfBounds { .. }) => {
                        return Err(RuntimeError::Invariant(
                            "bounded RegExp start was rejected by executor",
                        ));
                    }
                }
            };

            let Some(matched) = matched else {
                if (global || sticky)
                    && let Some(value) = self.set_regexp_last_index(realm, regexp, 0)?
                {
                    return Ok(Completion::Throw(value));
                }
                break;
            };
            let complete = matched.capture(0).ok_or(RuntimeError::Invariant(
                "successful RegExp execution omitted capture zero",
            ))?;
            if complete.start < next_source_position {
                return Err(RuntimeError::Invariant(
                    "direct RegExp matcher moved backward",
                ));
            }
            if next_source_position < complete.start {
                output.append_range(input, next_source_position, complete.start);
                if output.error().is_some() {
                    return self.complete_regexp_replacement_buffer(realm, output);
                }
            }
            if !replacement.is_empty() {
                let status = self.append_get_substitution(
                    realm,
                    &mut output,
                    SubstitutionInput {
                        matched: SubstitutionMatch::InputRange {
                            start: complete.start,
                            end: complete.end,
                        },
                        input,
                        position: complete.start,
                        captures: Some(SubstitutionCaptures::MatchRanges(matched.captures())),
                        named_captures: None,
                        replacement,
                    },
                )?;
                let status = match status {
                    Ok(status) => status,
                    Err(value) => return Ok(Completion::Throw(value)),
                };
                if matches!(status, SubstitutionStatus::BufferFailed) || output.error().is_some() {
                    return self.complete_regexp_replacement_buffer(realm, output);
                }
            }
            next_source_position = complete.end;
            if !global {
                if sticky {
                    let end = i32::try_from(complete.end).map_err(|_| {
                        RuntimeError::Invariant("RegExp match end exceeded signed String range")
                    })?;
                    if let Some(value) = self.set_regexp_last_index(realm, regexp, end)? {
                        return Ok(Completion::Throw(value));
                    }
                }
                break;
            }
            last_index = u64::try_from(complete.end)
                .map_err(|_| RuntimeError::Invariant("RegExp match end did not fit u64"))?;
            if complete.end == complete.start {
                last_index = advance_string_index(input, last_index, flags.is_unicode());
            }
        }

        if next_source_position < input.len() {
            output.append_range(input, next_source_position, input.len());
        }
        self.complete_regexp_replacement_buffer(realm, output)
    }

    fn complete_regexp_replacement_buffer(
        &self,
        realm: ContextId,
        output: ReplacementStringBuffer,
    ) -> Result<Completion, RuntimeError> {
        match self.finish_replacement_buffer(realm, output)? {
            NativeConversion::Value(value) => Ok(Completion::Return(Value::String(value))),
            NativeConversion::Throw(value) => Ok(Completion::Throw(value)),
        }
    }
}

fn raw_regexp_data_property_matches(
    heap: &Heap,
    object: ObjectId,
    atom: Atom,
    expected: NativeFunctionId,
) -> Result<bool, RuntimeError> {
    let Some(slot) = raw_regexp_property_slot(heap, object, atom)? else {
        return Ok(false);
    };
    let PropertySlot::Data(RawValue::Object(function)) = slot else {
        return Ok(false);
    };
    raw_native_function_matches(heap, *function, expected)
}

fn raw_regexp_getter_matches(
    heap: &Heap,
    object: ObjectId,
    atom: Atom,
    expected: NativeFunctionId,
) -> Result<bool, RuntimeError> {
    let Some(slot) = raw_regexp_property_slot(heap, object, atom)? else {
        return Ok(false);
    };
    let PropertySlot::Accessor {
        get: Some(function),
        ..
    } = slot
    else {
        return Ok(false);
    };
    raw_native_function_matches(heap, *function, expected)
}

fn raw_regexp_property_slot(
    heap: &Heap,
    object: ObjectId,
    atom: Atom,
) -> Result<Option<&PropertySlot>, RuntimeError> {
    let mut cursor = Some(object);
    let mut receiver = true;
    while let Some(object) = cursor {
        let object = heap.object(object)?;
        if !receiver && regexp_chain_object_is_exotic(object) {
            return Ok(None);
        }
        let shape = heap.shape(object.shape)?;
        if let Some(index) = shape.find(atom) {
            let index = usize::try_from(index)
                .map_err(|_| RuntimeError::Invariant("shape index does not fit usize"))?;
            return object
                .slots
                .get(index)
                .map(Some)
                .ok_or(RuntimeError::Invariant(
                    "shape property had no parallel object slot",
                ));
        }
        cursor = shape.prototype();
        receiver = false;
    }
    Ok(None)
}

fn regexp_chain_object_is_exotic(object: &ObjectData) -> bool {
    matches!(
        &object.payload,
        ObjectPayload::Array { .. }
            | ObjectPayload::Arguments { .. }
            | ObjectPayload::Primitive(PrimitiveObjectData::String(_))
    )
}

fn raw_native_function_matches(
    heap: &Heap,
    object: ObjectId,
    expected: NativeFunctionId,
) -> Result<bool, RuntimeError> {
    Ok(matches!(
        &heap.object(object)?.payload,
        ObjectPayload::NativeFunction { data, .. } if data.target == expected
    ))
}

pub(crate) enum RegExpReplaceStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpReplaceResume,
    },
    Primitive {
        value: Value,
        hint: ToPrimitiveHint,
        resume: RegExpReplaceResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: RegExpReplaceResume,
    },
    Exec {
        regexp: Value,
        input: Value,
        resume: RegExpReplaceResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: RegExpReplaceResume,
    },
}
pub(crate) struct RegExpReplaceResume {
    realm: ContextId,
    phase: ReplacePhase,
}
struct ReplaceInput {
    regexp: ObjectRef,
    replacement: Value,
    output: ReplacementStringBuffer,
}
struct ReplaceState {
    regexp: ObjectRef,
    input: JsString,
    functional: Option<CallableRef>,
    replacement: Option<JsString>,
    output: ReplacementStringBuffer,
    results: Vec<ObjectRef>,
    zero: Option<PropertyKey>,
    global: bool,
    unicode: bool,
}
struct ResultState {
    state: ReplaceState,
    results: std::vec::IntoIter<ObjectRef>,
    length_key: PropertyKey,
    index_key: PropertyKey,
    groups_key: PropertyKey,
    next_source: usize,
}
struct MatchState {
    state: ResultState,
    result: ObjectRef,
    capture_count: u32,
    matched: JsString,
    position: usize,
    captures: Vec<Value>,
}
struct NamedState {
    matched: MatchState,
    groups: Option<ObjectRef>,
    buffer: ReplacementStringBuffer,
    cursor: usize,
}
enum ReplacePhase {
    Input(ReplaceInput),
    Replacement(ReplaceState),
    Flags(ReplaceState),
    FlagsString(ReplaceState),
    InitialSet(ReplaceState),
    Exec(ReplaceState),
    EmptyMatch(ReplaceState),
    EmptyString(ReplaceState),
    LastIndex(ReplaceState),
    LastIndexNumber(ReplaceState),
    AdvancedSet(ReplaceState),
    Length {
        state: ResultState,
        result: ObjectRef,
    },
    LengthNumber {
        state: ResultState,
        result: ObjectRef,
    },
    Matched {
        state: ResultState,
        result: ObjectRef,
        count: u32,
    },
    MatchedString {
        state: ResultState,
        result: ObjectRef,
        count: u32,
    },
    Position(MatchState),
    PositionNumber(MatchState),
    Capture(MatchState),
    CaptureString(MatchState),
    Groups(MatchState),
    Callback(MatchState),
    CallbackString(MatchState),
    Named(NamedState),
    NamedString(NamedState),
}
impl RegExpReplaceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value: regexp } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@replace did not receive a generic invocation",
            ));
        };
        let Value::Object(regexp) = regexp else {
            return throw_replace(runtime, realm, NativeErrorKind::Type, "not an object");
        };
        let input = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "RegExp @@replace input argv was not padded",
            ))?
            .clone();
        let replacement = arguments
            .readable
            .get(1)
            .ok_or(RuntimeError::Invariant(
                "RegExp @@replace replacement argv was not padded",
            ))?
            .clone();
        Ok(Self::Primitive {
            value: input,
            hint: ToPrimitiveHint::String,
            resume: RegExpReplaceResume {
                realm,
                phase: ReplacePhase::Input(ReplaceInput {
                    regexp: regexp.clone(),
                    replacement,
                    output: ReplacementStringBuffer::new(0),
                }),
            },
        })
    }
}
fn throw_replace(
    runtime: &Runtime,
    realm: ContextId,
    kind: NativeErrorKind,
    message: &str,
) -> Result<RegExpReplaceStep, RuntimeError> {
    Ok(RegExpReplaceStep::Complete(Completion::Throw(
        runtime.new_native_error(realm, kind, message)?,
    )))
}
fn converted_string(
    runtime: &Runtime,
    realm: ContextId,
    value: Value,
) -> Result<NativeConversion<JsString>, RuntimeError> {
    if matches!(value, Value::Object(_)) {
        return Err(RuntimeError::Invariant(
            "RegExp replacement conversion returned an object",
        ));
    }
    runtime.native_to_js_string(realm, &value)
}
impl RegExpReplaceResume {
    fn read(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        phase: ReplacePhase,
    ) -> RegExpReplaceStep {
        RegExpReplaceStep::Read {
            object,
            key,
            resume: Self { realm, phase },
        }
    }
    fn primitive(
        realm: ContextId,
        value: Value,
        hint: ToPrimitiveHint,
        phase: ReplacePhase,
    ) -> RegExpReplaceStep {
        RegExpReplaceStep::Primitive {
            value,
            hint,
            resume: Self { realm, phase },
        }
    }
    fn set_index(
        runtime: &Runtime,
        realm: ContextId,
        state: ReplaceState,
        value: Value,
        initial: bool,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        Ok(RegExpReplaceStep::Set {
            object: state.regexp.clone(),
            key: runtime.intern_property_key("lastIndex")?,
            value,
            resume: Self {
                realm,
                phase: if initial {
                    ReplacePhase::InitialSet(state)
                } else {
                    ReplacePhase::AdvancedSet(state)
                },
            },
        })
    }
    fn prepared(
        runtime: &Runtime,
        realm: ContextId,
        state: ReplaceState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        if state.functional.is_none()
            && let Some(standard) = runtime.standard_regexp_replace(&state.regexp)?
        {
            // The existing raw predicate proves all direct matcher operations
            // are callback-free, including its numeric lastIndex.
            return Ok(RegExpReplaceStep::Complete(
                runtime.call_standard_regexp_replace(
                    realm,
                    &state.regexp,
                    &state.input,
                    state
                        .replacement
                        .as_ref()
                        .expect("non-functional replacement was not converted"),
                    standard,
                )?,
            ));
        }
        Ok(Self::read(
            realm,
            state.regexp.clone(),
            runtime.intern_property_key("flags")?,
            ReplacePhase::Flags(state),
        ))
    }
    fn execute(
        runtime: &Runtime,
        realm: ContextId,
        mut state: ReplaceState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        if state.zero.is_none() {
            state.zero = Some(runtime.intern_property_key("0")?);
        }
        Ok(RegExpReplaceStep::Exec {
            regexp: Value::Object(state.regexp.clone()),
            input: Value::String(state.input.clone()),
            resume: Self {
                realm,
                phase: ReplacePhase::Exec(state),
            },
        })
    }
    fn collected(
        runtime: &Runtime,
        realm: ContextId,
        mut state: ReplaceState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let results = std::mem::take(&mut state.results).into_iter();
        let state = ResultState {
            state,
            results,
            length_key: runtime.intern_property_key("length")?,
            index_key: runtime.intern_property_key("index")?,
            groups_key: runtime.intern_property_key("groups")?,
            next_source: 0,
        };
        Self::next_result(runtime, realm, state)
    }
    fn next_result(
        runtime: &Runtime,
        realm: ContextId,
        mut state: ResultState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        if let Some(result) = state.results.next() {
            return Ok(Self::read(
                realm,
                result.clone(),
                state.length_key.clone(),
                ReplacePhase::Length { state, result },
            ));
        }
        if state.next_source < state.state.input.len() {
            state.state.output.append_range(
                &state.state.input,
                state.next_source,
                state.state.input.len(),
            );
        }
        Ok(RegExpReplaceStep::Complete(
            runtime.complete_regexp_replacement_buffer(realm, state.state.output)?,
        ))
    }
    fn next_capture(
        runtime: &Runtime,
        realm: ContextId,
        state: MatchState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let index = state.captures.len();
        if index < state.capture_count as usize {
            return Ok(Self::read(
                realm,
                state.result.clone(),
                runtime.intern_property_key(&index.to_string())?,
                ReplacePhase::Capture(state),
            ));
        }
        Ok(Self::read(
            realm,
            state.result.clone(),
            state.state.groups_key.clone(),
            ReplacePhase::Groups(state),
        ))
    }
    fn capture(
        runtime: &Runtime,
        realm: ContextId,
        mut state: MatchState,
        value: Value,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        if state.captures.try_reserve(1).is_err() {
            return throw_replace(runtime, realm, NativeErrorKind::Internal, "out of memory");
        }
        state.captures.push(value);
        Self::next_capture(runtime, realm, state)
    }
    fn append_result(
        runtime: &Runtime,
        realm: ContextId,
        mut state: MatchState,
        replacement: JsString,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        if state.position >= state.state.next_source {
            state.state.state.output.append_range(
                &state.state.state.input,
                state.state.next_source,
                state.position,
            );
            state.state.state.output.append_js_string(&replacement);
            state.state.next_source = state.position.saturating_add(state.matched.len());
        }
        Self::next_result(runtime, realm, state.state)
    }
    fn named(
        runtime: &Runtime,
        realm: ContextId,
        mut state: NamedState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let action = runtime.advance_get_substitution(
            &mut state.buffer,
            SubstitutionInput {
                matched: SubstitutionMatch::Converted(&state.matched.matched),
                input: &state.matched.state.state.input,
                position: state.matched.position,
                captures: Some(SubstitutionCaptures::Converted(&state.matched.captures)),
                named_captures: state.groups.as_ref(),
                replacement: state
                    .matched
                    .state
                    .state
                    .replacement
                    .as_ref()
                    .expect("non-functional replacement was not converted"),
            },
            &mut state.cursor,
        )?;
        match action {
            SubstitutionAction::Complete(_) => Self::finish_named(runtime, realm, state),
            SubstitutionAction::Named(key) => Ok(Self::read(
                realm,
                state
                    .groups
                    .as_ref()
                    .expect("named captures disappeared")
                    .clone(),
                key,
                ReplacePhase::Named(state),
            )),
        }
    }
    fn finish_named(
        runtime: &Runtime,
        realm: ContextId,
        state: NamedState,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        match runtime.finish_replacement_buffer(realm, state.buffer)? {
            NativeConversion::Value(value) => {
                Self::append_result(runtime, realm, state.matched, value)
            }
            NativeConversion::Throw(value) => {
                Ok(RegExpReplaceStep::Complete(Completion::Throw(value)))
            }
        }
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let key = runtime.intern_property_key("lastIndex")?;
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            ReplacePhase::InitialSet(state) | ReplacePhase::AdvancedSet(state) => {
                Self::execute(runtime, self.realm, state)
            }
            _ => Err(RuntimeError::Invariant(
                "RegExp replacement received an unexpected set reply",
            )),
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            ReplacePhase::Input(input) => {
                let source = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                let functional = match &input.replacement {
                    Value::Object(object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let state = ReplaceState {
                    regexp: input.regexp,
                    input: source,
                    functional,
                    replacement: None,
                    output: input.output,
                    results: Vec::new(),
                    zero: None,
                    global: false,
                    unicode: false,
                };
                if state.functional.is_none() {
                    Ok(Self::primitive(
                        realm,
                        input.replacement,
                        ToPrimitiveHint::String,
                        ReplacePhase::Replacement(state),
                    ))
                } else {
                    Self::prepared(runtime, realm, state)
                }
            }
            ReplacePhase::Replacement(mut state) => {
                state.replacement = Some(match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                });
                Self::prepared(runtime, realm, state)
            }
            ReplacePhase::Flags(state) => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::String,
                ReplacePhase::FlagsString(state),
            )),
            ReplacePhase::FlagsString(mut state) => {
                let flags = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                state.global = flags.utf16_units().any(|unit| unit == u16::from(b'g'));
                state.unicode = state.global
                    && flags
                        .utf16_units()
                        .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                if state.global {
                    Self::set_index(runtime, realm, state, Value::Int(0), true)
                } else {
                    Self::execute(runtime, realm, state)
                }
            }
            ReplacePhase::Exec(mut state) => {
                let result = match value {
                    Value::Null => return Self::collected(runtime, realm, state),
                    Value::Object(object) => object,
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "RegExpExec returned neither an object nor null",
                        ));
                    }
                };
                if state.results.try_reserve(1).is_err() {
                    return throw_replace(
                        runtime,
                        realm,
                        NativeErrorKind::Internal,
                        "out of memory",
                    );
                }
                state.results.push(result.clone());
                if !state.global {
                    return Self::collected(runtime, realm, state);
                }
                Ok(Self::read(
                    realm,
                    result,
                    state
                        .zero
                        .as_ref()
                        .expect("replace collection omitted zero key")
                        .clone(),
                    ReplacePhase::EmptyMatch(state),
                ))
            }
            ReplacePhase::EmptyMatch(state) => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::String,
                ReplacePhase::EmptyString(state),
            )),
            ReplacePhase::EmptyString(state) => {
                let matched = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                if matched.is_empty() {
                    Ok(Self::read(
                        realm,
                        state.regexp.clone(),
                        runtime.intern_property_key("lastIndex")?,
                        ReplacePhase::LastIndex(state),
                    ))
                } else {
                    Self::execute(runtime, realm, state)
                }
            }
            ReplacePhase::LastIndex(state) => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::Number,
                ReplacePhase::LastIndexNumber(state),
            )),
            ReplacePhase::LastIndexNumber(state) => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp replace lastIndex conversion returned an object",
                    ));
                }
                let current = match runtime.native_to_length(realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                let next = advance_string_index(&state.input, current, state.unicode);
                Self::set_index(runtime, realm, state, Value::number(next as f64), false)
            }
            ReplacePhase::Length { state, result } => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::Number,
                ReplacePhase::LengthNumber { state, result },
            )),
            ReplacePhase::LengthNumber { state, result } => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp replace result length conversion returned an object",
                    ));
                }
                let count = match runtime.native_to_number(realm, &value)? {
                    NativeConversion::Value(value) => Runtime::to_uint32_number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(Self::read(
                    realm,
                    result.clone(),
                    state
                        .state
                        .zero
                        .as_ref()
                        .expect("replace collection omitted zero key")
                        .clone(),
                    ReplacePhase::Matched {
                        state,
                        result,
                        count,
                    },
                ))
            }
            ReplacePhase::Matched {
                state,
                result,
                count,
            } => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::String,
                ReplacePhase::MatchedString {
                    state,
                    result,
                    count,
                },
            )),
            ReplacePhase::MatchedString {
                state,
                result,
                count,
            } => {
                let matched = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                let key = state.index_key.clone();
                let state = MatchState {
                    state,
                    result: result.clone(),
                    capture_count: count,
                    matched,
                    position: 0,
                    captures: Vec::new(),
                };
                Ok(Self::read(
                    realm,
                    result,
                    key,
                    ReplacePhase::Position(state),
                ))
            }
            ReplacePhase::Position(state) => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::Number,
                ReplacePhase::PositionNumber(state),
            )),
            ReplacePhase::PositionNumber(mut state) => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp replace position conversion returned an object",
                    ));
                }
                let position = match runtime.native_to_length(realm, &value)? {
                    NativeConversion::Value(value) => {
                        value.min(state.state.state.input.len() as u64)
                    }
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                state.position = usize::try_from(position).map_err(|_| {
                    RuntimeError::Invariant("RegExp replace position did not fit usize")
                })?;
                let matched = Value::String(state.matched.clone());
                Self::capture(runtime, realm, state, matched)
            }
            ReplacePhase::Capture(state) => {
                if matches!(value, Value::Undefined) {
                    Self::capture(runtime, realm, state, value)
                } else {
                    Ok(Self::primitive(
                        realm,
                        value,
                        ToPrimitiveHint::String,
                        ReplacePhase::CaptureString(state),
                    ))
                }
            }
            ReplacePhase::CaptureString(state) => {
                let capture = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                Self::capture(runtime, realm, state, Value::String(capture))
            }
            ReplacePhase::Groups(mut state) => {
                if let Some(callable) = &state.state.state.functional {
                    let extra = if matches!(value, Value::Undefined) {
                        2
                    } else {
                        3
                    };
                    let position = Value::Int(i32::try_from(state.position).map_err(|_| {
                        RuntimeError::Invariant("RegExp replace position exceeded signed range")
                    })?);
                    if state.captures.try_reserve(extra).is_err() {
                        return throw_replace(
                            runtime,
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        );
                    }
                    state.captures.push(position);
                    state
                        .captures
                        .push(Value::String(state.state.state.input.clone()));
                    if !matches!(value, Value::Undefined) {
                        state.captures.push(value);
                    }
                    if state.captures.len() > MAX_REPLACER_ARGUMENTS {
                        return throw_replace(
                            runtime,
                            realm,
                            NativeErrorKind::Range,
                            "too many arguments in function call (only 65534 allowed)",
                        );
                    }
                    let target = DirectCallTarget::Callable(callable.clone());
                    let arguments = std::mem::take(&mut state.captures);
                    Ok(RegExpReplaceStep::Call {
                        target,
                        receiver: Value::Undefined,
                        arguments,
                        resume: Self {
                            realm,
                            phase: ReplacePhase::Callback(state),
                        },
                    })
                } else {
                    let groups = if matches!(value, Value::Undefined) {
                        None
                    } else {
                        match runtime.native_to_object(realm, value)? {
                            NativeConversion::Value(value) => Some(value),
                            NativeConversion::Throw(value) => {
                                return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                            }
                        }
                    };
                    Self::named(
                        runtime,
                        realm,
                        NamedState {
                            matched: state,
                            groups,
                            buffer: ReplacementStringBuffer::new(0),
                            cursor: 0,
                        },
                    )
                }
            }
            ReplacePhase::Callback(state) => Ok(Self::primitive(
                realm,
                value,
                ToPrimitiveHint::String,
                ReplacePhase::CallbackString(state),
            )),
            ReplacePhase::CallbackString(state) => {
                let text = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                Self::append_result(runtime, realm, state, text)
            }
            ReplacePhase::Named(state) => match named_substitution_capture(&state.buffer, value) {
                NamedSubstitutionCapture::Skip => Self::named(runtime, realm, state),
                NamedSubstitutionCapture::Failed => Self::finish_named(runtime, realm, state),
                NamedSubstitutionCapture::Convert(value) => Ok(Self::primitive(
                    realm,
                    value,
                    ToPrimitiveHint::String,
                    ReplacePhase::NamedString(state),
                )),
            },
            ReplacePhase::NamedString(mut state) => {
                let text = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                state.buffer.append_js_string(&text);
                if state.buffer.error().is_some() {
                    Self::finish_named(runtime, realm, state)
                } else {
                    Self::named(runtime, realm, state)
                }
            }
            ReplacePhase::InitialSet(_) | ReplacePhase::AdvancedSet(_) => Err(
                RuntimeError::Invariant("RegExp replace set received an untyped reply"),
            ),
        }
    }
}
fn finish_replace(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpReplaceStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpReplaceStep::Complete(result) => return Ok(result),
            RegExpReplaceStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            RegExpReplaceStep::Primitive {
                value,
                hint,
                resume,
            } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, hint)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            RegExpReplaceStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "RegExp replacement requested an invalid call target",
                    ));
                };
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
            RegExpReplaceStep::Exec {
                regexp,
                input,
                resume,
            } => resume.resume(runtime, runtime.regexp_exec_abstract(realm, regexp, input)?)?,
            RegExpReplaceStep::Set {
                object,
                key,
                value,
                resume,
            } => resume.set(
                runtime,
                runtime.internal_set(realm, &object, &key, value, Value::Object(object.clone()))?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collected_replace_results_and_callback_survive_gc_until_abandonment() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let regexp = runtime.new_object(None).unwrap();
        let regexp_id = regexp.object_id();
        let callback = context.eval("(function(){return 'x'})").unwrap();
        let Value::Object(function) = &callback else {
            panic!("expected callback")
        };
        let callback_id = function.object_id();
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(regexp),
        };
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![Value::String(JsString::from_static("a")), callback],
        };
        let RegExpReplaceStep::Primitive { resume, .. } =
            RegExpReplaceStep::start(&runtime, context.realm, &invocation, &arguments).unwrap()
        else {
            panic!("expected input conversion")
        };
        drop(invocation);
        drop(arguments);
        let RegExpReplaceStep::Read { resume, .. } = resume
            .resume(
                &runtime,
                Completion::Return(Value::String(JsString::from_static("a"))),
            )
            .unwrap()
        else {
            panic!("expected flags read")
        };
        let RegExpReplaceStep::Primitive { resume, .. } = resume
            .resume(
                &runtime,
                Completion::Return(Value::String(JsString::from_static(""))),
            )
            .unwrap()
        else {
            panic!("expected flags conversion")
        };
        let RegExpReplaceStep::Exec { resume, .. } = resume
            .resume(
                &runtime,
                Completion::Return(Value::String(JsString::from_static(""))),
            )
            .unwrap()
        else {
            panic!("expected exec request")
        };
        let result = runtime.new_object(None).unwrap();
        let result_id = result.object_id();
        let RegExpReplaceStep::Read { resume, .. } = resume
            .resume(&runtime, Completion::Return(Value::Object(result)))
            .unwrap()
        else {
            panic!("expected result length request")
        };
        runtime.run_gc().unwrap();
        for id in [regexp_id, callback_id, result_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
        runtime.run_gc().unwrap();
        for id in [regexp_id, callback_id, result_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
