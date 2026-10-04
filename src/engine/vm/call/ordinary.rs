//! Authenticated direct ordinary calls. No wrapper dispatch or materialized argv.
use crate::engine::{
    api::{Error, runtime::Runtime, runtime_error::RuntimeError},
    code::{
        function::metadata::FunctionKind,
        runtime::{OrdinaryAuthentication, PublishedFunctionSnapshot},
    },
    heap::{FunctionBytecodeId, ObjectId, ObjectPayload, VarRefId},
    object::ObjectRef,
    value::JsValue,
    vm::closure::FrameFunction,
};

mod callback;
pub(in crate::engine::vm) use callback::{CallbackSelection, RawCallbackGuard, RawCallbackInputs};

#[cfg(test)]
use crate::engine::value::Value;

// Only this module can authenticate or construct this witness.
pub(in crate::engine::vm) struct OrdinaryCall {
    function: ObjectId,
    // Callback preparation may outlive its source root.
    // Direct slot calls keep the source operand until its edge is transferred.
    owner: Option<ObjectRef>,
    executable: PublishedFunctionSnapshot,
    closure: std::rc::Rc<[VarRefId]>,
}
// Selection may read metadata but does not publish a frame or consume operands.
// Any malformed metadata error is returned only after the original domain check.
pub(in crate::engine::vm) struct OrdinarySelection {
    function: ObjectId,
    bytecode: FunctionBytecodeId,
    authentication: Option<OrdinaryAuthentication>,
    closure: std::rc::Rc<[VarRefId]>,
}
// Only DirectSelection can create this proof: payload metadata and borrowed
// slot/owner originate from the same heap lookup. Promotion cannot accept a caller's
// detached metadata or an unrelated object.
pub(crate) struct NativeSelection<'a> {
    runtime: &'a Runtime,
    function: ObjectId,
    data: crate::engine::builtins::native::NativeFunctionData,
}
impl<'a> NativeSelection<'a> {
    /// Transfer the same selected payload without another lookup or owner.
    pub(crate) fn into_linked_parts(
        self,
    ) -> (
        u64,
        ObjectId,
        crate::engine::builtins::native::NativeFunctionData,
    ) {
        (self.runtime.domain_id(), self.function, self.data)
    }
}

pub(in crate::engine::vm) enum DirectSelection<'a> {
    Ordinary(OrdinarySelection),
    Native(NativeSelection<'a>),
    General,
}

impl<'a> DirectSelection<'a> {
    /// One payload inspection for ordinary, native and general callees. Any
    /// metadata error is held by the caller until operand domains are checked.
    #[cfg(test)]
    pub(in crate::engine::vm) fn select(
        runtime: &'a Runtime,
        value: &'a Value,
    ) -> Result<Self, RuntimeError> {
        let Value::Object(function) = value else {
            return Ok(Self::General);
        };
        Self::select_object(runtime, function)
    }
    pub(in crate::engine::vm) fn select_object(
        runtime: &'a Runtime,
        function: &'a ObjectRef,
    ) -> Result<Self, RuntimeError> {
        if !function.belongs_to(runtime) {
            return Ok(Self::General);
        }
        Self::select_id(runtime, function.object_id())
    }
    pub(in crate::engine::vm) fn select_jsvalue(
        runtime: &'a Runtime,
        value: &'a JsValue,
    ) -> Result<Self, RuntimeError> {
        let JsValue::Object(function) = value else {
            return Ok(Self::General);
        };
        Self::select_id(runtime, *function)
    }
    fn select_id(runtime: &'a Runtime, function: ObjectId) -> Result<Self, RuntimeError> {
        Self::select_in_state(runtime, &runtime.0.state.borrow(), function)
    }

    pub(in crate::engine::vm) fn select_in_state(
        runtime: &'a Runtime,
        state: &crate::engine::heap::runtime::RuntimeState,
        function: ObjectId,
    ) -> Result<Self, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "direct_callee_payload_selection",
        );
        let object = state.heap.object(function)?;
        if let ObjectPayload::NativeFunction { data, .. } = &object.payload {
            // Unregistered native kinds retain the checked general entry.
            let Some(_) = data.operation() else {
                return Ok(Self::General);
            };
            let defining_realm = data.realm.ok_or(RuntimeError::Invariant(
                "native function was called before its defining realm was attached",
            ))?;
            state.heap.context(defining_realm)?;
            return Ok(Self::Native(NativeSelection {
                runtime,
                function,
                data: *data,
            }));
        }
        let ObjectPayload::BytecodeFunction {
            bytecode,
            closure_slots,
            authentication,
            ..
        } = &object.payload
        else {
            return Ok(Self::General);
        };
        let cached = authentication.borrow();
        let authentication = if let Some(facts) = cached
            .as_ref()
            .filter(|facts| facts.publish_generation == bytecode.publish_generation())
        {
            if closure_slots.len() != facts.closure_count {
                return Err(RuntimeError::Invariant(
                    "function object closure slot count does not match bytecode metadata",
                ));
            }
            Some(facts.clone())
        } else {
            let data = state.heap.function_bytecode(*bytecode)?;
            if data.metadata.function_kind != FunctionKind::Normal {
                return Ok(Self::General);
            }
            if closure_slots.len() != usize::from(data.metadata.closure_count) {
                return Err(RuntimeError::Invariant(
                    "function object closure slot count does not match bytecode metadata",
                ));
            }
            None
        };
        Ok(Self::Ordinary(OrdinarySelection {
            function,
            authentication,
            bytecode: *bytecode,
            closure: std::rc::Rc::clone(closure_slots),
        }))
    }
}

impl OrdinaryCall {
    pub(in crate::engine::vm) fn select_callback(
        runtime: &Runtime,
        function: &ObjectRef,
    ) -> Result<Option<Self>, RuntimeError> {
        match DirectSelection::select_object(runtime, function)? {
            DirectSelection::Ordinary(selected) => selected.authenticate(runtime).map(Some),
            _ => Ok(None),
        }
    }

    #[cfg(test)]
    pub(in crate::engine::vm) fn authenticate(
        runtime: &Runtime,
        value: &Value,
    ) -> Result<Option<Self>, RuntimeError> {
        match DirectSelection::select(runtime, value)? {
            DirectSelection::Ordinary(selected) => selected.authenticate(runtime).map(Some),
            _ => Ok(None),
        }
    }
    pub(in crate::engine::vm) fn executable(&self) -> &PublishedFunctionSnapshot {
        &self.executable
    }
    /// Property callbacks use the same authenticated lazy activation as Call.
    /// Arguments already belong to the callback protocol; zero-argument getters
    /// therefore require no outgoing argument allocation.
    pub(in crate::engine::vm) fn prepare_callback(
        self,
        runtime: &Runtime,
        storage: &mut crate::engine::vm::frame::CallStorage,
        receiver: crate::engine::value::JsValue,
        arguments: Vec<crate::engine::value::JsValue>,
        caller_realm: crate::engine::heap::ContextId,
        return_to: crate::engine::vm::frame::ReturnTarget,
    ) -> Result<crate::engine::vm::frame::FrameEntry, Error> {
        let input = crate::engine::vm::CallInput::new(runtime, receiver, JsValue::Undefined, None);
        let entry =
            self.prepare_input(runtime, storage, input, arguments, caller_realm, return_to)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "property_callback_lazy_install",
        );
        Ok(entry)
    }

    fn prepare_input(
        self,
        runtime: &Runtime,
        storage: &mut crate::engine::vm::frame::CallStorage,
        input: crate::engine::vm::CallInput,
        arguments: Vec<JsValue>,
        caller_realm: crate::engine::heap::ContextId,
        return_to: crate::engine::vm::frame::ReturnTarget,
    ) -> Result<crate::engine::vm::frame::FrameEntry, Error> {
        use crate::engine::vm::{
            frame::{FrameCold, FrameEntry},
            stack::{FrameStorage, FrameStorageGuard},
        };
        let mut input = crate::engine::vm::protocol::CallInputGuard::new(runtime, input);
        let mut frame_storage = FrameStorageGuard::new(
            runtime,
            FrameStorage {
                original_arguments: arguments,
                parameters: Vec::new(),
                locals: Vec::new(),
                operands: Vec::new(),
            },
        );
        let function = self.owner.expect("selected ordinary call owns its callee");
        storage.reserve()?;
        let (flags, flag_bytes) = if self.executable.has_captured_locals {
            storage.capture_flags(self.executable.local_definitions.len())?
        } else {
            (Vec::new(), 0)
        };
        let (cold, frame_bytes) = storage.install(FrameCold {
            rare: std::cell::OnceCell::new(),
            return_to: Some(return_to),
            entry_guard: None,
            function: FrameFunction::shared(
                runtime,
                function.into_execution_handle(),
                self.closure,
            )
            .into(),
            reusable_captured_locals: flags,
            input: input.take().into(),
        });
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_call_storage(
            frame_bytes,
            flag_bytes,
            frame_storage.storage_mut().original_arguments.capacity() * size_of::<JsValue>(),
        );
        #[cfg(not(feature = "profiling"))]
        let _ = (frame_bytes, flag_bytes);
        Ok(FrameEntry {
            property_generation: 0,
            iterator_generation: 0,
            caller_realm,
            active_frame: crate::engine::vm::frames::ActiveFrameToken::unmaterialized(),
            initialize_bindings: true,
            executable: self.executable,
            cold,
            storage: frame_storage.take(),
        })
    }

    pub(in crate::engine::vm) fn install(
        self,
        runtime: &Runtime,
        execution: &mut crate::engine::vm::execution::RunningExecution,
        parent: crate::engine::vm::frame::FrameId,
        checked: crate::engine::vm::stack::CheckedOrdinaryCallOperands,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<(), Error> {
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let mut execution = crate::engine::vm::stack::FrameExecution::admit(execution, parent)?;
        execution.install_ordinary(runtime, &mut state, self, checked, tail, fallthrough)
    }

    /// Consume facts whose callee edge remains in the admitted caller slot.
    pub(in crate::engine::vm) fn into_slot_parts(
        self,
    ) -> (ObjectId, PublishedFunctionSnapshot, std::rc::Rc<[VarRefId]>) {
        debug_assert!(
            self.owner.is_none(),
            "slot installation transfers its source owner"
        );
        (self.function, self.executable, self.closure)
    }
}

impl OrdinarySelection {
    pub(in crate::engine::vm) fn authenticate(
        self,
        runtime: &Runtime,
    ) -> Result<OrdinaryCall, RuntimeError> {
        self.authenticate_impl(runtime, true)
    }

    /// The checked callee slot must stay unchanged until install consumes it.
    pub(in crate::engine::vm) fn authenticate_slot(
        self,
        runtime: &Runtime,
    ) -> Result<OrdinaryCall, RuntimeError> {
        self.authenticate_impl(runtime, false)
    }

    fn authenticate_impl(
        self,
        runtime: &Runtime,
        retain_owner: bool,
    ) -> Result<OrdinaryCall, RuntimeError> {
        let mut call = self.authenticate_slot_in_state(runtime, &runtime.0.state.borrow())?;
        call.owner = retain_owner
            .then(|| ObjectRef::from_borrowed_handle(runtime.clone(), call.function))
            .transpose()?;
        Ok(call)
    }

    /// The original callee operand pins the selected payload and publication
    /// until installation transfers that same edge into the new frame.
    pub(in crate::engine::vm) fn authenticate_slot_in_state(
        self,
        runtime: &Runtime,
        state: &crate::engine::heap::runtime::RuntimeState,
    ) -> Result<OrdinaryCall, RuntimeError> {
        let facts = if let Some(facts) = self.authentication {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_call_auth_cache_hit",
            );
            facts
        } else {
            let facts = state
                .authenticate_ordinary_bytecode(self.bytecode, self.closure.len())?
                .ok_or(RuntimeError::Invariant(
                    "selected ordinary function changed kind",
                ))?;
            let object = state.heap.object(self.function)?;
            let ObjectPayload::BytecodeFunction { authentication, .. } = &object.payload else {
                return Err(RuntimeError::Invariant(
                    "selected ordinary function changed kind",
                ));
            };
            *authentication.borrow_mut() = Some(facts.clone());
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_call_authenticated",
            );
            facts
        };
        Ok(OrdinaryCall {
            function: self.function,
            owner: None,
            closure: self.closure,
            executable: PublishedFunctionSnapshot::from_authentication_in_domain(
                runtime.domain_id(),
                self.bytecode,
                facts,
            ),
        })
    }
}

#[cfg(test)]
mod direct_selection_tests {
    use super::*;

    #[test]
    fn method_receiver_survives_nested_return_and_caught_throw() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval("(function(){var token={};var receiver={mark:41,m:function(arg){var saved=this;try{arg.fail()}catch(error){if(error!==token)return -1}return saved===receiver?this.mark+arg.bump():-2}};var arg={fail:function(){throw token},bump:function(){return 1}};return receiver.m(arg)})()")
            .unwrap();
        assert_eq!(result, Value::Int(42));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn method_receiver_general_and_native_fallback_stay_callable() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for source in [
            "(function(){var o={m:Math.max};return o.m(41,42)===42?42:0})()",
            "(function(){var o={x:42,m:new Proxy(function(){return this.x},{})};return o.m()})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn authentication_cache_is_rootless_and_rejects_a_different_publication() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let first = context.eval("(function(){ return 11 })").unwrap();
        let second = context.eval("(function(){ return 22 })").unwrap();
        let selected = OrdinaryCall::authenticate(&runtime, &first)
            .unwrap()
            .unwrap();
        assert!(selected.executable.root().is_none());
        let id = selected.executable.bytecode_id().unwrap();
        drop(selected);
        let get_facts = |value: &Value| {
            let Value::Object(object) = value else {
                unreachable!()
            };
            let state = runtime.0.state.borrow();
            let ObjectPayload::BytecodeFunction { authentication, .. } =
                &state.heap.object(object.object_id()).unwrap().payload
            else {
                unreachable!()
            };
            authentication.borrow().clone().unwrap()
        };
        let facts = get_facts(&first);
        let warmed = OrdinaryCall::authenticate(&runtime, &first)
            .unwrap()
            .unwrap();
        assert!(std::rc::Rc::ptr_eq(&facts.data, &get_facts(&first).data));
        assert_eq!(warmed.executable.bytecode_id(), Some(id));
        assert!(warmed.executable.root().is_none());
        assert!(warmed.executable.belongs_to(&runtime));
        let foreign = Runtime::new();
        assert!(!warmed.executable.belongs_to(&foreign));
        assert!(matches!(
            warmed.executable.ensure_root(&foreign),
            Err(RuntimeError::WrongRuntime("function bytecode"))
        ));
        warmed.executable.ensure_root(&runtime).unwrap();
        assert_eq!(warmed.executable.root().unwrap().bytecode_id(), id);
        drop(warmed);
        // A transplanted/stale certificate must miss even if closure arity is
        // identical. Publication identity is stronger than code pointer/shape.
        {
            let Value::Object(object) = &second else {
                unreachable!()
            };
            let state = runtime.0.state.borrow();
            let ObjectPayload::BytecodeFunction { authentication, .. } =
                &state.heap.object(object.object_id()).unwrap().payload
            else {
                unreachable!()
            };
            *authentication.borrow_mut() = Some(facts);
        }
        let refreshed = OrdinaryCall::authenticate(&runtime, &second)
            .unwrap()
            .unwrap();
        assert_ne!(refreshed.executable.bytecode_id(), Some(id));
        assert_eq!(
            get_facts(&second).publish_generation,
            refreshed
                .executable
                .bytecode_id()
                .unwrap()
                .publish_generation()
        );
        assert!(!std::rc::Rc::ptr_eq(
            &get_facts(&first).data,
            &get_facts(&second).data
        ));
    }

    #[test]
    fn heap_authentication_cache_does_not_retain_runtime() {
        let weak = {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let value = context.eval("(function(){ return 1 })").unwrap();
            drop(OrdinaryCall::authenticate(&runtime, &value).unwrap());
            std::rc::Rc::downgrade(&runtime.0)
        };
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn direct_selection_borrows_owners_and_preserves_general_fallback() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for (source, kind) in [
            ("(function(x){return x})", 0),
            ("Math.min", 1),
            ("(function(x){return x}).bind(null)", 2),
            ("new Proxy(function(){},{})", 2),
            ("new Proxy({},{})", 2),
            ("(function*(){})", 2),
            ("({})", 2),
            ("17", 2),
        ] {
            let value = context.eval(source).unwrap();
            let owners = std::rc::Rc::strong_count(&runtime.0);
            let selected = DirectSelection::select(&runtime, &value).unwrap();
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners, "{source}");
            assert_eq!(
                match selected {
                    DirectSelection::Ordinary(_) => 0,
                    DirectSelection::Native(_) => 1,
                    DirectSelection::General => 2,
                },
                kind,
                "{source}"
            );
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners, "{source}");
        }
        let foreign = Runtime::new();
        let value = Value::Object(foreign.new_object(None).unwrap());
        assert!(matches!(
            DirectSelection::select(&runtime, &value).unwrap(),
            DirectSelection::General
        ));
    }
}
