//! ES modules: compiling and running a module, a module loader over an
//! in-memory file system, JSON modules, import attributes, `import.meta` and
//! dynamic `import()`. Backs apps/web/docs/modules.md.
//!
//! ```sh
//! cargo run --quiet --example modules
//! ```

use std::collections::HashMap;

use quickjs_oxide::engine::api::core::{
    Context, JsString, ModuleImportAttributes, ModuleLoader, ModuleScope, ModuleSource,
    NativeErrorKind, PromiseState, Runtime, RuntimeError, Value,
};

/// The value of `globalThis[name]`.
fn global(context: &mut Context, name: &str) -> Value {
    context.eval(name).unwrap()
}

fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_utf8_lossy(),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn run_module() {
    // docs:start run-module
    let runtime = Runtime::new();
    let context = runtime.context();

    let module = context
        .compile_module_with_filename(
            "await Promise.resolve(); globalThis.answer = 6 * 7;",
            "main.mjs",
        )
        .unwrap();
    assert_eq!(module.name(), "main.mjs");

    // The result is the promise of the module's evaluation.
    let Value::Object(promise) = context.execute_module(&module).unwrap() else {
        panic!("expected a promise");
    };
    runtime.run_jobs().unwrap();

    let snapshot = runtime.promise_snapshot(&promise).unwrap().unwrap();
    assert_eq!(snapshot.state(), PromiseState::Fulfilled);
    assert_eq!(context.eval("answer").unwrap(), Value::Int(42));
    // docs:end run-module
}

// docs:start loader
/// Modules from an in-memory map of normalized names to source text.
struct MemoryLoader {
    files: HashMap<String, String>,
}

impl ModuleLoader for MemoryLoader {
    // `normalize` keeps its default: a specifier starting with `.` resolves
    // against the importing module's directory, anything else is kept as is.

    fn load(
        &mut self,
        scope: &mut ModuleScope<'_>,
        name: &str,
        attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value> {
        let Some(text) = self.files.get(name) else {
            let message = format!("could not load module filename '{name}'");
            return Err(scope
                .new_native_error(NativeErrorKind::Reference, &message)
                .unwrap_or(Value::Undefined));
        };
        if name.ends_with(".json") || import_type_is(attributes, "json") {
            return Ok(ModuleSource::Json(text.clone().into_bytes()));
        }
        let url = JsString::try_from_utf8(&format!("mem:///{name}")).unwrap();
        Ok(ModuleSource::Source {
            text: text.clone().into_bytes(),
            import_meta: vec![("url".to_owned(), Value::String(url))],
        })
    }
}

/// Whether the import has `with { type: expected }`.
fn import_type_is(attributes: &ModuleImportAttributes, expected: &str) -> bool {
    attributes.effective().is_some_and(|attributes| {
        attributes.iter().any(|attribute| {
            attribute.key.to_utf8_lossy() == "type" && attribute.value.to_utf8_lossy() == expected
        })
    })
}
// docs:end loader

fn use_loader() {
    // docs:start use-loader
    let files = HashMap::from([
        (
            "lib/math.mjs".to_owned(),
            "export const square = (x) => x * x; export const where = import.meta.url;".to_owned(),
        ),
        ("lib/config.json".to_owned(), r#"{ "size": 9 }"#.to_owned()),
    ]);
    let runtime = Runtime::new();
    runtime.set_module_loader(MemoryLoader { files }).unwrap();
    let mut context = runtime.context();

    let module = context
        .compile_module_with_filename(
            r#"
            import { square, where } from "./math.mjs";
            import config from "./config.json" with { type: "json" };
            globalThis.result = square(config.size);
            globalThis.where = where;
            "#,
            "lib/main.mjs",
        )
        .unwrap();
    context.execute_module(&module).unwrap();
    runtime.run_jobs().unwrap();

    assert_eq!(global(&mut context, "result"), Value::Int(81));
    assert_eq!(text(&global(&mut context, "where")), "mem:///lib/math.mjs");
    // docs:end use-loader
}

fn missing_import() {
    let runtime = Runtime::new();
    runtime
        .set_module_loader(MemoryLoader {
            files: HashMap::new(),
        })
        .unwrap();
    let context = runtime.context();
    // docs:start missing-import
    // A module that cannot be loaded fails execute_module itself.
    let module = context
        .compile_module_with_filename("import './gone.mjs';", "main.mjs")
        .unwrap();
    let Err(RuntimeError::Exception(exception)) = context.execute_module(&module) else {
        panic!("expected a ReferenceError");
    };
    assert!(
        runtime
            .print_value(&exception)
            .unwrap()
            .starts_with("ReferenceError: could not load module filename 'gone.mjs'")
    );
    // docs:end missing-import
}

fn dynamic_import() {
    let files = HashMap::from([(
        "greet.mjs".to_owned(),
        "export default (name) => `hello, ${name}`;".to_owned(),
    )]);
    let runtime = Runtime::new();
    runtime.set_module_loader(MemoryLoader { files }).unwrap();
    let mut context = runtime.context();

    // docs:start dynamic-import
    context
        .eval(
            "import('greet.mjs').then((m) => { globalThis.greeting = m.default('module'); });
             import('nope.mjs').catch((e) => { globalThis.failure = e.message; });",
        )
        .unwrap();
    runtime.run_jobs().unwrap();

    assert_eq!(text(&global(&mut context, "greeting")), "hello, module");
    assert_eq!(
        text(&global(&mut context, "failure")),
        "could not load module filename 'nope.mjs'"
    );
    // docs:end dynamic-import
}

fn import_meta() {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    // docs:start import-meta
    let module = context
        .compile_module_bytes_with_import_meta(
            b"globalThis.meta = [import.meta.url, import.meta.main].join(' ');",
            "app.mjs",
            vec![
                (
                    "url".to_owned(),
                    Value::String(JsString::from_static("file:///app.mjs")),
                ),
                ("main".to_owned(), Value::Bool(true)),
            ],
        )
        .unwrap();
    context.execute_module(&module).unwrap();
    runtime.run_jobs().unwrap();
    assert_eq!(text(&global(&mut context, "meta")), "file:///app.mjs true");
    // docs:end import-meta
}

fn failures() {
    let runtime = Runtime::new();
    let context = runtime.context();
    // docs:start failures
    // A syntax error is thrown by the compile call.
    let Err(RuntimeError::Exception(exception)) =
        context.compile_module_with_filename("export {", "bad.mjs")
    else {
        panic!("expected a SyntaxError");
    };
    assert!(
        runtime
            .print_value(&exception)
            .unwrap()
            .starts_with("SyntaxError")
    );

    // An error while the module runs rejects the evaluation promise.
    let module = context
        .compile_module_with_filename("throw new RangeError('boom');", "throws.mjs")
        .unwrap();
    let Value::Object(promise) = context.execute_module(&module).unwrap() else {
        panic!("expected a promise");
    };
    runtime.run_jobs().unwrap();
    let snapshot = runtime.promise_snapshot(&promise).unwrap().unwrap();
    assert_eq!(snapshot.state(), PromiseState::Rejected);
    let reason = runtime.print_value(snapshot.result()).unwrap();
    assert!(reason.starts_with("RangeError: boom"));
    // docs:end failures
}

// docs:start normalize
/// A loader that maps `std:NAME` to one built-in module and compiles it
/// through the scope.
struct StdLoader;

impl ModuleLoader for StdLoader {
    fn normalize(
        &mut self,
        scope: &mut ModuleScope<'_>,
        _referrer: &str,
        specifier: &str,
        _attributes: &ModuleImportAttributes,
    ) -> Result<String, Value> {
        match specifier.strip_prefix("std:") {
            Some(name) => Ok(format!("<std>/{name}.mjs")),
            None => Err(scope
                .new_native_error(NativeErrorKind::Type, "only std: modules")
                .unwrap_or(Value::Undefined)),
        }
    }

    fn load(
        &mut self,
        scope: &mut ModuleScope<'_>,
        name: &str,
        _attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value> {
        let text = format!(
            "globalThis.evaluations = (globalThis.evaluations ?? 0) + 1;
             export const name = {name:?};"
        );
        let module = scope.compile_module(name, text.as_bytes(), Vec::new())?;
        Ok(ModuleSource::Compiled(module))
    }
}
// docs:end normalize

fn custom_normalize() {
    let runtime = Runtime::new();
    runtime.set_module_loader(StdLoader).unwrap();
    let mut context = runtime.context();
    let module = context
        .compile_module_with_filename(
            "import { name } from 'std:os'; globalThis.loaded = name;",
            "main.mjs",
        )
        .unwrap();
    context.execute_module(&module).unwrap();
    runtime.run_jobs().unwrap();
    assert_eq!(text(&global(&mut context, "loaded")), "<std>/os.mjs");

    context
        .eval("import('./x.mjs').catch((e) => { globalThis.refused = e.message; })")
        .unwrap();
    runtime.run_jobs().unwrap();
    assert_eq!(text(&global(&mut context, "refused")), "only std: modules");

    // A second import of the same name reuses the loaded module.
    context
        .eval("import('std:os').then((m) => { globalThis.again = m.name; })")
        .unwrap();
    runtime.run_jobs().unwrap();
    assert_eq!(text(&global(&mut context, "again")), "<std>/os.mjs");
    assert_eq!(global(&mut context, "evaluations"), Value::Int(1));
}

fn main() {
    run_module();
    custom_normalize();
    use_loader();
    missing_import();
    dynamic_import();
    import_meta();
    failures();
    println!("modules: ok");
}
