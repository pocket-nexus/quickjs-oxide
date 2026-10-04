//! Explicit native owners and their concrete storage/boundary guards.
use super::{NativeActivation, PreparedNativeCall};
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    builtins::native::NativeFunctionId,
    heap::{ContextId, ObjectId, runtime::RuntimeState},
    object::{CallableRef, ObjectRef},
    value::JsValue,
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation, NativeInvokeMode, NativeInvokeOutcome},
        frames::{NativeClassification, NativeStatePublication},
    },
};
use std::{
    cell::Cell,
    ops::{Deref, DerefMut},
};

fn empty_invocation() -> NativeInvocation {
    NativeInvocation::Getter {
        this_value: JsValue::Undefined,
    }
}
fn skip_cleanup(poisoned: &Cell<bool>) -> bool {
    if std::thread::panicking() {
        poisoned.set(true);
    }
    poisoned.get()
}

/// Transient call ownership outside a state lease. The caller already lends
/// the runtime; this guard adds neither a strong nor a weak runtime owner.
#[must_use]
pub(in crate::engine::vm) struct NativeCallGuard<'a> {
    runtime: &'a Runtime,
    call: Option<PreparedNativeCall>,
}
impl<'a> NativeCallGuard<'a> {
    pub(in crate::engine::vm) fn new(runtime: &'a Runtime, call: PreparedNativeCall) -> Self {
        Self {
            runtime,
            call: Some(call),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn from_operands(
        runtime: &'a Runtime,
        function: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        mode: NativeInvokeMode,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self::new(
            runtime,
            PreparedNativeCall::from_operands(function, realm, target, mode, invocation, arguments),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn from_callable(
        runtime: &'a Runtime,
        callable: CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        mode: NativeInvokeMode,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
    ) -> Result<Self, RuntimeError> {
        let mut inputs = super::PendingNativeOwners {
            runtime,
            invocation: Some(invocation),
            readable: arguments,
        };
        if !callable.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("native callable"));
        }
        Ok(Self::from_operands(
            runtime,
            callable.into_object().into_execution_handle(),
            realm,
            target,
            mode,
            inputs.invocation.take().expect("native invocation"),
            std::mem::take(&mut inputs.readable),
        ))
    }
    pub(in crate::engine::vm) fn publish(
        self,
        minimum: u8,
        selected: Option<&NativeClassification>,
    ) -> Result<Self, RuntimeError> {
        let runtime = self.runtime;
        let _unwind = runtime.unwind_guard();
        #[cfg(feature = "profiling")]
        let _phase = crate::engine::api::profiling::PhaseTimer::start_vm("native.prepare");
        // Acquire before surrendering the borrowed owner. A rejected lease
        // still abandons its complete unpublished call at the boundary.
        let mut state = runtime.0.state.try_borrow_mut().map_err(|_| {
            RuntimeError::Invariant("native preparation state was already borrowed")
        })?;
        let mut owner = NativeStateGuard::new(&mut state, &runtime.0.poisoned, self.into_inner());
        owner.publish(runtime.domain_id(), minimum, true, selected)?;
        Ok(Self::new(runtime, owner.into_inner()))
    }
    /// Overflow construction historically releases inputs while the callee
    /// remains live. Preserve that ordering without creating a public root.
    pub(in crate::engine::vm) fn release_inputs(&mut self) -> Result<(), RuntimeError> {
        self.runtime.check_poison()?;
        let _unwind = self.runtime.unwind_guard();
        let mut state = self.runtime.0.state.borrow_mut();
        self.call
            .as_mut()
            .expect("native call owner")
            .release_inputs(&mut state, &self.runtime.0.poisoned)
    }
    #[cfg(test)]
    pub(in crate::engine::vm) fn finish(
        self,
        result: Result<NativeInvokeOutcome, RuntimeError>,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let runtime = self.runtime;
        self.into_inner()
            .finish_reusing(
                &mut runtime.0.state.borrow_mut(),
                &runtime.0.poisoned,
                result,
            )
            .0
    }
    #[cfg(test)]
    pub(in crate::engine::vm) fn own_continuation(&mut self) -> Result<(), RuntimeError> {
        let call = self.call.as_mut().expect("native call owner");
        call.activation
            .own_continuation(&mut self.runtime.0.state.borrow_mut())
    }
    /// Only a standalone test owner needs to extend the runtime lifetime.
    /// Keep the borrowed guard armed until that lifetime owner exists.
    #[cfg(test)]
    pub(in crate::engine::vm) fn into_standalone(self) -> RootedNativeCall {
        let runtime = self.runtime.clone();
        RootedNativeCall {
            runtime,
            call: Some(self.into_inner()),
        }
    }
    pub(in crate::engine::vm) fn into_inner(mut self) -> PreparedNativeCall {
        self.call.take().expect("native call owner")
    }
}
impl Deref for NativeCallGuard<'_> {
    type Target = PreparedNativeCall;
    fn deref(&self) -> &Self::Target {
        self.call.as_ref().expect("native call owner")
    }
}
impl DerefMut for NativeCallGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.call.as_mut().expect("native call owner")
    }
}
impl Drop for NativeCallGuard<'_> {
    fn drop(&mut self) {
        if let Some(call) = self.call.take() {
            call.abandon_at_boundary(self.runtime);
        }
    }
}

/// A standalone test adapter may outlive the Runtime binding that prepared
/// the call. Production consumers lend the runtime through NativeCallGuard.
#[cfg(test)]
#[must_use]
pub(in crate::engine::vm) struct RootedNativeCall {
    runtime: Runtime,
    call: Option<PreparedNativeCall>,
}
#[cfg(test)]
impl Deref for RootedNativeCall {
    type Target = PreparedNativeCall;
    fn deref(&self) -> &Self::Target {
        self.call.as_ref().expect("native call owner")
    }
}
#[cfg(test)]
impl Drop for RootedNativeCall {
    fn drop(&mut self) {
        if let Some(call) = self.call.take() {
            call.abandon_at_boundary(&self.runtime);
        }
    }
}

/// A held-state call guard owns the complete invocation, argv, callee and
/// frame restore. Splitting short borrows never surrenders cleanup ownership.
#[must_use]
pub(in crate::engine::vm) struct NativeStateGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    call: Option<PreparedNativeCall>,
}
impl<'a> NativeStateGuard<'a> {
    pub(in crate::engine::vm) fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        call: PreparedNativeCall,
    ) -> Self {
        Self {
            state,
            poisoned,
            call: Some(call),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn from_operands(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        function: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        mode: NativeInvokeMode,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self::new(
            state,
            poisoned,
            PreparedNativeCall::from_operands(function, realm, target, mode, invocation, arguments),
        )
    }

    pub(in crate::engine::vm) fn publish(
        &mut self,
        domain: u64,
        minimum: u8,
        continuation: bool,
        selected: Option<&NativeClassification>,
    ) -> Result<(), RuntimeError> {
        if self.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let poisoned = self.poisoned;
        let (state, call) = self.parts();
        let function = call.activation.callable.expect("native callee owner");
        let realm = call.activation.realm;
        let target = call.activation.target;
        let mode = call.activation.mode;
        let proof = match selected {
            Some(selected) => NativeStatePublication::from_classification(
                state, domain, function, realm, target, minimum, mode, selected,
            )?,
            None => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "native_publication_checked",
                );
                NativeStatePublication::validate(state, function, realm, target, minimum, mode)?
            }
        };
        let actual = call.activation.arguments.actual_arg_count;
        let readable = actual.max(usize::from(minimum));
        let before = call.activation.arguments.readable.capacity();
        call.activation
            .arguments
            .readable
            .try_reserve(readable - actual)
            .map_err(|_| RuntimeError::Invariant("native readable arguments allocation failed"))?;
        while call.activation.arguments.readable.len() < readable {
            call.activation.arguments.readable.push(JsValue::Undefined);
        }
        #[cfg(feature = "profiling")]
        {
            use crate::engine::api::profiling::*;
            record_call_buffer_capacity(
                "native.readable",
                before,
                call.activation.arguments.readable.capacity(),
                size_of::<JsValue>(),
            );
            record_call_buffer_observed("native.inbound_argv", before, size_of::<JsValue>());
            record_call_buffer_observed(
                "native.readable",
                call.activation.arguments.readable.capacity(),
                size_of::<JsValue>(),
            );
            record_call_buffer_initialized("native.readable", readable - actual);
        }
        #[cfg(not(feature = "profiling"))]
        let _ = before;
        call.activation.active_frame =
            Some(proof.publish(state, actual, readable, continuation)?);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_activation_prepared");
        let _ = poisoned;
        Ok(())
    }

    pub(in crate::engine::vm) fn parts(&mut self) -> (&mut RuntimeState, &mut PreparedNativeCall) {
        (self.state, self.call.as_mut().expect("native state call"))
    }
    pub(in crate::engine::vm) fn into_inner(mut self) -> PreparedNativeCall {
        self.call.take().expect("native state call")
    }
}
impl Drop for NativeStateGuard<'_> {
    fn drop(&mut self) {
        if let Some(mut call) = self.call.take() {
            call.abandon(self.state, self.poisoned);
        }
    }
}

impl PreparedNativeCall {
    fn from_operands(
        function: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        mode: NativeInvokeMode,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self {
            activation: NativeActivation {
                active_frame: None,
                callable: Some(function),
                realm,
                target,
                mode,
                arguments: NativeArguments {
                    actual_arg_count: arguments.len(),
                    readable: arguments,
                },
            },
            invocation,
        }
    }
    fn release_inputs(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.release_invocation(state, poisoned)?;
        for value in self.activation.arguments.readable.drain(..) {
            state.release_owned_jsvalue(poisoned, value)?;
        }
        Ok(())
    }
    pub(in crate::engine::vm) fn release_invocation(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        std::mem::replace(&mut self.invocation, empty_invocation())
            .release_in_state(state, poisoned)
    }
    pub(in crate::engine::vm) fn finish_reusing(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<NativeInvokeOutcome, RuntimeError>,
    ) -> (Result<NativeInvokeOutcome, RuntimeError>, Vec<JsValue>) {
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(poisoned);
        let PreparedNativeCall {
            activation,
            invocation,
        } = self;
        let (result, arguments) = activation.finish_reusing(state, poisoned, result);
        let released = if poisoned.get() {
            Ok(())
        } else {
            invocation.release_in_state(state, poisoned)
        };
        match released {
            Ok(()) => (result, arguments),
            Err(error) => (Err(error), arguments),
        }
    }
    pub(in crate::engine::vm) fn finish_completion_reusing(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<Completion, RuntimeError>,
    ) -> (Result<Completion, RuntimeError>, Vec<JsValue>) {
        let (result, arguments) =
            self.finish_reusing(state, poisoned, result.map(NativeInvokeOutcome::Completion));
        (
            result.and_then(|result| match result {
                NativeInvokeOutcome::Completion(result) => Ok(result),
                NativeInvokeOutcome::IteratorNextRaw { .. } => Err(RuntimeError::Invariant(
                    "completion finish received raw iterator result",
                )),
            }),
            arguments,
        )
    }
    /// Match the old abandonment order: input, ascending argv, descriptor,
    /// then callee. Destructive failure quarantines before the next owner.
    pub(in crate::engine::vm) fn abandon(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) {
        if skip_cleanup(poisoned) {
            return;
        }
        if self.release_invocation(state, poisoned).is_err() {
            return;
        }
        self.activation.abandon(state, poisoned);
    }
    pub(in crate::engine::vm) fn abandon_at_boundary(mut self, runtime: &Runtime) {
        if runtime.skip_cleanup() {
            return;
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            self.abandon(&mut state, &runtime.0.poisoned);
            return;
        }
        // Standalone external/query teardown may run while another admitted
        // operation owns state. Keep that existing boundary fallback priority.
        let _ = std::mem::replace(&mut self.invocation, empty_invocation()).release(runtime);
        for value in self.activation.arguments.readable.drain(..) {
            let _ = runtime.release_jsvalue(value);
            if runtime.is_poisoned() {
                return;
            }
        }
        if let Some(restore) = self.activation.active_frame.take() {
            runtime.pop_active_frame_fallback(restore.token(), restore.registry_depth());
        }
        if let Some(function) = self.activation.callable.take() {
            let _ = runtime.release_jsvalue(JsValue::Object(function));
        }
    }
}

impl NativeActivation {
    pub(in crate::engine::vm) fn function(&self) -> ObjectId {
        self.callable.expect("native callee owner")
    }
    #[cfg(test)]
    pub(in crate::engine::vm) fn own_continuation(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        let restore = self.active_frame.as_ref().expect("native frame restore");
        state.mark_native_frame_continuation(restore.token(), restore.registry_depth())
    }
    fn abandon(&mut self, state: &mut RuntimeState, poisoned: &Cell<bool>) {
        for value in self.arguments.readable.drain(..) {
            if state.release_owned_jsvalue(poisoned, value).is_err() {
                return;
            }
        }
        if let Some(restore) = self.active_frame.take() {
            state
                .active_frames
                .retire(restore.token(), restore.registry_depth());
        }
        if let Some(function) = self.callable.take() {
            let _ = state.release_owned_jsvalue(poisoned, JsValue::Object(function));
        }
    }
    /// Only unmigrated domain bodies get the legacy callable adapter. The same
    /// callee owner moves out and back; no checked retain or body replay occurs.
    pub(in crate::engine::vm) fn with_legacy_callable<T>(
        &mut self,
        runtime: &Runtime,
        body: impl FnOnce(&CallableRef, &NativeArguments) -> T,
    ) -> T {
        struct LegacyCallee<'a> {
            destination: &'a mut Option<ObjectId>,
            callable: Option<CallableRef>,
        }
        impl Drop for LegacyCallee<'_> {
            fn drop(&mut self) {
                *self.destination = self
                    .callable
                    .take()
                    .map(|v| v.into_object().into_execution_handle());
            }
        }
        let boundary = runtime.clone();
        let function = self.callable.take().expect("native callee owner");
        let owner = LegacyCallee {
            destination: &mut self.callable,
            callable: Some(CallableRef::from_validated_object(
                ObjectRef::from_owned_handle(boundary, function),
            )),
        };
        body(
            owner.callable.as_ref().expect("legacy native callee"),
            &self.arguments,
        )
    }
    pub(in crate::engine::vm) fn finish_reusing(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<NativeInvokeOutcome, RuntimeError>,
    ) -> (Result<NativeInvokeOutcome, RuntimeError>, Vec<JsValue>) {
        self.finish_reusing_with(
            state,
            poisoned,
            result,
            |value| NativeInvokeOutcome::Completion(Completion::Throw(value)),
            |outcome| match outcome {
                NativeInvokeOutcome::Completion(Completion::Return(v) | Completion::Throw(v))
                | NativeInvokeOutcome::IteratorNextRaw { value: v, .. } => v,
            },
        )
    }
    fn finish_reusing_with<T>(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<T, RuntimeError>,
        throw: impl FnOnce(JsValue) -> T,
        into_owned_value: impl FnOnce(T) -> JsValue,
    ) -> (Result<T, RuntimeError>, Vec<JsValue>) {
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(poisoned);
        if skip_cleanup(poisoned) {
            return (Err(RuntimeError::Poisoned), Vec::new());
        }
        // Construct observable engine errors while the native descriptor and
        // defining realm still exist. Existing thrown values pass through.
        let result = match result {
            Err(RuntimeError::Engine(error))
                if NativeErrorKind::from_javascript_error(error.kind()).is_some() =>
            {
                let kind = NativeErrorKind::from_javascript_error(error.kind())
                    .expect("native JavaScript error");
                let message = error
                    .native_message()
                    .cloned()
                    .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
                state
                    .new_native_error_from_message(poisoned, self.realm, kind, message)
                    .map(|id| throw(JsValue::Object(id)))
            }
            other => other,
        };
        let mut result = if poisoned.get() {
            result.and_then(|_| Err(RuntimeError::Poisoned))
        } else {
            match self
                .active_frame
                .take()
                .expect("native frame restore")
                .finish(state)
            {
                Ok(()) => result,
                Err(error) => {
                    if let Ok(value) = result {
                        let _ = state.release_owned_jsvalue(poisoned, into_owned_value(value));
                    }
                    Err(error)
                }
            }
        };
        if !poisoned.get() {
            if let Some(function) = self.callable.take() {
                if let Err(error) = state.release_owned_jsvalue(poisoned, JsValue::Object(function))
                {
                    result = match result {
                        Err(primary) => Err(primary),
                        Ok(_) => Err(error),
                    };
                }
            }
        }
        let mut arguments = std::mem::take(&mut self.arguments.readable);
        if !poisoned.get() {
            for value in arguments.drain(..) {
                if let Err(error) = state.release_owned_jsvalue(poisoned, value) {
                    result = match result {
                        Err(primary) => Err(primary),
                        Ok(_) => Err(error),
                    };
                    break;
                }
            }
        } else {
            arguments.clear();
        }
        (result, arguments)
    }
}

#[cfg(test)]
impl RuntimeState {
    /// Inputs already own their edges; this kernel duplicates neither the
    /// callee nor argv. All padding precedes diagnostic publication.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn prepare_native_call(
        &mut self,
        poisoned: &Cell<bool>,
        domain: u64,
        function: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        minimum: u8,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
        mode: NativeInvokeMode,
        continuation: bool,
        selected: Option<&NativeClassification>,
    ) -> Result<PreparedNativeCall, RuntimeError> {
        let mut owner = NativeStateGuard::from_operands(
            self, poisoned, function, realm, target, mode, invocation, arguments,
        );
        owner.publish(domain, minimum, continuation, selected)?;
        Ok(owner.into_inner())
    }
}
