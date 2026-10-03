mod active;
mod native_state;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
pub(crate) use active::ActiveFrames;
pub(in crate::engine::vm) use native_state::NativeStatePublication;

use crate::engine::builtins::native::{NativeCProto, NativeFunctionId};
use crate::engine::code::function::metadata::EvalKind;
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::runtime::DeferredRefOp;
use crate::source::LineColumn;

use crate::engine::heap::{ContextId, FunctionBytecodeId, ObjectId, ObjectPayload};
use crate::engine::object::ObjectRef;
use crate::engine::value::JsString;
use crate::engine::vm::BytecodePc;

/// Validated facts from direct native classification. The selected callable
/// owner keeps the fixed payload and defining realm live; during direct entry
/// that owner stays in the authenticated callee slot until it is transferred.
/// This record adds no root. Reuse checks callable identity and runtime domain.
pub(in crate::engine::vm) struct NativeClassification {
    function: ObjectId,
    domain: u64,
    data: crate::engine::builtins::native::NativeFunctionData,
    operation: Option<crate::engine::builtins::continuation::NativeOperation>,
}

impl NativeClassification {
    /// The authenticated caller transaction keeps its callee slot unchanged
    /// until it transfers that owner into the activation's CallableRef.
    pub(in crate::engine::vm) fn classify_selected(
        selection: super::call::ordinary::NativeSelection<'_>,
    ) -> Self {
        let (domain, function, data) = selection.into_linked_parts();
        Self {
            function,
            domain,
            operation: data.operation(),
            data,
        }
    }

    pub(in crate::engine::vm) fn classify_linked(
        runtime: &Runtime,
        selection: crate::engine::object::LinkedNativeSelection,
        value: &crate::engine::value::JsValue,
    ) -> Option<Self> {
        let crate::engine::value::JsValue::Object(function) = value else {
            return None;
        };
        let data = selection.into_parts_jsvalue(runtime, *function)?;
        Some(Self {
            function: *function,
            domain: runtime.domain_id(),
            operation: data.operation(),
            data,
        })
    }

    pub(in crate::engine::vm) fn select(
        runtime: &Runtime,
        callable: &crate::engine::object::CallableRef,
    ) -> Result<Option<Self>, RuntimeError> {
        let _operation = runtime.operation()?;
        if !callable.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("callable"));
        }
        let state = runtime.0.state.borrow();
        let object = state.heap.object(callable.as_object().object_id())?;
        let ObjectPayload::NativeFunction { data, .. } = &object.payload else {
            return Ok(None);
        };
        let defining_realm = data.realm.ok_or(RuntimeError::Invariant(
            "native function was called before its defining realm was attached",
        ))?;
        state.heap.context(defining_realm)?;
        let data = *data;
        let operation = data.operation();
        drop(state);
        Ok(Some(Self {
            function: callable.as_object().object_id(),
            domain: runtime.domain_id(),
            data,
            operation,
        }))
    }
    pub(crate) fn into_linked_parts(
        self,
    ) -> (
        u64,
        ObjectId,
        crate::engine::builtins::native::NativeFunctionData,
    ) {
        (self.domain, self.function, self.data)
    }
    pub(in crate::engine::vm) fn take_operation(
        &mut self,
    ) -> Option<crate::engine::builtins::continuation::NativeOperation> {
        self.operation.take()
    }
    pub(in crate::engine::vm) fn target(&self) -> NativeFunctionId {
        self.data.target
    }
    pub(in crate::engine::vm) fn defining_realm(&self) -> ContextId {
        self.data.realm.expect("selected native realm")
    }
    pub(in crate::engine::vm) fn minimum(&self) -> u8 {
        self.data.min_readable_args
    }
}

/// Read the sealed dispatch fact for a previously normalized callable. Native
/// publication owns the derivation; callers do not classify its target again.
pub(in crate::engine::vm) fn native_operation(
    runtime: &Runtime,
    callable: &crate::engine::object::CallableRef,
) -> Result<Option<crate::engine::builtins::continuation::NativeOperation>, RuntimeError> {
    if !callable.belongs_to(runtime) {
        return Err(RuntimeError::WrongRuntime("callable"));
    }
    native_operation_in_state(&runtime.0.state.borrow(), callable.as_object().object_id())
}

pub(in crate::engine::vm) fn native_operation_in_state(
    state: &crate::engine::heap::runtime::RuntimeState,
    function: ObjectId,
) -> Result<Option<crate::engine::builtins::continuation::NativeOperation>, RuntimeError> {
    let object = state.heap.object(function)?;
    Ok(match &object.payload {
        ObjectPayload::NativeFunction { data, .. } => data.operation(),
        _ => None,
    })
}

/// The external adapter borrows the public root until the shared publication
/// proof is consumed. Internal callers pin the same edge in execution storage.
pub(in crate::engine::vm) struct NativePublicationWitness<'a> {
    runtime: &'a Runtime,
    _callable: &'a crate::engine::object::CallableRef,
    proof: NativeStatePublication,
}

impl<'a> NativePublicationWitness<'a> {
    pub(in crate::engine::vm) fn validate(
        runtime: &'a Runtime,
        callable: &'a crate::engine::object::CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        mode: super::call::NativeInvokeMode,
    ) -> Result<Self, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("native_publication_checked");
        if !callable.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("native callable"));
        }
        let proof = NativeStatePublication::validate(
            &runtime.0.state.borrow(),
            callable.as_object().object_id(),
            realm,
            target,
            min_readable_args,
            mode,
        )?;
        Ok(Self {
            runtime,
            _callable: callable,
            proof,
        })
    }

    pub(in crate::engine::vm) fn from_classification(
        runtime: &'a Runtime,
        callable: &'a crate::engine::object::CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        mode: super::call::NativeInvokeMode,
        selected: &NativeClassification,
    ) -> Result<Self, RuntimeError> {
        if !callable.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("native callable"));
        }
        let proof = NativeStatePublication::from_classification(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            callable.as_object().object_id(),
            realm,
            target,
            min_readable_args,
            mode,
            selected,
        )?;
        Ok(Self {
            runtime,
            _callable: callable,
            proof,
        })
    }

    pub(in crate::engine::vm) fn publish(
        self,
        actual_arg_count: usize,
        readable_arg_count: usize,
        continuation: bool,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        let restore = self.proof.publish(
            &mut self.runtime.0.state.borrow_mut(),
            actual_arg_count,
            readable_arg_count,
            continuation,
        )?;
        Ok(ActiveFrameGuard {
            runtime: self.runtime.clone(),
            token: restore.token,
            depth: restore.depth,
            active: true,
            _function_root: None,
            _bytecode_root: None,
        })
    }
}

impl Runtime {
    pub(crate) fn push_active_collection_record(
        &self,
        record: ActiveCollectionRecord,
    ) -> ActiveCollectionRecordGuard {
        let depth = {
            let mut state = self.0.state.borrow_mut();
            let depth = state.active_collection_records.len();
            state.active_collection_records.push(record);
            depth
        };
        ActiveCollectionRecordGuard {
            runtime: self.clone(),
            record,
            depth,
            active: true,
        }
    }

    pub(crate) fn push_active_frame(
        &self,
        function_root: ObjectRef,
        bytecode_root: Option<FunctionBytecodeRef>,
        realm: ContextId,
        flags: ActiveFrameFlags,
        kind: ActiveFrameKind,
        native_iterator_next_fast_path: bool,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        self.push_active_frame_with_continuation(
            function_root,
            bytecode_root,
            realm,
            flags,
            kind,
            native_iterator_next_fast_path,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn push_active_frame_with_continuation(
        &self,
        function_root: ObjectRef,
        bytecode_root: Option<FunctionBytecodeRef>,
        realm: ContextId,
        flags: ActiveFrameFlags,
        kind: ActiveFrameKind,
        native_iterator_next_fast_path: bool,
        native_continuation: bool,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        if !function_root.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("active-frame function"));
        }
        if bytecode_root
            .as_ref()
            .is_some_and(|root| !root.belongs_to(self))
        {
            return Err(RuntimeError::WrongRuntime("active-frame bytecode"));
        }

        {
            let state = self.0.state.borrow_mut();
            state.heap.context(realm)?;
            let object = state.heap.object(function_root.object_id())?;
            match (kind, &object.payload, bytecode_root.as_ref()) {
                (
                    ActiveFrameKind::Bytecode { bytecode, .. },
                    ObjectPayload::BytecodeFunction {
                        bytecode: object_bytecode,
                        ..
                    },
                    Some(root),
                ) if *object_bytecode == bytecode && root.bytecode_id() == bytecode => {
                    if state.heap.function_bytecode(bytecode)?.realm != realm {
                        return Err(RuntimeError::Invariant(
                            "bytecode active frame realm disagrees with its bytecode",
                        ));
                    }
                }
                (
                    ActiveFrameKind::Native {
                        target,
                        actual_arg_count,
                        readable_arg_count,
                    },
                    ObjectPayload::NativeFunction { data, .. },
                    None,
                ) if data.target == target
                    && data.realm.is_some_and(|defining_realm| {
                        target.uses_calling_realm()
                            || defining_realm == realm
                            || (native_iterator_next_fast_path
                                && target.descriptor().cproto == NativeCProto::IteratorNext)
                    })
                    && readable_arg_count
                        == actual_arg_count.max(usize::from(data.min_readable_args)) => {}
                (ActiveFrameKind::Bytecode { .. }, _, _) => {
                    return Err(RuntimeError::Invariant(
                        "bytecode active frame disagrees with its rooted callable",
                    ));
                }
                (ActiveFrameKind::Native { .. }, _, _) => {
                    return Err(RuntimeError::Invariant(
                        "native active frame disagrees with its rooted callable",
                    ));
                }
            }
        }
        self.publish_validated_active_frame(
            function_root,
            bytecode_root,
            realm,
            flags,
            kind,
            native_continuation,
        )
    }

    // Only the checked entry above and a consumed native witness reach this
    // publication kernel. All token, ownership and accounting order stays shared.
    #[allow(clippy::too_many_arguments)]
    fn publish_validated_active_frame(
        &self,
        function_root: ObjectRef,
        bytecode_root: Option<FunctionBytecodeRef>,
        realm: ContextId,
        flags: ActiveFrameFlags,
        kind: ActiveFrameKind,
        native_continuation: bool,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        let restore = self.0.state.borrow_mut().publish_validated_active_frame(
            function_root.object_id(),
            realm,
            flags,
            kind,
            native_continuation,
            false,
        )?;

        Ok(ActiveFrameGuard {
            runtime: self.clone(),
            token: restore.token,
            depth: restore.depth,
            active: true,
            _function_root: Some(function_root),
            _bytecode_root: bytecode_root,
        })
    }

    pub(crate) fn push_bytecode_active_frame(
        &self,
        function_root: ObjectRef,
        bytecode_root: FunctionBytecodeRef,
        realm: ContextId,
        strict: bool,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        let bytecode = bytecode_root.bytecode_id();
        self.push_active_frame(
            function_root,
            Some(bytecode_root),
            realm,
            ActiveFrameFlags {
                strict,
                ..ActiveFrameFlags::default()
            },
            ActiveFrameKind::Bytecode { bytecode, pc: None },
            false,
        )
    }

    pub(crate) fn push_native_active_frame(
        &self,
        function_root: ObjectRef,
        realm: ContextId,
        target: NativeFunctionId,
        actual_arg_count: usize,
        readable_arg_count: usize,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        self.push_active_frame(
            function_root,
            None,
            realm,
            ActiveFrameFlags::default(),
            ActiveFrameKind::Native {
                target,
                actual_arg_count,
                readable_arg_count,
            },
            false,
        )
    }

    pub(crate) fn update_active_bytecode_pc(
        &self,
        token: ActiveFrameToken,
        pc: BytecodePc,
    ) -> Result<(), RuntimeError> {
        self.0
            .state
            .borrow_mut()
            .update_active_bytecode_pc(token, pc)
    }

    /// Return the debug name of the active Script or Module, mirroring
    /// QuickJS `JS_GetScriptOrModuleName(ctx, 0)`.
    ///
    /// Synthetic direct and indirect eval roots inherit the first enclosing
    /// non-eval bytecode name. A visible native frame is a hard host boundary;
    /// internal hidden native frames are absent from the observable stack and
    /// are skipped. Backtrace barriers deliberately do not participate in
    /// this lookup.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn active_script_or_module_name(&self) -> Result<Option<JsString>, RuntimeError> {
        let state = self.0.state.borrow();
        for frame in state.active_frames.iter().rev() {
            if frame.flags.backtrace_hidden {
                continue;
            }
            let ActiveFrameKind::Bytecode { bytecode, .. } = frame.kind else {
                return Ok(None);
            };
            let bytecode = state.heap.function_bytecode(bytecode)?;
            if bytecode.metadata.eval_kind != EvalKind::None {
                continue;
            }
            let Some(debug) = &bytecode.debug else {
                return Ok(None);
            };
            return Ok(Some(state.atoms.to_js_string(debug.filename)?));
        }
        Ok(None)
    }

    /// QuickJS `JS_EVAL_FLAG_BACKTRACE_BARRIER` temporarily marks the frame
    /// which existed before eval begins. New eval/nested frames remain visible
    /// and stack traversal stops before printing this caller frame.
    pub(crate) fn install_backtrace_barrier(
        &self,
        enabled: bool,
    ) -> Result<BacktraceBarrierGuard, RuntimeError> {
        if !enabled {
            return Ok(BacktraceBarrierGuard {
                runtime: self.clone(),
                token: None,
                previous: false,
                active: true,
            });
        }
        let (token, previous) = {
            let mut state = self.0.state.borrow_mut();
            let Some(frame) = state.active_frames.last_mut() else {
                return Ok(BacktraceBarrierGuard {
                    runtime: self.clone(),
                    token: None,
                    previous: false,
                    active: true,
                });
            };
            let previous = frame.flags.backtrace_barrier;
            frame.flags.backtrace_barrier = true;
            (frame.token, previous)
        };
        Ok(BacktraceBarrierGuard {
            runtime: self.clone(),
            token: Some(token),
            previous,
            active: true,
        })
    }

    pub(crate) fn restore_backtrace_barrier(
        &self,
        token: ActiveFrameToken,
        previous: bool,
    ) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let frame = state
            .active_frames
            .iter_mut()
            .find(|frame| frame.token == token)
            .ok_or(RuntimeError::Invariant(
                "backtrace-barrier caller frame disappeared during eval",
            ))?;
        frame.flags.backtrace_barrier = previous;
        Ok(())
    }

    pub(crate) fn restore_backtrace_barrier_fallback(
        &self,
        token: ActiveFrameToken,
        previous: bool,
    ) {
        if self.skip_cleanup() {
            return;
        }
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            if let Some(frame) = state
                .active_frames
                .iter_mut()
                .find(|frame| frame.token == token)
            {
                frame.flags.backtrace_barrier = previous;
            }
        } else {
            self.0
                .deferred_references
                .push_front(DeferredRefOp::BacktraceBarrierRestore { token, previous });
        }
    }

    pub(crate) fn pop_active_collection_record(
        &self,
        record: ActiveCollectionRecord,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        if state.active_collection_records.len() == depth + 1
            && state.active_collection_records.last() == Some(&record)
        {
            state.active_collection_records.pop();
            return Ok(());
        }

        state.active_collection_records.truncate(depth);
        Err(RuntimeError::Invariant(
            "active collection record stack was not restored in LIFO order",
        ))
    }

    pub(crate) fn pop_active_collection_record_fallback(&self, depth: usize) {
        if self.skip_cleanup() {
            return;
        }
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            state.active_collection_records.truncate(depth);
        } else {
            self.0
                .deferred_references
                .push_front(DeferredRefOp::ActiveCollectionRecordsTruncate { depth });
        }
    }

    pub(crate) fn pop_active_frame(
        &self,
        token: ActiveFrameToken,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        self.0.state.borrow_mut().pop_active_frame(token, depth)
    }

    pub(crate) fn pop_active_frame_fallback(&self, token: ActiveFrameToken, depth: usize) {
        if self.skip_cleanup() {
            return;
        }
        if let Ok(mut state) = self.0.state.try_borrow_mut() {
            state.active_frames.retire(token, depth);
        } else {
            self.0
                .deferred_references
                .push_front(DeferredRefOp::ActiveFramePop { token, depth });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActiveCollectionRecord {
    Map { object: ObjectId, index: usize },
    Set { object: ObjectId, index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveFrameRecord {
    /// The native algorithm is an owned continuation, with no suspended Rust body.
    pub(crate) native_continuation: bool,
    pub(crate) token: ActiveFrameToken,
    pub(crate) function: ObjectId,
    pub(crate) realm: ContextId,
    pub(crate) flags: ActiveFrameFlags,
    pub(crate) kind: ActiveFrameKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveFrameToken(pub(in crate::engine::vm) u64);
impl ActiveFrameToken {
    pub(in crate::engine::vm) const fn unmaterialized() -> Self {
        Self(0)
    }

    pub(in crate::engine::vm) fn is_materialized(self) -> bool {
        self.0 != 0
    }
}

/// Flags which belong to a QuickJS stack frame rather than to the callable
/// heap object. Raw IteratorNext dispatch keeps a rooted validation frame but
/// hides it from JavaScript backtraces because QuickJS calls that native
/// function pointer without pushing a visible `JSStackFrame`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ActiveFrameFlags {
    pub(crate) strict: bool,
    pub(crate) is_async: bool,
    pub(crate) backtrace_barrier: bool,
    pub(crate) backtrace_hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActiveFrameKind {
    Bytecode {
        bytecode: FunctionBytecodeId,
        pc: Option<BytecodePc>,
    },
    Native {
        target: NativeFunctionId,
        actual_arg_count: usize,
        readable_arg_count: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ExplicitBacktraceLocation {
    pub(crate) filename: JsString,
    pub(crate) position: LineColumn,
}

/// Stack-owned root set and LIFO token for one active execution frame.
///
/// Normal execution calls [`Self::finish`] so token/order corruption becomes
/// an engine error. `Drop` is a no-fail fallback for unwinding paths and keeps
/// stale diagnostic frames from escaping their invocation.
pub(crate) struct ActiveFrameGuard {
    pub(crate) runtime: Runtime,
    pub(crate) token: ActiveFrameToken,
    pub(crate) depth: usize,
    pub(crate) active: bool,
    pub(crate) _function_root: Option<ObjectRef>,
    pub(crate) _bytecode_root: Option<FunctionBytecodeRef>,
}

/// Internal frame restoration owns explicit heap edges, never Runtime. The
/// executing state, or its weak execution/suspension fallback, consumes it.
pub(in crate::engine::vm) struct ActiveFrameRestore {
    token: ActiveFrameToken,
    depth: usize,
    function: Option<ObjectId>,
    bytecode: Option<FunctionBytecodeId>,
}
impl ActiveFrameRestore {
    pub(in crate::engine::vm) const fn token(&self) -> ActiveFrameToken {
        self.token
    }
    pub(in crate::engine::vm) fn registry_depth(&self) -> usize {
        self.depth
    }
    pub(in crate::engine::vm) fn finish(
        mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
    ) -> Result<(), RuntimeError> {
        let restored = state.pop_active_frame(self.token, self.depth);
        if let Some(function) = self.function.take() {
            state.release_object_handle(function)?;
        }
        if let Some(bytecode) = self.bytecode.take() {
            state.release_function_bytecode_handle(bytecode)?;
        }
        restored
    }
}

/// LIFO scope for one Map/Set record exposed to QuickJS-style diagnostics.
///
/// Normal execution calls [`Self::finish`] so stack corruption becomes an
/// engine error. `Drop` truncates the scope suffix during unwinding, ensuring
/// a panicking callback cannot leave a stale record visible to later prints.
pub(crate) struct ActiveCollectionRecordGuard {
    pub(crate) runtime: Runtime,
    pub(crate) record: ActiveCollectionRecord,
    pub(crate) depth: usize,
    pub(crate) active: bool,
}

pub(crate) struct BacktraceBarrierGuard {
    pub(crate) runtime: Runtime,
    pub(crate) token: Option<ActiveFrameToken>,
    pub(crate) previous: bool,
    pub(crate) active: bool,
}

impl ActiveFrameGuard {
    pub(in crate::engine::vm) fn into_internal(mut self) -> ActiveFrameRestore {
        self.active = false;
        ActiveFrameRestore {
            token: self.token,
            depth: self.depth,
            function: self
                ._function_root
                .take()
                .map(ObjectRef::into_execution_handle),
            bytecode: self
                ._bytecode_root
                .take()
                .map(FunctionBytecodeRef::into_execution_handle),
        }
    }
    pub(crate) const fn token(&self) -> ActiveFrameToken {
        self.token
    }

    pub(crate) fn finish(mut self) -> Result<(), RuntimeError> {
        let result = self.runtime.pop_active_frame(self.token, self.depth);
        self.active = false;
        result
    }
}

impl Drop for ActiveFrameGuard {
    fn drop(&mut self) {
        if self.active {
            self.runtime
                .pop_active_frame_fallback(self.token, self.depth);
            self.active = false;
        }
    }
}

impl ActiveCollectionRecordGuard {
    pub(crate) fn finish(mut self) -> Result<(), RuntimeError> {
        let result = self
            .runtime
            .pop_active_collection_record(self.record, self.depth);
        self.active = false;
        result
    }
}

impl Drop for ActiveCollectionRecordGuard {
    fn drop(&mut self) {
        if self.active {
            self.runtime
                .pop_active_collection_record_fallback(self.depth);
            self.active = false;
        }
    }
}

impl BacktraceBarrierGuard {
    pub(crate) fn finish(mut self) -> Result<(), RuntimeError> {
        if let Some(token) = self.token {
            self.runtime
                .restore_backtrace_barrier(token, self.previous)?;
        }
        self.active = false;
        Ok(())
    }
}

impl Drop for BacktraceBarrierGuard {
    fn drop(&mut self) {
        if self.active {
            if let Some(token) = self.token {
                self.runtime
                    .restore_backtrace_barrier_fallback(token, self.previous);
            }
            self.active = false;
        }
    }
}

impl crate::engine::heap::runtime::RuntimeState {
    /// Called only by FrameStore's observation protocol. Frame owners retain
    /// the function and immutable executable until this restore is retired.
    pub(in crate::engine::vm) fn materialize_owned_frame(
        &mut self,
        frame: &super::frame::Frame,
    ) -> Result<ActiveFrameRestore, RuntimeError> {
        let token = ActiveFrameToken(self.next_active_frame_token);
        self.next_active_frame_token = token.0.checked_add(1).ok_or(RuntimeError::Invariant(
            "active-frame token space was exhausted",
        ))?;
        let depth = self.active_frames.len();
        self.active_frames.push(ActiveFrameRecord {
            token,
            native_continuation: false,
            function: frame.cold.function.object_id(),
            realm: frame.executable.realm,
            flags: ActiveFrameFlags {
                strict: frame.executable.metadata.strict,
                ..Default::default()
            },
            kind: ActiveFrameKind::Bytecode {
                bytecode: frame
                    .executable
                    .bytecode_id()
                    .ok_or(RuntimeError::Invariant(
                        "owned frame has no bytecode identity",
                    ))?,
                pc: Some(BytecodePc::new(frame.fault_pc)),
            },
        });
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("lazy_frame_materialized");
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("frame_materialization_new");
        Ok(ActiveFrameRestore {
            token,
            depth,
            function: None,
            bytecode: None,
        })
    }

    pub(in crate::engine::vm) fn publish_materialized_pc(
        &mut self,
        token: ActiveFrameToken,
        depth: Option<usize>,
        pc: BytecodePc,
    ) -> Result<(), RuntimeError> {
        let index = depth
            .or_else(|| self.active_frames.iter().rposition(|f| f.token == token))
            .ok_or(RuntimeError::Invariant("materialized frame is absent"))?;
        let frame = self
            .active_frames
            .get_mut(index)
            .filter(|f| f.token == token)
            .ok_or(RuntimeError::Invariant(
                "materialized frame identity changed",
            ))?;
        let ActiveFrameKind::Bytecode { pc: stored, .. } = &mut frame.kind else {
            return Err(RuntimeError::Invariant(
                "materialized PC targets a native frame",
            ));
        };
        if *stored == Some(pc) {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "runtime_pc_publication_repeated",
            );
            return Ok(());
        }
        *stored = Some(pc);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("runtime_pc_publication");
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "runtime_pc_publication_materialized",
        );
        Ok(())
    }

    pub(crate) fn update_active_bytecode_pc(
        &mut self,
        token: ActiveFrameToken,
        pc: BytecodePc,
    ) -> Result<(), RuntimeError> {
        let frame = self
            .active_frames
            .last_mut()
            .ok_or(RuntimeError::Invariant(
                "bytecode PC update ran without an active frame",
            ))?;
        if frame.token != token {
            return Err(RuntimeError::Invariant(
                "bytecode PC update did not target the top active frame",
            ));
        }
        let ActiveFrameKind::Bytecode { pc: frame_pc, .. } = &mut frame.kind else {
            return Err(RuntimeError::Invariant(
                "bytecode PC update targeted a native active frame",
            ));
        };
        if *frame_pc == Some(pc) {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "runtime_pc_publication_repeated",
            );
            return Ok(());
        }
        *frame_pc = Some(pc);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("runtime_pc_publication");
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "runtime_pc_publication_active",
        );
        Ok(())
    }
}
