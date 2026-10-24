//! The front end and the lowering as the core's compile host:
//! scripts, `eval` code and modules are compiled by the
//! shared front end and lowered to register code in the running runtime.

use oxide_core::vm::compile::{CompileError, CompileKind, CompileRequest, Compiler};
use oxide_core::vm::module::ModuleSource;

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::compiler::register::{LowerError, LoweredScript, lower_owned_script_into};
use crate::engine::compiler::{
    EvalCompileContext, compile_unlinked_eval_source_with_filename,
    compile_unlinked_script_bytes_with_filename, compile_unlinked_script_source_with_filename,
};
use crate::source::SourceText;

/// The core's error kind for a front-end error.
pub(crate) fn core_error_kind(error: &Error) -> oxide_core::ErrorKind {
    match error.kind() {
        ErrorKind::Syntax => oxide_core::ErrorKind::Syntax,
        ErrorKind::Range => oxide_core::ErrorKind::Range,
        ErrorKind::Reference => oxide_core::ErrorKind::Reference,
        ErrorKind::Type => oxide_core::ErrorKind::Type,
        _ => oxide_core::ErrorKind::Internal,
    }
}

/// The one-based line and column of a syntax error in `source`, as QuickJS
/// reports them; `None` for other errors.
pub(crate) fn syntax_location(error: &Error, source: &[u8]) -> Option<(u32, u32)> {
    if error.kind() != ErrorKind::Syntax {
        return None;
    }
    let span = error.span()?;
    crate::source::coordinates::QuickJsSourceLocator::from_bytes(source)
        .locate_byte_offset(span.start.byte_offset)
        .ok()?
        .one_based()
}

/// A front-end error as the error the core throws, located at its position
/// in `source` (named `filename`).
pub(crate) fn front_compile_error(error: &Error, source: &[u8], filename: &str) -> CompileError {
    let failure = CompileError::new(core_error_kind(error), error.message().to_owned());
    match syntax_location(error, source) {
        Some((line, column)) => failure.at(filename, line, column),
        None => failure,
    }
}

/// A lowering failure as the error the core throws.
pub(crate) fn lower_compile_error(error: LowerError) -> CompileError {
    match error {
        LowerError::Syntax(message) => CompileError::new(oxide_core::ErrorKind::Syntax, message),
        other => CompileError::new(oxide_core::ErrorKind::Internal, other.to_string()),
    }
}

/// Why a script did not compile.
#[derive(Debug)]
pub(crate) enum ScriptCompileError {
    /// The front end's error, with its position.
    Front(Error),
    /// The lowering declined a construct or failed.
    Lower(LowerError),
}

/// Which front end compiles: 0 not yet decided (from `OXIDE_FRONTEND`), 1
/// the stack-code one, 2 the new one.
static FRONT_END: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Selects the new front end (AST, HIR, MIR) or the stack-code one for
/// every later compilation; the default is `OXIDE_FRONTEND=new|old`, else
/// the stack-code one.
pub fn use_new_front_end(new: bool) {
    FRONT_END.store(
        if new { 2 } else { 1 },
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// Whether the new front end compiles.
pub(crate) fn new_front_end() -> bool {
    use std::sync::atomic::Ordering;
    match FRONT_END.load(Ordering::Relaxed) {
        0 => {
            let new = std::env::var("OXIDE_FRONTEND").is_ok_and(|value| value == "new");
            FRONT_END.store(if new { 2 } else { 1 }, Ordering::Relaxed);
            new
        }
        value => value == 2,
    }
}

/// Compiles and lowers the script `source` into `compiler`.
pub(crate) fn compile_script(
    compiler: &mut Compiler<'_>,
    source: &[u8],
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<LoweredScript, ScriptCompileError> {
    if new_front_end() {
        crate::engine::compiler::validate_source_length(source.len())
            .map_err(ScriptCompileError::Front)?;
        let text = SourceText::try_from_raw_bytes(source)
            .map_err(|error| ScriptCompileError::Front(error.into()))?;
        return crate::engine::mir::compile::compile_script(
            compiler,
            &text,
            filename,
            debug_info,
            crate::engine::ast::ProgramKind::Script,
        );
    }
    let script = compile_unlinked_script_bytes_with_filename(source, filename, debug_info)
        .map_err(ScriptCompileError::Front)?;
    lower_owned_script_into(compiler, script).map_err(ScriptCompileError::Lower)
}

/// The debug-information mode `eval` code compiles with: the running API
/// runtime's, else full.
fn eval_debug_info() -> DebugInfoMode {
    super::runtime::current_debug_info().unwrap_or(DebugInfoMode::Full)
}

/// The core's compile host: `eval` code, the `Function` constructors'
/// source and `$262.evalScript`.
pub(crate) fn compile_for_core(
    compiler: &mut Compiler<'_>,
    request: &CompileRequest<'_>,
) -> Result<oxide_core::ObjectId, CompileError> {
    let source = SourceText::try_from_utf16(request.source.iter().copied())
        .map_err(|error| CompileError::new(oxide_core::ErrorKind::Range, error.to_string()))?;
    let debug_info = eval_debug_info();
    if new_front_end() {
        let kind = match request.kind {
            CompileKind::Script => crate::engine::ast::ProgramKind::Script,
            CompileKind::IndirectEval => crate::engine::ast::ProgramKind::Eval,
        };
        let lowered = crate::engine::mir::compile::compile_script(
            compiler,
            &source,
            request.filename,
            debug_info,
            kind,
        )
        .map_err(|error| match error {
            ScriptCompileError::Front(error) => {
                front_compile_error(&error, source.raw_bytes(), request.filename)
            }
            ScriptCompileError::Lower(error) => lower_compile_error(error),
        })?;
        return Ok(lowered.function);
    }
    let compiled = match request.kind {
        CompileKind::Script => {
            compile_unlinked_script_source_with_filename(&source, request.filename, debug_info)
        }
        CompileKind::IndirectEval => compile_unlinked_eval_source_with_filename(
            &source,
            request.filename,
            debug_info,
            EvalCompileContext::indirect(),
        ),
    };
    let script = compiled
        .map_err(|error| front_compile_error(&error, source.raw_bytes(), request.filename))?;
    let lowered = lower_owned_script_into(compiler, script).map_err(lower_compile_error)?;
    Ok(lowered.function)
}

/// QuickJS's `js_module_normalize_name`: a specifier starting with `.`
/// resolves against the referrer's directory; any other is a name as is.
pub(crate) fn normalize_module_name(referrer: &str, specifier: &str) -> String {
    if !specifier.starts_with('.') {
        return specifier.to_owned();
    }
    let mut directory = match referrer.rfind('/') {
        Some(slash) => referrer[..slash].to_owned(),
        None => String::new(),
    };
    let mut rest = specifier;
    loop {
        if let Some(next) = rest.strip_prefix("./") {
            rest = next;
        } else if let Some(next) = rest.strip_prefix("../") {
            if directory.is_empty() {
                break;
            }
            let last = directory.rfind('/').map_or(0, |slash| slash + 1);
            let component = &directory[last..];
            if component == "." || component == ".." {
                break;
            }
            directory.truncate(last.saturating_sub(1));
            rest = next;
        } else {
            break;
        }
    }
    if directory.is_empty() {
        rest.to_owned()
    } else {
        format!("{directory}/{rest}")
    }
}

/// Why a module did not compile: the front end's error, with its position,
/// or the lowering's.
pub(crate) enum ModuleCompileError {
    Front(Error),
    Core(CompileError),
}

/// Compiles and lowers the module `name` from `source`; also returns its
/// root unit.
pub(crate) fn compile_module(
    compiler: &mut Compiler<'_>,
    name: &str,
    source: &[u8],
    debug_info: DebugInfoMode,
) -> Result<(ModuleSource, oxide_core::CodeUnitId), ModuleCompileError> {
    if new_front_end() {
        return crate::engine::mir::compile::compile_module(compiler, name, source, debug_info);
    }
    let failure = |kind: oxide_core::ErrorKind, message: String| {
        ModuleCompileError::Core(CompileError::new(kind, message))
    };
    let module_name = crate::engine::value::JsString::try_from_utf8(name)
        .map_err(|error| failure(oxide_core::ErrorKind::Internal, error.to_string()))?;
    let module = crate::engine::compiler::compile_unlinked_module_bytes_with_name(
        source,
        module_name,
        debug_info,
    )
    .map_err(ModuleCompileError::Front)?;
    crate::engine::compiler::register::lower_module_tree_into(compiler, &module)
        .map_err(|error| ModuleCompileError::Core(lower_compile_error(error)))
}

/// `$262.agent.start`'s runner: an agent's script and then its jobs.
#[cfg(feature = "test262-host")]
pub(crate) fn run_agent_script(
    runtime: &mut oxide_core::Runtime,
    source: &str,
) -> Result<(), String> {
    let lowered = compile_script(
        &mut runtime.compiler(),
        source.as_bytes(),
        "<agent>",
        DebugInfoMode::Full,
    )
    .map_err(|error| match error {
        ScriptCompileError::Front(error) => error.to_string(),
        ScriptCompileError::Lower(error) => error.to_string(),
    })?;
    let function = oxide_core::Value::object(lowered.function);
    let handle = runtime.hold(function);
    let mut result = runtime.run_script(lowered.function);
    runtime.release(handle);
    if result.is_ok() {
        result = runtime.run_jobs().map(|()| oxide_core::Value::UNDEFINED);
    }
    result.map(drop).map_err(|thrown| {
        runtime
            .display(thrown.0)
            .unwrap_or_else(|_| "exception while printing the exception".into())
    })
}
