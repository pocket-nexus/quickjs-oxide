//! The front end end to end: source to AST, HIR, MIR and register code.

use super::build::BuildError;
use super::lower::{Debug, Error, lower};
use crate::engine::api::core::compile::DebugInfoMode;
use crate::engine::api::core::compile::{
    ModuleCompileError, ScriptCompileError, lower_compile_error,
};
use crate::engine::ast::ProgramKind;
use crate::engine::ast::parser::Parser;
use crate::source::SourceText;
use oxide_core::bytecode::ValidateError;
use oxide_core::vm::compile::Compiler;
use oxide_core::{CodeUnitId, HeapError, ObjectId};
use std::fmt;

/// Why the middle end or the lowering did not produce code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LowerError {
    /// A construct the lowering does not support.
    Unsupported(String),
    /// A dropped feature that is an early `SyntaxError` (`with`).
    Syntax(String),
    /// The input broke an invariant an earlier stage guarantees.
    Internal(String),
    Heap(HeapError),
    /// The publication validator rejected a lowered unit.
    Invalid(ValidateError),
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LowerError::Unsupported(construct) => write!(f, "unsupported: {construct}"),
            LowerError::Syntax(message) => write!(f, "SyntaxError: {message}"),
            LowerError::Internal(message) => write!(f, "internal lowering error: {message}"),
            LowerError::Heap(error) => write!(f, "{error}"),
            LowerError::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl From<HeapError> for LowerError {
    fn from(error: HeapError) -> Self {
        LowerError::Heap(error)
    }
}

impl From<ValidateError> for LowerError {
    fn from(error: ValidateError) -> Self {
        LowerError::Invalid(error)
    }
}

/// A script lowered and instantiated on the core.
pub(crate) struct LoweredScript {
    pub(crate) root: CodeUnitId,
    /// The script's function object. Unrooted: the caller holds it before
    /// its next allocation.
    pub(crate) function: ObjectId,
}

/// Compiles script or indirect eval code (`kind`) and instantiates it in
/// `compiler`.
pub(crate) fn compile_script(
    compiler: &mut Compiler<'_>,
    source: &SourceText,
    filename: &str,
    debug_info: DebugInfoMode,
    kind: ProgramKind,
) -> Result<LoweredScript, ScriptCompileError> {
    crate::engine::limits::validate_source_length(source.raw_bytes().len())
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
    if has_with {
        return Err(ScriptCompileError::Lower(LowerError::Syntax(
            "with statements are not supported".into(),
        )));
    }
    let mut built =
        crate::engine::hir::build::build(program, filename).map_err(ScriptCompileError::Front)?;
    let build_error = |error| match error {
        BuildError::StackOverflow => {
            ScriptCompileError::Front(crate::engine::api::error::Error::new(
                crate::engine::api::error::ErrorKind::JsInternal,
                "stack overflow",
            ))
        }
        error => ScriptCompileError::Lower(lower_error(error)),
    };
    let mut stream = super::build::Stream::new(&mut built.program, &built.regexps, has_with)
        .map_err(build_error)?;
    let raw = source.raw_bytes();
    let debug = Debug {
        source: (debug_info != DebugInfoMode::StripDebug).then_some(raw),
        keep_source: debug_info == DebugInfoMode::Full,
        filename: (debug_info != DebugInfoMode::StripDebug).then(|| std::rc::Rc::from(filename)),
    };
    let lowered = lower(compiler, &mut stream, &debug).map_err(|error| match error {
        Error::Build(error) => build_error(error),
        Error::Lower(error) => ScriptCompileError::Lower(error),
    })?;
    Ok(LoweredScript {
        root: lowered.root,
        function: lowered.function,
    })
}

/// Compiles the module `name` and instantiates its root in `compiler`:
/// the record's source and the root's unit.
pub(crate) fn compile_module(
    compiler: &mut Compiler<'_>,
    name: &str,
    source: &[u8],
    debug_info: DebugInfoMode,
) -> Result<(oxide_core::vm::module::ModuleSource, oxide_core::CodeUnitId), ModuleCompileError> {
    use oxide_core::vm::module as core;
    let lower = |error: LowerError| ModuleCompileError::Core(lower_compile_error(error));
    crate::engine::limits::validate_source_length(source.len())
        .map_err(ModuleCompileError::Front)?;
    let text = SourceText::try_from_raw_bytes(source)
        .map_err(|error| ModuleCompileError::Front(error.into()))?;
    let program = Parser::parse_module(&text).map_err(ModuleCompileError::Front)?;
    let mut built =
        crate::engine::hir::build::build(program, name).map_err(ModuleCompileError::Front)?;
    let mut stream = super::build::Stream::new(&mut built.program, &built.regexps, false)
        .map_err(|error| lower(lower_error(error)))?;
    let raw = text.raw_bytes();
    let debug = Debug {
        source: (debug_info != DebugInfoMode::StripDebug).then_some(raw),
        keep_source: debug_info == DebugInfoMode::Full,
        filename: (debug_info != DebugInfoMode::StripDebug).then(|| std::rc::Rc::from(name)),
    };
    let lowered =
        super::lower::lower(compiler, &mut stream, &debug).map_err(|error| match error {
            Error::Build(error) => lower(lower_error(error)),
            Error::Lower(error) => lower(error),
        })?;
    let Some(info) = stream.module() else {
        return Err(lower(LowerError::Internal(
            "a module without its record".into(),
        )));
    };
    let strings = stream.strings();
    let text = |index: u32| strings[index as usize].to_utf8_lossy();
    let tables = core::ModuleTables {
        bindings: info
            .bindings
            .iter()
            .map(|binding| match binding {
                super::build::ModuleBinding::Import => core::ModuleBinding::Import,
                super::build::ModuleBinding::ImportMeta => core::ModuleBinding::ImportMeta,
                super::build::ModuleBinding::Declaration { lexical } => {
                    core::ModuleBinding::Declaration { lexical: *lexical }
                }
            })
            .collect(),
        requests: info
            .requests
            .iter()
            .map(|(specifier, attributes)| core::ModuleRequest {
                specifier: text(*specifier),
                attributes: attributes
                    .iter()
                    .map(|&(key, value)| (text(key), text(value)))
                    .collect(),
            })
            .collect(),
        imports: info
            .imports
            .iter()
            .map(|&(request, name, binding)| core::ModuleImport {
                request,
                name: name.map(text),
                binding,
            })
            .collect(),
        exports: info
            .exports
            .iter()
            .map(|&(name, target)| core::ModuleExport {
                name: text(name),
                target: match target {
                    super::build::ExportTarget::Local(binding) => {
                        core::ExportTarget::Local(binding)
                    }
                    super::build::ExportTarget::Indirect { request, name } => {
                        core::ExportTarget::Indirect {
                            request,
                            name: name.map(text),
                        }
                    }
                },
            })
            .collect(),
        star_exports: info.star_exports.clone(),
        initializers: info
            .initializers
            .iter()
            .map(|&(binding, function)| core::ModuleInitializer { binding, function })
            .collect(),
        has_top_level_await: info.has_top_level_await,
    };
    Ok((
        core::ModuleSource::Compiled {
            function: lowered.function,
            tables,
            import_meta: Vec::new(),
        },
        lowered.root,
    ))
}

fn lower_error(error: BuildError) -> LowerError {
    match error {
        BuildError::Syntax(message) => LowerError::Syntax(message),
        BuildError::Unsupported(what) => LowerError::Unsupported(what),
        BuildError::Internal(message) => LowerError::Internal(message),
        BuildError::StackOverflow => LowerError::Internal("stack overflow".into()),
    }
}
