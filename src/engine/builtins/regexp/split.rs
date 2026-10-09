//! `RegExp.prototype[Symbol.split]`.

use super::match_protocol::advance_string_index;
use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::{
    ContextId, ObjectId, ObjectPayload, PropertySlot, RawValue, RegExpObjectData,
};
use crate::engine::object::{ObjectRef, PropertyKey, WellKnownSymbol, operations::InternalSetResult};
use crate::engine::value::conversion::NativeConversion;

use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::call::{ConstructorRef, NativeArguments, NativeInvocation};
use crate::engine::vm::{Completion, ToPrimitiveHint};

use crate::engine::builtins::native::{NativeFunctionId, RegExpNativeKind};
use crate::regexp::{
    CompiledRegExp, ExecError, RegExpFlags, execute_latin1_with_interrupt, execute_with_interrupt,
};
use std::rc::Rc;

impl Runtime {
    /// Rust port of pinned QuickJS `js_regexp_Symbol_split`.
    pub(crate) fn call_regexp_symbol_split(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            finish(
                self,
                realm,
                RegExpSplitStep::start(self, realm, invocation, arguments)?,
            )
        })
    }

    fn append_regexp_split_value(
        &self,
        result: &ObjectRef,
        length: &mut u32,
        value: JsValue,
    ) -> Result<(), RuntimeError> {
        let index = *length;
        let Some(next) = index.checked_add(1) else {
            self.release_jsvalue(value)?;
            return Err(RuntimeError::Invariant(
                "RegExp split output index exceeded Uint32",
            ));
        };
        // The output is an intrinsic fresh Array, never the species-created
        // splitter and never exposed to exec/capture callbacks. Preserve the
        // original allocation and append timing using the shared constructor
        // kernel, which defines own C/W/E data without inherited setters.
        self.append_fresh_array_value_jsvalue(result, value)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_result.split_append");
        *length = next;
        Ok(())
    }

    /// Raw standard-RegExp predicate for `@@split`, in the shape of the
    /// `@@replace` predicate: a genuine compiled receiver, a primitive String
    /// input (ToString observes nothing), an immediate limit (ToUint32 of a
    /// primitive observes nothing), the standard `flags` getter, and the
    /// default species (`constructor` missing or the realm `%RegExp%` whose
    /// `@@species` getter is the builtin). `None` keeps the general steps.
    fn standard_regexp_split(
        &self,
        realm: ContextId,
        regexp: ObjectId,
        input: &JsValue,
        limit: &JsValue,
    ) -> Result<Option<StandardRegExpSplit>, RuntimeError> {
        use crate::engine::atom::pinned::PinnedAtom;
        let JsValue::String(input_id) = input else {
            return Ok(None);
        };
        let limit = match limit {
            JsValue::Undefined => u32::MAX,
            JsValue::Int(value) => (*value as i64).rem_euclid(1 << 32) as u32,
            _ => return Ok(None),
        };
        let state = self.0.state.borrow();
        let object = state.heap.object(regexp)?;
        let ObjectPayload::RegExp(RegExpObjectData::Compiled { pattern, program }) = &object.payload
        else {
            return Ok(None);
        };
        let pinned = |atom| state.pinned_atoms.get(atom);
        if !super::replace::raw_regexp_getter_matches(
            &state.heap,
            regexp,
            pinned(PinnedAtom::Flags),
            NativeFunctionId::RegExp(RegExpNativeKind::Flags),
        )? {
            return Ok(None);
        }
        let constructor = state
            .heap
            .context(realm)?
            .regexp
            .as_ref()
            .ok_or(RuntimeError::Invariant("realm has no RegExp intrinsic"))?
            .constructor;
        match super::replace::raw_regexp_property_slot(
            &state.heap,
            regexp,
            pinned(PinnedAtom::Constructor),
        )? {
            // A missing `constructor` selects the default `%RegExp%`.
            None => {}
            Some(PropertySlot::Data(RawValue::Object(id))) if *id == constructor => {}
            Some(_) => return Ok(None),
        }
        let species = state.well_known_symbols[&WellKnownSymbol::Species];
        if !super::replace::raw_regexp_getter_matches(
            &state.heap,
            constructor,
            species,
            NativeFunctionId::RegExp(RegExpNativeKind::Species),
        )? {
            return Ok(None);
        }
        Ok(Some(StandardRegExpSplit {
            input: state.heap.string(*input_id)?.clone(),
            pattern: pattern.clone(),
            flags: program.flags(),
            limit,
        }))
    }

    /// Rust port of the pinned QuickJS `js_regexp_Symbol_split` matching
    /// loop for the standard predicate. The virtual splitter is the
    /// receiver's program recompiled with `y`, exactly what the constructor
    /// would compile for `new C(rx, flags + "y")`; it never escapes, so its
    /// `lastIndex` traffic stays inside this loop. Each piece string is
    /// allocated as an owner that moves into a dense element under one State
    /// access per append.
    #[inline(never)]
    fn call_standard_regexp_split(
        &self,
        realm: ContextId,
        standard: StandardRegExpSplit,
    ) -> Result<Completion, RuntimeError> {
        let mut flags = standard.flags.canonical_string();
        if !standard.flags.contains(RegExpFlags::STICKY) {
            flags.push('y');
        }
        let program = Runtime::compile_regexp_program(
            &standard.pattern,
            &JsString::from_owned_latin1(flags.into_bytes()),
        )?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_runtime_event(
            "regexp_split.standard",
            "core.regexp_split.standard",
        );
        let input = standard.input;
        let flat = input.linearize();
        let size = flat.len();
        let unicode = standard.flags.is_unicode();
        let limit = standard.limit;
        let array = self.0.state.borrow_mut().new_array(&self.0.poisoned, realm)?;
        let mut length = 0_u32;
        match self.standard_regexp_split_loop(
            &program,
            &input,
            &flat,
            size,
            unicode,
            limit,
            array,
            &mut length,
        ) {
            Ok(StandardSplitOutcome::Complete) => Ok(Completion::Return(JsValue::Object(array))),
            Ok(StandardSplitOutcome::Throw(message)) => {
                let _ = self.release_jsvalue(JsValue::Object(array));
                Ok(Completion::Throw(
                    self.new_native_error_jsvalue(realm, NativeErrorKind::Internal, message)?,
                ))
            }
            Err(error) => {
                let _ = self.release_jsvalue(JsValue::Object(array));
                Err(error)
            }
        }
    }

    /// The split loop itself, so `call_standard_regexp_split` stays a thin
    /// shell around allocation and error cleanup. A `Throw` outcome leaves
    /// `array` released by the caller.
    fn standard_regexp_split_loop(
        &self,
        program: &Rc<CompiledRegExp>,
        input: &JsString,
        flat: &JsString,
        size: usize,
        unicode: bool,
        limit: u32,
        array: ObjectId,
        length: &mut u32,
    ) -> Result<StandardSplitOutcome, RuntimeError> {
        let mut p = 0_usize;
        let mut q = 0_usize;
        if limit == 0 {
            return Ok(StandardSplitOutcome::Complete);
        }
        if size == 0 {
            return match self.standard_regexp_exec_at(program, flat, 0)? {
                // No match: append the whole (empty) input, as `add_tail`.
                StandardSplitExec::Matched(None) => {
                    self.push_standard_split_piece(array, input, Some((0, 0)), length)?;
                    Ok(StandardSplitOutcome::Complete)
                }
                StandardSplitExec::Matched(Some(_)) => Ok(StandardSplitOutcome::Complete),
                StandardSplitExec::Throw(message) => Ok(StandardSplitOutcome::Throw(message)),
            };
        }
        while q < size {
            let matched = match self.standard_regexp_exec_at(program, flat, q)? {
                StandardSplitExec::Matched(matched) => matched,
                StandardSplitExec::Throw(message) => {
                    return Ok(StandardSplitOutcome::Throw(message));
                }
            };
            let Some(matched) = matched else {
                q = usize::try_from(advance_string_index(input, q as u64, unicode))
                    .map_err(|_| RuntimeError::Invariant("advanced split index did not fit usize"))?;
                continue;
            };
            let complete = matched.capture(0).ok_or(RuntimeError::Invariant(
                "successful RegExp execution omitted capture zero",
            ))?;
            let e = complete.end.min(size);
            if e == p {
                q = usize::try_from(advance_string_index(input, q as u64, unicode))
                    .map_err(|_| RuntimeError::Invariant("advanced split index did not fit usize"))?;
                continue;
            }
            self.push_standard_split_piece(array, input, Some((p, q)), length)?;
            if *length == limit {
                return Ok(StandardSplitOutcome::Complete);
            }
            p = e;
            for range in matched.captures()[1..].iter() {
                self.push_standard_split_piece(
                    array,
                    input,
                    range.as_ref().map(|r| (r.start, r.end)),
                    length,
                )?;
                if *length == limit {
                    return Ok(StandardSplitOutcome::Complete);
                }
            }
            q = p;
        }
        self.push_standard_split_piece(array, input, Some((p.min(size), size)), length)?;
        Ok(StandardSplitOutcome::Complete)
    }

    /// One anchored match of the sticky splitter program at `start`,
    /// mapping executor failures the way the plain exec fast path does.
    fn standard_regexp_exec_at(
        &self,
        program: &Rc<CompiledRegExp>,
        flat: &JsString,
        start: usize,
    ) -> Result<StandardSplitExec, RuntimeError> {
        let execution = if let Some(units) = flat.flat_latin1() {
            execute_latin1_with_interrupt(program.as_ref(), units, start, || false)
        } else {
            execute_with_interrupt(
                program.as_ref(),
                flat.flat_utf16().expect("linearized input"),
                start,
                || false,
            )
        };
        match execution {
            Ok(value) => Ok(StandardSplitExec::Matched(value)),
            Err(ExecError::OutOfMemory) => Ok(StandardSplitExec::Throw(
                "out of memory in regexp execution",
            )),
            Err(ExecError::Interrupted) => Ok(StandardSplitExec::Throw("interrupted")),
            Err(ExecError::InvalidProgram(_)) => Err(RuntimeError::Invariant(
                "compiled RegExp program failed executor validation",
            )),
            Err(ExecError::StartOutOfBounds { .. }) => Err(RuntimeError::Invariant(
                "bounded RegExp start was rejected by executor",
            )),
        }
    }

    /// Allocate one piece string and move it into the result array as a
    /// dense element; no piece takes a retain/release pair or a property
    /// query. `None` appends the spec's `undefined` for a missing capture.
    fn push_standard_split_piece(
        &self,
        array: ObjectId,
        input: &JsString,
        range: Option<(usize, usize)>,
        length: &mut u32,
    ) -> Result<(), RuntimeError> {
        let value = match range {
            Some((start, end)) => JsValue::String(
                self.0
                    .state
                    .borrow_mut()
                    .heap
                    .allocate_string(input.sub_string(start, end))?,
            ),
            None => JsValue::Undefined,
        };
        self.0.state.borrow_mut().append_fresh_array_value_jsvalue(
            &self.0.poisoned,
            array,
            value,
        )?;
        *length = length
            .checked_add(1)
            .ok_or(RuntimeError::Invariant("RegExp split output index exceeded Uint32"))?;
        Ok(())
    }
}

struct StandardRegExpSplit {
    input: JsString,
    pattern: JsString,
    flags: RegExpFlags,
    limit: u32,
}

enum StandardSplitExec {
    Matched(Option<crate::regexp::RegExpMatch>),
    Throw(&'static str),
}

enum StandardSplitOutcome {
    Complete,
    Throw(&'static str),
}

pub(crate) enum RegExpSplitStep {
    Complete(Completion),
    Primitive { resume: RegExpSplitResume },
    Read { resume: RegExpSplitResume },
    Species { resume: RegExpSplitResume },
    Construct { resume: RegExpSplitResume },
    Set { resume: RegExpSplitResume },
    Exec { resume: RegExpSplitResume },
}
pub(crate) struct RegExpSplitResume(Box<RegExpSplitResumeState>);
impl std::ops::Deref for RegExpSplitResume {
    type Target = RegExpSplitResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpSplitResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpSplitResume>() <= 8);
pub(crate) struct RegExpSplitResumeState {
    limit_value: JsValue,
    input_value: JsValue,
    step_pending: RegExpSplitStepPending,
    realm: ContextId,
    phase: Phase,
}
impl Drop for RegExpSplitResumeState {
    fn drop(&mut self) {
        let runtime = &self.step_pending.runtime;
        let _ =
            runtime.release_jsvalue(std::mem::replace(&mut self.limit_value, JsValue::Undefined));
        let _ =
            runtime.release_jsvalue(std::mem::replace(&mut self.input_value, JsValue::Undefined));
    }
}
enum Phase {
    Vacant,
    Input {
        regexp: ObjectRef,
    },
    Species {
        regexp: ObjectRef,
        input: JsString,
    },
    Flags {
        regexp: ObjectRef,
        input: JsString,
        constructor: ConstructorRef,
    },
    FlagsPrimitive {
        regexp: ObjectRef,
        input: JsString,
        constructor: ConstructorRef,
    },
    Construct {
        input: JsString,
        unicode: bool,
    },
    Limit(SplitState),
    Empty(SplitState),
    Set(SplitState),
    Exec(SplitState),
    End {
        state: SplitState,
        matched: ObjectRef,
    },
    EndPrimitive {
        state: SplitState,
        matched: ObjectRef,
    },
    Count {
        state: SplitState,
        matched: ObjectRef,
    },
    CountPrimitive {
        state: SplitState,
        matched: ObjectRef,
    },
    Capture {
        state: SplitState,
        matched: ObjectRef,
        index: u64,
        count: u64,
    },
}
struct SplitState {
    input_value: JsValue,
    input: JsString,
    splitter: ObjectRef,
    result: ObjectRef,
    unicode: bool,
    limit: u32,
    length: u32,
    p: usize,
    q: usize,
}
impl Drop for SplitState {
    fn drop(&mut self) {
        let _ = self
            .splitter
            .runtime()
            .release_jsvalue(std::mem::replace(&mut self.input_value, JsValue::Undefined));
    }
}
impl SplitState {
    fn complete(self) -> Result<RegExpSplitStep, crate::engine::api::RuntimeError> {
        Ok({
            RegExpSplitStep::Complete(Completion::Return(JsValue::Object(
                self.result.try_clone()?.into_handle(),
            )))
        })
    }
    fn append(&mut self, runtime: &Runtime, value: JsValue) -> Result<(), RuntimeError> {
        runtime.append_regexp_split_value(&self.result, &mut self.length, value)
    }
    fn advance(&mut self) -> Result<(), RuntimeError> {
        self.q = usize::try_from(advance_string_index(
            &self.input,
            self.q as u64,
            self.unicode,
        ))
        .map_err(|_| RuntimeError::Invariant("advanced split index did not fit usize"))?;
        Ok(())
    }
}
impl RegExpSplitResume {
    // The driver has taken the previous request's fields before delivering its
    // reply. Keep this allocation through the split loop; the guest input and
    // limit have already moved to their original owners before these methods.
    fn next(
        mut self,
        mut state: SplitState,
        runtime: &Runtime,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        debug_assert!(self.0.step_pending.is_empty());
        if state.q >= state.input.len() {
            let value = Value::String(
                state
                    .input
                    .sub_string(state.p.min(state.input.len()), state.input.len()),
            );
            state.append(runtime, runtime.into_jsvalue(value)?)?;
            return state.complete();
        }
        let value = JsValue::Int(i32::try_from(state.q).map_err(|_| {
            RuntimeError::Invariant("RegExp split index exceeded signed String range")
        })?);
        let object = state.splitter.try_clone()?;
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        self.0.phase = Phase::Set(state);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_split.next_box_reused");
        Ok(RegExpSplitStep::make_set(object, key, value, self))
    }
    fn execute(
        mut self,
        state: SplitState,
        runtime: &Runtime,
        empty: bool,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        debug_assert!(self.0.step_pending.is_empty());
        let input = runtime.dup_jsvalue(&state.input_value)?;
        let regexp = JsValue::Object(state.splitter.try_clone()?.into_handle());
        self.0.phase = if empty {
            Phase::Empty(state)
        } else {
            Phase::Exec(state)
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_split.exec_box_reused");
        Ok(RegExpSplitStep::make_exec(regexp, input, self))
    }
    fn captures(
        mut self,
        mut state: SplitState,
        runtime: &Runtime,
        matched: ObjectRef,
        index: u64,
        count: u64,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        debug_assert!(self.0.step_pending.is_empty());
        if index >= count {
            state.q = state.p;
            return self.next(state, runtime);
        }
        let object = matched.try_clone()?;
        let key = runtime.intern_property_key(&index.to_string())?;
        self.0.phase = Phase::Capture {
            state,
            matched,
            index,
            count,
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "regexp_split.capture_box_reused",
        );
        Ok(RegExpSplitStep::make_read(object, key, self))
    }
}
impl RegExpSplitStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@split did not receive a generic invocation",
            ));
        };
        let JsValue::Object(id) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        // Standard split: a genuine compiled receiver, primitive String
        // input, immediate limit, standard `flags` getter and default
        // species run the matching loop directly; every per-piece protocol
        // step (lastIndex write/read, abstract exec, capture reads, dense
        // defines) collapses into one in-State append per piece.
        if let (Some(input @ JsValue::String(_)), Some(limit)) =
            (arguments.readable.first(), arguments.readable.get(1))
            && let Some(standard) = runtime.standard_regexp_split(realm, *id, input, limit)?
        {
            return Ok(Self::Complete(
                runtime.call_standard_regexp_split(realm, standard)?,
            ));
        }
        let regexp = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
        let mut resume = RegExpSplitResume::new(runtime, realm, Phase::Input { regexp });
        resume.0.limit_value = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
            RuntimeError::Invariant("RegExp @@split limit argv was not padded"),
        )?)?;
        let input = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("RegExp @@split input argv was not padded"),
        )?)?;
        Ok(Self::make_primitive(input, ToPrimitiveHint::String, resume))
    }
}
impl RegExpSplitResume {
    fn new(runtime: &Runtime, realm: ContextId, phase: Phase) -> Self {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_split.resident_box");
        Self(Box::new(RegExpSplitResumeState {
            step_pending: RegExpSplitStepPending::new(runtime),
            realm,
            phase,
            limit_value: JsValue::Undefined,
            input_value: JsValue::Undefined,
        }))
    }
    pub(crate) fn species(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorRef>,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let constructor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Species { regexp, input } = std::mem::replace(&mut self.0.phase, Phase::Vacant)
        else {
            return Err(RuntimeError::Invariant(
                "RegExp split species reply in wrong phase",
            ));
        };
        let object = regexp.try_clone()?;
        self.0.phase = Phase::Flags {
            regexp,
            input,
            constructor,
        };
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Flags)?;
        Ok(RegExpSplitStep::make_read(object, key, self))
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        let result = match runtime.finish_set_property_or_throw(self.0.realm, &key, result)? {
            Some(value) => Completion::Throw(value),
            None => Completion::Return(JsValue::Undefined),
        };
        self.resume(runtime, result)
    }
    fn string_reply(
        &mut self,
        runtime: &Runtime,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        runtime.string_from_primitive_jsvalue(
            self.0.realm,
            self.0
                .step_pending
                .value
                .as_ref()
                .expect("split primitive reply"),
        )
    }
    fn number_reply(&mut self, runtime: &Runtime) -> Result<NativeConversion<f64>, RuntimeError> {
        let value = self
            .0
            .step_pending
            .value
            .take()
            .expect("split numeric reply");
        let number = runtime.number_from_primitive_jsvalue(self.0.realm, &value);
        runtime.release_jsvalue(value)?;
        number
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
            }
        };
        self.0.step_pending.value = Some(value);
        let realm = self.0.realm;
        match std::mem::replace(&mut self.0.phase, Phase::Vacant) {
            Phase::Input { regexp } => {
                let input = match self.string_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let value = self.0.step_pending.value.take().unwrap();
                self.0.input_value = if matches!(value, JsValue::String(_)) {
                    value
                } else {
                    runtime.release_jsvalue(value)?;
                    runtime.into_jsvalue(Value::String(input.clone()))?
                };
                let object = regexp.try_clone()?;
                self.0.phase = Phase::Species { regexp, input };
                Ok(RegExpSplitStep::make_species(object, self))
            }
            Phase::Flags {
                regexp,
                input,
                constructor,
            } => {
                self.0.phase = Phase::FlagsPrimitive {
                    regexp,
                    input,
                    constructor,
                };
                let value = self.0.step_pending.value.take().unwrap();
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::String,
                    self,
                ))
            }
            Phase::FlagsPrimitive {
                regexp,
                input,
                constructor,
            } => {
                let flags = match self.string_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let unicode = flags
                    .utf16_units()
                    .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                let sticky = flags.utf16_units().any(|unit| unit == u16::from(b'y'));
                let flags = if sticky {
                    flags
                } else {
                    flags.try_concat(&JsString::from_static("y"))?
                };
                self.0.step_pending.arguments = Some(Vec::new());
                if self
                    .0
                    .step_pending
                    .arguments
                    .as_mut()
                    .unwrap()
                    .try_reserve_exact(2)
                    .is_err()
                {
                    return Ok(RegExpSplitStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                self.0
                    .step_pending
                    .arguments
                    .as_mut()
                    .unwrap()
                    .push(JsValue::Object(regexp.into_handle()));
                let value = self.0.step_pending.value.take().unwrap();
                let flags_value = if sticky && matches!(value, JsValue::String(_)) {
                    value
                } else {
                    runtime.release_jsvalue(value)?;
                    runtime.into_jsvalue(Value::String(flags))?
                };
                self.0
                    .step_pending
                    .arguments
                    .as_mut()
                    .unwrap()
                    .push(flags_value);
                self.0.step_pending.constructor = Some(constructor);
                self.0.phase = Phase::Construct { input, unicode };
                Ok(RegExpSplitStep::Construct { resume: self })
            }
            Phase::Construct { input, unicode } => {
                let splitter = match self.0.step_pending.value.take().unwrap() {
                    JsValue::Object(id) => ObjectRef::from_owned_handle(runtime.clone(), id),
                    value => {
                        runtime.release_jsvalue(value)?;
                        return Err(RuntimeError::Invariant(
                            "RegExp species constructor returned a primitive",
                        ));
                    }
                };
                let result = runtime.new_array(realm)?;
                let state = SplitState {
                    input,
                    input_value: std::mem::replace(&mut self.0.input_value, JsValue::Undefined),
                    splitter,
                    result,
                    unicode,
                    limit: u32::MAX,
                    length: 0,
                    p: 0,
                    q: 0,
                };
                if matches!(self.0.limit_value, JsValue::Undefined) {
                    return self.after_limit(state, runtime);
                }
                let value = std::mem::replace(&mut self.0.limit_value, JsValue::Undefined);
                self.0.phase = Phase::Limit(state);
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::Number,
                    self,
                ))
            }
            Phase::Limit(mut state) => {
                let number = match self.number_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                state.limit = Runtime::to_uint32_number(number);
                self.after_limit(state, runtime)
            }
            Phase::Empty(mut state) => {
                let value = self.0.step_pending.value.take().unwrap();
                let valid = matches!(value, JsValue::Null | JsValue::Object(_));
                let empty = matches!(value, JsValue::Null);
                runtime.release_jsvalue(value)?;
                if !valid {
                    return Err(RuntimeError::Invariant(
                        "RegExpExec returned neither an object nor null",
                    ));
                }
                if empty {
                    let input = runtime.dup_jsvalue(&state.input_value)?;
                    state.append(runtime, input)?;
                }
                Ok(state.complete()?)
            }
            Phase::Set(state) => {
                runtime.release_jsvalue(self.0.step_pending.value.take().unwrap())?;
                self.execute(state, runtime, false)
            }
            Phase::Exec(mut state) => match self.0.step_pending.value.take().unwrap() {
                JsValue::Null => {
                    state.advance()?;
                    self.next(state, runtime)
                }
                JsValue::Object(id) => {
                    let matched = ObjectRef::from_owned_handle(runtime.clone(), id);
                    let object = state.splitter.try_clone()?;
                    self.0.phase = Phase::End { state, matched };
                    let key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
                    Ok(RegExpSplitStep::make_read(object, key, self))
                }
                value => {
                    runtime.release_jsvalue(value)?;
                    Err(RuntimeError::Invariant(
                        "RegExpExec returned neither an object nor null",
                    ))
                }
            },
            Phase::End { state, matched } => {
                self.0.phase = Phase::EndPrimitive { state, matched };
                let value = self.0.step_pending.value.take().unwrap();
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::Number,
                    self,
                ))
            }
            Phase::EndPrimitive { mut state, matched } => {
                let number = match self.number_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let end = usize::try_from(
                    Runtime::length_from_number(number).min(state.input.len() as u64),
                )
                .map_err(|_| RuntimeError::Invariant("split end index did not fit usize"))?;
                if end == state.p {
                    state.advance()?;
                    return self.next(state, runtime);
                }
                let part = runtime
                    .into_jsvalue(Value::String(state.input.sub_string(state.p, state.q)))?;
                state.append(runtime, part)?;
                if state.length == state.limit {
                    return state.complete();
                }
                state.p = end;
                let object = matched.try_clone()?;
                self.0.phase = Phase::Count { state, matched };
                let key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
                Ok(RegExpSplitStep::make_read(object, key, self))
            }
            Phase::Count { state, matched } => {
                self.0.phase = Phase::CountPrimitive { state, matched };
                let value = self.0.step_pending.value.take().unwrap();
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::Number,
                    self,
                ))
            }
            Phase::CountPrimitive { state, matched } => {
                let number = match self.number_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                self.captures(
                    state,
                    runtime,
                    matched,
                    1,
                    Runtime::length_from_number(number),
                )
            }
            Phase::Capture {
                mut state,
                matched,
                index,
                count,
            } => {
                state.append(runtime, self.0.step_pending.value.take().unwrap())?;
                if state.length == state.limit {
                    return state.complete();
                }
                self.captures(state, runtime, matched, index + 1, count)
            }
            Phase::Species { .. } | Phase::Vacant => Err(RuntimeError::Invariant(
                "RegExp split completion in species phase",
            )),
        }
    }
    fn after_limit(
        self,
        state: SplitState,
        runtime: &Runtime,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        if state.limit == 0 {
            return state.complete();
        }
        if state.input.is_empty() {
            return self.execute(state, runtime, true);
        }
        // Preserve key allocation before the first observable splitter write.
        runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
        self.next(state, runtime)
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpSplitStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpSplitStep::Complete(result) => return Ok(result),
            RegExpSplitStep::Primitive { mut resume } => {
                let value = resume.take_primitive_value();
                let hint = resume.take_primitive_hint();
                {
                    let result = if matches!(value, JsValue::Object(_)) {
                        runtime.to_primitive_jsvalue(realm, value, hint)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(runtime, result)?
                }
            }
            RegExpSplitStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.internal_get_jsvalue(
                        realm,
                        &object,
                        &key,
                        JsValue::Object(object.try_clone()?.into_handle()),
                    )?,
                )?
            }
            RegExpSplitStep::Species { mut resume } => {
                let regexp = resume.take_species_regexp();
                resume.species(runtime, runtime.regexp_species_constructor(realm, &regexp)?)?
            }
            RegExpSplitStep::Construct { mut resume } => {
                let constructor = resume.take_construct_constructor();
                let new_target = constructor.try_clone()?;
                let arguments = resume.take_construct_arguments();
                resume.resume(
                    runtime,
                    runtime.construct_internal_jsvalue(
                        realm,
                        &constructor,
                        crate::engine::vm::call::ConstructNewTarget::Validated(new_target),
                        arguments,
                    )?,
                )?
            }
            RegExpSplitStep::Set { mut resume } => {
                let object = resume.take_set_object();
                let key = resume.take_set_key();
                let value = resume.take_set_value();
                resume.set(
                    runtime,
                    runtime.internal_set_jsvalue(
                        realm,
                        &object,
                        &key,
                        value,
                        JsValue::Object(object.try_clone()?.into_handle()),
                    )?,
                )?
            }
            RegExpSplitStep::Exec { mut resume } => {
                let regexp = resume.take_exec_regexp();
                let input = resume.take_exec_input();
                resume.resume(runtime, runtime.regexp_exec_abstract(realm, regexp, input)?)?
            }
        }
    }
}

pub(crate) struct RegExpSplitStepPending {
    runtime: Runtime,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    object: Option<ObjectRef>,
    key: Option<PropertyKey>,
    regexp: Option<ObjectRef>,
    constructor: Option<ConstructorRef>,
    arguments: Option<Vec<JsValue>>,
    exec_regexp: Option<JsValue>,
    input: Option<JsValue>,
}
impl RegExpSplitStepPending {
    fn is_empty(&self) -> bool {
        self.value.is_none()
            && self.hint.is_none()
            && self.object.is_none()
            && self.key.is_none()
            && self.regexp.is_none()
            && self.constructor.is_none()
            && self.arguments.is_none()
            && self.exec_regexp.is_none()
            && self.input.is_none()
    }
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            value: None,
            hint: None,
            object: None,
            key: None,
            regexp: None,
            constructor: None,
            arguments: None,
            exec_regexp: None,
            input: None,
        }
    }

    /// Release the internal edges still owned when the request is abandoned
    /// before its step consumed them. Taken fields are empty here.
    fn release_owned(&mut self) {
        for value in [
            self.value.take(),
            self.exec_regexp.take(),
            self.input.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        for argument in self.arguments.take().into_iter().flatten() {
            let _ = self.runtime.release_jsvalue(argument);
        }
    }
}
impl Drop for RegExpSplitStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl RegExpSplitStep {
    pub(crate) fn make_primitive(
        value: JsValue,
        hint: ToPrimitiveHint,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.value = Some(value);
        resume.0.step_pending.hint = Some(hint);
        Self::Primitive { resume }
    }
    pub(crate) fn make_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_species(regexp: ObjectRef, mut resume: RegExpSplitResume) -> Self {
        resume.0.step_pending.regexp = Some(regexp);
        Self::Species { resume }
    }
    pub(crate) fn make_set(
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        resume.0.step_pending.value = Some(value);
        Self::Set { resume }
    }
    pub(crate) fn make_exec(
        regexp: JsValue,
        input: JsValue,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.exec_regexp = Some(regexp);
        resume.0.step_pending.input = Some(input);
        Self::Exec { resume }
    }
}
impl RegExpSplitResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpSplitStep::Primitive lost value")
    }
    pub(crate) fn take_primitive_hint(&mut self) -> ToPrimitiveHint {
        self.0
            .step_pending
            .hint
            .take()
            .expect("RegExpSplitStep::Primitive lost hint")
    }

    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpSplitStep::Read lost object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpSplitStep::Read lost key")
    }

    pub(crate) fn take_species_regexp(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .regexp
            .take()
            .expect("RegExpSplitStep::Species lost regexp")
    }

    pub(crate) fn take_construct_constructor(&mut self) -> ConstructorRef {
        self.0
            .step_pending
            .constructor
            .take()
            .expect("RegExpSplitStep::Construct lost constructor")
    }
    pub(crate) fn take_construct_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .step_pending
            .arguments
            .take()
            .expect("RegExpSplitStep::Construct lost arguments")
    }

    pub(crate) fn take_set_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpSplitStep::Set lost object")
    }
    pub(crate) fn take_set_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpSplitStep::Set lost key")
    }
    pub(crate) fn take_set_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpSplitStep::Set lost value")
    }

    pub(crate) fn take_exec_regexp(&mut self) -> JsValue {
        self.0
            .step_pending
            .exec_regexp
            .take()
            .expect("RegExpSplitStep::Exec lost regexp")
    }
    pub(crate) fn take_exec_input(&mut self) -> JsValue {
        self.0
            .step_pending
            .input
            .take()
            .expect("RegExpSplitStep::Exec lost input")
    }
}

const _: () = assert!(std::mem::size_of::<RegExpSplitStep>() <= 64);

#[cfg(test)]
mod tests;

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpSplitStep>() <= 64);
