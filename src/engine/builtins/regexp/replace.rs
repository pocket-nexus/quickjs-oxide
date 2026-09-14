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
    #[cfg(feature = "stack-vm")]
    PreparedSet {
        step: Box<crate::engine::object::SetStep>,
        resume: RegExpReplaceResume,
    },
    PreparedRead {
        read: crate::engine::object::OrdinaryRead,
        key: PropertyKey,
        resume: RegExpReplaceResume,
    },
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
    state: ReplaceState,
    result: Option<ResultCursor>,
    matched: Option<MatchCursor>,
    named: Option<NamedCursor>,
}
struct ReplaceState {
    regexp: ObjectRef,
    replacement_value: Option<Value>,
    input: Option<JsString>,
    functional: Option<CallableRef>,
    replacement: Option<JsString>,
    output: Option<ReplacementStringBuffer>,
    results: Vec<ObjectRef>,
    zero: Option<PropertyKey>,
    global: bool,
    unicode: bool,
}
struct ResultCursor {
    results: std::vec::IntoIter<ObjectRef>,
    length_key: PropertyKey,
    index_key: PropertyKey,
    groups_key: PropertyKey,
    next_source: usize,
}
struct MatchCursor {
    result: ObjectRef,
    capture_count: u32,
    matched: Option<JsString>,
    position: usize,
    captures: Vec<Value>,
}
struct NamedCursor {
    groups: Option<ObjectRef>,
    buffer: ReplacementStringBuffer,
    cursor: usize,
}
#[derive(Clone, Copy)]
enum ReplacePhase {
    Input,
    Replacement,
    Flags,
    FlagsString,
    InitialSet,
    Exec,
    EmptyMatch,
    EmptyString,
    LastIndex,
    LastIndexNumber,
    AdvancedSet,
    Length,
    LengthNumber,
    Matched,
    MatchedString,
    Position,
    PositionNumber,
    Capture,
    CaptureString,
    Groups,
    Callback,
    CallbackString,
    Named,
    NamedString,
}
enum ReadTarget {
    RegExp,
    CollectedLast,
    Match,
    Named,
}
// No accumulated result vector, output buffer or native owner travels with a
// local phase. It stays in one resume until a selected effect really waits.
enum ReplaceAction {
    Complete(Completion),
    Read {
        target: ReadTarget,
        key: PropertyKey,
    },
    Primitive {
        value: Value,
        hint: ToPrimitiveHint,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
    },
    Exec,
    Set {
        key: PropertyKey,
        value: Value,
    },
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
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
            )));
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
        RegExpReplaceResume {
            realm,
            phase: ReplacePhase::Input,
            state: ReplaceState {
                regexp: regexp.clone(),
                replacement_value: Some(replacement),
                input: None,
                functional: None,
                replacement: None,
                output: Some(ReplacementStringBuffer::new(0)),
                results: Vec::new(),
                zero: None,
                global: false,
                unicode: false,
            },
            result: None,
            matched: None,
            named: None,
        }
        .deliver(
            runtime,
            ReplaceAction::Primitive {
                value: input,
                hint: ToPrimitiveHint::String,
            },
        )
    }
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
    fn source(&self) -> &JsString {
        self.state
            .input
            .as_ref()
            .expect("replacement input was not converted")
    }
    fn result_cursor(&self) -> &ResultCursor {
        self.result
            .as_ref()
            .expect("replacement result cursor disappeared")
    }
    fn match_cursor(&self) -> &MatchCursor {
        self.matched
            .as_ref()
            .expect("replacement match cursor disappeared")
    }
    fn throw(
        &self,
        runtime: &Runtime,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<ReplaceAction, RuntimeError> {
        Ok(ReplaceAction::Complete(Completion::Throw(
            runtime.new_native_error(self.realm, kind, message)?,
        )))
    }
    fn primitive(
        &mut self,
        value: Value,
        hint: ToPrimitiveHint,
        phase: ReplacePhase,
    ) -> ReplaceAction {
        self.phase = phase;
        ReplaceAction::Primitive { value, hint }
    }
    fn read(&mut self, target: ReadTarget, key: PropertyKey, phase: ReplacePhase) -> ReplaceAction {
        self.phase = phase;
        ReplaceAction::Read { target, key }
    }
    fn read_object(&self, target: ReadTarget) -> &ObjectRef {
        match target {
            ReadTarget::RegExp => &self.state.regexp,
            ReadTarget::CollectedLast => self
                .state
                .results
                .last()
                .expect("replacement collection lost last result"),
            ReadTarget::Match => &self.match_cursor().result,
            ReadTarget::Named => self
                .named
                .as_ref()
                .and_then(|state| state.groups.as_ref())
                .expect("named captures disappeared"),
        }
    }
    fn deliver(
        mut self,
        runtime: &Runtime,
        mut action: ReplaceAction,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        loop {
            action = match action {
                ReplaceAction::Complete(result) => return Ok(RegExpReplaceStep::Complete(result)),
                ReplaceAction::Primitive { value, .. } if !matches!(value, Value::Object(_)) => {
                    #[cfg(all(feature = "stack-vm", feature = "profiling"))]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "regexpreplace_primitive_local",
                    );
                    self.advance(runtime, Completion::Return(value))?
                }
                ReplaceAction::Read { target, key } => {
                    let object = self.read_object(target);
                    let receiver = Value::Object(object.clone());
                    match runtime.prepare_ordinary_read_borrowed(object, &key, &receiver)? {
                        crate::engine::object::OrdinaryRead::Complete(value) => {
                            #[cfg(all(feature = "stack-vm", feature = "profiling"))]
                            crate::engine::api::profiling::record_owned_execution_event(
                                "regexpreplace_read_local",
                            );
                            self.advance(
                                runtime,
                                Completion::Return(value.unwrap_or(Value::Undefined)),
                            )?
                        }
                        read => {
                            return Ok(RegExpReplaceStep::PreparedRead {
                                read,
                                key,
                                resume: self,
                            });
                        }
                    }
                }
                #[cfg(feature = "stack-vm")]
                ReplaceAction::Set { key, value } => {
                    use crate::engine::object::{SetStep, operations::PropertySetAction};
                    let mut pending = None;
                    let selected = SetStep::start_receiver_into(
                        runtime,
                        self.realm,
                        &key,
                        value,
                        Value::Object(self.state.regexp.clone()),
                        |step| pending = Some(step),
                    )?;
                    let step = match selected {
                        Some(action) => SetStep::Complete(action),
                        None => pending
                            .ok_or(RuntimeError::Invariant(
                                "replacement Set lost selected effect",
                            ))?
                            .advance_without_callback(runtime)?,
                    };
                    match step {
                        SetStep::Complete(action)
                            if !matches!(action, PropertySetAction::Call { .. }) =>
                        {
                            let result = local_set_result(action)?;
                            self.set_once(runtime, result)?
                        }
                        step => {
                            return Ok(RegExpReplaceStep::PreparedSet {
                                step: Box::new(step),
                                resume: self,
                            });
                        }
                    }
                }
                ReplaceAction::Primitive { value, hint } => {
                    return Ok(RegExpReplaceStep::Primitive {
                        value,
                        hint,
                        resume: self,
                    });
                }
                ReplaceAction::Call {
                    target,
                    receiver,
                    arguments,
                } => {
                    return Ok(RegExpReplaceStep::Call {
                        target,
                        receiver,
                        arguments,
                        resume: self,
                    });
                }
                ReplaceAction::Exec => {
                    return Ok(RegExpReplaceStep::Exec {
                        regexp: Value::Object(self.state.regexp.clone()),
                        input: Value::String(self.source().clone()),
                        resume: self,
                    });
                }
                #[cfg(not(feature = "stack-vm"))]
                ReplaceAction::Set { key, value } => {
                    return Ok(RegExpReplaceStep::Set {
                        object: self.state.regexp.clone(),
                        key,
                        value,
                        resume: self,
                    });
                }
            };
        }
    }
    fn set_index(
        &mut self,
        runtime: &Runtime,
        value: Value,
        initial: bool,
    ) -> Result<ReplaceAction, RuntimeError> {
        let key = runtime.intern_property_key("lastIndex")?;
        self.phase = if initial {
            ReplacePhase::InitialSet
        } else {
            ReplacePhase::AdvancedSet
        };
        Ok(ReplaceAction::Set { key, value })
    }
    fn prepared(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        if self.state.functional.is_none()
            && let Some(standard) = runtime.standard_regexp_replace(&self.state.regexp)?
        {
            // Preserve the existing raw predicate and matcher unchanged.
            return Ok(ReplaceAction::Complete(
                runtime.call_standard_regexp_replace(
                    self.realm,
                    &self.state.regexp,
                    self.source(),
                    self.state
                        .replacement
                        .as_ref()
                        .expect("non-functional replacement was not converted"),
                    standard,
                )?,
            ));
        }
        Ok(self.read(
            ReadTarget::RegExp,
            runtime.intern_property_key("flags")?,
            ReplacePhase::Flags,
        ))
    }
    fn execute(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        if self.state.zero.is_none() {
            self.state.zero = Some(runtime.intern_property_key("0")?);
        }
        self.phase = ReplacePhase::Exec;
        Ok(ReplaceAction::Exec)
    }
    fn collected(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        let results = std::mem::take(&mut self.state.results).into_iter();
        self.result = Some(ResultCursor {
            results,
            length_key: runtime.intern_property_key("length")?,
            index_key: runtime.intern_property_key("index")?,
            groups_key: runtime.intern_property_key("groups")?,
            next_source: 0,
        });
        self.next_result(runtime)
    }
    fn next_result(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        let state = self
            .result
            .as_mut()
            .expect("replacement result cursor disappeared");
        if let Some(result) = state.results.next() {
            self.matched = Some(MatchCursor {
                result,
                capture_count: 0,
                matched: None,
                position: 0,
                captures: Vec::new(),
            });
            let key = state.length_key.clone();
            return Ok(self.read(ReadTarget::Match, key, ReplacePhase::Length));
        }
        let next_source = state.next_source;
        let input = self
            .state
            .input
            .as_ref()
            .expect("replacement input was not converted");
        let mut output = self
            .state
            .output
            .take()
            .expect("replacement output disappeared");
        if next_source < input.len() {
            output.append_range(input, next_source, input.len());
        }
        Ok(ReplaceAction::Complete(
            runtime.complete_regexp_replacement_buffer(self.realm, output)?,
        ))
    }
    fn next_capture(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        let state = self.match_cursor();
        let index = state.captures.len();
        if index < state.capture_count as usize {
            return Ok(self.read(
                ReadTarget::Match,
                runtime.intern_property_key(&index.to_string())?,
                ReplacePhase::Capture,
            ));
        }
        Ok(self.read(
            ReadTarget::Match,
            self.result_cursor().groups_key.clone(),
            ReplacePhase::Groups,
        ))
    }
    fn capture(&mut self, runtime: &Runtime, value: Value) -> Result<ReplaceAction, RuntimeError> {
        let state = self
            .matched
            .as_mut()
            .expect("replacement match cursor disappeared");
        if state.captures.try_reserve(1).is_err() {
            return self.throw(runtime, NativeErrorKind::Internal, "out of memory");
        }
        state.captures.push(value);
        self.next_capture(runtime)
    }
    fn append_result(
        &mut self,
        runtime: &Runtime,
        replacement: JsString,
    ) -> Result<ReplaceAction, RuntimeError> {
        // Release each finished match at the same per-result boundary; do not
        // retain all captures or groups until the overall replacement ends.
        let matched = self
            .matched
            .take()
            .expect("replacement match cursor disappeared");
        let result = self
            .result
            .as_mut()
            .expect("replacement result cursor disappeared");
        if matched.position >= result.next_source {
            let input = self
                .state
                .input
                .as_ref()
                .expect("replacement input was not converted");
            let output = self
                .state
                .output
                .as_mut()
                .expect("replacement output disappeared");
            output.append_range(input, result.next_source, matched.position);
            output.append_js_string(&replacement);
            result.next_source = matched.position.saturating_add(
                matched
                    .matched
                    .as_ref()
                    .expect("replacement match was not converted")
                    .len(),
            );
        }
        // The old consuming helper selected the next action (including final
        // output allocation) before dropping this completed match's owners.
        let action = self.next_result(runtime);
        drop(matched);
        action
    }
    fn named(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        let matched = self
            .matched
            .as_ref()
            .expect("replacement match cursor disappeared");
        let named = self
            .named
            .as_mut()
            .expect("replacement named cursor disappeared");
        let action = runtime.advance_get_substitution(
            &mut named.buffer,
            SubstitutionInput {
                matched: SubstitutionMatch::Converted(
                    matched
                        .matched
                        .as_ref()
                        .expect("replacement match was not converted"),
                ),
                input: self
                    .state
                    .input
                    .as_ref()
                    .expect("replacement input was not converted"),
                position: matched.position,
                captures: Some(SubstitutionCaptures::Converted(&matched.captures)),
                named_captures: named.groups.as_ref(),
                replacement: self
                    .state
                    .replacement
                    .as_ref()
                    .expect("non-functional replacement was not converted"),
            },
            &mut named.cursor,
        )?;
        match action {
            SubstitutionAction::Complete(_) => self.finish_named(runtime),
            SubstitutionAction::Named(key) => {
                Ok(self.read(ReadTarget::Named, key, ReplacePhase::Named))
            }
        }
    }
    fn finish_named(&mut self, runtime: &Runtime) -> Result<ReplaceAction, RuntimeError> {
        let state = self
            .named
            .take()
            .expect("replacement named cursor disappeared");
        match runtime.finish_replacement_buffer(self.realm, state.buffer)? {
            NativeConversion::Value(value) => self.append_result(runtime, value),
            NativeConversion::Throw(value) => Ok(ReplaceAction::Complete(Completion::Throw(value))),
        }
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let action = self.set_once(runtime, result)?;
        self.deliver(runtime, action)
    }
    fn set_once(
        &mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<ReplaceAction, RuntimeError> {
        let key = runtime.intern_property_key("lastIndex")?;
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(ReplaceAction::Complete(Completion::Throw(value)));
        }
        match self.phase {
            ReplacePhase::InitialSet | ReplacePhase::AdvancedSet => self.execute(runtime),
            _ => Err(RuntimeError::Invariant(
                "RegExp replacement received an unexpected set reply",
            )),
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpReplaceStep, RuntimeError> {
        let action = self.advance(runtime, result)?;
        self.deliver(runtime, action)
    }
    fn advance(
        &mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ReplaceAction, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ReplaceAction::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            ReplacePhase::Input => {
                let source = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let replacement = self
                    .state
                    .replacement_value
                    .take()
                    .expect("replacement argument disappeared");
                self.state.functional = match &replacement {
                    Value::Object(object) => runtime.as_callable(object)?,
                    _ => None,
                };
                self.state.input = Some(source);
                if self.state.functional.is_none() {
                    Ok(self.primitive(
                        replacement,
                        ToPrimitiveHint::String,
                        ReplacePhase::Replacement,
                    ))
                } else {
                    let action = self.prepared(runtime);
                    drop(replacement);
                    action
                }
            }
            ReplacePhase::Replacement => {
                self.state.replacement = Some(match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                });
                self.prepared(runtime)
            }
            ReplacePhase::Flags => {
                Ok(self.primitive(value, ToPrimitiveHint::String, ReplacePhase::FlagsString))
            }
            ReplacePhase::FlagsString => {
                let flags = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                self.state.global = flags.utf16_units().any(|unit| unit == u16::from(b'g'));
                self.state.unicode = self.state.global
                    && flags
                        .utf16_units()
                        .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                if self.state.global {
                    self.set_index(runtime, Value::Int(0), true)
                } else {
                    self.execute(runtime)
                }
            }
            ReplacePhase::Exec => {
                let result = match value {
                    Value::Null => return self.collected(runtime),
                    Value::Object(object) => object,
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "RegExpExec returned neither an object nor null",
                        ));
                    }
                };
                if self.state.results.try_reserve(1).is_err() {
                    return self.throw(runtime, NativeErrorKind::Internal, "out of memory");
                }
                self.state.results.push(result);
                if !self.state.global {
                    return self.collected(runtime);
                }
                Ok(self.read(
                    ReadTarget::CollectedLast,
                    self.state
                        .zero
                        .as_ref()
                        .expect("replace collection omitted zero key")
                        .clone(),
                    ReplacePhase::EmptyMatch,
                ))
            }
            ReplacePhase::EmptyMatch => {
                Ok(self.primitive(value, ToPrimitiveHint::String, ReplacePhase::EmptyString))
            }
            ReplacePhase::EmptyString => {
                let matched = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                if matched.is_empty() {
                    Ok(self.read(
                        ReadTarget::RegExp,
                        runtime.intern_property_key("lastIndex")?,
                        ReplacePhase::LastIndex,
                    ))
                } else {
                    self.execute(runtime)
                }
            }
            ReplacePhase::LastIndex => Ok(self.primitive(
                value,
                ToPrimitiveHint::Number,
                ReplacePhase::LastIndexNumber,
            )),
            ReplacePhase::LastIndexNumber => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp replace lastIndex conversion returned an object",
                    ));
                }
                let current = match runtime.native_to_length(realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let next = advance_string_index(self.source(), current, self.state.unicode);
                self.set_index(runtime, Value::number(next as f64), false)
            }
            ReplacePhase::Length => {
                Ok(self.primitive(value, ToPrimitiveHint::Number, ReplacePhase::LengthNumber))
            }
            ReplacePhase::LengthNumber => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp replace result length conversion returned an object",
                    ));
                }
                let count = match runtime.native_to_number(realm, &value)? {
                    NativeConversion::Value(value) => Runtime::to_uint32_number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                self.matched
                    .as_mut()
                    .expect("replacement match cursor disappeared")
                    .capture_count = count;
                Ok(self.read(
                    ReadTarget::Match,
                    self.state
                        .zero
                        .as_ref()
                        .expect("replace collection omitted zero key")
                        .clone(),
                    ReplacePhase::Matched,
                ))
            }
            ReplacePhase::Matched => {
                Ok(self.primitive(value, ToPrimitiveHint::String, ReplacePhase::MatchedString))
            }
            ReplacePhase::MatchedString => {
                let matched = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                self.matched
                    .as_mut()
                    .expect("replacement match cursor disappeared")
                    .matched = Some(matched);
                Ok(self.read(
                    ReadTarget::Match,
                    self.result_cursor().index_key.clone(),
                    ReplacePhase::Position,
                ))
            }
            ReplacePhase::Position => {
                Ok(self.primitive(value, ToPrimitiveHint::Number, ReplacePhase::PositionNumber))
            }
            ReplacePhase::PositionNumber => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp replace position conversion returned an object",
                    ));
                }
                let position = match runtime.native_to_length(realm, &value)? {
                    NativeConversion::Value(value) => value.min(self.source().len() as u64),
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let state = self
                    .matched
                    .as_mut()
                    .expect("replacement match cursor disappeared");
                state.position = usize::try_from(position).map_err(|_| {
                    RuntimeError::Invariant("RegExp replace position did not fit usize")
                })?;
                let matched = Value::String(
                    state
                        .matched
                        .as_ref()
                        .expect("replacement match was not converted")
                        .clone(),
                );
                self.capture(runtime, matched)
            }
            ReplacePhase::Capture => {
                if matches!(value, Value::Undefined) {
                    self.capture(runtime, value)
                } else {
                    Ok(self.primitive(value, ToPrimitiveHint::String, ReplacePhase::CaptureString))
                }
            }
            ReplacePhase::CaptureString => {
                let capture = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                self.capture(runtime, Value::String(capture))
            }
            ReplacePhase::Groups => {
                if let Some(callable) = &self.state.functional {
                    let extra = if matches!(value, Value::Undefined) {
                        2
                    } else {
                        3
                    };
                    let state = self
                        .matched
                        .as_mut()
                        .expect("replacement match cursor disappeared");
                    let position = Value::Int(i32::try_from(state.position).map_err(|_| {
                        RuntimeError::Invariant("RegExp replace position exceeded signed range")
                    })?);
                    if state.captures.try_reserve(extra).is_err() {
                        return self.throw(runtime, NativeErrorKind::Internal, "out of memory");
                    }
                    state.captures.push(position);
                    state.captures.push(Value::String(
                        self.state
                            .input
                            .as_ref()
                            .expect("replacement input was not converted")
                            .clone(),
                    ));
                    if !matches!(value, Value::Undefined) {
                        state.captures.push(value);
                    }
                    if state.captures.len() > MAX_REPLACER_ARGUMENTS {
                        return self.throw(
                            runtime,
                            NativeErrorKind::Range,
                            "too many arguments in function call (only 65534 allowed)",
                        );
                    }
                    let target = DirectCallTarget::Callable(callable.clone());
                    let arguments = std::mem::take(&mut state.captures);
                    self.phase = ReplacePhase::Callback;
                    Ok(ReplaceAction::Call {
                        target,
                        receiver: Value::Undefined,
                        arguments,
                    })
                } else {
                    let groups = if matches!(value, Value::Undefined) {
                        None
                    } else {
                        match runtime.native_to_object(realm, value)? {
                            NativeConversion::Value(value) => Some(value),
                            NativeConversion::Throw(value) => {
                                return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                            }
                        }
                    };
                    self.named = Some(NamedCursor {
                        groups,
                        buffer: ReplacementStringBuffer::new(0),
                        cursor: 0,
                    });
                    self.named(runtime)
                }
            }
            ReplacePhase::Callback => {
                Ok(self.primitive(value, ToPrimitiveHint::String, ReplacePhase::CallbackString))
            }
            ReplacePhase::CallbackString => {
                let text = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                self.append_result(runtime, text)
            }
            ReplacePhase::Named => match named_substitution_capture(
                &self
                    .named
                    .as_ref()
                    .expect("replacement named cursor disappeared")
                    .buffer,
                value,
            ) {
                NamedSubstitutionCapture::Skip => self.named(runtime),
                NamedSubstitutionCapture::Failed => self.finish_named(runtime),
                NamedSubstitutionCapture::Convert(value) => {
                    Ok(self.primitive(value, ToPrimitiveHint::String, ReplacePhase::NamedString))
                }
            },
            ReplacePhase::NamedString => {
                let text = match converted_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(ReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let named = self
                    .named
                    .as_mut()
                    .expect("replacement named cursor disappeared");
                named.buffer.append_js_string(&text);
                if named.buffer.error().is_some() {
                    self.finish_named(runtime)
                } else {
                    self.named(runtime)
                }
            }
            ReplacePhase::InitialSet | ReplacePhase::AdvancedSet => Err(RuntimeError::Invariant(
                "RegExp replace set received an untyped reply",
            )),
        }
    }
}
#[cfg(feature = "stack-vm")]
fn local_set_result(
    action: crate::engine::object::operations::PropertySetAction,
) -> Result<NativeConversion<InternalSetResult>, RuntimeError> {
    use crate::engine::object::operations::PropertySetAction;
    Ok(match action {
        PropertySetAction::Complete => NativeConversion::Value(InternalSetResult::Accepted),
        PropertySetAction::Rejected(reason) => {
            NativeConversion::Value(InternalSetResult::Rejected(reason))
        }
        PropertySetAction::RejectedProxyTrap => {
            NativeConversion::Value(InternalSetResult::RejectedProxyTrap)
        }
        PropertySetAction::Throw(value) => NativeConversion::Throw(value),
        PropertySetAction::Call { .. } => {
            return Err(RuntimeError::Invariant(
                "replacement setter has not completed",
            ));
        }
    })
}
fn finish_replace(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpReplaceStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpReplaceStep::Complete(result) => return Ok(result),
            #[cfg(feature = "stack-vm")]
            RegExpReplaceStep::PreparedSet { step, resume } => {
                use crate::engine::object::{SetStep, operations::PropertySetAction};
                let mut step = *step;
                let result = loop {
                    match step {
                        SetStep::Complete(PropertySetAction::Call {
                            setter,
                            receiver,
                            argument,
                        }) => {
                            break match runtime.call_internal(
                                realm,
                                &setter,
                                receiver,
                                &[argument],
                            )? {
                                Completion::Return(_) => {
                                    NativeConversion::Value(InternalSetResult::Accepted)
                                }
                                Completion::Throw(value) => NativeConversion::Throw(value),
                            };
                        }
                        SetStep::Complete(action) => break local_set_result(action)?,
                        pending => step = pending.finish_sync(runtime)?,
                    }
                };
                resume.set(runtime, result)?
            }
            RegExpReplaceStep::PreparedRead { read, key, resume } => {
                let result = match runtime.finish_prepared_read(realm, &key, read)? {
                    NativeConversion::Value(value) => {
                        Completion::Return(value.unwrap_or(Value::Undefined))
                    }
                    NativeConversion::Throw(value) => Completion::Throw(value),
                };
                resume.resume(runtime, result)?
            }
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
    fn fresh_lazy_exec_preserves_the_selected_flags_getter() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(regexp) = context
            .eval("globalThis.coldReplace = /a/g; coldReplace")
            .unwrap()
        else {
            panic!("RegExp object");
        };
        // The original direct-matcher guard intentionally rejects lazy exec.
        assert!(runtime.standard_regexp_replace(&regexp).unwrap().is_none());
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(regexp),
        };
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![
                Value::String(JsString::from_static("aa")),
                Value::String(JsString::from_static("b")),
            ],
        };
        let step =
            RegExpReplaceStep::start(&runtime, context.realm, &invocation, &arguments).unwrap();
        assert!(matches!(
            &step,
            RegExpReplaceStep::PreparedRead {
                read: crate::engine::object::OrdinaryRead::Call { .. },
                resume: RegExpReplaceResume {
                    phase: ReplacePhase::Flags,
                    ..
                },
                ..
            }
        ));
        // Consuming the already-selected intrinsic must not probe flags again.
        context.eval("Object.defineProperty(coldReplace, 'flags', { get() { throw 'repeated flags'; } });").unwrap();
        assert_eq!(
            finish_replace(&runtime, context.realm, step).unwrap(),
            Completion::Return(Value::String(JsString::from_static("bb")))
        );
        assert_eq!(
            context.eval("coldReplace.lastIndex").unwrap(),
            Value::Int(0)
        );
    }

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
        // Primitive input/flags now complete locally. Pause at actual exec,
        // then at a selected result getter so abandonment still owns the
        // collected result, callback and original RegExp receiver together.
        let RegExpReplaceStep::Exec { resume, .. } =
            RegExpReplaceStep::start(&runtime, context.realm, &invocation, &arguments).unwrap()
        else {
            panic!("expected exec request")
        };
        drop(invocation);
        drop(arguments);
        let Value::Object(result) = context.eval("({get length(){return 1;}})").unwrap() else {
            panic!("result object")
        };
        let result_id = result.object_id();
        let RegExpReplaceStep::PreparedRead { resume, .. } = resume
            .resume(&runtime, Completion::Return(Value::Object(result)))
            .unwrap()
        else {
            panic!("expected selected result length getter")
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
