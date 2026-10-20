use crate::runtime_observation::host_runtime;
use quickjs_oxide::engine::api::core::{
    JsString, ModuleImportAttributes, ModuleLoader, ModuleScope, ModuleSource, NativeErrorKind,
    PromiseState, Value,
};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::rc::Rc;

const MODULE_GLOBAL_SHADOW_TRANSCRIPT: &str =
    include_str!("../fixtures/expected/module-global-shadow/quickjs-2026-06-04.txt");

#[derive(Debug, PartialEq, Eq)]
struct CallbackEvent {
    phase: &'static str,
    subject: String,
}

/// A loader that compiles every module through its `ModuleScope`, for `import()`
/// of a two-module graph.
#[derive(Debug)]
struct DynamicReentryLoader {
    events: Rc<RefCell<Vec<CallbackEvent>>>,
}

impl ModuleLoader for DynamicReentryLoader {
    fn normalize(
        &mut self,
        scope: &mut ModuleScope<'_>,
        base_name: &str,
        specifier: &str,
        _attributes: &ModuleImportAttributes,
    ) -> Result<String, Value> {
        self.events.borrow_mut().push(CallbackEvent {
            phase: "normalize",
            subject: format!("{base_name}|{specifier}"),
        });
        match (base_name, specifier) {
            ("dynamic-reentry-entry.js", "./outer.js") => Ok("outer.js".to_owned()),
            ("outer.js", "./inner.js") => Ok("inner.js".to_owned()),
            _ => Err(scope
                .new_native_error(NativeErrorKind::Internal, "unexpected module normalization")
                .unwrap_or(Value::Undefined)),
        }
    }

    fn load(
        &mut self,
        scope: &mut ModuleScope<'_>,
        name: &str,
        _attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value> {
        self.events.borrow_mut().push(CallbackEvent {
            phase: "load",
            subject: name.to_owned(),
        });
        let source: &[u8] = match name {
            "outer.js" => {
                b"import { value } from './inner.js'; \
                 globalThis.__dynamicReentryOrder.push('outer'); \
                 export const answer = value + 1;"
            }
            "inner.js" => {
                b"globalThis.__dynamicReentryOrder.push('inner'); export const value = 41;"
            }
            _ => {
                return Err(scope
                    .new_native_error(NativeErrorKind::Internal, "unexpected module load")
                    .unwrap_or(Value::Undefined));
            }
        };
        scope
            .compile_module(name, source, Vec::new())
            .map(ModuleSource::Compiled)
    }
}

#[test]
fn dynamic_import_accepts_modules_compiled_through_the_loader_scope() {
    let runtime = host_runtime();
    let events = Rc::new(RefCell::new(Vec::new()));
    runtime
        .set_module_loader(DynamicReentryLoader {
            events: events.clone(),
        })
        .unwrap();
    let context = runtime.new_context().expect("create context");

    let Value::Object(promise) = context
        .eval_with_filename(
            "globalThis.__dynamicReentryOrder = []; \
             globalThis.__dynamicReentryResult = 'pending'; \
             import('./outer.js').then(function (namespace) { \
               globalThis.__dynamicReentryOrder.push('entry'); \
               globalThis.__dynamicReentryResult = namespace.answer; \
               return namespace.answer; \
             });",
            "dynamic-reentry-entry.js",
        )
        .unwrap()
    else {
        panic!("dynamic import did not return a Promise");
    };

    assert!(events.borrow().is_empty(), "dynamic load ran synchronously");
    let mut executed_jobs = 0;
    while runtime.execute_pending_job().unwrap().executed() {
        executed_jobs += 1;
        assert!(executed_jobs <= 8, "dynamic import jobs did not quiesce");
    }

    let event = |phase, subject: &str| CallbackEvent {
        phase,
        subject: subject.to_owned(),
    };
    assert_eq!(
        events.borrow().as_slice(),
        [
            event("normalize", "dynamic-reentry-entry.js|./outer.js"),
            event("load", "outer.js"),
            event("normalize", "outer.js|./inner.js"),
            event("load", "inner.js"),
        ]
    );
    assert_eq!(
        context
            .eval("JSON.stringify(__dynamicReentryOrder)")
            .unwrap(),
        Value::String(JsString::try_from_utf8("[\"inner\",\"outer\",\"entry\"]").unwrap())
    );
    assert_eq!(
        context.eval("__dynamicReentryResult").unwrap(),
        Value::Int(42)
    );
    let snapshot = runtime
        .promise_snapshot(&promise)
        .unwrap()
        .expect("dynamic import result was not a Promise");
    assert_eq!(snapshot.state(), PromiseState::Fulfilled);
    assert_eq!(snapshot.result(), &Value::Int(42));
    assert!(!runtime.is_job_pending().expect("runtime state"));
}

#[test]
fn module_global_binding_survives_nested_and_sibling_lexical_shadowing() {
    let fixture = module_global_shadow_fixture_dir();
    let oxide =
        run_module_global_shadow_file(env!("CARGO_BIN_EXE_qjs").as_ref(), &fixture, "entry.mjs");
    assert_module_global_shadow_success("quickjs-oxide", "entry.mjs", &oxide);
    assert_eq!(
        String::from_utf8_lossy(&oxide.stdout),
        MODULE_GLOBAL_SHADOW_TRANSCRIPT,
        "quickjs-oxide confused the exported module cell with a nested or sibling lexical",
    );

    for invalid in ["same-scope.mjs", "overlapping-scope.mjs"] {
        let rejected =
            run_module_global_shadow_file(env!("CARGO_BIN_EXE_qjs").as_ref(), &fixture, invalid);
        assert_module_global_shadow_rejection("quickjs-oxide", invalid, &rejected);
    }

    let oracle = require_pinned_qjs_oracle!("module-global shadow differential");
    let quickjs = run_module_global_shadow_file(&oracle, &fixture, "entry.mjs");
    assert_module_global_shadow_success("pinned QuickJS", "entry.mjs", &quickjs);
    assert_eq!(
        String::from_utf8_lossy(&quickjs.stdout),
        MODULE_GLOBAL_SHADOW_TRANSCRIPT,
        "pinned QuickJS transcript differs from the frozen module-global shadow fixture",
    );
    assert_eq!(
        oxide.stdout, quickjs.stdout,
        "quickjs-oxide module-global shadow transcript differs from pinned QuickJS",
    );

    for invalid in ["same-scope.mjs", "overlapping-scope.mjs"] {
        let rejected = run_module_global_shadow_file(&oracle, &fixture, invalid);
        assert_module_global_shadow_rejection("pinned QuickJS", invalid, &rejected);
    }
}

fn module_global_shadow_fixture_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("apps/cli/tests/fixtures/inputs/module-global-shadow")
}

fn run_module_global_shadow_file(executable: &OsStr, fixture: &Path, filename: &str) -> Output {
    Command::new(executable)
        .args(["--module", filename])
        .current_dir(fixture)
        .output()
        .unwrap_or_else(|error| {
            panic!("could not run {executable:?} for module-global shadow {filename}: {error}")
        })
}

fn assert_module_global_shadow_success(engine: &str, filename: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{engine} rejected module-global shadow fixture {filename}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.stderr.is_empty(),
        "{engine} wrote stderr for module-global shadow fixture {filename}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_module_global_shadow_rejection(engine: &str, filename: &str, output: &Output) {
    assert!(
        !output.status.success(),
        "{engine} accepted conflicting module declarations in {filename}",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("SyntaxError"),
        "{engine} did not report SyntaxError for {filename}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}
