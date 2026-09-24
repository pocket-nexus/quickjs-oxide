use super::{
    ActiveFrameFlags, ActiveFrameKind, DeferredRefOp, PropertyGetAction, PropertySetAction,
    Runtime, RuntimeError,
};
use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind, NativeErrorMessage};

use crate::engine::api::{EvalOptions, JsBigInt};
use crate::engine::builtins::native::{
    ArrayJoinKind, DynamicFunctionKind, FunctionDebugPosition, NativeCProto, NativeFunctionId,
    PrimitiveKind,
};
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::debug::{DebugInfoMode, Pc2LineEntry, Pc2LineTable};
use crate::engine::code::dynamic_source::DynamicSourceBuilder;
use crate::source::LineColumn;

use crate::engine::atom::AtomIdx;
use crate::engine::code::function::metadata::{
    ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName, ConstructorKind,
    EvalKind, FunctionMetadata,
};
use crate::engine::code::function::{
    UnlinkedConstant, UnlinkedFunction, UnlinkedFunctionDebug, UnlinkedVariableDefinition,
};
use crate::engine::compiler::{
    CompileOptions, EvalCompileContext, compile_unlinked_eval_with_filename,
    compile_unlinked_module_with_filename,
};
use crate::engine::heap::roots::VarRefRoot;
use crate::engine::heap::{HeapError, ObjectPayload, PrimitiveObjectData, PropertySlot, RawValue};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{
    AccessorValue, CallableRef, CompleteOrdinaryPropertyDescriptor, DescriptorField,
    OrdinaryPropertyDescriptor, PropertyKey, WellKnownSymbol,
};
use crate::engine::value::{JsString, JsStringError, JsValue, Value};
use crate::engine::vm::call::CallableExecution;

use crate::engine::vm::{Completion, ToPrimitiveHint};

fn expect_string_value(value: Value) -> JsString {
    let Value::String(value) = value else {
        panic!("scalar String execution returned another value kind");
    };
    value
}

fn data_descriptor(
    value: Value,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) -> OrdinaryPropertyDescriptor {
    OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(value),
        writable: DescriptorField::Present(writable),
        enumerable: DescriptorField::Present(enumerable),
        configurable: DescriptorField::Present(configurable),
        ..OrdinaryPropertyDescriptor::new()
    }
}

fn set_property(
    runtime: &Runtime,
    object: &crate::engine::api::ObjectRef,
    key: &PropertyKey,
    value: Value,
) -> Result<bool, RuntimeError> {
    match runtime.prepare_set_property(object, key, value)? {
        PropertySetAction::Complete => Ok(true),
        PropertySetAction::Rejected(_) | PropertySetAction::RejectedProxyTrap => Ok(false),
        PropertySetAction::Throw(value) => {
            runtime.release_jsvalue(value)?;
            Err(RuntimeError::Invariant(
                "context-free property test produced a JavaScript throw",
            ))
        }
        PropertySetAction::Call { .. } => Err(RuntimeError::Invariant(
            "ordinary-property test helper unexpectedly reached a setter",
        )),
    }
}

fn set_property_with_receiver(
    runtime: &Runtime,
    object: &crate::engine::api::ObjectRef,
    key: &PropertyKey,
    value: Value,
    receiver: Value,
) -> Result<bool, RuntimeError> {
    match runtime.prepare_set_property_with_receiver(object, key, value, receiver)? {
        PropertySetAction::Complete => Ok(true),
        PropertySetAction::Rejected(_) | PropertySetAction::RejectedProxyTrap => Ok(false),
        PropertySetAction::Throw(value) => {
            runtime.release_jsvalue(value)?;
            Err(RuntimeError::Invariant(
                "context-free property test produced a JavaScript throw",
            ))
        }
        PropertySetAction::Call { .. } => Err(RuntimeError::Invariant(
            "ordinary-property test helper unexpectedly reached a setter",
        )),
    }
}

fn get_property(
    runtime: &Runtime,
    object: &crate::engine::api::ObjectRef,
    key: &PropertyKey,
) -> Result<Value, RuntimeError> {
    match runtime.prepare_get_property(object, key)? {
        PropertyGetAction::Complete(value) => Ok(value),
        PropertyGetAction::Call { .. } => Err(RuntimeError::Invariant(
            "ordinary-property test helper unexpectedly reached a getter",
        )),
    }
}

fn global_callable(
    runtime: &Runtime,
    context: &mut crate::engine::api::context::Context,
    name: &str,
) -> CallableRef {
    let key = runtime.intern_property_key(name).unwrap();
    let Value::Object(object) = context
        .get_property(&context.global_object().unwrap(), &key)
        .unwrap()
    else {
        panic!("global {name} was not an object");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("global {name} was not callable"))
}

fn eval_callable(
    runtime: &Runtime,
    context: &mut crate::engine::api::context::Context,
    source: &str,
) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("callable source did not produce an object: {source:?}");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("source did not produce a callable: {source:?}"))
}

fn property_callable(
    runtime: &Runtime,
    context: &mut crate::engine::api::context::Context,
    object: &crate::engine::api::ObjectRef,
    name: &str,
) -> CallableRef {
    let key = runtime.intern_property_key(name).unwrap();
    let Value::Object(value) = context.get_property(object, &key).unwrap() else {
        panic!("property {name} was not an object");
    };
    runtime
        .as_callable(&value)
        .unwrap()
        .unwrap_or_else(|| panic!("property {name} was not callable"))
}

fn own_key_names(runtime: &Runtime, object: &crate::engine::api::ObjectRef) -> Vec<String> {
    runtime
        .own_property_keys(object)
        .unwrap()
        .into_iter()
        .map(|key| {
            runtime
                .property_key_to_js_string(&key)
                .unwrap()
                .to_utf8_lossy()
        })
        .collect()
}

fn own_data_value(runtime: &Runtime, object: &crate::engine::api::ObjectRef, name: &str) -> Value {
    let key = runtime.intern_property_key(name).unwrap();
    let Some(CompleteOrdinaryPropertyDescriptor::Data { value, .. }) =
        runtime.get_own_property(object, &key).unwrap()
    else {
        panic!("{name} was not an own data property");
    };
    value
}

fn own_stack_string(runtime: &Runtime, object: &crate::engine::api::ObjectRef) -> JsString {
    let Value::String(stack) = own_data_value(runtime, object, "stack") else {
        panic!("stack was not a string");
    };
    stack
}

fn take_error_message(
    runtime: &Runtime,
    context: &mut crate::engine::api::context::Context,
) -> JsString {
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("pending exception was not an Error object");
    };
    let message = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Message)
        .unwrap();
    let Value::String(message) = context.get_property(&error, &message).unwrap() else {
        panic!("Error.message was not a string");
    };
    message
}

fn bytecode_callable(
    runtime: &Runtime,
    context: &crate::engine::api::context::Context,
    code: Vec<Instruction>,
    metadata: FunctionMetadata,
) -> CallableRef {
    let function = runtime
        .publish_unlinked_function(
            context.realm,
            UnlinkedFunction::fixture(code, Vec::new(), metadata),
        )
        .unwrap();
    runtime
        .new_bytecode_closure(context.realm, &function)
        .unwrap()
}

fn push_named_script_active_frame(
    runtime: &Runtime,
    context: &mut crate::engine::api::context::Context,
    filename: &str,
) -> super::ActiveFrameGuard {
    let bytecode = context
        .compile_with_filename("'use strict'; void 0;", filename)
        .unwrap();
    let callable = runtime
        .new_bytecode_closure(context.realm, &bytecode)
        .unwrap();
    runtime
        .push_bytecode_active_frame(callable.as_object().clone(), bytecode, context.realm, true)
        .unwrap()
}

fn push_named_eval_active_frame(
    runtime: &Runtime,
    context: &crate::engine::api::context::Context,
    filename: &str,
    kind: EvalKind,
) -> super::ActiveFrameGuard {
    let compile_context = match kind {
        EvalKind::Direct => EvalCompileContext::direct(false, Vec::new()),
        EvalKind::Indirect => EvalCompileContext::indirect(),
        EvalKind::None => panic!("test eval frame requires an eval kind"),
    };
    let function = compile_unlinked_eval_with_filename(
        "",
        filename,
        runtime.debug_info_mode(),
        compile_context,
    )
    .unwrap();
    let function = crate::engine::code::bytecode_publish::VerifiedFunction::eval(
        function,
        crate::engine::api::compile::eval_publication_input(&match kind {
            EvalKind::Direct => EvalCompileContext::direct(false, Vec::new()),
            EvalKind::Indirect => EvalCompileContext::indirect(),
            EvalKind::None => unreachable!(),
        }),
    )
    .unwrap();
    let bytecode = runtime
        .publish_verified_unlinked_function(context.realm, function)
        .unwrap();
    let callable = runtime
        .new_bytecode_closure_with_slots(context.realm, &bytecode, &[])
        .unwrap();
    runtime
        .push_bytecode_active_frame(callable.as_object().clone(), bytecode, context.realm, false)
        .unwrap()
}

fn push_named_module_active_frame(
    runtime: &Runtime,
    context: &crate::engine::api::context::Context,
    filename: &str,
) -> super::ActiveFrameGuard {
    let module =
        compile_unlinked_module_with_filename("", filename, runtime.debug_info_mode()).unwrap();
    let function = crate::engine::code::bytecode_publish::VerifiedFunction::module(module)
        .unwrap()
        .function;
    let bytecode = runtime
        .publish_verified_unlinked_function(context.realm, function)
        .unwrap();
    let callable = runtime
        .new_bytecode_closure_with_slots(context.realm, &bytecode, &[])
        .unwrap();
    runtime
        .push_bytecode_active_frame(callable.as_object().clone(), bytecode, context.realm, true)
        .unwrap()
}

fn incrementing_closure(source: ClosureSource) -> UnlinkedFunction {
    UnlinkedFunction::fixture_with_closure_variables(
        vec![
            Instruction::GetVarRef(0),
            Instruction::PushI32(1),
            Instruction::Add,
            Instruction::SetVarRef(0),
            Instruction::Return,
        ],
        Vec::new(),
        FunctionMetadata {
            closure_count: 1,
            max_stack: 2,
            strict: true,
            ..FunctionMetadata::default()
        },
        vec![ClosureVariable {
            source,
            name: crate::engine::code::function::metadata::ClosureVariableName::None,
            is_lexical: false,
            is_const: false,
            kind: ClosureVariableKind::Normal,
        }],
    )
}

fn debug_draft(debug: UnlinkedFunctionDebug) -> UnlinkedFunction {
    UnlinkedFunction::fixture(
        vec![Instruction::Undefined, Instruction::Return],
        Vec::new(),
        FunctionMetadata {
            max_stack: 1,
            ..FunctionMetadata::default()
        },
    )
    .with_debug(debug)
}

mod atoms;

mod host_policy;

mod errors;

mod dynamic_functions;

mod eval;

mod arrays;

mod host_gc;

mod realms;

mod primitive_intrinsics;

mod strings;

mod symbols;

mod globals;

mod source;

mod boolean_objects;

mod iterators;

mod publication;

mod calls;

mod function_objects;

mod constructors;

mod debug;

mod bound_functions;

mod backtraces;

mod coercion;

mod properties;

mod native_calls;

mod active_frames;

mod accessors;

mod lexical_cells;

mod exceptions;

mod closures;

mod ownership;

mod gc;

mod shapes;

mod weak_references;

mod dynamic_import;
