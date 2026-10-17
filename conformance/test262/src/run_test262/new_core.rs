//! The worker (new-core phase 4, section 7.1; the runner's one backend since
//! phase 6 M2): the harness order, admissions, phases and classification
//! over `api::core::test262`.

use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use quickjs_oxide::engine::api::core::test262::{
    Diagnostic, Failure, ModuleResolver, ModuleStep, Settled, Worker,
};

use super::super::admissions::{AdmissionCatalog, DynamicImportRootPolicy, ModuleGraphRootGoal};
use super::super::metadata::Metadata;
use super::super::report::WorkerResult;
use super::super::requirements::{
    ExactModuleTest, load_exact_dynamic_import_fixture, load_exact_module_fixture,
    normalize_exact_dynamic_import_request, normalize_exact_module_request,
};
use super::super::{Variant, WorkerOptions};
use super::{
    ExceptionDiagnostic, classify_async_print_log, classify_completion, classify_normal,
    first_quickjs_stack_source_location,
};

/// What the shared worker prologue established about the test.
pub(super) struct NewCoreTest<'a> {
    pub(super) options: &'a WorkerOptions,
    pub(super) admissions: Rc<AdmissionCatalog>,
    pub(super) path: &'a Path,
    pub(super) source: String,
    pub(super) metadata: &'a Metadata,
    pub(super) exact_module: Option<ExactModuleTest>,
    pub(super) exact_dynamic_import: bool,
    pub(super) dynamic_import_policy: Option<DynamicImportRootPolicy>,
}

struct ExactResolver {
    admissions: Rc<AdmissionCatalog>,
    suite: PathBuf,
    root: PathBuf,
    goal: ModuleGraphRootGoal,
}

impl ModuleResolver for ExactResolver {
    fn normalize(&self, referrer: &str, specifier: &str) -> Result<String, String> {
        match self.goal {
            ModuleGraphRootGoal::StaticModule => {
                normalize_exact_module_request(&self.admissions, &self.root, referrer, specifier)
            }
            ModuleGraphRootGoal::DynamicImportScript => normalize_exact_dynamic_import_request(
                &self.admissions,
                &self.root,
                referrer,
                specifier,
            ),
        }
    }

    fn load(&self, name: &str) -> Result<String, String> {
        match self.goal {
            ModuleGraphRootGoal::StaticModule => {
                load_exact_module_fixture(&self.admissions, &self.suite, &self.root, name)
            }
            ModuleGraphRootGoal::DynamicImportScript => {
                load_exact_dynamic_import_fixture(&self.admissions, &self.suite, &self.root, name)
            }
        }
    }
}

/// The current worker's reading: an explicit position, else the first
/// located frame of the backtrace.
fn diagnostic(diagnostic: Diagnostic) -> ExceptionDiagnostic {
    let (line, column) = match (diagnostic.line, diagnostic.column) {
        (None, None) => diagnostic
            .stack
            .as_deref()
            .and_then(first_quickjs_stack_source_location)
            .map_or((None, None), |(line, column)| (Some(line), Some(column))),
        position => position,
    };
    ExceptionDiagnostic {
        error_type: diagnostic.error_type,
        message: diagnostic.message,
        line,
        column,
    }
}

fn engine_fault(outcome: &str, phase: &str, detail: String) -> WorkerResult {
    WorkerResult::failure(outcome, phase, "EngineError", detail)
}

pub(super) fn run_worker(test: NewCoreTest<'_>) -> Result<WorkerResult, String> {
    let NewCoreTest {
        options,
        admissions,
        path,
        source,
        metadata,
        exact_module,
        exact_dynamic_import,
        dynamic_import_policy,
    } = test;
    let parse_rejected_dynamic_import =
        dynamic_import_policy == Some(DynamicImportRootPolicy::ParseRejected);
    let async_test = metadata.is_async();
    let mut worker = Worker::new(
        !metadata.flags.contains("CanBlockIsFalse"),
        Box::new(quickjs_oxide_host::SystemHost::default()),
    )
    .map_err(|error| format!("create new-core worker runtime: {error}"))?;
    let goal = if exact_module.is_some_and(ExactModuleTest::is_fixture_graph) {
        Some(ModuleGraphRootGoal::StaticModule)
    } else if exact_dynamic_import && !parse_rejected_dynamic_import {
        Some(ModuleGraphRootGoal::DynamicImportScript)
    } else {
        None
    };
    if let Some(goal) = goal {
        worker.set_module_resolver(Box::new(ExactResolver {
            admissions: admissions.clone(),
            suite: options.suite.clone(),
            root: options.test.clone(),
            goal,
        }));
    }

    let mut includes = Vec::new();
    if !metadata.is_raw() {
        includes.extend(["assert.js".to_owned(), "sta.js".to_owned()]);
        includes.extend(metadata.includes.iter().cloned());
    }
    if async_test {
        includes.push("doneprintHandle.js".to_owned());
    }
    for include in includes {
        let include_path = options.suite.join("harness").join(&include);
        let harness = fs::read_to_string(&include_path)
            .map_err(|error| format!("read {}: {error}", include_path.display()))?;
        let script = match worker.compile_script(&harness, &include_path.to_string_lossy()) {
            Ok(script) => script,
            Err(Failure::Unsupported(message)) => {
                return Ok(WorkerResult::failure(
                    "unsupported-harness-parser",
                    "harness",
                    "Unsupported",
                    format!("{include}: {message}"),
                ));
            }
            Err(Failure::Thrown(thrown)) => {
                let thrown = diagnostic(thrown);
                return Ok(WorkerResult::failure_with_diagnostic(
                    "harness-error",
                    "harness",
                    thrown.error_type,
                    format!("{include}: {}", thrown.message),
                    thrown.line,
                    thrown.column,
                ));
            }
            Err(Failure::Internal(message)) => {
                return Ok(engine_fault(
                    "harness-engine-fault",
                    "harness-compile",
                    format!("{include}: {message}"),
                ));
            }
        };
        match worker.run_script(script) {
            Ok(()) => {}
            Err(Failure::Unsupported(message)) => {
                return Ok(WorkerResult::failure(
                    "unsupported-harness-runtime",
                    "harness-runtime",
                    "Unsupported",
                    format!("{include}: {message}"),
                ));
            }
            Err(Failure::Thrown(thrown)) => {
                let thrown = diagnostic(thrown);
                return Ok(WorkerResult::failure_with_diagnostic(
                    "harness-error",
                    "harness-runtime",
                    thrown.error_type,
                    format!("{include}: {}", thrown.message),
                    thrown.line,
                    thrown.column,
                ));
            }
            Err(Failure::Internal(message)) => {
                return Ok(engine_fault(
                    "harness-engine-fault",
                    "harness-runtime",
                    format!("{include}: {message}"),
                ));
            }
        }
    }

    if let Some(exact_module) = exact_module {
        let result = run_exact_module(&mut worker, options, path, &source, metadata, exact_module);
        worker.join_agents();
        return result;
    }

    let authored = if options.variant == Variant::Strict {
        format!("\"use strict\";\n{source}")
    } else {
        source
    };
    let filename = if exact_dynamic_import {
        options.test.to_string_lossy()
    } else {
        path.to_string_lossy()
    };
    let script = match worker.compile_script(&authored, &filename) {
        Ok(script) => script,
        Err(Failure::Unsupported(message)) => {
            if parse_rejected_dynamic_import {
                return Err(format!(
                    "parse-rejected Test262 dynamic-import root reached the lowering instead of SyntaxError: {}",
                    options.test.display()
                ));
            }
            return Ok(WorkerResult::failure(
                "unsupported-parser",
                "parse",
                "Unsupported",
                message,
            ));
        }
        Err(Failure::Thrown(thrown)) => {
            if parse_rejected_dynamic_import && thrown.error_type != "SyntaxError" {
                return Err(format!(
                    "parse-rejected Test262 dynamic-import root produced {} instead of SyntaxError: {}",
                    thrown.error_type,
                    options.test.display()
                ));
            }
            return Ok(classify_completion(metadata, "parse", &diagnostic(thrown)));
        }
        Err(Failure::Internal(message)) => {
            return Ok(engine_fault("engine-fault", "parse", message));
        }
    };
    authenticate_dynamic_import(
        script.contains_dynamic_import,
        dynamic_import_policy,
        &options.test,
        "Script",
    )?;
    if negative_phase(metadata).is_some_and(|phase| matches!(phase, "parse" | "early")) {
        return Ok(classify_normal(metadata));
    }
    let result = match worker.run_script(script) {
        Ok(()) if async_test => finish_async_test(&mut worker, metadata),
        Ok(()) => classify_normal(metadata),
        Err(failure) => classify_failure(metadata, "runtime", failure),
    };
    worker.join_agents();
    Ok(result)
}

fn negative_phase(metadata: &Metadata) -> Option<&str> {
    metadata
        .negative
        .as_ref()
        .and_then(|negative| negative.phase.as_deref())
}

fn classify_failure(metadata: &Metadata, phase: &str, failure: Failure) -> WorkerResult {
    match failure {
        Failure::Unsupported(message) => WorkerResult::failure(
            format!("unsupported-{phase}"),
            phase,
            "Unsupported",
            message,
        ),
        Failure::Thrown(thrown) => classify_completion(metadata, phase, &diagnostic(thrown)),
        Failure::Internal(message) => engine_fault("engine-fault", phase, message),
    }
}

fn finish_async_test(worker: &mut Worker, metadata: &Metadata) -> WorkerResult {
    if let Err(failure) = worker.run_jobs() {
        return classify_failure(metadata, "runtime", failure);
    }
    classify_async_print_log(metadata, &worker.take_print_log())
}

/// The current worker's bytecode authentication of the admitted
/// dynamic-import roots, over the lowered code.
fn authenticate_dynamic_import(
    contains_dynamic_import: bool,
    policy: Option<DynamicImportRootPolicy>,
    path: &Path,
    goal: &str,
) -> Result<(), String> {
    match (contains_dynamic_import, policy) {
        (true, None) => Err(format!(
            "Test262 dynamic-import {goal} worker rejected unaudited path: {}",
            path.display()
        )),
        (false, Some(DynamicImportRootPolicy::InitialImportTree)) => Err(format!(
            "authenticated Test262 dynamic-import {goal} compiled without an import call: {}",
            path.display()
        )),
        (true, Some(DynamicImportRootPolicy::RuntimeCompiledImport)) => Err(format!(
            "runtime-compiled Test262 dynamic-import {goal} unexpectedly contained an initial import call: {}",
            path.display()
        )),
        (_, Some(DynamicImportRootPolicy::ParseRejected)) => Err(format!(
            "parse-rejected Test262 dynamic-import {goal} unexpectedly compiled: {}",
            path.display()
        )),
        (true, Some(DynamicImportRootPolicy::InitialImportTree))
        | (false, Some(DynamicImportRootPolicy::RuntimeCompiledImport))
        | (false, None) => Ok(()),
    }
}

fn run_exact_module(
    worker: &mut Worker,
    options: &WorkerOptions,
    path: &Path,
    source: &str,
    metadata: &Metadata,
    exact_module: ExactModuleTest,
) -> Result<WorkerResult, String> {
    let name = if exact_module.is_fixture_graph() {
        options.test.to_string_lossy()
    } else {
        path.to_string_lossy()
    };
    if !exact_module.is_fixture_graph() {
        // A dependency-free module resolves nothing: any import fails.
        worker.set_module_resolver(Box::new(NoImports));
    }
    let settled = worker.run_module(&name, source);
    if settled.is_ok()
        || matches!(
            settled,
            Err((ModuleStep::Runtime | ModuleStep::Resolution, _))
        )
    {
        authenticate_dynamic_import(
            worker.module_graph_contains_dynamic_import(),
            exact_module.dynamic_import_policy(),
            &options.test,
            "Module graph",
        )?;
    }
    let expected = negative_phase(metadata);
    Ok(match settled {
        Err((step, failure)) => {
            let phase = match step {
                ModuleStep::Parse => "parse",
                ModuleStep::Resolution => "resolution",
                ModuleStep::Runtime => "runtime",
            };
            match failure {
                Failure::Unsupported(message) => WorkerResult::failure(
                    if step == ModuleStep::Runtime {
                        "unsupported-runtime"
                    } else if step == ModuleStep::Resolution {
                        "unsupported-resolution"
                    } else {
                        "unsupported-parser"
                    },
                    phase,
                    "Unsupported",
                    message,
                ),
                failure
                    if step == ModuleStep::Runtime
                        && expected.is_some_and(|phase| {
                            matches!(phase, "parse" | "early" | "resolution")
                        }) =>
                {
                    // The current worker stops before evaluating once the
                    // expected phase passed without an error.
                    let _ = failure;
                    classify_normal(metadata)
                }
                failure => classify_failure(metadata, phase, failure),
            }
        }
        Ok(_)
            if expected.is_some_and(|phase| matches!(phase, "parse" | "early" | "resolution")) =>
        {
            classify_normal(metadata)
        }
        Ok(Settled::Rejected(thrown)) => {
            classify_completion(metadata, "runtime", &diagnostic(thrown))
        }
        Ok(Settled::Pending) => WorkerResult::failure(
            "fail-runtime",
            "runtime",
            "TypeError",
            "module evaluation promise is pending after the job queue drained",
        ),
        Ok(Settled::Fulfilled) if metadata.is_async() => {
            classify_async_print_log(metadata, &worker.take_print_log())
        }
        Ok(Settled::Fulfilled) => classify_normal(metadata),
    })
}

struct NoImports;

impl ModuleResolver for NoImports {
    fn normalize(&self, _referrer: &str, specifier: &str) -> Result<String, String> {
        Err(format!(
            "dependency-free Test262 module imported '{specifier}'"
        ))
    }

    fn load(&self, name: &str) -> Result<String, String> {
        Err(format!("dependency-free Test262 module loaded '{name}'"))
    }
}
