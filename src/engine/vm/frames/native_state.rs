//! Shared native diagnostic publication. The caller owns the function edge
//! throughout this no-callback interval; this proof adds no root.
use super::{
    ActiveFrameFlags, ActiveFrameKind, ActiveFrameRecord, ActiveFrameRestore, ActiveFrameToken,
    NativeClassification,
};
use crate::engine::{
    api::runtime_error::RuntimeError,
    builtins::native::{NativeCProto, NativeFunctionId},
    heap::{ContextId, ObjectId, ObjectPayload, runtime::RuntimeState},
    vm::call::NativeInvokeMode,
};

pub(in crate::engine::vm) struct NativeStatePublication {
    function: ObjectId,
    realm: ContextId,
    target: NativeFunctionId,
    min_readable_args: u8,
    iterator_next_raw: bool,
    realm_allowed: bool,
}

impl NativeStatePublication {
    pub(in crate::engine::vm) fn validate(
        state: &RuntimeState,
        function: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        mode: NativeInvokeMode,
    ) -> Result<Self, RuntimeError> {
        state.heap.context(realm)?;
        let object = state.heap.object(function)?;
        let ObjectPayload::NativeFunction { data, .. } = &object.payload else {
            return Err(RuntimeError::Invariant(
                "native invocation target was not a native function",
            ));
        };
        let defining_realm = data.realm.ok_or(RuntimeError::Invariant(
            "native function lost its defining realm",
        ))?;
        if data.target != target
            || (matches!(mode, NativeInvokeMode::Ordinary)
                && !target.uses_calling_realm()
                && defining_realm != realm)
            || data.min_readable_args != min_readable_args
        {
            return Err(RuntimeError::Invariant(
                "native invocation metadata changed after snapshot",
            ));
        }
        if defining_realm != realm {
            state.heap.context(defining_realm)?;
        }
        Ok(Self {
            function,
            realm,
            target,
            min_readable_args,
            iterator_next_raw: matches!(mode, NativeInvokeMode::IteratorNextRaw),
            realm_allowed: defining_realm == realm
                || target.uses_calling_realm()
                || target.descriptor().cproto == NativeCProto::IteratorNext,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn from_classification(
        state: &RuntimeState,
        domain: u64,
        function: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        mode: NativeInvokeMode,
        selected: &NativeClassification,
    ) -> Result<Self, RuntimeError> {
        if selected.domain != domain
            || selected.function != function
            || selected.target() != target
            || selected.minimum() != min_readable_args
            || (matches!(mode, NativeInvokeMode::Ordinary)
                && !target.uses_calling_realm()
                && realm != selected.defining_realm())
        {
            return Err(RuntimeError::Invariant(
                "native invocation metadata changed after snapshot",
            ));
        }
        if realm != selected.defining_realm() {
            state.heap.context(realm)?;
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_classification_reused");
        Ok(Self {
            function,
            realm,
            target,
            min_readable_args,
            iterator_next_raw: matches!(mode, NativeInvokeMode::IteratorNextRaw),
            realm_allowed: realm == selected.defining_realm()
                || target.uses_calling_realm()
                || target.descriptor().cproto == NativeCProto::IteratorNext,
        })
    }

    pub(in crate::engine::vm) fn publish(
        self,
        state: &mut RuntimeState,
        actual_arg_count: usize,
        readable_arg_count: usize,
        continuation: bool,
    ) -> Result<ActiveFrameRestore, RuntimeError> {
        if !self.realm_allowed
            || readable_arg_count != actual_arg_count.max(usize::from(self.min_readable_args))
        {
            return Err(RuntimeError::Invariant(
                "native active frame disagrees with its rooted callable",
            ));
        }
        // Raw IteratorNext and Promise resolving class calls do not expose a
        // QuickJS JSStackFrame in guest backtraces.
        let hidden =
            self.iterator_next_raw || matches!(self.target, NativeFunctionId::PromiseResolving(_));
        state.publish_validated_active_frame(
            self.function,
            self.realm,
            ActiveFrameFlags {
                backtrace_hidden: hidden,
                ..Default::default()
            },
            ActiveFrameKind::Native {
                target: self.target,
                actual_arg_count,
                readable_arg_count,
            },
            continuation,
            true,
        )
    }
}

impl RuntimeState {
    /// Publication uses the original scheduling fact: native invocation proofs
    /// keep a lazy tail; generic rooted frame entry eagerly publishes its record.
    /// Validation and owned function lifetime precede this no-callback kernel.
    pub(in crate::engine::vm) fn publish_validated_active_frame(
        &mut self,
        function: ObjectId,
        realm: ContextId,
        flags: ActiveFrameFlags,
        kind: ActiveFrameKind,
        native_continuation: bool,
        lazy_native: bool,
    ) -> Result<ActiveFrameRestore, RuntimeError> {
        let token = ActiveFrameToken(self.next_active_frame_token);
        self.next_active_frame_token =
            self.next_active_frame_token
                .checked_add(1)
                .ok_or(RuntimeError::Invariant(
                    "active-frame token space was exhausted",
                ))?;
        let depth = self.active_frames.len();
        let record = ActiveFrameRecord {
            token,
            native_continuation,
            function,
            realm,
            flags,
            kind,
        };
        if lazy_native {
            self.active_frames.push_lazy_native(record);
        } else {
            self.active_frames.push(record);
        }
        Ok(ActiveFrameRestore {
            token,
            depth,
            function: None,
            bytecode: None,
        })
    }

    pub(crate) fn pop_active_frame(
        &mut self,
        token: ActiveFrameToken,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if self.active_frames.len() == depth + 1
            && self.active_frames.last().map(|frame| frame.token) == Some(token)
        {
            self.active_frames.pop();
            return Ok(());
        }
        self.active_frames.retire(token, depth);
        Err(RuntimeError::Invariant(
            "active frame stack was not restored in LIFO order",
        ))
    }

    #[cfg(test)]
    pub(in crate::engine::vm) fn mark_native_frame_continuation(
        &mut self,
        token: ActiveFrameToken,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let frame = self
            .active_frames
            .get_mut(depth)
            .filter(|frame| {
                frame.token == token && matches!(frame.kind, ActiveFrameKind::Native { .. })
            })
            .ok_or(RuntimeError::Invariant(
                "native continuation has no matching active frame",
            ))?;
        if frame.native_continuation {
            return Err(RuntimeError::Invariant(
                "native continuation was registered twice",
            ));
        }
        self.active_frames.mark_native_continuation(depth, token);
        Ok(())
    }
}
