//! The new front end end to end: source to AST, HIR, MIR and register code.

use super::build::BuildError;
use super::lower::{Debug, lower};
use crate::engine::api::core::compile::ScriptCompileError;
use crate::engine::ast::ProgramKind;
use crate::engine::ast::parser::Parser;
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::compiler::register::{LowerError, LoweredScript};
use crate::source::SourceText;
use oxide_core::vm::compile::Compiler;

/// Compiles script or indirect eval code (`kind`) and instantiates it in
/// `compiler`.
pub(crate) fn compile_script(
    compiler: &mut Compiler<'_>,
    source: &SourceText,
    filename: &str,
    debug_info: DebugInfoMode,
    kind: ProgramKind,
) -> Result<LoweredScript, ScriptCompileError> {
    crate::engine::compiler::validate_source_length(source.raw_bytes().len())
        .map_err(ScriptCompileError::Front)?;
    let program = match kind {
        ProgramKind::Script => Parser::parse_script(source),
        ProgramKind::Eval => Parser::parse_eval(source),
        ProgramKind::Module => {
            return Err(ScriptCompileError::Lower(LowerError::Unsupported(
                "modules through compile_script".into(),
            )));
        }
    }
    .map_err(ScriptCompileError::Front)?;
    let has_with = program.has_with;
    let built =
        crate::engine::hir::build::build(program, filename).map_err(ScriptCompileError::Front)?;
    let mir = super::build::build(&built.program, &built.regexps, has_with)
        .map_err(|error| ScriptCompileError::Lower(lower_error(error)))?;
    let raw = source.raw_bytes();
    let debug = Debug {
        source: (debug_info != DebugInfoMode::StripDebug).then_some(raw),
        keep_source: debug_info == DebugInfoMode::Full,
        filename: (debug_info != DebugInfoMode::StripDebug).then(|| std::rc::Rc::from(filename)),
    };
    let lowered =
        lower(compiler, &mir, &built.program, &debug).map_err(ScriptCompileError::Lower)?;
    Ok(LoweredScript {
        root: lowered.root,
        function: lowered.function,
    })
}

fn lower_error(error: BuildError) -> LowerError {
    match error {
        BuildError::Syntax(message) => LowerError::Syntax(message),
        BuildError::Unsupported(what) => LowerError::Unsupported(what),
        BuildError::Internal(message) => LowerError::Internal(message),
    }
}
