//! The front end and the lowering as the core's compile host:
//! scripts, `eval` code and modules are compiled to register code in the
//! running runtime.

use oxide_core::vm::compile::{CompileError, CompileKind, CompileRequest, Compiler};
use oxide_core::vm::module::ModuleSource;

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::mir::compile::{LowerError, LoweredScript};
use crate::source::SourceText;

/// Default filename used by the Rust convenience compile/eval APIs.
pub const DEFAULT_EVAL_FILENAME: &str = "<input>";

/// Named source compilation options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub filename: String,
    /// The code is freed once nothing names it (a REPL's line), instead of
    /// living as long as the runtime.
    pub transient: bool,
}

impl CompileOptions {
    #[must_use]
    pub fn new(filename: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
            transient: false,
        }
    }

    /// These options with [`CompileOptions::transient`] set.
    #[must_use]
    pub fn transient(mut self) -> Self {
        self.transient = true;
        self
    }
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self::new(DEFAULT_EVAL_FILENAME)
    }
}

/// Runtime-wide policy for the currently represented, JS-observable portion
/// of QuickJS `JS_SetStripInfo`.
///
/// The selected mode is sampled when a function is compiled. Existing
/// bytecode keeps the debug payload it was published with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DebugInfoMode {
    /// Keep filename, PC positions and exact authored function source.
    #[default]
    Full,
    /// Keep filename and PC positions but omit authored function source.
    StripSource,
    /// Omit function source and location metadata from the bytecode payload.
    StripDebug,
}

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

/// Compiles and lowers the script `source` into `compiler`.
pub(crate) fn compile_script(
    compiler: &mut Compiler<'_>,
    source: &[u8],
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<LoweredScript, ScriptCompileError> {
    crate::engine::limits::validate_source_length(source.len())
        .map_err(ScriptCompileError::Front)?;
    let text = SourceText::try_from_raw_bytes(source)
        .map_err(|error| ScriptCompileError::Front(error.into()))?;
    crate::engine::mir::compile::compile_script(
        compiler,
        &text,
        filename,
        debug_info,
        crate::engine::ast::ProgramKind::Script,
    )
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
    crate::engine::mir::compile::compile_module(compiler, name, source, debug_info)
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
