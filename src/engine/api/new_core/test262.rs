//! The new core's side of the Test262 runner (phase 4, section 7.1): one
//! worker runtime with `$262`, `print` captured instead of written, the
//! harness and the test as separate scripts, an exact module resolver
//! supplied by the runner, and diagnostics read without running
//! JavaScript. The runner's `--engine new-core` backend is its only user.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use oxide_core::Runtime;
use oxide_core::heap::gc::Handle;
use oxide_core::vm::compile::{CompileError, Compiler};
use oxide_core::vm::module::{ModuleHost, ModulePhase, ModuleSource};

use super::{NewCoreError, NewCoreOptions, core_error_kind};
use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::compiler::compile_unlinked_script_bytes_with_filename;
use crate::engine::compiler::register::{lower_script, tree_has_import_call};
use crate::source::coordinates::QuickJsSourceLocator;

/// A thrown value as the runner records it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostic {
    /// The error's name (`SyntaxError`), `ThrownObject` for an object
    /// without one, `Thrown<type>` for a primitive.
    pub error_type: String,
    pub message: String,
    /// The one-based position of a syntax error.
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// The thrown object's own `stack`, QuickJS's backtrace form.
    pub stack: Option<String>,
}

/// Why a step failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The front end or the lowering declined a construct.
    Unsupported(String),
    /// JavaScript threw.
    Thrown(Diagnostic),
    /// The engine failed internally.
    Internal(String),
}

/// Where a module run failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleStep {
    /// Compiling the root module.
    Parse,
    /// Resolving, compiling or linking the rest of the graph.
    Resolution,
    /// Evaluating it.
    Runtime,
}

/// The runner's module loader: names resolve and load only inside the
/// admitted graph of the test.
pub trait ModuleResolver {
    /// The normalized name of `specifier` imported by `referrer`.
    fn normalize(&self, referrer: &str, specifier: &str) -> Result<String, String>;
    /// The source text of the module named `name`.
    fn load(&self, name: &str) -> Result<String, String>;
}

/// The settled state of a module evaluation after the job queue drained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Settled {
    Fulfilled,
    Pending,
    Rejected(Diagnostic),
}

/// A compiled script, held until run.
pub struct Script {
    function: Handle,
    /// Whether its code has an `import()` call.
    pub contains_dynamic_import: bool,
}

struct ResolverState {
    resolver: Box<dyn ModuleResolver>,
    /// The root module's name and source, loaded without counting as
    /// resolution.
    root: RefCell<Option<(String, String)>>,
    /// Set once a name other than the root's is normalized or loaded.
    resolution_started: Cell<bool>,
    /// Whether a module of the graph has an `import()` call.
    dynamic_import: Cell<bool>,
    /// The located diagnostic of the last module that failed to compile.
    load_error: RefCell<Option<Diagnostic>>,
}

thread_local! {
    static RESOLVER: RefCell<Option<Rc<ResolverState>>> = const { RefCell::new(None) };
}

fn resolver() -> Option<Rc<ResolverState>> {
    RESOLVER.with(|state| state.borrow().clone())
}

/// One Test262 worker's runtime.
pub struct Worker {
    runtime: Runtime,
}

impl Worker {
    /// A runtime with `$262` and the agent host, `print` captured, the
    /// clock and time zone of `services`.
    pub fn new(
        can_block: bool,
        services: Rc<dyn crate::engine::host::HostServices>,
    ) -> Result<Worker, String> {
        let options = NewCoreOptions {
            test262: true,
            cannot_block: !can_block,
            services: Some(services),
            ..NewCoreOptions::default()
        };
        let mut runtime = super::runtime(&options).map_err(|error| error.to_string())?;
        runtime.capture_output();
        runtime.set_module_host(ModuleHost {
            normalize: normalize_module,
            load: load_module,
        });
        Ok(Worker { runtime })
    }

    /// Installs the loader the graph of the next module run resolves
    /// through; without one every import fails.
    pub fn set_module_resolver(&mut self, resolver: Box<dyn ModuleResolver>) {
        let state = ResolverState {
            resolver,
            root: RefCell::new(None),
            resolution_started: Cell::new(false),
            dynamic_import: Cell::new(false),
            load_error: RefCell::new(None),
        };
        RESOLVER.with(|slot| *slot.borrow_mut() = Some(Rc::new(state)));
    }

    /// Compiles and lowers a script; a syntax error carries its position.
    pub fn compile_script(&mut self, source: &str, filename: &str) -> Result<Script, Failure> {
        let script = compile_unlinked_script_bytes_with_filename(
            source.as_bytes(),
            filename,
            DebugInfoMode::Full,
        )
        .map_err(|error| compile_failure(&error, source.as_bytes()))?;
        let lowered = lower_script(&mut self.runtime, &script)
            .map_err(|error| lower_failure(error.into(), source.as_bytes()))?;
        let function = self
            .runtime
            .hold(oxide_core::Value::object(lowered.function));
        Ok(Script {
            function,
            contains_dynamic_import: tree_has_import_call(self.runtime.code(), lowered.root),
        })
    }

    /// Runs a compiled script with the global object as `this`.
    pub fn run_script(&mut self, script: Script) -> Result<(), Failure> {
        let function = self.runtime.held(script.function);
        let result = match function.as_object() {
            Some(function) => self.runtime.run_script(function),
            None => unreachable!("a script function"),
        };
        self.runtime.release(script.function);
        result.map(drop).map_err(|thrown| self.thrown(thrown.0))
    }

    /// Runs pending jobs until the queue is empty.
    pub fn run_jobs(&mut self) -> Result<(), Failure> {
        self.runtime
            .run_jobs()
            .map_err(|thrown| self.thrown(thrown.0))
    }

    /// Loads, links and evaluates the module `name` with source `source`
    /// through the installed resolver, then drains the job queue and reads
    /// the evaluation's promise.
    pub fn run_module(
        &mut self,
        name: &str,
        source: &str,
    ) -> Result<Settled, (ModuleStep, Failure)> {
        let Some(state) = resolver() else {
            return Err((
                ModuleStep::Parse,
                Failure::Internal("no module resolver".into()),
            ));
        };
        *state.root.borrow_mut() = Some((name.to_owned(), source.to_owned()));
        state.resolution_started.set(false);
        state.dynamic_import.set(false);
        let promise = match self.runtime.run_module_phased(name) {
            Ok(promise) => promise,
            Err((phase, thrown)) => {
                let step = match phase {
                    ModulePhase::Load if state.resolution_started.get() => ModuleStep::Resolution,
                    ModulePhase::Load => ModuleStep::Parse,
                    ModulePhase::Link => ModuleStep::Resolution,
                    ModulePhase::Evaluate => ModuleStep::Runtime,
                };
                let located = match phase {
                    ModulePhase::Load => state.load_error.borrow_mut().take(),
                    _ => None,
                };
                let failure = match located {
                    Some(diagnostic) => Failure::Thrown(diagnostic),
                    None => self.thrown(thrown.0),
                };
                return Err((step, failure));
            }
        };
        let handle = self.runtime.hold(promise);
        let jobs = self.runtime.run_jobs();
        let settled = self.runtime.promise_state(promise);
        self.runtime.release(handle);
        if let Err(thrown) = jobs {
            return Err((ModuleStep::Runtime, self.thrown(thrown.0)));
        }
        Ok(match settled {
            Some((0, _)) => Settled::Pending,
            Some((2, reason)) => Settled::Rejected(self.diagnostic(reason)),
            _ => Settled::Fulfilled,
        })
    }

    /// Whether a module of the last graph has an `import()` call.
    pub fn module_graph_contains_dynamic_import(&self) -> bool {
        resolver().is_some_and(|state| state.dynamic_import.get())
    }

    /// Whether a name outside the root was normalized or loaded in the
    /// last module run.
    pub fn module_resolution_started(&self) -> bool {
        resolver().is_some_and(|state| state.resolution_started.get())
    }

    /// The `print` output so far, one entry per line.
    pub fn take_print_log(&mut self) -> Vec<String> {
        self.runtime
            .take_output()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Waits for the agents `$262.agent.start` started.
    pub fn join_agents(&mut self) {
        self.runtime.join_agents();
    }

    fn thrown(&mut self, value: oxide_core::Value) -> Failure {
        Failure::Thrown(self.diagnostic(value))
    }

    /// The runner's view of a thrown value, read without running
    /// JavaScript.
    fn diagnostic(&mut self, value: oxide_core::Value) -> Diagnostic {
        if let Some(summary) = self.runtime.thrown_summary(value) {
            return Diagnostic {
                error_type: summary.name.unwrap_or_else(|| "ThrownObject".into()),
                message: summary.message.unwrap_or_default(),
                line: None,
                column: None,
                stack: summary.stack,
            };
        }
        use oxide_core::value::Kind;
        let kind = match value.kind() {
            Kind::Undefined | Kind::Empty => "undefined",
            Kind::Null => "null",
            Kind::Bool => "boolean",
            Kind::Number | Kind::Int32 => "number",
            Kind::BigInt => "bigint",
            Kind::String => "string",
            Kind::Symbol => "symbol",
            Kind::Object => "object",
        };
        let message = if kind == "symbol" {
            "Symbol()".to_owned()
        } else {
            self.runtime.display(value).unwrap_or_default()
        };
        Diagnostic {
            error_type: format!("Thrown{kind}"),
            message,
            line: None,
            column: None,
            stack: None,
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        RESOLVER.with(|slot| slot.borrow_mut().take());
    }
}

fn compile_failure(error: &Error, source: &[u8]) -> Failure {
    let name = match error.kind() {
        ErrorKind::Syntax => "SyntaxError",
        ErrorKind::Type => "TypeError",
        ErrorKind::Reference => "ReferenceError",
        ErrorKind::Range => "RangeError",
        ErrorKind::JsInternal => "InternalError",
        ErrorKind::Unsupported => return Failure::Unsupported(error.message().to_owned()),
        ErrorKind::Internal | ErrorKind::Io => return Failure::Internal(error.to_string()),
    };
    let position = (error.kind() == ErrorKind::Syntax)
        .then(|| error.span())
        .flatten()
        .and_then(|span| {
            QuickJsSourceLocator::from_bytes(source)
                .locate_byte_offset(span.start.byte_offset)
                .ok()
        })
        .and_then(|position| position.one_based());
    Failure::Thrown(Diagnostic {
        error_type: name.to_owned(),
        message: error.message().to_owned(),
        line: position.map(|(line, _)| line),
        column: position.map(|(_, column)| column),
        stack: None,
    })
}

fn lower_failure(error: NewCoreError, source: &[u8]) -> Failure {
    match error {
        NewCoreError::Unsupported(construct) => Failure::Unsupported(construct),
        NewCoreError::Compile(error) => compile_failure(&error, source),
        other => Failure::Internal(other.to_string()),
    }
}

fn normalize_module(
    _: &mut Compiler<'_>,
    referrer: &str,
    specifier: &str,
    _attributes: &[(String, String)],
) -> Result<String, CompileError> {
    let state = resolver().ok_or_else(|| reference_error("no module resolver".into()))?;
    state.resolution_started.set(true);
    state
        .resolver
        .normalize(referrer, specifier)
        .map_err(reference_error)
}

fn load_module(
    compiler: &mut Compiler<'_>,
    name: &str,
    attributes: &[(String, String)],
) -> Result<ModuleSource, CompileError> {
    let state = resolver().ok_or_else(|| reference_error("no module resolver".into()))?;
    let root = state.root.borrow().clone();
    let source = match root {
        Some((root, source)) if root == name => source,
        _ => {
            state.resolution_started.set(true);
            state.resolver.load(name).map_err(reference_error)?
        }
    };
    let json = attributes
        .iter()
        .any(|(key, value)| key == "type" && value == "json");
    if json {
        return Ok(ModuleSource::Json(source.encode_utf16().collect()));
    }
    let failed = |error: CompileError, diagnostic: Option<Diagnostic>| {
        *state.load_error.borrow_mut() = diagnostic;
        error
    };
    match super::compile_module(compiler, name, source.as_bytes(), DebugInfoMode::Full) {
        Ok((module, root)) => {
            if tree_has_import_call(compiler.code(), root) {
                state.dynamic_import.set(true);
            }
            Ok(module)
        }
        Err(super::ModuleCompileError::Front(error)) => {
            let diagnostic = match compile_failure(&error, source.as_bytes()) {
                Failure::Thrown(diagnostic) => Some(diagnostic),
                _ => None,
            };
            Err(failed(
                CompileError::new(core_error_kind(&error), error.message().to_owned()),
                diagnostic,
            ))
        }
        Err(super::ModuleCompileError::Core(error)) => Err(failed(error, None)),
    }
}

fn reference_error(message: String) -> CompileError {
    CompileError::new(oxide_core::ErrorKind::Reference, message)
}
