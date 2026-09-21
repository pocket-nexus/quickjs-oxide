//! Pure opcode leaves shared by the synchronous consumer and owned cold entry.
use crate::engine::{
    api::{
        error::{Error, ErrorKind},
        runtime::Runtime,
    },
    code::runtime::PublishedFunctionSnapshot,
    heap::ContextId,
    heap::{BytecodeConstant, ObjectPayload},
    value::{JsString, JsValue},
    vm::{Completion, exception::runtime_error_to_vm_error},
};

fn allocate_string_node(runtime: &Runtime, string: JsString) -> Result<JsValue, Error> {
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .allocate_string(string)
        .map(JsValue::String)
        .map_err(|error| Error::internal(error.to_string()))
}

fn value_is_html_dda(runtime: &Runtime, value: &JsValue) -> Result<bool, Error> {
    let JsValue::Object(id) = value else {
        return Ok(false);
    };
    Ok(runtime
        .0
        .state
        .borrow()
        .heap
        .object(*id)
        .map_err(|error| Error::internal(error.to_string()))?
        .is_html_dda)
}

fn value_is_callable(runtime: &Runtime, value: &JsValue) -> Result<bool, Error> {
    let JsValue::Object(id) = value else {
        return Ok(false);
    };
    let state = runtime.0.state.borrow();
    let object = state
        .heap
        .object(*id)
        .map_err(|error| Error::internal(error.to_string()))?;
    Ok(matches!(
        &object.payload,
        ObjectPayload::NativeFunction { .. }
            | ObjectPayload::BoundFunction { .. }
            | ObjectPayload::BytecodeFunction { .. }
            | ObjectPayload::Proxy(crate::engine::heap::ProxyData {
                is_callable: true,
                ..
            })
    ))
}

/// Load a published value constant while its executable owns the raw edge.
/// Template objects and Symbols need the same checked retain as the old host.
pub(super) fn load_value_constant(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    index: u32,
) -> Result<JsValue, Error> {
    let constant = executable
        .constant(index)
        .ok_or_else(|| Error::internal("constant index is out of bounds"))?;
    match constant {
        BytecodeConstant::Value(value) => {
            let value = JsValue::from_raw(value.clone())
                .ok_or_else(|| Error::internal("constant sentinel escaped"))?;
            runtime
                .dup_jsvalue(&value)
                .map_err(runtime_error_to_vm_error)
        }
        BytecodeConstant::Function(_) => Err(Error::internal(
            "child function bytecode was loaded with a value-constant opcode",
        )),
        BytecodeConstant::RegExp { .. } => Err(Error::internal(
            "RegExp program was loaded with a value-constant opcode",
        )),
    }
}

/// QuickJS `OP_typeof` converts one of its predefined type atoms back to
/// the atom's canonical String cell. Runtime construction pins the full
/// result set, so every realm reuses the same representation while sibling
/// runtimes remain isolated.
fn canonical_typeof_string(runtime: &Runtime, spelling: &'static str) -> Result<JsString, Error> {
    let mut state = runtime.0.state.borrow_mut();
    let atom = state
        .atoms
        .intern_static(spelling)
        .map_err(|error| runtime_error_to_vm_error(error.into()))?;
    state
        .atoms
        .to_js_string(atom)
        .map_err(|error| runtime_error_to_vm_error(error.into()))
}

pub(super) fn type_of(runtime: &Runtime, value: &JsValue) -> Result<JsString, Error> {
    let JsValue::Object(object) = value else {
        return canonical_typeof_string(runtime, value.type_of());
    };
    let state = runtime.0.state.borrow();
    let object = state
        .heap
        .object(*object)
        .map_err(|error| Error::internal(error.to_string()))?;
    if object.is_html_dda {
        drop(state);
        return canonical_typeof_string(runtime, "undefined");
    }
    let spelling = match &object.payload {
        ObjectPayload::NativeFunction { .. }
        | ObjectPayload::BoundFunction { .. }
        | ObjectPayload::BytecodeFunction { .. } => "function",
        ObjectPayload::Proxy(proxy) if proxy.is_callable => "function",
        ObjectPayload::Proxy(_) => "object",
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
        | ObjectPayload::AsyncGenerator(_) => "object",
    };
    drop(state);
    canonical_typeof_string(runtime, spelling)
}

pub(super) fn create_regexp(
    runtime: &Runtime,
    realm: ContextId,
    executable: &PublishedFunctionSnapshot,
    index: u32,
) -> Result<Completion, Error> {
    let (pattern, program) = match executable.constant(index) {
        Some(BytecodeConstant::RegExp { pattern, program }) => (pattern.clone(), program.clone()),
        Some(BytecodeConstant::Value(_) | BytecodeConstant::Function(_)) => {
            return Err(Error::internal(
                "RegExp opcode referenced a non-RegExp constant",
            ));
        }
        None => return Err(Error::internal("constant index is out of bounds")),
    };
    let object = runtime
        .new_compiled_regexp_literal(realm, pattern, program)
        .map_err(runtime_error_to_vm_error)?;
    let id = object.object_id();
    runtime
        .retain_object_handle(id)
        .map_err(|error| runtime_error_to_vm_error(error.into()))?;
    Ok(Completion::Return(JsValue::Object(id)))
}

pub(super) fn set_object_prototype(
    runtime: &Runtime,
    object: JsValue,
    prototype: JsValue,
) -> Result<Completion, Error> {
    let result = set_object_prototype_ref(runtime, &object, &prototype);
    runtime
        .release_jsvalue(object)
        .map_err(runtime_error_to_vm_error)?;
    runtime
        .release_jsvalue(prototype)
        .map_err(runtime_error_to_vm_error)?;
    result
}

fn set_object_prototype_ref(
    runtime: &Runtime,
    object: &JsValue,
    prototype: &JsValue,
) -> Result<Completion, Error> {
    let JsValue::Object(object) = object else {
        return Err(Error::internal(
            "object-literal prototype target was not an Object",
        ));
    };
    let prototype = match prototype {
        JsValue::Object(prototype) => Some(
            crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), *prototype)
                .map_err(|error| runtime_error_to_vm_error(error.into()))?,
        ),
        JsValue::Null => None,
        // Pinned QuickJS `OP_set_proto` consumes every primitive without
        // changing the fresh literal.
        _ => return Ok(Completion::Return(JsValue::Undefined)),
    };
    let object = crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), *object)
        .map_err(|error| runtime_error_to_vm_error(error.into()))?;
    let changed = runtime
        .set_prototype_of(&object, prototype.as_ref())
        .map_err(runtime_error_to_vm_error)?;
    if !changed {
        return Err(Error::new(ErrorKind::Type, "prototype is immutable"));
    }
    Ok(Completion::Return(JsValue::Undefined))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PureOperation {
    Constant(u32),
    AtomValue(u32),
    RegExp(u32),
    DeleteSuper,
    ConstructorWithoutNew,
    IteratorCheckObject,
    IteratorMissingThrow,
    InitializeClosure { index: u16, derived: bool },
    InitializeModuleImportCollision(u16),
    SetPrototype,
    TypeOf,
    IsUndefinedOrNull,
    IsUndefined,
    IsNull,
    TypeOfIsUndefined,
    TypeOfIsFunction,
    Branch { target: u32, when: bool },
}

pub(super) fn step(
    runtime: &Runtime,
    execution: &mut super::execution::RunningExecution,
    id: super::frame::FrameId,
    operation: PureOperation,
) -> Result<super::driver::CallStep, Error> {
    use super::driver::CallStep;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let outcome = perform(runtime, execution, id, operation);
    match outcome {
        Ok(target) => {
            let frame = execution.frames.current_mut(id)?;
            frame.resume_pc = match target {
                Some(target) => target,
                None => frame
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("pure operation resume PC overflow"))?,
            };
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            Ok(CallStep::Entered)
        }
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            let value = runtime
                .new_native_error_from_error_jsvalue(realm, kind, &error)
                .map_err(runtime_error_to_vm_error)?;
            Ok(CallStep::Complete(Completion::Throw(value)))
        }
    }
}

fn perform(
    runtime: &Runtime,
    execution: &mut super::execution::RunningExecution,
    id: super::frame::FrameId,
    operation: PureOperation,
) -> Result<Option<usize>, Error> {
    let frame = execution.frames.current_mut(id)?;
    let slots = &mut execution.slots;
    use PureOperation as P;
    let result = match operation {
        P::Constant(index) => {
            let value = load_value_constant(runtime, &frame.executable, index)?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_storage(
                crate::engine::api::profiling::OwnedStorageEvent::Copy {
                    heap_root: matches!(value, JsValue::Object(_) | JsValue::Symbol(_)),
                },
            );
            value
        }
        P::IteratorCheckObject => {
            let value = slots.peek(&frame.window, 0)?;
            if !matches!(value, JsValue::Object(_)) {
                return Err(Error::new(
                    ErrorKind::Type,
                    "iterator must return an object",
                ));
            }
            return Ok(None);
        }
        P::IteratorMissingThrow => return Err(super::iterator_support::missing_throw()),
        P::AtomValue(value) => {
            allocate_string_node(runtime, JsString::from_fresh_decimal_u32(value))?
        }
        P::RegExp(index) => {
            match create_regexp(runtime, frame.executable.realm, &frame.executable, index)? {
                Completion::Return(value) => value,
                Completion::Throw(value) => {
                    runtime
                        .release_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?;
                    return Err(Error::internal(
                        "pure RegExp literal unexpectedly returned a completion throw",
                    ));
                }
            }
        }
        P::DeleteSuper => {
            for _ in 0..3 {
                let discarded = slots.pop(&mut frame.window)?;
                runtime
                    .release_jsvalue(discarded)
                    .map_err(runtime_error_to_vm_error)?;
            }
            return Err(Error::new(
                ErrorKind::Reference,
                "unsupported reference to 'super'",
            ));
        }
        P::ConstructorWithoutNew => {
            return Err(Error::new(
                ErrorKind::Type,
                "class constructors must be invoked with 'new'",
            ));
        }
        P::InitializeModuleImportCollision(index) => {
            let value = slots.pop(&mut frame.window)?;
            let descriptor = frame
                .executable
                .closure_variables
                .get(usize::from(index))
                .copied()
                .ok_or_else(|| Error::internal("closure variable index is out of bounds"))?;
            super::bindings::validate_module_import_collision(descriptor)?;
            let root = frame
                .cold
                .closure_slots
                .get(usize::from(index))
                .ok_or_else(|| Error::internal("closure variable index is out of bounds"))?;
            runtime
                .write_var_ref(&root, value)
                .map_err(runtime_error_to_vm_error)?;
            return Ok(None);
        }
        P::InitializeClosure { index, derived } => {
            let value = slots.pop(&mut frame.window)?;
            let root = frame
                .cold
                .closure_slots
                .get(usize::from(index))
                .ok_or_else(|| Error::internal("closure variable index is out of bounds"))?
                .clone();
            if derived {
                let descriptor = frame
                    .executable
                    .closure_variables
                    .get(usize::from(index))
                    .copied()
                    .ok_or_else(|| Error::internal("closure variable index is out of bounds"))?;
                super::bindings::initialize_derived_closure(runtime, &root, descriptor, value)?;
            } else {
                runtime
                    .write_var_ref(&root, value)
                    .map_err(runtime_error_to_vm_error)?;
            }
            return Ok(None);
        }
        P::SetPrototype => {
            let prototype = slots.pop(&mut frame.window)?;
            let object = slots.pop(&mut frame.window)?;
            let retained = runtime
                .dup_jsvalue(&object)
                .map_err(runtime_error_to_vm_error)?;
            match set_object_prototype(runtime, object, prototype)? {
                Completion::Return(value) => {
                    runtime
                        .release_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?;
                    retained
                }
                Completion::Throw(value) => {
                    runtime
                        .release_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?;
                    runtime
                        .release_jsvalue(retained)
                        .map_err(runtime_error_to_vm_error)?;
                    return Err(Error::internal(
                        "pure literal prototype unexpectedly returned a completion throw",
                    ));
                }
            }
        }
        P::Branch { target, when } => {
            let value = slots.pop(&mut frame.window)?;
            let truthy = runtime
                .value_to_boolean_jsvalue(&value)
                .map_err(runtime_error_to_vm_error)?;
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            return Ok((truthy == when).then_some(target as usize));
        }
        P::TypeOf
        | P::IsUndefinedOrNull
        | P::IsUndefined
        | P::IsNull
        | P::TypeOfIsUndefined
        | P::TypeOfIsFunction => {
            let value = slots.pop(&mut frame.window)?;
            let result = match operation {
                P::TypeOf => allocate_string_node(runtime, type_of(runtime, &value)?)?,
                P::IsUndefinedOrNull => {
                    JsValue::Bool(matches!(value, JsValue::Null | JsValue::Undefined))
                }
                P::IsUndefined => JsValue::Bool(matches!(value, JsValue::Undefined)),
                P::IsNull => JsValue::Bool(matches!(value, JsValue::Null)),
                P::TypeOfIsUndefined => JsValue::Bool(
                    matches!(value, JsValue::Undefined) || value_is_html_dda(runtime, &value)?,
                ),
                P::TypeOfIsFunction => JsValue::Bool(
                    !value_is_html_dda(runtime, &value)? && value_is_callable(runtime, &value)?,
                ),
                _ => unreachable!(),
            };
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            result
        }
    };
    slots.push(&mut frame.window, result)?;
    Ok(None)
}
