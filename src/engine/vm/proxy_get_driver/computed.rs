//! The computed instruction owns its moved operands in its actual Query.
use super::{Completion, Finish, Query, Resume, Step};
use crate::engine::{
    api::{RuntimeError, runtime::Runtime},
    atom::{Atom, AtomIdx},
    heap::{
        ContextId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::LinkedNativeSelection,
    value::{JsValue, conversion::property_key::PropertyKeyAtomStep},
    vm::{execute::FallthroughPc, stack::ReadOperandCommit},
};
use std::cell::Cell;

/// Acquire the existing Query storage for the actual computed instruction.
/// Construction stays inside its owning driver; no storage API escapes.
pub(in crate::engine::vm) fn acquire_query(
    storage: &mut super::QueryStorage,
    realm: ContextId,
    finish: Finish,
) -> Query {
    storage.acquire(realm, Vec::new(), finish)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::engine::vm) enum ComputedPhase {
    Key,
    Read,
    Getter,
    Proxy,
    Complete,
}

pub(in crate::engine::vm) struct ComputedRead {
    #[cfg(feature = "profiling")]
    pub(in crate::engine::vm) depth: usize,
    pub(in crate::engine::vm) fallthrough: FallthroughPc,
    pub(in crate::engine::vm) consume: u8,
    pub(in crate::engine::vm) keep_receiver: bool,
    pub(in crate::engine::vm) keep_key: bool,
    pub(in crate::engine::vm) base: Option<JsValue>,
    pub(in crate::engine::vm) retained_key: Option<JsValue>,
    pub(in crate::engine::vm) atom: Option<Atom>,
    pub(in crate::engine::vm) native: Option<LinkedNativeSelection>,
    pub(in crate::engine::vm) prefix_published: bool,
    pub(in crate::engine::vm) cycle_published: bool,
    pub(in crate::engine::vm) phase: ComputedPhase,
}
impl ComputedRead {
    pub(in crate::engine::vm) fn new(
        _depth: usize,
        fallthrough: FallthroughPc,
        keep_receiver: bool,
        keep_key: bool,
    ) -> Self {
        Self {
            #[cfg(feature = "profiling")]
            depth: _depth,
            fallthrough,
            consume: 2,
            keep_receiver,
            keep_key,
            base: None,
            retained_key: None,
            atom: None,
            native: None,
            prefix_published: false,
            cycle_published: false,
            phase: ComputedPhase::Key,
        }
    }
    pub(in crate::engine::vm) fn callback_commit(&mut self) -> ReadOperandCommit<'_> {
        ReadOperandCommit {
            consume: self.consume,
            keep_receiver: self.keep_receiver,
            retained_key: Some(&mut self.retained_key),
            prefix_published: Some(&mut self.prefix_published),
        }
    }
    pub(in crate::engine::vm) fn retire_atom(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if let Some(atom) = self.atom.take() {
            // The coordinator's Symbol branch is the concrete atom-owner
            // retirement kernel, including poison/unwind coverage.
            state
                .release_owned_jsvalue(poisoned, JsValue::Symbol(AtomIdx::from_raw(atom.raw())))?;
        }
        Ok(())
    }
    pub(in crate::engine::vm) fn retire_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_atom(state, poisoned)?;
        if let Some(key) = self.retained_key.take() {
            state.release_owned_jsvalue(poisoned, key)?;
        }
        if let Some(base) = self.base.take() {
            state.release_owned_jsvalue(poisoned, base)?;
        }
        Ok(())
    }
    pub(super) fn retire_at_boundary(&mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        if let Some(atom) = self.atom.take() {
            runtime.release_jsvalue(JsValue::Symbol(AtomIdx::from_raw(atom.raw())))?;
            runtime.check_poison()?;
        }
        if let Some(key) = self.retained_key.take() {
            runtime.release_jsvalue(key)?;
            runtime.check_poison()?;
        }
        if let Some(base) = self.base.take() {
            runtime.release_jsvalue(base)?;
            runtime.check_poison()?;
        }
        Ok(())
    }
    /// Direct keys retain their original Int representation; a converted Int
    /// reply retains a formatted String. Both use the one State formatter.
    pub(in crate::engine::vm) fn preserve_key(
        &mut self,
        state: &mut RuntimeState,
        value: &JsValue,
        direct: bool,
    ) -> Result<(), RuntimeError> {
        if !self.keep_key {
            return Ok(());
        }
        self.retained_key = Some(
            if matches!(value, JsValue::String(_) | JsValue::Symbol(_))
                || direct && matches!(value, JsValue::Int(_))
            {
                state.dup_jsvalue(value)?
            } else {
                let text = state
                    .to_js_string_jsvalue(value)
                    .map_err(RuntimeError::Engine)?;
                JsValue::String(state.heap.allocate_string(text)?)
            },
        );
        Ok(())
    }
    pub(in crate::engine::vm) fn selected_step(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        domain: u64,
        realm: ContextId,
        borrowed_base: Option<&JsValue>,
    ) -> Result<Step, RuntimeError> {
        let atom = self.atom.expect("computed key atom owner");
        let base = borrowed_base
            .or(self.base.as_ref())
            .ok_or(RuntimeError::Invariant("computed read lost base"))?;
        let selected = state.prepare_value_read_in_state(
            poisoned,
            domain,
            realm,
            base,
            atom,
            Some(&mut self.native),
        );
        self.phase = ComputedPhase::Read;
        match selected {
            Ok(read) => Ok(Step::RawRead {
                read: Some(read),
                key: atom,
                resume: Some(Resume::Identity),
            }),
            Err(RuntimeError::Engine(error))
                if crate::engine::api::error::NativeErrorKind::from_javascript_error(
                    error.kind(),
                )
                .is_some() =>
            {
                Ok(Step::ComputedError(Some(error)))
            }
            Err(error) => Err(error),
        }
    }
}

impl Query {
    pub(in crate::engine::vm) fn computed_read_mut(&mut self) -> Option<&mut ComputedRead> {
        match self.finish.as_mut() {
            Some(Finish::ComputedRead(input)) => Some(input),
            _ => None,
        }
    }
    pub(in crate::engine::vm) fn final_read_pending(&self) -> bool {
        matches!(self.finish.as_ref(), Some(Finish::ComputedRead(input)) if input.phase == ComputedPhase::Read)
    }
    pub(in crate::engine::vm) fn final_getter_pending(&self) -> bool {
        matches!(self.finish.as_ref(), Some(Finish::ComputedRead(input)) if input.phase == ComputedPhase::Getter && !input.prefix_published)
    }
    /// The fact belongs to this instruction's concrete owned continuation.
    pub(in crate::engine::vm) fn carry_computed_publication(&mut self) -> bool {
        match self.finish.as_mut() {
            Some(Finish::ComputedRead(input)) => input.cycle_published = true,
            Some(Finish::PropertyKeyValue {
                cycle_published, ..
            }) => *cycle_published = true,
            _ => return false,
        }
        true
    }
    pub(in crate::engine::vm) fn has_computed_publication(&self) -> bool {
        match self.finish.as_ref() {
            Some(Finish::ComputedRead(input)) => input.cycle_published,
            Some(Finish::PropertyKeyValue {
                cycle_published, ..
            }) => *cycle_published,
            _ => false,
        }
    }
    pub(in crate::engine::vm) fn clear_computed_publication(&mut self) {
        match self.finish.as_mut() {
            Some(Finish::ComputedRead(input)) => input.cycle_published = false,
            Some(Finish::PropertyKeyValue {
                cycle_published, ..
            }) => *cycle_published = false,
            _ => {}
        }
    }
    pub(super) fn is_computed_instruction(&self) -> bool {
        matches!(
            self.finish,
            Some(Finish::ComputedRead(_) | Finish::PropertyKeyValue { .. })
        )
    }
    /// Only the actual final selected read owns this publication prefix. A
    /// conversion's earlier getters continue to use their existing operands.
    pub(super) fn publish_selected_prefix_at_boundary(
        &mut self,
        runtime: &Runtime,
        execution: &mut crate::engine::vm::execution::RunningExecution,
        owner: crate::engine::vm::frame::ReturnOwner,
    ) -> Result<(), crate::engine::api::Error> {
        let Some(input) = self.computed_read_mut().filter(|input| {
            matches!(input.phase, ComputedPhase::Getter | ComputedPhase::Proxy)
                && !input.prefix_published
        }) else {
            return Ok(());
        };
        let mut state = runtime.0.state.borrow_mut();
        let mut frame = crate::engine::vm::stack::FrameExecution::admit(execution, owner.frame()?)?;
        let commit = ReadOperandCommit {
            consume: input.consume,
            keep_receiver: input.keep_receiver,
            retained_key: Some(&mut input.retained_key),
            prefix_published: Some(&mut input.prefix_published),
        };
        frame.commit_property_read_operands(
            &mut state,
            &runtime.0.poisoned,
            &mut input.base,
            commit,
        )
    }
    pub(super) fn computed_key_reply(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        completion: Completion,
    ) -> Result<Step, RuntimeError> {
        let Completion::Return(value) = completion else {
            return Ok(Step::Complete(Some(completion)));
        };
        let mut reply = OwnedValueGuard::new(state, &runtime.0.poisoned, value);
        let (state, value) = reply.parts();
        let realm = self.realm;
        let input = self.computed_read_mut().ok_or(RuntimeError::Invariant(
            "computed key lost final continuation",
        ))?;
        // Converted-key keeper precedes the consumed atom suffix. In contrast,
        // the direct instruction calls these same methods in the reverse order.
        input.preserve_key(state, value.as_ref().expect("primitive key reply"), false)?;
        match state.property_key_from_primitive_jsvalue_with_publication(
            &runtime.0.poisoned,
            realm,
            value.take().expect("primitive key reply"),
        )? {
            PropertyKeyAtomStep::Value(atom) => input.atom = Some(atom),
            PropertyKeyAtomStep::CyclePublishedThrow(value) => {
                input.cycle_published = true;
                return Ok(Step::Complete(Some(Completion::Throw(value))));
            }
        }
        input.selected_step(state, &runtime.0.poisoned, runtime.domain_id(), realm, None)
    }
}

impl Finish {
    pub(in crate::engine::vm) fn retire_computed_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if let Self::ComputedRead(input) = self {
            input.retire_in_state(state, poisoned)?;
        }
        Ok(())
    }
    pub(super) fn retire_computed_at_boundary(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        if let Self::ComputedRead(input) = self {
            input.retire_at_boundary(runtime)?;
        }
        Ok(())
    }
}

impl super::PendingProxyGet {
    pub(in crate::engine::vm) fn take_computed_publication(&mut self) -> bool {
        let published = self.query.has_computed_publication();
        self.query.clear_computed_publication();
        published
    }
    pub(in crate::engine::vm) fn computed_callback_commit(
        &mut self,
    ) -> Option<ReadOperandCommit<'_>> {
        self.query
            .computed_read_mut()
            .filter(|input| input.phase == ComputedPhase::Getter && !input.prefix_published)
            .map(ComputedRead::callback_commit)
    }
}

/// A real selected boundary reenters the existing State Query consumer. Its
/// current phases/owners are moved once; no public getter root or lookup is made.
pub(super) fn resume_at_boundary(
    runtime: &Runtime,
    execution: &mut crate::engine::vm::execution::RunningExecution,
    owner: crate::engine::vm::frame::ReturnOwner,
    query: &mut Query,
    step: &mut Step,
) -> Result<super::Next, crate::engine::api::Error> {
    use crate::engine::vm::{proxy_get_driver::StateNativeProgress, stack::FrameExecution};
    let fallthrough = match query.finish.as_ref() {
        Some(Finish::ComputedRead(input)) => input.fallthrough,
        Some(Finish::PropertyKeyValue { fallthrough, .. }) => *fallthrough,
        _ => {
            return Err(crate::engine::api::Error::internal(
                "computed boundary lost final continuation",
            ));
        }
    };
    let empty = Query {
        native_runtime: std::rc::Weak::new(),
        #[cfg(feature = "profiling")]
        had_callback: false,
        realm: query.realm,
        parents: super::Parents::default(),
        natives: Vec::new(),
        saved_native_depth: 0,
        spare_parents: Vec::new(),
        finish: None,
    };
    let moved = std::mem::replace(query, empty);
    let selected = std::mem::replace(step, Step::Complete(None));
    let mut state = runtime.0.state.borrow_mut();
    let mut raw = super::RawNativeQuery::from_query(runtime, &mut state, moved, selected);
    let frame = match owner.frame() {
        Ok(frame) => frame,
        Err(error) => {
            raw.retire().map_err(super::runtime_error_to_vm_error)?;
            return Err(error);
        }
    };
    let admitted = FrameExecution::admit(execution, frame);
    let mut segment = match admitted {
        Ok(segment) => segment,
        Err(error) => {
            raw.retire().map_err(super::runtime_error_to_vm_error)?;
            return Err(error);
        }
    };
    let result = segment.consume_read_query(&mut raw, fallthrough)?;
    Ok(super::Next::Done(match result {
        StateNativeProgress::Published => super::Progress::Resident,
        StateNativeProgress::PublishedThrow => super::Progress::ResidentThrow,
        StateNativeProgress::Entered | StateNativeProgress::Boundary => {
            super::Progress::Call(super::CallStep::Entered)
        }
        StateNativeProgress::Complete(completion) => {
            super::Progress::Call(super::CallStep::Complete(completion))
        }
    }))
}
