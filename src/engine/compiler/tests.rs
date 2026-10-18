use super::destructuring::ParenthesizedParameterScan;
use super::*;
use crate::engine::api::core::{Context, Runtime, RuntimeError, Value};
use crate::engine::api::error::ErrorKind;
use crate::engine::compiler::model::ir::function::FunctionIr;
use crate::engine::compiler::model::ir::function::FunctionIrOptions;
use crate::engine::compiler::model::ir::function::FunctionKind;
use crate::engine::compiler::model::ir::function::FunctionSourceInfo;
use crate::engine::compiler::model::ir::function::SuperCapabilities;
use crate::engine::compiler::names::NameTable;
use crate::engine::compiler::parser::context::InMode;
use crate::engine::compiler::parser::context::ModuleDeclarationExport;
use crate::engine::compiler::parser::context::Parser;
use crate::engine::compiler::parser::diagnostics::lex_error;
use crate::engine::test_support::test_context;

use crate::engine::code::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, DynamicEnvironmentSource, EvalVariableSource,
    Instruction, IteratorCallKind,
};
use crate::engine::code::debug::DebugInfoMode;

use crate::engine::api::core::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, DescriptorField, OrdinaryPropertyDescriptor,
    PropertyKey, WellKnownSymbol,
};
use crate::engine::code::function::metadata::{
    ClassInitializerKind, ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName,
    ConstructorKind, EvalBindingSource, EvalCallerProfile, EvalCallerVariableTarget, EvalKind,
    EvalRootBinding, EvalScopeKind, EvalVariableEnvironment, FunctionKind as BytecodeFunctionKind,
    ParameterDefaultSource,
};
use crate::engine::code::module::{
    ModuleExportTarget, ModuleImportAttributes, ModuleImportCollisionDeclaration, ModuleImportName,
};
use crate::engine::compiler::lexer::{LexError, LexErrorKind, Lexer, Position, Span};
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;
use crate::source::text::SourceText;

use super::{
    ACTIVE_FUNCTION_LOCAL_NAME, BindingKind, BindingStorage, EVAL_VARIABLE_OBJECT_LOCAL_NAME,
    EvalCompileContext, HOME_OBJECT_LOCAL_NAME, MAX_BYTECODE_STACK, MAX_CALL_ARGUMENTS,
    MAX_LOCAL_VARIABLES, NEW_TARGET_LOCAL_NAME, ScopeId, ScopeKind, SourceOffset, THIS_LOCAL_NAME,
    WITH_OBJECT_LOCAL_NAME, compile_script, compile_unlinked_eval_with_filename,
    compile_unlinked_module_bytes_with_name, compile_unlinked_module_with_filename,
    compile_unlinked_script, compile_unlinked_script_source_with_filename,
    compile_unlinked_script_with_filename, ensure_closure_variable, lower_unlinked_tree,
    resolve_identifiers, validate_source_length,
};

fn evaluate(source: &str) -> Value {
    Runtime::new()
        .new_context()
        .expect("create context")
        .eval(source)
        .unwrap()
}

fn evaluate_in_context(source: &str) -> Value {
    Runtime::new()
        .new_context()
        .expect("create context")
        .eval(source)
        .unwrap()
}

fn evaluate_error(runtime: &Runtime, context: &mut Context, source: &str) -> (JsString, JsString) {
    assert_eq!(
        context.eval(source),
        Err(RuntimeError::Exception),
        "{source}"
    );
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("source did not throw an Error object: {source}");
    };
    let name = runtime.intern_property_key("name").unwrap();
    let message = runtime.intern_property_key("message").unwrap();
    let Value::String(name) = context.get_property(&error, &name).unwrap() else {
        panic!("Error.name was not a string: {source}");
    };
    let Value::String(message) = context.get_property(&error, &message).unwrap() else {
        panic!("Error.message was not a string: {source}");
    };
    (name, message)
}

fn evaluate_function_name(source: &str) -> (JsString, bool, bool, bool) {
    let (runtime, mut context) = test_context();
    let Value::Object(function) = context.eval(source).unwrap() else {
        panic!("source did not evaluate to a function object");
    };
    let name = runtime.intern_property_key("name").unwrap();
    let CompleteOrdinaryPropertyDescriptor::Data {
        value: Value::String(value),
        writable,
        enumerable,
        configurable,
    } = runtime.get_own_property(&function, &name).unwrap().unwrap()
    else {
        panic!("function name did not have the ordinary data descriptor");
    };
    (value, writable, enumerable, configurable)
}

mod generators;

mod async_functions;

mod class_diagnostics;

mod async_iteration;

mod modules;

mod scopes;

mod statements;

mod lexical_bindings;

mod program_declarations;

mod operators;

mod control_flow;

mod global_bindings;

mod calls;

mod parameters;

mod eval;

mod function_declarations;

mod assignment;

mod closures;

mod syntax;

mod quickjs_diagnostics;

mod dynamic_import;

mod limits;

mod exceptions;

mod destructuring;

mod literals;

mod object_literals;

mod array_literals;

mod debug;

mod raw_source;
