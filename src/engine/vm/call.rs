pub(super) mod ordinary;

mod protocol;

mod request;

pub(in crate::engine::vm) use request::{
    BytecodeCallRequest, NormalizedCallback, normalize_callback,
};

mod native;

pub(in crate::engine::vm) use native::PreparedNativeCall;

pub(in crate::engine::vm) mod prepare;
pub(crate) mod prototype;

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::builtins::native::{NativeCProto, NativeFunctionId};
use crate::engine::code::function::metadata::ConstructorKind;
use crate::engine::code::rooted::FunctionBytecodeRef;

use crate::engine::heap::{ContextId, ObjectPayload};
use crate::engine::object::{CallableRef, ObjectRef};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsValue, Value};
use crate::engine::vm::Completion;

impl Runtime {
    pub(crate) fn bytecode_for_callable(
        &self,
        callable: &CallableRef,
    ) -> Result<CallableExecution, RuntimeError> {
        let _operation = self.operation();
        if !callable.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("callable"));
        }
        let (bytecode, closure_slots) = {
            let state = self.0.state.borrow();
            let object = state.heap.object(callable.as_object().object_id())?;
            match &object.payload {
                ObjectPayload::NativeFunction { data, .. } => {
                    let realm = data.realm.ok_or(RuntimeError::Invariant(
                        "native function was called before its defining realm was attached",
                    ))?;
                    state.heap.context(realm)?;
                    return Ok(CallableExecution::Native {
                        target: data.target,
                        realm,
                        min_readable_args: data.min_readable_args,
                    });
                }
                ObjectPayload::BoundFunction {
                    target,
                    this_value,
                    arguments,
                } => {
                    let target = *target;
                    let this_value = this_value.clone();
                    let arguments = arguments.clone();
                    #[cfg(feature = "profiling")]
                    {
                        // Cloning this Rc slice shares storage: no raw element
                        // copy, new allocation, or root retain is inferred.
                        crate::engine::api::profiling::record_call_buffer_share(
                            "bound.raw_snapshot",
                            arguments.len(),
                            size_of::<crate::engine::heap::RawValue>(),
                        );
                    }
                    drop(state);
                    let target = ObjectRef::from_borrowed_handle(self.clone(), target)?;
                    let target = CallableRef::from_validated_object(target);
                    let to_internal =
                        |raw: &crate::engine::heap::RawValue| -> Result<JsValue, RuntimeError> {
                            let value = JsValue::from_raw(raw.clone()).ok_or(
                                RuntimeError::Invariant("bound value was an internal sentinel"),
                            )?;
                            self.dup_jsvalue(&value)
                        };
                    let mut owned_arguments = Vec::new();
                    owned_arguments
                        .try_reserve_exact(arguments.len())
                        .map_err(|_| {
                            RuntimeError::Invariant("bound argument snapshot allocation failed")
                        })?;
                    let this_value = to_internal(&this_value)?;
                    for raw in arguments.iter() {
                        match to_internal(raw) {
                            Ok(value) => owned_arguments.push(value),
                            Err(error) => {
                                let _ = self.release_jsvalue(this_value);
                                for value in owned_arguments {
                                    let _ = self.release_jsvalue(value);
                                }
                                return Err(error);
                            }
                        }
                    }
                    let arguments = owned_arguments;
                    #[cfg(feature = "profiling")]
                    {
                        crate::engine::api::profiling::record_call_buffer_observed(
                            "bound.rooted_snapshot",
                            arguments.capacity(),
                            size_of::<JsValue>(),
                        );
                        crate::engine::api::profiling::record_call_buffer_js_value_copies(
                            "bound.rooted_snapshot",
                            &arguments,
                        );
                    }
                    return Ok(CallableExecution::Bound {
                        target,
                        this_value,
                        arguments,
                    });
                }
                ObjectPayload::BytecodeFunction {
                    bytecode,
                    closure_slots,
                    ..
                } => (*bytecode, closure_slots.clone()),
                ObjectPayload::Proxy(data) if data.is_callable => {
                    return Ok(CallableExecution::Proxy);
                }
                ObjectPayload::Proxy(_) => {
                    return Err(RuntimeError::Engine(Error::new(
                        ErrorKind::Type,
                        "not a function",
                    )));
                }
                ObjectPayload::Ordinary
                | ObjectPayload::ArrayBuffer(_)
                | ObjectPayload::SharedArrayBuffer(_)
                | ObjectPayload::DataView(_)
                | ObjectPayload::TypedArray(_)
                | ObjectPayload::AsyncFunctionState(_)
                | ObjectPayload::RawJson
                | ObjectPayload::Promise(_)
                | ObjectPayload::Date(_)
                | ObjectPayload::RegExp(_)
                | ObjectPayload::Array { .. }
                | ObjectPayload::Arguments { .. }
                | ObjectPayload::ArrayIterator { .. }
                | ObjectPayload::IteratorHelper(_)
                | ObjectPayload::IteratorWrap(_)
                | ObjectPayload::AsyncFromSyncIterator(_)
                | ObjectPayload::IteratorConcat(_)
                | ObjectPayload::Map { .. }
                | ObjectPayload::MapIterator { .. }
                | ObjectPayload::Set { .. }
                | ObjectPayload::WeakMap { .. }
                | ObjectPayload::WeakSet { .. }
                | ObjectPayload::WeakRef { .. }
                | ObjectPayload::FinalizationRegistry(_)
                | ObjectPayload::SetIterator { .. }
                | ObjectPayload::ForInIterator(_)
                | ObjectPayload::Primitive(_)
                | ObjectPayload::GlobalObject { .. }
                | ObjectPayload::Error
                | ObjectPayload::StringIterator { .. }
                | ObjectPayload::RegExpStringIterator { .. }
                | ObjectPayload::Generator { .. }
                | ObjectPayload::AsyncGenerator(_) => {
                    return Err(RuntimeError::Engine(Error::new(
                        ErrorKind::Type,
                        "not a function",
                    )));
                }
            }
        };
        let bytecode = FunctionBytecodeRef::from_borrowed_handle(self.clone(), bytecode)?;
        let closure_slots =
            super::closure::ClosureSlots::shared(callable.as_object().clone(), closure_slots);
        Ok(CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        })
    }

    /// Snapshot only a direct native callable. Bound and bytecode functions
    /// deliberately return `None`: QuickJS's iterator-next fast path tests the
    /// method object itself and does not unwrap wrappers before deciding which
    /// ABI to use.
    pub(crate) fn direct_native_callable_metadata(
        &self,
        callable: &CallableRef,
    ) -> Result<Option<(NativeFunctionId, ContextId, u8)>, RuntimeError> {
        let _operation = self.operation();
        if !callable.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("native callable"));
        }
        let state = self.0.state.borrow();
        let object = state.heap.object(callable.as_object().object_id())?;
        match &object.payload {
            ObjectPayload::NativeFunction { data, .. } => {
                let realm = data.realm.ok_or(RuntimeError::Invariant(
                    "native function was called before its defining realm was attached",
                ))?;
                state.heap.context(realm)?;
                Ok(Some((data.target, realm, data.min_readable_args)))
            }
            ObjectPayload::BoundFunction { .. }
            | ObjectPayload::BytecodeFunction { .. }
            | ObjectPayload::Proxy(_) => Ok(None),
            ObjectPayload::Ordinary
            | ObjectPayload::ArrayBuffer(_)
            | ObjectPayload::SharedArrayBuffer(_)
            | ObjectPayload::DataView(_)
            | ObjectPayload::TypedArray(_)
            | ObjectPayload::AsyncFunctionState(_)
            | ObjectPayload::RawJson
            | ObjectPayload::Promise(_)
            | ObjectPayload::Date(_)
            | ObjectPayload::RegExp(_)
            | ObjectPayload::Array { .. }
            | ObjectPayload::Arguments { .. }
            | ObjectPayload::ArrayIterator { .. }
            | ObjectPayload::IteratorHelper(_)
            | ObjectPayload::IteratorWrap(_)
            | ObjectPayload::AsyncFromSyncIterator(_)
            | ObjectPayload::IteratorConcat(_)
            | ObjectPayload::Map { .. }
            | ObjectPayload::MapIterator { .. }
            | ObjectPayload::Set { .. }
            | ObjectPayload::WeakMap { .. }
            | ObjectPayload::WeakSet { .. }
            | ObjectPayload::WeakRef { .. }
            | ObjectPayload::FinalizationRegistry(_)
            | ObjectPayload::SetIterator { .. }
            | ObjectPayload::ForInIterator(_)
            | ObjectPayload::Primitive(_)
            | ObjectPayload::GlobalObject { .. }
            | ObjectPayload::Error
            | ObjectPayload::StringIterator { .. }
            | ObjectPayload::RegExpStringIterator { .. }
            | ObjectPayload::Generator { .. }
            | ObjectPayload::AsyncGenerator(_) => Err(RuntimeError::Invariant(
                "validated callable no longer has a callable payload",
            )),
        }
    }

    /// Invoke a direct `NativeCProto::IteratorNext` method in the outer
    /// iterator operation's current realm while retaining its raw value/done
    /// result for the VM. Pinned QuickJS calls the C iterator-next pointer
    /// directly in `JS_IteratorNext2`, bypassing the ordinary C-function realm
    /// switch. All other callable shapes use the generic JavaScript call and
    /// iterator-result parsing path.
    pub(crate) fn try_call_native_iterator_next_raw(
        &self,
        realm: ContextId,
        callable: &CallableRef,
        iterator: Value,
    ) -> Result<Option<NativeInvokeOutcome>, RuntimeError> {
        self.validate_value_domain(&iterator, "iterator-next receiver")?;
        let Some((target, _defining_realm, min_readable_args)) =
            self.direct_native_callable_metadata(callable)?
        else {
            return Ok(None);
        };
        if target.descriptor().cproto != NativeCProto::IteratorNext {
            return Ok(None);
        }
        self.invoke_native_function(
            callable,
            realm,
            target,
            min_readable_args,
            NativeInvocation::Call {
                this_value: self.unroot_value(&iterator)?,
            },
            &[],
            NativeInvokeMode::IteratorNextRaw,
        )
        .map(Some)
    }

    pub(crate) fn callable_from_value(&self, value: Value) -> Result<CallableRef, RuntimeError> {
        let Value::Object(object) = value else {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "not a function",
            )));
        };
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("callable"));
        }
        let is_callable = matches!(
            self.0
                .state
                .borrow()
                .heap
                .object(object.object_id())?
                .payload,
            ObjectPayload::NativeFunction { .. }
                | ObjectPayload::BoundFunction { .. }
                | ObjectPayload::BytecodeFunction { .. }
                | ObjectPayload::Proxy(crate::engine::heap::ProxyData {
                    is_callable: true,
                    ..
                })
        );
        if !is_callable {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "not a function",
            )));
        }
        Ok(CallableRef::from_validated_object(object))
    }

    /// QuickJS `JS_CallConstructor2` entry for VM operands whose `newTarget`
    /// has not passed through the public Reflect/Context constructor check.
    #[cfg(test)]
    pub(crate) fn construct_value_with_raw_new_target_internal(
        &self,
        caller_realm: ContextId,
        function: Value,
        new_target: Value,
        arguments: &[Value],
    ) -> Result<Completion, RuntimeError> {
        let constructor = match self.constructor_from_value(caller_realm, function)? {
            NativeConversion::Value(constructor) => constructor,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(self.unroot_value(&value)?));
            }
        };
        self.construct_constructor_with_raw_new_target_internal(
            caller_realm,
            &constructor,
            new_target,
            arguments,
        )
    }

    #[cfg(test)]
    pub(crate) fn construct_constructor_with_raw_new_target_internal(
        &self,
        caller_realm: ContextId,
        constructor: &ConstructorRef,
        new_target: Value,
        arguments: &[Value],
    ) -> Result<Completion, RuntimeError> {
        self.construct_internal_with_new_target(
            caller_realm,
            constructor,
            ConstructNewTarget::Raw(self.unroot_value(&new_target)?),
            arguments,
        )
    }

    pub(crate) fn constructor_from_value(
        &self,
        caller_realm: ContextId,
        value: Value,
    ) -> Result<NativeConversion<ConstructorRef>, RuntimeError> {
        let Value::Object(object) = value else {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "not a function",
            )));
        };
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("constructor"));
        }
        let is_constructor = {
            let state = self.0.state.borrow();
            let object_data = state.heap.object(object.object_id())?;
            object_data.is_constructor
        };
        if !is_constructor {
            let value = Value::Object(object);
            return Ok(NativeConversion::Throw(
                self.new_not_constructor_error(caller_realm, &value)?,
            ));
        }
        Ok(NativeConversion::Value(
            ConstructorRef::from_validated_object(object),
        ))
    }

    #[cfg(test)]
    pub(crate) fn construct_internal(
        &self,
        caller_realm: ContextId,
        constructor: &CallableRef,
        new_target: &CallableRef,
        arguments: &[Value],
    ) -> Result<Completion, RuntimeError> {
        let (constructor, new_target) =
            match self.prepare_constructor_pair(caller_realm, constructor, new_target)? {
                NativeConversion::Value(pair) => pair,
                NativeConversion::Throw(value) => {
                    return Ok(Completion::Throw(self.unroot_value(&value)?));
                }
            };
        self.construct_constructor_internal(caller_realm, &constructor, &new_target, arguments)
    }

    pub(crate) fn prepare_constructor_pair(
        &self,
        caller_realm: ContextId,
        constructor: &CallableRef,
        new_target: &CallableRef,
    ) -> Result<NativeConversion<(ConstructorRef, ConstructorRef)>, RuntimeError> {
        if !constructor.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("constructor"));
        }
        if !self.is_constructor(constructor.as_object())? {
            return Ok(NativeConversion::Throw(self.new_not_constructor_error(
                caller_realm,
                &Value::Object(constructor.as_object().clone()),
            )?));
        }
        if !new_target.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("constructor"));
        }
        if !self.is_constructor(new_target.as_object())? {
            return Ok(NativeConversion::Throw(self.new_not_constructor_error(
                caller_realm,
                &Value::Object(new_target.as_object().clone()),
            )?));
        }
        let constructor = ConstructorRef::from_validated_callable(constructor);
        let new_target = ConstructorRef::from_validated_callable(new_target);
        Ok(NativeConversion::Value((constructor, new_target)))
    }

    pub(crate) fn construct_constructor_internal(
        &self,
        caller_realm: ContextId,
        constructor: &ConstructorRef,
        new_target: &ConstructorRef,
        arguments: &[Value],
    ) -> Result<Completion, RuntimeError> {
        self.construct_internal_with_new_target(
            caller_realm,
            constructor,
            ConstructNewTarget::Validated(new_target.clone()),
            arguments,
        )
    }

    pub(crate) fn normalize_constructor(
        &self,
        caller_realm: ContextId,
        mut constructor: ConstructorRef,
        new_target: ConstructNewTarget,
        mut arguments: Vec<crate::engine::value::JsValue>,
    ) -> Result<NativeConversion<NormalizedConstructor>, RuntimeError> {
        let mut new_target = Some(new_target);
        let result = (|| {
            self.0.state.borrow().heap.context(caller_realm)?;
            if !constructor.as_object().belongs_to(self) {
                return Err(RuntimeError::WrongRuntime("constructor"));
            }
            match new_target.as_ref().expect("new target") {
                ConstructNewTarget::Validated(target) if !target.as_object().belongs_to(self) => {
                    return Err(RuntimeError::WrongRuntime("constructor"));
                }
                // Internal values carry no runtime branding.
                _ => {}
            }
            loop {
                if !self.is_constructor(constructor.as_object())? {
                    for argument in arguments.drain(..) {
                        self.release_jsvalue(argument)?;
                    }
                    new_target.take().expect("new target").release(self)?;
                    return Ok(NativeConversion::Throw(self.new_not_constructor_error(
                        caller_realm,
                        &Value::Object(constructor.as_object().clone()),
                    )?));
                }
                if self.is_proxy_object(constructor.as_object())? {
                    return Ok(NativeConversion::Value(NormalizedConstructor {
                        target: ConstructorTarget::Proxy(constructor),
                        new_target: new_target.take().expect("new target"),
                        arguments: std::mem::take(&mut arguments),
                    }));
                }
                let callable = self.as_callable(constructor.as_object())?.ok_or_else(|| {
                    RuntimeError::Engine(Error::new(ErrorKind::Type, "not a function"))
                })?;
                match self.bytecode_for_callable(&callable)? {
                    CallableExecution::Bound {
                        target,
                        this_value,
                        arguments: bound,
                    } => {
                        // Construction ignores bound this, but classification still
                        // handed us its owned edge.
                        self.release_jsvalue(this_value)?;
                        // The bound payload roots transfer into internal values
                        // without a retain/release pair; the accumulated argument
                        // edges move into the merged buffer.
                        arguments = match self.concatenate_bound_arguments_jsvalue(
                            caller_realm,
                            bound,
                            std::mem::take(&mut arguments),
                        )? {
                            NativeConversion::Value(arguments) => arguments,
                            NativeConversion::Throw(value) => {
                                return Ok(NativeConversion::Throw(value));
                            }
                        };
                        new_target
                            .as_mut()
                            .expect("new target")
                            .retarget_bound_identity(self, &constructor, &target)?;
                        constructor = ConstructorRef::from_validated_callable(&target);
                    }
                    classification => {
                        return Ok(NativeConversion::Value(NormalizedConstructor {
                            target: ConstructorTarget::Ordinary {
                                callable,
                                classification,
                            },
                            new_target: new_target.take().expect("new target"),
                            arguments: std::mem::take(&mut arguments),
                        }));
                    }
                }
            }
        })();
        if let Some(new_target) = new_target {
            let _ = new_target.release(self);
        }
        for argument in arguments {
            let _ = self.release_jsvalue(argument);
        }
        result
    }

    pub(crate) fn construct_internal_with_new_target(
        &self,
        caller_realm: ContextId,
        constructor: &ConstructorRef,
        new_target: ConstructNewTarget,
        arguments: &[Value],
    ) -> Result<Completion, RuntimeError> {
        let mut converted = Vec::new();
        for argument in arguments {
            match self.unroot_value(argument) {
                Ok(value) => converted.push(value),
                Err(error) => {
                    let _ = new_target.release(self);
                    for value in converted {
                        let _ = self.release_jsvalue(value);
                    }
                    return Err(error);
                }
            }
        }
        self.construct_internal_jsvalue(caller_realm, constructor, new_target, converted)
    }

    pub(crate) fn construct_internal_jsvalue(
        &self,
        caller_realm: ContextId,
        constructor: &ConstructorRef,
        new_target: ConstructNewTarget,
        arguments: Vec<JsValue>,
    ) -> Result<Completion, RuntimeError> {
        let NormalizedConstructor {
            target,
            new_target,
            arguments,
        } = match self.normalize_constructor(
            caller_realm,
            constructor.clone(),
            new_target,
            arguments,
        )? {
            NativeConversion::Value(result) => result,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(self.into_jsvalue(value)?));
            }
        };
        let mut new_target = Some(new_target);
        let mut arguments = arguments;
        let result = (|| {
            let (callable, classification) = match target {
                ConstructorTarget::Proxy(constructor) => {
                    return self.construct_proxy_jsvalue(
                        caller_realm,
                        &constructor,
                        new_target.take().expect("new target"),
                        std::mem::take(&mut arguments),
                    );
                }
                ConstructorTarget::Ordinary {
                    callable,
                    classification,
                } => (callable, classification),
            };
            match classification {
                CallableExecution::Native {
                    target,
                    realm,
                    min_readable_args,
                } => {
                    let execution_realm = if target.uses_calling_realm() {
                        caller_realm
                    } else {
                        realm
                    };
                    Self::ordinary_native_completion(self.invoke_native_function_jsvalue(
                        &callable,
                        execution_realm,
                        target,
                        min_readable_args,
                        NativeInvocation::Construct {
                            new_target: new_target.take().expect("new target").into_value(),
                        },
                        std::mem::take(&mut arguments),
                        NativeInvokeMode::Ordinary,
                    )?)
                }
                CallableExecution::Bytecode {
                    bytecode,
                    closure_slots,
                } => {
                    let constructor_kind = self
                        .0
                        .state
                        .borrow()
                        .heap
                        .function_bytecode(bytecode.bytecode_id())?
                        .metadata
                        .constructor_kind;
                    match constructor_kind {
                        ConstructorKind::None => {
                            return Err(RuntimeError::Invariant(
                                "constructor bit disagrees with bytecode constructor metadata",
                            ));
                        }
                        ConstructorKind::Derived => {
                            let completion = self.execute_bytecode_callable_jsvalue(
                                caller_realm,
                                &callable,
                                JsValue::Undefined,
                                new_target.take().expect("new target").into_value(),
                                std::mem::take(&mut arguments),
                                bytecode,
                                closure_slots,
                            )?;
                            return match completion {
                                completion @ (Completion::Return(JsValue::Object(_))
                                | Completion::Throw(_)) => Ok(completion),
                                Completion::Return(value) => {
                                    self.release_jsvalue(value)?;
                                    Err(RuntimeError::Invariant(
                                        "derived constructor bytecode returned an unvalidated primitive",
                                    ))
                                }
                            };
                        }
                        ConstructorKind::Base => {}
                    }
                    let this_value = {
                        // Prototype lookup's public adapter borrows this root;
                        // the constructor argv remains internal throughout.
                        let target_root =
                            self.root_value(&new_target.as_ref().expect("new target").value())?;
                        match self.create_from_constructor_value(caller_realm, &target_root)? {
                            Completion::Return(value) => value,
                            Completion::Throw(value) => return Ok(Completion::Throw(value)),
                        }
                    };
                    let this_argument = match self.dup_jsvalue(&this_value) {
                        Ok(value) => value,
                        Err(error) => {
                            let _ = self.release_jsvalue(this_value);
                            return Err(error);
                        }
                    };
                    let completion = self.execute_bytecode_callable_jsvalue(
                        caller_realm,
                        &callable,
                        this_argument,
                        new_target.take().expect("new target").into_value(),
                        std::mem::take(&mut arguments),
                        bytecode,
                        closure_slots,
                    );
                    match completion {
                        Ok(Completion::Return(value @ JsValue::Object(_))) => {
                            self.release_jsvalue(this_value)?;
                            Ok(Completion::Return(value))
                        }
                        Ok(Completion::Throw(value)) => {
                            self.release_jsvalue(this_value)?;
                            Ok(Completion::Throw(value))
                        }
                        Ok(Completion::Return(value)) => {
                            self.release_jsvalue(value)?;
                            Ok(Completion::Return(this_value))
                        }
                        Err(error) => {
                            self.release_jsvalue(this_value)?;
                            Err(error)
                        }
                    }
                }
                CallableExecution::Proxy | CallableExecution::Bound { .. } => Err(
                    RuntimeError::Invariant("constructor dispatch was not normalized"),
                ),
            }
        })();
        if let Some(new_target) = new_target {
            let _ = new_target.release(self);
        }
        for argument in arguments {
            let _ = self.release_jsvalue(argument);
        }
        result
    }

    pub(crate) fn constructor_prototype_source(
        &self,
        caller_realm: ContextId,
        new_target: &Value,
    ) -> Result<NativeConversion<ConstructorPrototypeSource>, RuntimeError> {
        prototype::finish(
            self,
            caller_realm,
            prototype::ProtoSourceStep::start(self, caller_realm, new_target.clone())?,
        )
    }

    pub(crate) fn create_from_constructor_value(
        &self,
        caller_realm: ContextId,
        new_target: &Value,
    ) -> Result<Completion, RuntimeError> {
        let reply = if matches!(new_target, Value::Undefined) {
            Completion::Return(JsValue::Undefined)
        } else {
            let key =
                self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
            self.get_value_property_in_realm(caller_realm, new_target.clone(), &key)?
        };
        self.create_from_constructor_prototype_reply(caller_realm, new_target, reply)
    }

    pub(crate) fn create_from_constructor_prototype_reply(
        &self,
        caller_realm: ContextId,
        new_target: &Value,
        reply: Completion,
    ) -> Result<Completion, RuntimeError> {
        let prototype = match reply {
            result @ Completion::Throw(_) => return Ok(result),
            Completion::Return(JsValue::Object(prototype)) => {
                ObjectRef::from_owned_handle(self.clone(), prototype)
            }
            Completion::Return(value) => {
                self.release_jsvalue(value)?;
                let realm = if matches!(new_target, Value::Undefined) {
                    caller_realm
                } else {
                    match self.function_realm_from_value(caller_realm, new_target)? {
                        NativeConversion::Value(realm) => realm,
                        NativeConversion::Throw(value) => {
                            return Ok(Completion::Throw(self.into_jsvalue(value)?));
                        }
                    }
                };
                let prototype = self.0.state.borrow().heap.context(realm)?.object_prototype;
                ObjectRef::from_borrowed_handle(self.clone(), prototype)?
            }
        };
        Ok(Completion::Return(JsValue::Object(
            self.new_object(Some(&prototype))?.into_handle(),
        )))
    }

    pub(crate) fn ordinary_native_completion(
        outcome: NativeInvokeOutcome,
    ) -> Result<Completion, RuntimeError> {
        match outcome {
            NativeInvokeOutcome::Completion(completion) => Ok(completion),
            NativeInvokeOutcome::IteratorNextRaw { .. } => Err(RuntimeError::Invariant(
                "ordinary native call leaked an unwrapped iterator-next outcome",
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn invoke_native_function(
        &self,
        callable: &CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: &[Value],
        mode: NativeInvokeMode,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let native::PreparedNativeCall {
            activation,
            invocation,
        } = self.prepare_native_invocation(
            callable,
            realm,
            target,
            min_readable_args,
            invocation,
            arguments,
            mode,
        )?;
        self.invoke_prepared_native(native::PreparedNativeCall {
            activation,
            invocation,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn invoke_native_function_jsvalue(
        &self,
        callable: &CallableRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        invocation: NativeInvocation,
        arguments: Vec<JsValue>,
        mode: NativeInvokeMode,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let prepared = self.prepare_native_invocation_jsvalue(
            callable,
            realm,
            target,
            min_readable_args,
            invocation,
            arguments,
            mode,
        )?;
        self.invoke_prepared_native(prepared)
    }

    fn invoke_prepared_native(
        &self,
        prepared: native::PreparedNativeCall,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let native::PreparedNativeCall {
            activation,
            invocation,
        } = prepared;
        let result = match activation.mode {
            NativeInvokeMode::Ordinary => self
                .dispatch_native_function(
                    &activation.callable,
                    activation.target,
                    activation.realm,
                    invocation,
                    &activation.arguments,
                )
                .map(NativeInvokeOutcome::Completion),
            NativeInvokeMode::IteratorNextRaw => {
                if activation.target.descriptor().cproto != NativeCProto::IteratorNext {
                    invocation.release(self)?;
                    Err(RuntimeError::Invariant(
                        "raw iterator-next dispatch targeted another native cproto",
                    ))
                } else {
                    self.dispatch_native_iterator_next_raw(
                        activation.target,
                        activation.realm,
                        invocation,
                        &activation.arguments,
                    )
                }
            }
        };
        activation.finish(result)
    }

    pub(crate) fn active_function(&self) -> Result<ObjectRef, RuntimeError> {
        let function = self
            .0
            .state
            .borrow()
            .active_frames
            .last()
            .ok_or(RuntimeError::Invariant(
                "active function was requested without an active frame",
            ))?
            .function;
        Ok(ObjectRef::from_borrowed_handle(self.clone(), function)?)
    }
}

pub(crate) enum NativeInvocation {
    Call {
        this_value: crate::engine::value::JsValue,
    },
    Construct {
        new_target: crate::engine::value::JsValue,
    },
    Getter {
        this_value: crate::engine::value::JsValue,
    },
    Setter {
        this_value: crate::engine::value::JsValue,
    },
}

impl NativeInvocation {
    /// Release the internal edge this invocation owns. Boundary adapters that
    /// duplicate an invocation through [`NativeInvocation::dup`] must release
    /// their own copy once the borrowed step has captured its own edges.
    pub(crate) fn release(
        self,
        runtime: &crate::engine::api::runtime::Runtime,
    ) -> Result<(), crate::engine::api::runtime_error::RuntimeError> {
        let value = match self {
            Self::Call { this_value }
            | Self::Getter { this_value }
            | Self::Setter { this_value } => this_value,
            Self::Construct { new_target } => new_target,
        };
        runtime.release_jsvalue(value)
    }

    /// Duplicate the invocation's internal value edges. There is no automatic
    /// `Clone` because duplicating a handle needs the owning runtime.
    pub(crate) fn dup(
        &self,
        runtime: &crate::engine::api::runtime::Runtime,
    ) -> Result<Self, crate::engine::api::runtime_error::RuntimeError> {
        Ok(match self {
            Self::Call { this_value } => Self::Call {
                this_value: runtime.dup_jsvalue(this_value)?,
            },
            Self::Construct { new_target } => Self::Construct {
                new_target: runtime.dup_jsvalue(new_target)?,
            },
            Self::Getter { this_value } => Self::Getter {
                this_value: runtime.dup_jsvalue(this_value)?,
            },
            Self::Setter { this_value } => Self::Setter {
                this_value: runtime.dup_jsvalue(this_value)?,
            },
        })
    }
}

pub(crate) enum NativeInvocationAdaptation<I = NativeInvocation> {
    Invoke(I),
    Complete(Completion),
}

/// Result of invoking one native function before the public call adapter has
/// necessarily materialized its JavaScript result shape.
///
/// QuickJS's `JS_CFUNC_iterator_next` ABI returns the iterated value and a
/// side-channel `done` bit.  An ordinary JavaScript call wraps that pair in an
/// iterator-result object, while `JS_IteratorNext2` consumes it directly.  A
/// normal completion remains available for future iterator-next natives which
/// return an already-materialized result object (`pdone == 2`).
pub(crate) enum NativeInvokeOutcome {
    Completion(Completion),
    IteratorNextRaw {
        value: crate::engine::value::JsValue,
        done: bool,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum NativeInvokeMode {
    Ordinary,
    IteratorNextRaw,
}

pub(crate) struct NativeArguments {
    pub(crate) actual_arg_count: usize,
    pub(crate) readable: Vec<crate::engine::value::JsValue>,
}

/// Result of QuickJS `Get(newTarget, "prototype")` followed by
/// `JS_GetFunctionRealm` when the property is not an object.
///
/// Keeping the fallback realm separate lets each native constructor select
/// its own intrinsic prototype without first narrowing the raw `newTarget` to
/// a callable object.
pub(crate) enum ConstructorPrototypeSource {
    Explicit(ObjectRef),
    Realm(ContextId),
}

pub(crate) enum CallableExecution {
    Bytecode {
        bytecode: FunctionBytecodeRef,
        closure_slots: crate::engine::vm::closure::ClosureSlots,
    },
    Native {
        target: NativeFunctionId,
        realm: ContextId,
        min_readable_args: u8,
    },
    Bound {
        target: CallableRef,
        this_value: crate::engine::value::JsValue,
        arguments: Vec<crate::engine::value::JsValue>,
    },
    Proxy,
}

/// A rooted object whose `[[Construct]]` capability bit has been validated.
///
/// QuickJS keeps `[[Call]]` and `[[Construct]]` independent: in particular a
/// Proxy may carry only the latter and still dispatch its `construct` trap.
/// Keep that capability private instead of weakening public `CallableRef`.
#[derive(Clone)]
pub(crate) struct ConstructorRef(ObjectRef);

impl ConstructorRef {
    pub(crate) fn from_validated_object(object: ObjectRef) -> Self {
        Self(object)
    }

    pub(crate) fn from_validated_callable(callable: &CallableRef) -> Self {
        Self(callable.as_object().clone())
    }

    pub(crate) fn as_object(&self) -> &ObjectRef {
        &self.0
    }

    /// Consume this validated constructor root, transferring its one owned
    /// object edge to the caller without retaining or releasing.
    pub(crate) fn into_object(self) -> ObjectRef {
        self.0
    }
}

/// Whether one internal constructor entry carries an ECMAScript-validated
/// `newTarget` or QuickJS's raw `JS_CallConstructor2` value.
///
/// `OP_call_constructor`, `OP_apply` constructor mode, and derived `super()`
/// use the raw form. Public Context and Reflect entry points retain the
/// validated form and its existing constructor checks.
pub(crate) enum ConstructNewTarget {
    Validated(ConstructorRef),
    Raw(crate::engine::value::JsValue),
}

impl ConstructNewTarget {
    /// Drop this owner without consuming it, releasing the raw edge when the
    /// new-target arrived through QuickJS's raw form.
    pub(crate) fn release(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        match self {
            Self::Validated(constructor) => {
                drop(constructor);
                Ok(())
            }
            Self::Raw(value) => runtime.release_jsvalue(value),
        }
    }

    /// Consume into the internal new-target value, transferring the validated
    /// constructor's edge or moving the raw owner.
    pub(crate) fn into_value(self) -> crate::engine::value::JsValue {
        match self {
            Self::Validated(constructor) => {
                crate::engine::value::JsValue::Object(constructor.into_object().into_handle())
            }
            Self::Raw(value) => value,
        }
    }

    /// Borrow as the internal new-target value without transferring the edge.
    /// Callers must not release through the returned value.
    pub(crate) fn value(&self) -> crate::engine::value::JsValue {
        match self {
            Self::Validated(constructor) => {
                crate::engine::value::JsValue::Object(constructor.as_object().object_id())
            }
            Self::Raw(value) => match value {
                crate::engine::value::JsValue::Undefined => {
                    crate::engine::value::JsValue::Undefined
                }
                crate::engine::value::JsValue::Null => crate::engine::value::JsValue::Null,
                crate::engine::value::JsValue::Bool(value) => {
                    crate::engine::value::JsValue::Bool(*value)
                }
                crate::engine::value::JsValue::Int(value) => {
                    crate::engine::value::JsValue::Int(*value)
                }
                crate::engine::value::JsValue::Float(value) => {
                    crate::engine::value::JsValue::Float(*value)
                }
                crate::engine::value::JsValue::String(id) => {
                    crate::engine::value::JsValue::String(*id)
                }
                crate::engine::value::JsValue::BigInt(id) => {
                    crate::engine::value::JsValue::BigInt(*id)
                }
                crate::engine::value::JsValue::Symbol(index) => {
                    crate::engine::value::JsValue::Symbol(*index)
                }
                crate::engine::value::JsValue::Object(id) => {
                    crate::engine::value::JsValue::Object(*id)
                }
            },
        }
    }

    pub(crate) fn retarget_bound_identity(
        &mut self,
        runtime: &Runtime,
        bound: &ConstructorRef,
        target: &CallableRef,
    ) -> Result<(), RuntimeError> {
        let matches_bound = match self {
            Self::Validated(constructor) => constructor.as_object() == bound.as_object(),
            Self::Raw(JsValue::Object(object)) => *object == bound.as_object().object_id(),
            Self::Raw(_) => false,
        };
        if !matches_bound {
            return Ok(());
        }
        match self {
            Self::Validated(constructor) => {
                *constructor = ConstructorRef::from_validated_callable(target);
            }
            Self::Raw(value) => {
                let previous = std::mem::replace(
                    value,
                    JsValue::Object(target.as_object().clone().into_handle()),
                );
                runtime.release_jsvalue(previous)?;
            }
        }
        Ok(())
    }
}

/// Target selected by the bytecode `Call` path.
///
/// Pinned QuickJS deliberately enters a Proxy's call hook before checking the
/// cached callable bit, so a direct call of a non-callable Proxy can still
/// observe the handler's `apply` getter. Keep that narrow quirk out of
/// `CallableRef`, whose public invariant remains a genuine `[[Call]]`.
pub(crate) enum DirectCallTarget {
    Callable(CallableRef),
    NonCallableProxy(ObjectRef),
}

pub(crate) struct NormalizedConstructor {
    pub target: ConstructorTarget,
    pub new_target: ConstructNewTarget,
    pub arguments: Vec<crate::engine::value::JsValue>,
}
pub(crate) enum ConstructorTarget {
    Proxy(ConstructorRef),
    Ordinary {
        callable: CallableRef,
        classification: CallableExecution,
    },
}
