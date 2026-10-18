//! Source-to-bytecode compilation with late lexical-name resolution.
//!
//! QuickJS first emits scope-variable operations, then `resolve_scope_var`
//! rewrites them after every nested function and lexical scope is known.  Its
//! `get_closure_var` helper also installs relay closure slots on intervening
//! functions.  This module keeps the same boundary in typed form: parsing emits
//! `IrOp`s into a recursive `FunctionIr` arena, identifier resolution runs
//! child-first, and only then are stack-code instructions and recursive
//! unlinked function constants produced; `register` lowers that stack code
//! into `oxide-core` register code. The parser owns its Lexer and requests tokens
//! through fallible advances, so an error on later source cannot preempt a
//! diagnostic on the current token. Directive-prologue probes clone and seek
//! the lexer, then the committed stream is rescanned under its strict context.

pub mod lexer;
mod options;
pub use options::{CompileOptions, DEFAULT_EVAL_FILENAME, EvalCompileContext};
mod model;
mod names;
use model::ir::function::FunctionTree;
#[cfg(test)]
use model::ir::function::{FunctionIrOptions, FunctionSourceInfo, SuperCapabilities};

mod parser;

use parser::context::Parser;
#[cfg(test)]
use pseudo_binding::HOME_OBJECT_LOCAL_NAME;
#[cfg(test)]
use pseudo_binding::NEW_TARGET_LOCAL_NAME;

#[cfg(test)]
use model::bindings::BindingKind;
#[cfg(test)]
use model::bindings::BindingStorage;
use model::ir::{IrConstant, IrOp};

#[cfg(test)]
use model::scope::{IrScope, ScopeId, ScopeKind};

#[cfg(test)]
use parser::builder::FunctionBuilder;

mod resolution;
#[cfg(test)]
use resolution::ensure_closure_variable;
#[cfg(test)]
use resolution::ensure_string_constant;
use resolution::resolve_identifiers;
mod flow;
mod lowering;
mod optimize;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod register;
mod relocation;
use crate::engine::api::error::{Error, ErrorKind};

#[cfg(test)]
use crate::engine::code::bytecode::DetachedBytecode;

use crate::engine::code::bytecode::{Instruction, MAX_LOCAL_SLOTS};

use crate::engine::code::debug::DebugInfoMode;
#[cfg(test)]
use crate::source::SourceOffset;

use crate::engine::code::function::UnlinkedFunction;
use crate::engine::code::module::UnlinkedModule;

use crate::engine::compiler::lexer::Span;
use crate::engine::value::JsString;
#[cfg(test)]
use crate::engine::value::PrimitiveValue as Value;

use crate::source::text::SourceText;
#[cfg(test)]
use lowering::lower_detached_script;
use lowering::lower_unlinked_tree;

mod arrow;
mod class;
mod destructuring;
mod function;
mod generator;
mod module;
mod object_literal;
mod optional_chain;
mod private_reference;
mod pseudo_binding;
pub(crate) mod stack_guard;
mod template;

#[cfg(test)]
use pseudo_binding::{ACTIVE_FUNCTION_LOCAL_NAME, THIS_LOCAL_NAME};

/// Compile one ECMAScript script directly to stack bytecode.
///
/// # Errors
/// Returns a syntax error for invalid source and an unsupported diagnostic for
/// grammar which has not yet reached the feature-parity implementation path.
#[cfg(test)]
pub fn compile_script(source: &str) -> Result<DetachedBytecode<Value>, Error> {
    let mut tree = Parser::parse(source, JsString::from_static(DEFAULT_EVAL_FILENAME))?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    if tree.functions.len() != 1 {
        return Err(Error::unsupported(
            "nested function bytecode requires runtime publication; use Context::compile or Context::eval",
            tree.functions[1].source.span,
        ));
    }
    lower_detached_script(tree)
}

/// Compile a script into a runtime-independent draft ready for publication.
///
/// Runtime publication uses this production boundary to keep primitive
/// constants structural and to carry execution metadata into the heap node.
/// The older detached compiler result is test-only.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compile_unlinked_script(source: &str) -> Result<UnlinkedFunction, Error> {
    compile_unlinked_script_with_filename(source, DEFAULT_EVAL_FILENAME, DebugInfoMode::Full)
}

pub fn compile_unlinked_script_with_filename(
    source: &str,
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse(source, JsString::try_from_utf8(filename)?)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

/// Reject source buffers which cannot fit QuickJS's signed debug-source
/// length before any byte-oriented carrier allocation is attempted.
pub fn validate_source_length(source_len: usize) -> Result<(), Error> {
    i32::try_from(source_len).map(|_| ()).map_err(|_| {
        Error::new(
            ErrorKind::JsInternal,
            "source is too large for QuickJS debug metadata",
        )
    })
}

/// Compile one explicitly sized Script buffer after applying QuickJS's signed
/// source-length guard, before allocating its byte-exact carrier.
pub fn compile_unlinked_script_bytes_with_filename(
    source: &[u8],
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedFunction, Error> {
    validate_source_length(source.len())?;
    let source = SourceText::try_from_raw_bytes(source)?;
    compile_unlinked_script_source_with_filename(&source, filename, debug_info)
}

/// Compile one byte-exact Script carrier for the public byte-oriented
/// embedding boundary.
pub fn compile_unlinked_script_source_with_filename(
    source: &SourceText,
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse_script_source(source, JsString::try_from_utf8(filename)?)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

/// Compile one ECMAScript module into its runtime-independent module record.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compile_unlinked_module_with_filename(
    source: &str,
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, Error> {
    compile_unlinked_module_with_name(source, JsString::try_from_utf8(filename)?, debug_info)
}

/// Compile a module whose host-resolved identity may contain any ECMAScript
/// String code unit, including lone UTF-16 surrogates.
pub fn compile_unlinked_module_with_name(
    source: &str,
    name: JsString,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, Error> {
    let tree = Parser::parse_module(source, name)?;
    finish_unlinked_module_tree(tree, debug_info)
}

/// Compile one explicitly sized Module buffer after applying QuickJS's signed
/// source-length guard, before allocating its byte-exact carrier.
pub fn compile_unlinked_module_bytes_with_name(
    source: &[u8],
    name: JsString,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, Error> {
    validate_source_length(source.len())?;
    let source = SourceText::try_from_raw_bytes(source)?;
    let tree = Parser::parse_module_source(&source, name)?;
    finish_unlinked_module_tree(tree, debug_info)
}

fn finish_unlinked_module_tree(
    mut tree: FunctionTree,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, Error> {
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    let module = tree
        .module
        .take()
        .ok_or_else(|| Error::internal("module compiler produced no module record"))?;
    let has_top_level_await = tree.functions.first().is_some_and(|function| {
        function
            .ops
            .iter()
            .any(|operation| matches!(operation.op, IrOp::Bytecode(Instruction::Await)))
    });
    let function = lower_unlinked_tree(tree, debug_info)?;
    module::finish_module(function, has_top_level_await, module)
}

/// Compile one primitive-String eval as an independent synthetic root.
///
/// This deliberately does not reuse the ordinary Script root. QuickJS gives
/// eval its own body environment and, for direct eval, attaches that root to
/// the active caller's VarRefs only while publishing and executing it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compile_unlinked_eval_with_filename(
    source: &str,
    filename: &str,
    debug_info: DebugInfoMode,
    context: EvalCompileContext,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse_eval(source, JsString::try_from_utf8(filename)?, context)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

/// Compile dynamic eval source whose carrier may represent lone UTF-16
/// surrogates. Public Rust `&str` compilation remains on the ordinary wrapper
/// above; only JavaScript String eval uses this reversible internal boundary.
pub fn compile_unlinked_eval_source_with_filename(
    source: &SourceText,
    filename: &str,
    debug_info: DebugInfoMode,
    context: EvalCompileContext,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse_eval_source(source, JsString::try_from_utf8(filename)?, context)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

// QuickJS 2026-06-04 `JS_MAX_LOCAL_VARS` and `JS_STACK_SIZE_MAX` are both
// 65,534. Call opcodes encode one more argument count value; the resulting
// operand stack is checked against the smaller stack limit during lowering.
const MAX_LOCAL_VARIABLES: usize = MAX_LOCAL_SLOTS as usize;
const MAX_BYTECODE_STACK: usize = 65_534;
const MAX_CALL_ARGUMENTS: usize = 65_535;
// QuickJS `js_parse_program` allocates `JS_ATOM__ret_` as the first local of
// every script. Source text cannot spell this sentinel as an IdentifierName.
const EVAL_RET_LOCAL_NAME: &str = "<ret>";
// QuickJS `JS_ATOM__var_`: the null-prototype variable object used by sloppy
// direct eval. Source text cannot spell this binding identity.
const EVAL_VARIABLE_OBJECT_LOCAL_NAME: &str = "<var>";
// QuickJS `JS_ATOM__arg_var_`: the independent null-prototype variable
// object selected by sloppy direct eval while a non-simple parameter list is
// being evaluated. It remains live for the whole activation so body eval can
// consult it after the ordinary `<var>` object.
const ARG_EVAL_VARIABLE_OBJECT_LOCAL_NAME: &str = "<arg_var>";
// QuickJS `JS_ATOM__with_`: the object-environment binding owned by one
// sloppy `with` scope. Source text cannot spell this binding identity.
const WITH_OBJECT_LOCAL_NAME: &str = "<with>";
// A finally clause in script code must preserve the incoming completion value
// when it terminates normally. Keep those implementation-only save slots in
// the same explicit metadata domain as `<ret>` rather than letting an unbound
// ordinary local silently escape the scope-graph trust boundary.
const FINALLY_EVAL_RET_LOCAL_NAME: &str = "<finally-ret>";

#[cfg(test)]
mod tests;
