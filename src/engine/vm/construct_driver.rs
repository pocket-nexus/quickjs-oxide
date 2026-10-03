//! Bytecode constructor bodies run as explicit child frames. Prototype lookup
//! getter replies resume the pending constructor; exotic/native prototype reads
//! use the same owned property query as other Get operations.
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::value::JsValue;
use crate::engine::value::conversion::NativeConversion;
use crate::engine::vm::Completion;
use crate::engine::vm::call::{BytecodeCallRequest, CallableExecution};
use crate::engine::vm::driver::{CallStep, push_frame};
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{FrameId, ReturnTarget};

pub(super) fn enter(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    _identity: u64,
) -> Result<CallStep, Error> {
    let count = usize::from(count);
    if try_ordinary_base(runtime, execution, id, count)? {
        return Ok(CallStep::Entered);
    }
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    // The classifier consumes a duplicate; the slot owner retains its edge
    // until `start_construct` consumes the operands.
    let target = runtime
        .dup_jsvalue(execution.slots.peek(&frame.window, count + 1)?)
        .map_err(runtime_error_to_vm_error)?;
    let constructor = match runtime.constructor_from_jsvalue(realm, target) {
        Ok(NativeConversion::Value(target)) => target,
        Ok(NativeConversion::Throw(value)) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
        Err(error) => {
            return super::driver::rejected_call(runtime, realm, runtime_error_to_vm_error(error));
        }
    };
    let new_target = runtime
        .dup_jsvalue(execution.slots.peek(&frame.window, count)?)
        .map_err(runtime_error_to_vm_error)?;
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(count)
        .map_err(|_| Error::internal("construct arguments allocation failed"))?;
    for offset in (0..count).rev() {
        arguments.push(
            runtime
                .dup_jsvalue(execution.slots.peek(&frame.window, offset)?)
                .map_err(runtime_error_to_vm_error)?,
        );
    }
    super::proxy_get_driver::start_construct(
        runtime,
        execution,
        id,
        constructor,
        new_target,
        arguments,
        count + 2,
    )
}

/// Admit one synchronous constructor prefix while its operands are still
/// rooted. A miss performs no observable work and preserves the query protocol.
fn try_ordinary_base(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: usize,
) -> Result<bool, Error> {
    use crate::engine::{
        code::function::metadata::{ConstructorKind, FunctionKind},
        heap::{ObjectKind, ObjectPayload, PropertySlot, RawId, RawValue},
        object::ObjectRef,
        vm::{
            call::ordinary::{DirectSelection, OrdinaryCall},
            frame::{Frame, ReturnOwner, ReturnValue},
            stack::{FrameStorage, FrameStorageGuard},
        },
    };
    if runtime.0.deferred_references.has_pending()
        || !execution.frames.can_push_with_continuations(0)
        || runtime.bytecode_call_would_overflow()
    {
        return Ok(false);
    }
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let observed_depth = execution.slots.depth(&frame.window);
    let target = execution.slots.peek(&frame.window, count + 1)?;
    let new_target = execution.slots.peek(&frame.window, count)?;
    let (JsValue::Object(target_id), JsValue::Object(new_target_id)) = (target, new_target) else {
        return Ok(false);
    };
    let prototype = {
        let state = runtime.0.state.borrow();
        if state.heap.has_pending_zero_cleanup() {
            return Ok(false);
        }
        let target = state
            .heap
            .object(*target_id)
            .map_err(super::exception::heap_error_to_vm_error)?;
        if !target.is_constructor {
            return Ok(false);
        }
        let ObjectPayload::BytecodeFunction { bytecode, .. } = &target.payload else {
            return Ok(false);
        };
        let data = state
            .heap
            .function_bytecode(*bytecode)
            .map_err(super::exception::heap_error_to_vm_error)?;
        if data.metadata.function_kind != FunctionKind::Normal
            || data.metadata.constructor_kind != ConstructorKind::Base
        {
            return Ok(false);
        }
        let new_target = state
            .heap
            .object(*new_target_id)
            .map_err(super::exception::heap_error_to_vm_error)?;
        // Exotic [[Get]], inherited properties, lazy initialization and getters
        // keep the owning query. An own ordinary data slot needs no callback.
        if !new_target.is_constructor
            || !matches!(
                new_target.kind,
                ObjectKind::BytecodeFunction | ObjectKind::Ordinary
            )
        {
            return Ok(false);
        }
        let atom = state
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
        let shape = state
            .heap
            .shape(new_target.shape)
            .map_err(super::exception::heap_error_to_vm_error)?;
        let Some(slot) = shape.find(crate::engine::atom::AtomIdx::from_raw(atom.raw())) else {
            return Ok(false);
        };
        if shape.entries()[slot as usize].flags.storage
            != crate::engine::object::shape::PropertyStorageKind::Data
        {
            return Ok(false);
        }
        let Some(PropertySlot::Data(RawValue::Object(prototype))) =
            new_target.slots.get(slot as usize)
        else {
            return Ok(false);
        };
        // The canonical query keeps additional transient owners. Keep its
        // overflow and immortal transition behavior near saturation, including
        // every argument aliasing target, new target or prototype. Each input
        // may have an outgoing and a writable-parameter copy; eight further
        // edges conservatively cover the constructor/prototype query owners.
        let headroom = (count as u32).saturating_mul(2).saturating_add(8);
        let ready = |value: &JsValue| -> Result<bool, Error> {
            let id = match value {
                JsValue::Object(id) => RawId::Object(*id),
                JsValue::String(id) => RawId::String(*id),
                JsValue::BigInt(id) => RawId::BigInt(*id),
                JsValue::Symbol(index) => {
                    let atom = state
                        .atoms
                        .brand(*index)
                        .map_err(|error| Error::internal(error.to_string()))?;
                    return Ok(state
                        .atoms
                        .resolve(atom)
                        .map_err(|error| Error::internal(error.to_string()))?
                        .ref_count
                        .is_none_or(|count| count < u32::MAX - headroom));
                }
                _ => return Ok(true),
            };
            Ok(state
                .heap
                .strong_count(id)
                .map_err(super::exception::heap_error_to_vm_error)?
                < u32::MAX - headroom)
        };
        if !ready(&JsValue::Object(*target_id))?
            || !ready(&JsValue::Object(*new_target_id))?
            || !ready(&JsValue::Object(*prototype))?
            || state
                .heap
                .strong_count(RawId::FunctionBytecode(*bytecode))
                .map_err(super::exception::heap_error_to_vm_error)?
                >= u32::MAX - 4
        {
            return Ok(false);
        }
        for offset in (0..count).rev() {
            if !ready(execution.slots.peek(&frame.window, offset)?)? {
                return Ok(false);
            }
        }
        *prototype
    };
    let call: OrdinaryCall = match DirectSelection::select_jsvalue(runtime, target)
        .map_err(runtime_error_to_vm_error)?
    {
        DirectSelection::Ordinary(selected) => selected
            .authenticate(runtime)
            .map_err(runtime_error_to_vm_error)?,
        _ => return Ok(false),
    };
    let prototype = ObjectRef::from_borrowed_handle(runtime.clone(), prototype)
        .map_err(super::exception::heap_error_to_vm_error)?;
    // Preserve the old outgoing argument ownership protocol. The independent
    // owners make retiring each original operand non-observing; no borrowed
    // heap fact survives allocation or child installation.
    let new_target = runtime
        .dup_jsvalue(new_target)
        .map_err(runtime_error_to_vm_error)?;
    let mut input = super::CallInput::new(runtime, JsValue::Undefined, new_target, None);
    let mut arguments = FrameStorageGuard::new(
        runtime,
        FrameStorage {
            original_arguments: Vec::new(),
            parameters: Vec::new(),
            locals: Vec::new(),
            operands: Vec::new(),
        },
    );
    arguments
        .storage_mut()
        .original_arguments
        .try_reserve_exact(count)
        .map_err(|_| Error::internal("construct arguments allocation failed"))?;
    for offset in (0..count).rev() {
        arguments.storage_mut().original_arguments.push(
            runtime
                .dup_jsvalue(execution.slots.peek(&frame.window, offset)?)
                .map_err(runtime_error_to_vm_error)?,
        );
    }
    let resume = frame.next_pc()?;
    let receiver = runtime
        .new_object(Some(&prototype))
        .map_err(runtime_error_to_vm_error)?;
    let mut entry = call.prepare_constructor(
        &mut execution.call_storage,
        receiver,
        std::mem::replace(&mut input.new_target, JsValue::Undefined),
        arguments.take().original_arguments,
        realm,
        ReturnTarget {
            owner: ReturnOwner::Frame(id),
            value_use: ReturnValue::Push,
            tail: false,
            operation: None,
        },
    )?;
    let mut owned_storage = FrameStorageGuard::new(
        runtime,
        std::mem::replace(
            &mut entry.storage,
            FrameStorage {
                original_arguments: Vec::new(),
                parameters: Vec::new(),
                locals: Vec::new(),
                operands: Vec::new(),
            },
        ),
    );
    execution
        .call_storage
        .reserve_depth(execution.frames.depth() + 1)?;
    let mut prepared = execution.frames.prepare_push()?;
    let parent = prepared.current_mut(id)?;
    for _ in 0..count + 2 {
        runtime
            .release_jsvalue(execution.slots.pop(&mut parent.window)?)
            .map_err(runtime_error_to_vm_error)?;
    }
    parent.resume_pc = resume;
    let window = execution.slots.push_initialized_frame(
        runtime,
        &entry.executable.frame_layout(),
        owned_storage.take(),
        &entry.cold.function,
        entry.executable.metadata.function_name_local,
    )?;
    entry.cold.executable = entry.executable.into();
    entry.cold.window = window.into();
    prepared.install(Frame {
        property_generation: 0,
        iterator_generation: 0,
        caller_realm: realm,
        active_frame: entry.active_frame,
        cold: entry.cold,
        fault_pc: 0,
        resume_pc: 0,
    });
    #[cfg(feature = "profiling")]
    {
        crate::engine::api::profiling::record_owned_instruction(observed_depth);
        crate::engine::api::profiling::record_owned_execution_event(
            "constructor_base_lazy_install",
        );
    }
    Ok(true)
}

pub(super) fn enter_default_derived(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    _identity: u64,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    if matches!(frame.cold.input.new_target, JsValue::Undefined) {
        return super::driver::rejected_call(
            runtime,
            realm,
            Error::new(
                crate::engine::api::error::ErrorKind::Type,
                "class constructors must be invoked with 'new'",
            ),
        );
    }
    // Preserve the old entry's live prototype lookup, argument snapshot, then
    // constructor validation order, including null and non-constructor errors.
    let target = runtime
        .get_prototype_of(&frame.cold.function)
        .map_err(runtime_error_to_vm_error)?;
    let arguments = execution
        .slots
        .snapshot_actual_arguments(&frame.window, runtime)?;
    let new_target = runtime
        .dup_jsvalue(&frame.cold.input.new_target)
        .map_err(runtime_error_to_vm_error)?;
    let target = target.map_or(JsValue::Null, |object| {
        JsValue::Object(object.into_handle())
    });
    let constructor = match runtime.constructor_from_jsvalue(realm, target) {
        Ok(NativeConversion::Value(constructor)) => constructor,
        Ok(NativeConversion::Throw(value)) => {
            runtime
                .release_jsvalue(new_target)
                .map_err(runtime_error_to_vm_error)?;
            for argument in arguments {
                runtime
                    .release_jsvalue(argument)
                    .map_err(runtime_error_to_vm_error)?;
            }
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
        Err(error) => {
            runtime
                .release_jsvalue(new_target)
                .map_err(runtime_error_to_vm_error)?;
            for argument in arguments {
                runtime
                    .release_jsvalue(argument)
                    .map_err(runtime_error_to_vm_error)?;
            }
            return super::driver::rejected_call(runtime, realm, runtime_error_to_vm_error(error));
        }
    };
    super::proxy_get_driver::start_construct(
        runtime,
        execution,
        id,
        constructor,
        new_target,
        arguments,
        0,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InitializerKind {
    Install,
    Instance,
    Static,
    Block,
}

/// No replay after begin installs brands or commits static initialization. Publication authenticates
/// class initializers as ordinary bytecode functions with zero parameters.
#[inline(never)]
pub(super) fn initializer(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mode: InitializerKind,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let result = (|| -> Result<CallStep, Error> {
        let frame = execution.frames.current_mut(id)?;
        let (initializer, receiver) = match mode {
            InitializerKind::Install => {
                runtime
                    .install_class_instance_initializer(
                        realm,
                        execution.slots.peek(&frame.window, 2)?,
                        execution.slots.peek(&frame.window, 1)?,
                        execution.slots.peek(&frame.window, 0)?,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                (None, JsValue::Undefined)
            }
            InitializerKind::Instance => {
                let receiver = execution.slots.peek(&frame.window, 1)?;
                let callable = runtime
                    .begin_class_instance_initializer(
                        realm,
                        execution.slots.peek(&frame.window, 0)?,
                        receiver,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                let receiver = runtime
                    .dup_jsvalue(receiver)
                    .map_err(runtime_error_to_vm_error)?;
                (callable, receiver)
            }
            InitializerKind::Static => {
                let (callable, receiver) = runtime
                    .begin_class_static_initializer(
                        realm,
                        execution.slots.peek(&frame.window, 1)?,
                        execution.slots.peek(&frame.window, 0)?,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                (Some(callable), receiver)
            }
            InitializerKind::Block => {
                let receiver = &frame.cold.input.this_value;
                let callable = runtime
                    .begin_class_static_block(
                        realm,
                        &frame.cold.function,
                        receiver,
                        execution.slots.peek(&frame.window, 0)?,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                let receiver = runtime
                    .dup_jsvalue(receiver)
                    .map_err(runtime_error_to_vm_error)?;
                (Some(callable), receiver)
            }
        };
        let request = if let Some(callable) = initializer {
            let CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } = runtime
                .bytecode_for_callable(&callable)
                .map_err(runtime_error_to_vm_error)?
            else {
                return Err(Error::internal(
                    "authenticated class initializer is not bytecode",
                ));
            };
            let kind = runtime
                .0
                .state
                .borrow()
                .heap
                .function_bytecode(bytecode.bytecode_id())
                .map_err(|error| Error::internal(error.to_string()))?
                .metadata
                .function_kind;
            if kind != FunctionKind::Normal {
                return Err(Error::internal(
                    "authenticated class initializer is not ordinary bytecode",
                ));
            }
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                runtime
                    .release_jsvalue(receiver)
                    .map_err(runtime_error_to_vm_error)?;
                return runtime
                    .bytecode_stack_overflow_completion(realm, &bytecode)
                    .map(CallStep::Complete)
                    .map_err(runtime_error_to_vm_error);
            }
            Some(BytecodeCallRequest {
                callable,
                receiver,
                new_target: JsValue::Undefined,
                arguments: Vec::new(),
                bytecode,
                closure_slots,
                caller_realm: realm,
                return_to: ReturnTarget {
                    owner: crate::engine::vm::frame::ReturnOwner::Frame(id),
                    tail: false,
                    operation: None,
                    value_use: super::frame::ReturnValue::Discard,
                },
            })
        } else {
            runtime
                .release_jsvalue(receiver)
                .map_err(runtime_error_to_vm_error)?;
            None
        };
        let frame = execution.frames.current_mut(id)?;
        let discarded = execution.slots.pop(&mut frame.window)?;
        frame.resume_pc = frame.next_pc()?;
        runtime
            .release_jsvalue(discarded)
            .map_err(runtime_error_to_vm_error)?;
        if let Some(request) = request {
            let entry = request.prepare(runtime, &mut execution.call_storage)?;
            push_frame(execution, entry)?;
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        Ok(CallStep::Entered)
    })();
    match result {
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(CallStep::Complete(Completion::Throw(
                runtime
                    .new_native_error_from_error_jsvalue(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            )))
        }
        result => result,
    }
}

/// Object heritage owns its prototype lookup across every callback kind.
/// Other class publication steps are NoJs.
#[inline(never)]
pub(super) fn define_class(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    name: u32,
    has_heritage: bool,
    _identity: u64,
) -> Result<CallStep, Error> {
    use crate::engine::heap::{BytecodeConstant, RawValue};
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let parent = execution.slots.peek(&frame.window, 1)?;
    let Some(BytecodeConstant::Value(RawValue::String(name_id))) = frame.executable.constant(name)
    else {
        return Err(Error::internal("class name is not a published string"));
    };
    // The bytecode node owns the constant-pool edge for this string, so the
    // trusted read clones the payload Rc without retaining the arena node.
    let name = runtime.0.state.borrow().heap.string_fast(*name_id).clone();
    if has_heritage && let JsValue::Object(parent) = parent {
        let pending = PendingClass {
            frame: id,
            realm,
            parent: crate::engine::object::ObjectRef::from_borrowed_handle(
                runtime.clone(),
                *parent,
            )
            .map_err(super::exception::heap_error_to_vm_error)?,
            runtime: runtime.clone(),
            constructor: runtime
                .dup_jsvalue(execution.slots.peek(&frame.window, 0)?)
                .map_err(runtime_error_to_vm_error)?,
            name: name.clone(),
        };
        return enter_class_parent(runtime, execution, pending);
    }
    let result = runtime.define_class_pair(
        realm,
        parent,
        execution.slots.peek(&frame.window, 0)?,
        &name,
        has_heritage,
    );
    finish_class_result(runtime, execution, id, result)
}

fn finish_class_result(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    result: Result<
        crate::engine::vm::DefineClassOutcome,
        crate::engine::api::runtime_error::RuntimeError,
    >,
) -> Result<CallStep, Error> {
    use crate::engine::vm::DefineClassOutcome;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    // No replay once the fresh constructor/prototype pair is published.
    for _ in 0..2 {
        let discarded = execution.slots.pop(&mut frame.window)?;
        runtime
            .release_jsvalue(discarded)
            .map_err(runtime_error_to_vm_error)?;
    }
    frame.resume_pc = frame.next_pc()?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    match result {
        Ok(DefineClassOutcome::Defined {
            constructor,
            prototype,
        }) => {
            execution.slots.push(&mut frame.window, constructor)?;
            execution.slots.push(&mut frame.window, prototype)?;
            Ok(CallStep::Entered)
        }
        Ok(DefineClassOutcome::Throw(value)) => Ok(CallStep::Complete(Completion::Throw(value))),
        Err(error) => {
            let error = runtime_error_to_vm_error(error);
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(CallStep::Complete(Completion::Throw(
                runtime
                    .new_native_error_from_error_jsvalue(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            )))
        }
    }
}

pub(super) struct PendingClass {
    pub(super) frame: FrameId,
    pub(super) realm: crate::engine::heap::ContextId,
    pub(super) parent: crate::engine::object::ObjectRef,
    runtime: Runtime,
    constructor: JsValue,
    name: crate::engine::value::JsString,
}
impl Drop for PendingClass {
    /// Release the constructor edge still owned when heritage validation or
    /// parent linking abandons the pending pair.
    fn drop(&mut self) {
        let constructor = std::mem::replace(&mut self.constructor, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(constructor);
    }
}

#[inline(never)]
fn enter_class_parent(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingClass,
) -> Result<CallStep, Error> {
    if let Err(error) = runtime.validate_class_parent(&pending.parent) {
        return finish_class_result(runtime, execution, pending.frame, Err(error));
    }
    let parent = pending.parent.try_clone()?;
    let realm = pending.realm;
    let frame = pending.frame;
    super::proxy_get_driver::start_class_parent(
        runtime,
        execution,
        Box::new(pending),
        parent,
        realm,
        frame,
    )
}

pub(super) fn finish_class_reply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingClass,
    completion: Completion,
) -> Result<CallStep, Error> {
    let result = match completion {
        Completion::Throw(value) => Ok(crate::engine::vm::DefineClassOutcome::Throw(value)),
        Completion::Return(prototype) => runtime.finish_derived_class_pair(
            pending.realm,
            &pending.constructor,
            &pending.name,
            pending.parent.try_clone()?,
            prototype,
        ),
    };
    finish_class_result(runtime, execution, pending.frame, result)
}

/// Define a field or method on an ordinary object or bytecode constructor.
/// Classification precedes any property, name, or HomeObject mutation.
#[inline(never)]
pub(super) fn define_property(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    key: Option<u32>,
    method: Option<(crate::engine::code::bytecode::DefineMethodKind, bool)>,
) -> Result<CallStep, Error> {
    use crate::engine::object::PropertyKey;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let JsValue::Object(object) = execution
        .slots
        .peek(&frame.window, 1 + usize::from(key.is_none()))?
    else {
        if method.is_some() {
            return Err(Error::internal(if key.is_some() {
                "object-literal method target was not an Object"
            } else {
                "computed object-literal method target was not an Object"
            }));
        }
        return super::driver::rejected_call(
            runtime,
            realm,
            Error::new(crate::engine::api::error::ErrorKind::Type, "not an object"),
        );
    };
    let value = execution.slots.peek(&frame.window, 0)?;
    let computed = key.is_none();
    let key = match key {
        Some(index) => {
            let atom = frame
                .executable
                .property_key_atoms
                .as_ref()
                .and_then(|atoms| atoms.get(index as usize))
                .copied()
                .filter(|atom| !atom.is_null())
                .ok_or_else(|| Error::internal("definition has no linked property key"))?;
            PropertyKey::from_borrowed_atom(runtime.clone(), atom)
                .map_err(|e| Error::internal(e.to_string()))?
        }
        None => super::property_keys::canonical(runtime, execution.slots.peek(&frame.window, 1)?)?,
    };
    let depth = execution.slots.depth(&frame.window);
    if method.is_none() {
        let object =
            crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), *object)
                .map_err(super::exception::heap_error_to_vm_error)?;
        let value = execution.slots.pop(&mut frame.window)?;
        if computed {
            let discarded = execution.slots.pop(&mut frame.window)?;
            runtime
                .release_jsvalue(discarded)
                .map_err(runtime_error_to_vm_error)?;
        }
        return super::proxy_get_driver::start_public_field(
            runtime, execution, id, object, key, value, depth,
        );
    }
    let (kind, enumerable) = method.expect("method checked above");
    let object = crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), *object)
        .map_err(super::exception::heap_error_to_vm_error)?;
    let descriptor = match runtime
        .prepare_object_literal_method(&object, &key, value, kind, enumerable)
    {
        Ok(descriptor) => descriptor,
        Err(error) => {
            return super::driver::rejected_call(runtime, realm, runtime_error_to_vm_error(error));
        }
    };
    let discarded = execution.slots.pop(&mut frame.window)?;
    runtime
        .release_jsvalue(discarded)
        .map_err(runtime_error_to_vm_error)?;
    if computed {
        let discarded = execution.slots.pop(&mut frame.window)?;
        runtime
            .release_jsvalue(discarded)
            .map_err(runtime_error_to_vm_error)?;
    }
    super::proxy_get_driver::start_literal_definition(
        runtime,
        execution,
        id,
        crate::engine::object::object_literal::element::LiteralDefinitionStep::define(
            object, key, descriptor,
        ),
        depth,
    )
}

#[cfg(test)]
mod ordinary_constructor_tests {
    use crate::engine::{
        api::{Context, Runtime, Value},
        heap::RawId,
        object::ObjectRef,
        value::JsValue,
        vm::{
            call::ordinary::OrdinaryCall,
            execution::{ExecutionLimits, RunningExecution},
            frame::{FrameId, ReturnOwner, ReturnTarget, ReturnValue},
        },
    };

    fn operands(
        runtime: &Runtime,
        context: &mut Context,
        target: &ObjectRef,
        arguments: Vec<JsValue>,
        limits: ExecutionLimits,
    ) -> (RunningExecution, FrameId) {
        let Value::Object(parent) = context
            .eval("(function(){return new Object(1,2,3,4)})")
            .unwrap()
        else {
            panic!("parent function")
        };
        let mut execution = RunningExecution::new(runtime, limits).unwrap();
        let call = OrdinaryCall::select_callback(runtime, &parent)
            .unwrap()
            .unwrap();
        let entry = call
            .prepare_callback(
                &mut execution.call_storage,
                JsValue::Undefined,
                Vec::new(),
                context.realm,
                ReturnTarget {
                    owner: ReturnOwner::Root,
                    value_use: ReturnValue::Push,
                    tail: false,
                    operation: None,
                },
            )
            .unwrap();
        let id = crate::engine::vm::driver::push_frame(&mut execution, entry).unwrap();
        let frame = execution.frames.current_mut(id).unwrap();
        for value in [
            JsValue::Object(target.try_clone().expect("duplicate root").into_handle()),
            JsValue::Object(target.try_clone().expect("duplicate root").into_handle()),
        ]
        .into_iter()
        .chain(arguments)
        {
            execution.slots.push(&mut frame.window, value).unwrap();
        }
        (execution, id)
    }

    #[test]
    fn ordinary_constructor_preserves_inputs_new_target_and_return_contracts() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        for source in [
            "(()=>{let marker={};function C(a,b){this.a=a;this.b=b;this.target=new.target;return 3}let p=C.prototype;for(let i=0;i<16;i++){let o=new C(marker,i);if(o.a!==marker||o.b!==i||o.target!==C||Object.getPrototypeOf(o)!==p)return false}return true})()",
            "(()=>{let result={};function C(){this.side=1;return result}void C.prototype;return new C()===result})()",
            "(()=>{function C(a){this.a=a;return null}let first=new C(1);C.prototype={changed:42};let second=new C(2);return first.a===1&&second.a===2&&second.changed===42&&Object.getPrototypeOf(first)!==C.prototype})()",
            "(()=>{function C(){this.snapshot=arguments[0];arguments[0]=7;this.changed=arguments[0]}void C.prototype;let marker={};let o=new C(marker);return o.snapshot===marker&&o.changed===7})()",
            "(()=>{let expected={};function C(){throw expected}void C.prototype;try{new C()}catch(e){return e===expected}return false})()",
            "(()=>{function C(n){this.n=n;if(n)this.child=new C(n-1)}void C.prototype;let o=new C(4);return o.child.child.child.child.n===0})()",
            "(()=>{class C{field=42;constructor(a){this.a=a}}let o=new C(7);return o.field===42&&o.a===7})()",
            "(()=>{function Base(x){this.x=x;this.target=new.target}void Base.prototype;class Derived extends Base{constructor(){super(42)}}let o=new Derived();return o.x===42&&o.target===Derived&&o instanceof Derived})()",
        ] {
            assert_eq!(
                context
                    .eval(source)
                    .unwrap_or_else(|error| panic!("{source}: {error:?}")),
                Value::Bool(true),
                "{source}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        #[cfg(feature = "profiling")]
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("constructor_base_lazy_install")
                .copied()
                .unwrap_or(0)
                >= 16
        );
    }

    #[test]
    fn constructor_misses_preserve_bound_proxy_prototype_and_exception_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for source in [
            "(()=>{function C(x){this.x=x;this.target=new.target}let Bound=C.bind(null,42);let o=new Bound();return o.x===42&&o.target===C&&o instanceof C})()",
            "(()=>{let trace='';function C(x){trace+='body';this.x=x}let P=new Proxy(C,{get(t,k,r){if(k==='prototype')trace+='prototype:';return Reflect.get(t,k,r)}});let o=new P(42);return trace==='prototype:body'&&o.x===42})()",
            "(()=>{let marker={},calls=0;function C(){}let P=new Proxy(C,{get(t,k,r){if(k==='prototype'){calls++;throw marker}return Reflect.get(t,k,r)}});try{new P()}catch(e){return e===marker&&calls===1}return false})()",
            "(()=>{function C(){this.n=42;this.target=new.target}void C.prototype;class D extends C{constructor(){super()}}let calls=0,p=D.prototype;let P=new Proxy(D,{get(t,k,r){if(k==='prototype'){calls++;return p}return Reflect.get(t,k,r)}});let o=new P();return calls===1&&o.n===42&&o.target===P&&Object.getPrototypeOf(o)===p})()",
            "(()=>{function C(){this.x=42}C.prototype=7;let o=new C();return o.x===42&&Object.getPrototypeOf(o)===Object.prototype})()",
            "(()=>{function C(){}let count=0;let P=new Proxy(C,{construct(){count++;return {x:42}}});return new P().x===42&&count===1})()",
            "(()=>{let order='';try{new (()=>{})(order+='argument')}catch(e){return e instanceof TypeError&&order==='argument'}return false})()",
            "(()=>{let marker={};function C(){this.x=42}function N(){}Object.defineProperty(N,'prototype',{value:null});let o=Reflect.construct(C,[],N);return o.x===42&&Object.getPrototypeOf(o)===Object.prototype})()",
        ] {
            assert_eq!(
                context
                    .eval(source)
                    .unwrap_or_else(|error| panic!("{source}: {error:?}")),
                Value::Bool(true),
                "{source}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn lazy_constructor_materializes_its_call_pc_for_an_observed_error() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval_with_filename(
            "function C(){\n this.stack = new Error('observed').stack;\n}\nfunction outer(){\n return new C();\n}\nvoid C.prototype;\nvar object=outer();\nobject.stack.includes('at C (constructor-observe.js:2:') && object.stack.includes('at outer (constructor-observe.js:5:') && object.stack.split('at C (').length===2",
            "constructor-observe.js",
        ).unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn ordinary_constructor_uses_its_defining_realm_and_captured_values() {
        let runtime = Runtime::new();
        let mut defining = runtime.new_context().expect("create context");
        let function = defining.eval("let captured={};function Foreign(){this.captured=captured;this.array=Array;this.target=new.target}void Foreign.prototype;Foreign").unwrap();
        let expected_array = defining.eval("Array").unwrap();
        let expected_capture = defining.eval("captured").unwrap();
        let mut caller = runtime.new_context().expect("create context");
        let global = caller.global_object().unwrap();
        for (name, value) in [
            ("Foreign", function),
            ("ExpectedArray", expected_array),
            ("ExpectedCapture", expected_capture),
        ] {
            assert!(
                caller
                    .set_property(&global, &runtime.intern_property_key(name).unwrap(), value)
                    .unwrap()
            );
        }
        assert_eq!(caller.eval("let first=new Foreign();let second=new Foreign();first.captured===ExpectedCapture&&second.captured===ExpectedCapture&&first.array===ExpectedArray&&first.target===Foreign&&second.target===Foreign&&ExpectedArray!==Array").unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn constructor_admission_misses_do_not_promote_saturated_role_aliases() {
        for source in [
            "(function(){function C(){}C.prototype=C;return C})()",
            "(function(){class D extends Object{};void D.prototype;return D})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let Value::Object(target) = context.eval(source).unwrap() else {
                panic!("constructor")
            };
            let arguments = vec![
                JsValue::Object(target.try_clone().expect("duplicate root").into_handle()),
                JsValue::Object(target.try_clone().expect("duplicate root").into_handle()),
            ];
            let (mut execution, id) = operands(
                &runtime,
                &mut context,
                &target,
                arguments,
                ExecutionLimits::default(),
            );
            let roles = {
                let state = runtime.0.state.borrow();
                let data = state.heap.object(target.object_id()).unwrap();
                let crate::engine::heap::ObjectPayload::BytecodeFunction { bytecode, .. } =
                    &data.payload
                else {
                    panic!("bytecode constructor")
                };
                let key = state
                    .pinned_atoms
                    .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
                let slot = state
                    .heap
                    .shape(data.shape)
                    .unwrap()
                    .find(crate::engine::atom::AtomIdx::from_raw(key.raw()))
                    .unwrap();
                let crate::engine::heap::PropertySlot::Data(crate::engine::heap::RawValue::Object(
                    prototype,
                )) = &data.slots[slot as usize]
                else {
                    panic!("data prototype")
                };
                [
                    RawId::Object(target.object_id()),
                    RawId::Object(*prototype),
                    RawId::FunctionBytecode(*bytecode),
                ]
            };
            for role in roles {
                let original = runtime.0.state.borrow().heap.strong_count(role).unwrap();
                for count in [u32::MAX - 3, u32::MAX - 1, u32::MAX] {
                    runtime
                        .0
                        .state
                        .borrow_mut()
                        .heap
                        .set_strong_count_for_test(role, count);
                    assert!(!super::try_ordinary_base(&runtime, &mut execution, id, 2).unwrap());
                    assert_eq!(runtime.0.state.borrow().heap.strong_count(role), Ok(count));
                    let frame = execution.frames.current_mut(id).unwrap();
                    assert_eq!(execution.slots.depth(&frame.window), 4);
                    assert_eq!(frame.resume_pc, 0);
                    assert_eq!(execution.frames.depth(), 1);
                }
                runtime
                    .0
                    .state
                    .borrow_mut()
                    .heap
                    .set_strong_count_for_test(role, original);
            }
        }
    }

    #[test]
    fn constructor_admission_preserves_pending_cleanup_and_depth_misses() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(target) = context
            .eval("(function(){function C(){}void C.prototype;return C})()")
            .unwrap()
        else {
            panic!("constructor")
        };
        let (mut execution, id) = operands(
            &runtime,
            &mut context,
            &target,
            Vec::new(),
            ExecutionLimits::default(),
        );
        let pending = runtime.new_object(None).unwrap().into_handle();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .queue_release_for_test(RawId::Object(pending))
            .unwrap();
        assert!(!super::try_ordinary_base(&runtime, &mut execution, id, 0).unwrap());
        assert!(runtime.0.state.borrow().heap.has_pending_zero_cleanup());
        drop(runtime.new_object(None).unwrap());
        let pending = runtime.new_object(None).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            drop(pending);
        }
        assert!(!super::try_ordinary_base(&runtime, &mut execution, id, 0).unwrap());
        assert!(runtime.0.deferred_references.has_pending());
        runtime.drain_deferred_references().unwrap();
        execution.frames.materialize(&runtime).unwrap();
        let previous = runtime.0.host_stack_top.replace(Some(0));
        assert!(!super::try_ordinary_base(&runtime, &mut execution, id, 0).unwrap());
        runtime.0.host_stack_top.set(previous);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(frame.resume_pc, 0);
        drop(execution);

        runtime
            .set_recursion_limit(6)
            .expect("set runtime configuration");
        assert_eq!(context.eval("function C(n){if(n)new C(n-1)}void C.prototype;try{new C(Infinity);'missing'}catch(e){e.name+':'+e.message}").unwrap(), Value::String(crate::engine::value::JsString::from_static("InternalError:stack overflow")));
        assert_eq!(context.eval("new C(1);6*7").unwrap(), Value::Int(42));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn constructor_slot_limit_failure_releases_unpublished_child_inputs() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(target) = context.eval("(function(){function C(a,b,c){let local=a;this.result=b;return local}void C.prototype;return C})()").unwrap() else {
            panic!("constructor")
        };
        let marker = runtime.new_object(None).unwrap();
        let arguments = (0..3)
            .map(|_| JsValue::Object(marker.try_clone().expect("duplicate root").into_handle()))
            .collect();
        let (mut execution, id) = operands(
            &runtime,
            &mut context,
            &target,
            arguments,
            ExecutionLimits {
                frames: 4,
                slots: 8,
            },
        );
        runtime.run_gc().unwrap();
        let objects = runtime.heap_counts().expect("runtime state").object_nodes;
        let target_count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(target.object_id())
            .unwrap();
        let marker_count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(marker.object_id())
            .unwrap();
        let error = super::try_ordinary_base(&runtime, &mut execution, id, 3).unwrap_err();
        assert!(error.to_string().contains("execution slot limit exceeded"));
        assert_eq!(execution.frames.depth(), 1);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 0);
        runtime.run_gc().unwrap();
        assert_eq!(
            runtime.heap_counts().expect("runtime state").object_nodes,
            objects
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(target.object_id()),
            Ok(target_count - 2)
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(marker.object_id()),
            Ok(marker_count - 3)
        );
        drop(execution);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
    }

    #[test]
    fn constructor_allocation_preserves_inputs_until_ready_collection() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(target) = context
            .eval("(function(){function C(a){return a}void C.prototype;return C})()")
            .unwrap()
        else {
            panic!("constructor")
        };
        let Value::Object(marker) = context
            .eval("(()=>{let marker={};marker.self=marker;return marker})()")
            .unwrap()
        else {
            panic!("marker")
        };
        let marker_id = marker.object_id();
        let arguments = vec![JsValue::Object(marker.into_handle())];
        let (mut execution, id) = operands(
            &runtime,
            &mut context,
            &target,
            arguments,
            ExecutionLimits::default(),
        );
        runtime.0.gc_pressure.remaining.set(1);
        assert!(super::try_ordinary_base(&runtime, &mut execution, id, 1).unwrap());
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        let child = execution.frames.current_id().unwrap();
        // ready::run services this method before its next execute_frame. The
        // new receiver requests collection without collecting inside install.
        runtime.collect_if_requested().unwrap();
        assert!(matches!(
            crate::engine::vm::execute::execute_frame(&mut execution, child).unwrap(),
            crate::engine::vm::execute::VmAction::Complete
        ));
        assert!(runtime.0.gc_pressure.remaining.get() > 0);
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_ok());
        assert!(matches!(execution.pending, Some(JsValue::Object(object)) if object == marker_id));
        drop(execution);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_err());
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[cfg(all(test, feature = "profiling"))]
mod owned_definition_tests {
    use crate::engine::{
        api::{profiling::CostProfile, runtime::Runtime},
        value::Value,
        vm::Completion,
    };

    #[test]
    fn rejected_calls_and_default_super_stay_owned() {
        for source in [
            "(function(){try{(42)()}catch(e){return e instanceof TypeError&&e.message==='not a function'?42:0}})",
            "(function(){try{({f:{}}).f()}catch(e){return e instanceof TypeError&&e.message==='not a function'?42:0}})",
            "(function(){try{return (null)()}catch(e){return e instanceof TypeError?42:0}})",
            "(function(){class D extends Object{};Object.setPrototypeOf(D,null);try{new D}catch(e){return e instanceof TypeError&&e.message==='not a function'?42:0}})",
            "(function(){class D extends Object{};Object.setPrototypeOf(D,{});try{new D}catch(e){return e instanceof TypeError?42:0}})",
            "(function(){class D extends Object{};Object.setPrototypeOf(D,()=>1);try{new D}catch(e){return e instanceof TypeError?42:0}})",
            "(function(){class D extends Object{};try{D()}catch(e){return e instanceof TypeError&&e.message===\"class constructors must be invoked with 'new'\"?42:0}})",
            "(function(){var n=0,p=new Proxy({}, {get apply(){n++;return undefined}});try{p()}catch(e){return e instanceof TypeError&&n===1?42:0}})",
            "(function(){var marker={},p=new Proxy({}, {get apply(){throw marker}});try{p()}catch(e){return e===marker?42:0}})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let callable = runtime
                .callable_from_value(context.eval(source).unwrap())
                .unwrap();
            let profile = CostProfile::start();
            let result = runtime
                .call_internal(context.realm, &callable, Value::Undefined, &[])
                .unwrap();
            let _costs = profile.snapshot();
            assert!(
                matches!(
                    result,
                    Completion::Return(crate::engine::value::JsValue::Int(42))
                ),
                "{source}: {result:?}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn class_heritage_and_exotic_public_fields_keep_callbacks_in_owned_frames() {
        for source in [
            "(function(){var calls=0;var parent=new Proxy(function(){},{get(t,k,r){if(k==='prototype'){calls++;return Object.prototype}return Reflect.get(t,k,r)}});return function(){class C extends parent{}return calls===1?42:0}})()",
            "(function(){var calls=0,target={},proxy=new Proxy(target,{defineProperty(t,k,d){calls++;if(k!=='x'||d.value!==42||!d.writable||!d.enumerable||!d.configurable)throw 99;return Reflect.defineProperty(t,k,d)}});class Base{constructor(){return proxy}}return function(){class C extends Base{x=42}new C;return calls===1&&target.x===42?42:0}})()",
            "(function(){var calls=0,target=new Uint8Array(1);class Base{constructor(){return target}}return function(){class C extends Base{0={valueOf(){calls++;return 42}}}new C;return calls===1&&target[0]===42?42:0}})()",
            "(function(){var marker={},calls=0,proxy=new Proxy({}, {defineProperty(){calls++;throw marker}});class Base{constructor(){return proxy}}return function(){try{class C extends Base{x=42}new C}catch(e){return e===marker&&calls===1?42:0}return 0}})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let callable = runtime
                .callable_from_value(context.eval(source).unwrap())
                .unwrap();
            let profile = CostProfile::start();
            let result = runtime
                .call_internal(context.realm, &callable, Value::Undefined, &[])
                .unwrap();
            let _costs = profile.snapshot();
            assert!(
                matches!(
                    result,
                    Completion::Return(crate::engine::value::JsValue::Int(42))
                ),
                "{source}: {result:?}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }
}
