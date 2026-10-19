//! `qjs`: the command-line interpreter. Its options are QuickJS's `qjs`'s,
//! plus the core's measurement and test options.

use quickjs_oxide::QUICKJS_COMPAT_VERSION;
use quickjs_oxide::engine::api::core::{
    Context, DebugInfoMode, HeapLimits, JsString, ModuleImportAttributes, ModuleLoader,
    ModuleSource, NativeErrorKind, PromiseState, QjsHelpers, Runtime, RuntimeError, RuntimeOptions,
    Scope, Value, declare_thread_stack, number_to_string, quickjs_detect_module_bytes,
};
use quickjs_oxide_host::SystemHost;
use std::io::Write as _;
use std::process::ExitCode;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SourceGoal {
    #[default]
    Auto,
    Script,
    Module,
}

/// Host stack reserved for the evaluation thread.
///
/// QuickJS bounds its parser with a one-MiB C stack budget, where recursive
/// descent frames are small. The Rust parser approximates the same nesting
/// depths with a weighted budget, but some productions have larger Rust
/// frames, and the core re-enters its loop for calls from natives and
/// for generators. An explicitly sized thread lets them reach QuickJS's
/// depths. The reservation is virtual address space, not resident memory.
/// Debug frames are roughly an order of magnitude larger than release
/// frames, so the debug binary reserves proportionally more.
#[cfg(debug_assertions)]
const EVALUATION_STACK_SIZE: usize = 256 * 1024 * 1024;
#[cfg(not(debug_assertions))]
const EVALUATION_STACK_SIZE: usize = 32 * 1024 * 1024;

const USAGE: &str = "\
usage: qjs [options] [file [args]]
  -e, --eval EXPR       evaluate EXPR
  -m, --module          load as an ES module
      --script          load as a script
  -s                    strip all debug information
      --strip-source    strip only function source text
      --no-unhandled-rejection ignore unhandled promise rejections
      --print-result    print the script completion value
  -q, --quit            create the runtime and exit
      --std             define load and read
  -v, --version         show version and compatibility target
memory and measurement:
      --heap-cap-mib N  cap the heap at N MiB
      --min-heap-mib N  reserved heap budget floor (default 1)
      --gc-growth F     bytes allocated between collections per live byte (default 2)
      --stack-limit N   registers the register file may hold
      --gc-stats        print the collector's statistics at exit
      --memory-report   print the memory report at exit
      --ic-stats        print the inline-cache report at exit (ic-stats build)
      --gc-stress=N     collect before every N-th allocation (gc-stress build)
      --disassemble     print the register code of FILE and exit
Test262:
      --test262         define $262 (test262-host build)
      --cannot-block    forbid Atomics.wait from blocking";

/// Run `task` on a thread with [`EVALUATION_STACK_SIZE`] of host stack. The
/// runtime and all of its `Rc` state are created inside the closure and stay
/// on the worker thread for its whole lifetime.
fn run_on_evaluation_stack(task: impl FnOnce() -> ExitCode + Send + 'static) -> ExitCode {
    let handle = match std::thread::Builder::new()
        .name("qjs-evaluation".to_owned())
        .stack_size(EVALUATION_STACK_SIZE)
        .spawn(|| {
            declare_thread_stack(EVALUATION_STACK_SIZE);
            task()
        }) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("qjs: unable to start evaluation thread: {error}");
            return ExitCode::from(1);
        }
    };
    match handle.join() {
        Ok(exit_code) => exit_code,
        Err(_) => ExitCode::from(101),
    }
}

/// What the options select, moved onto the evaluation thread.
#[derive(Clone, Debug, Default)]
struct Options {
    debug_info: DebugInfoMode,
    print_result: bool,
    dump_unhandled_rejection: bool,
    std: bool,
    heap_cap: Option<usize>,
    min_heap: Option<usize>,
    growth: Option<f64>,
    stack_limit: Option<u32>,
    gc_stats: bool,
    memory_report: bool,
    ic_stats: bool,
    gc_stress: Option<u32>,
    disassemble: bool,
    test262: bool,
    cannot_block: bool,
}

impl Options {
    fn runtime_options(&self) -> RuntimeOptions {
        let defaults = RuntimeOptions::default();
        RuntimeOptions {
            heap: HeapLimits {
                max_bytes: self.heap_cap.unwrap_or(defaults.heap.max_bytes),
            },
            min_heap: self.min_heap.unwrap_or(defaults.min_heap),
            growth: self.growth.unwrap_or(defaults.growth),
            stack_limit: self.stack_limit.unwrap_or(defaults.stack_limit),
            ..defaults
        }
    }
}

fn version() -> ExitCode {
    println!(
        "quickjs-oxide {} (QuickJS {} compatibility target)",
        env!("CARGO_PKG_VERSION"),
        QUICKJS_COMPAT_VERSION
    );
    ExitCode::SUCCESS
}

fn parse_count<T: std::str::FromStr>(option: &str, value: Option<&String>) -> Result<T, String> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    value
        .parse()
        .map_err(|_| format!("invalid value for {option}: {value}"))
}

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match parse_and_run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("qjs: {message}");
            ExitCode::from(2)
        }
    }
}

fn parse_and_run(args: &[String]) -> Result<ExitCode, String> {
    let mut options = Options {
        dump_unhandled_rejection: true,
        ..Options::default()
    };
    let mut expression = None;
    let mut quit = false;
    let mut source_goal = SourceGoal::Auto;
    let mut index = 0;
    while index < args.len() && args[index].starts_with('-') && args[index] != "-" {
        let option = args[index].clone();
        index += 1;
        match option.as_str() {
            "--" => break,
            "--strip-source" => options.debug_info = DebugInfoMode::StripSource,
            "--no-unhandled-rejection" => options.dump_unhandled_rejection = false,
            "--print-result" => options.print_result = true,
            "--module" => source_goal = SourceGoal::Module,
            "--script" => source_goal = SourceGoal::Script,
            "--version" => return Ok(version()),
            "--help" => {
                println!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            "--quit" => quit = true,
            "--std" => options.std = true,
            "--gc-stats" => options.gc_stats = true,
            "--memory-report" => options.memory_report = true,
            "--ic-stats" => {
                if !Runtime::IC_STATS_AVAILABLE {
                    return Err("--ic-stats needs a build with the ic-stats feature".into());
                }
                options.ic_stats = true;
            }
            "--disassemble" => options.disassemble = true,
            "--test262" => {
                if !cfg!(feature = "test262-host") {
                    return Err("--test262 needs a build with the test262-host feature".into());
                }
                options.test262 = true;
            }
            "--cannot-block" => options.cannot_block = true,
            "--heap-cap-mib" => {
                let mib: usize = parse_count(&option, args.get(index))?;
                options.heap_cap = Some(mib << 20);
                index += 1;
            }
            "--min-heap-mib" => {
                let mib: usize = parse_count(&option, args.get(index))?;
                options.min_heap = Some(mib << 20);
                index += 1;
            }
            "--gc-growth" => {
                let growth: f64 = parse_count(&option, args.get(index))?;
                if growth.is_nan() || growth <= 1.0 {
                    return Err(format!("invalid value for {option}: {growth}"));
                }
                options.growth = Some(growth);
                index += 1;
            }
            "--stack-limit" => {
                options.stack_limit = Some(parse_count(&option, args.get(index))?);
                index += 1;
            }
            "--gc-stress" => options.gc_stress = Some(1),
            "--eval" => {
                let source = args.get(index).ok_or("-e requires an expression")?;
                expression = Some(source.clone());
                index += 1;
            }
            stress if stress.starts_with("--gc-stress=") => {
                let value = &stress["--gc-stress=".len()..];
                let interval = value
                    .parse()
                    .map_err(|_| format!("--gc-stress: not a count: {value}"))?;
                options.gc_stress = Some(interval);
            }
            short if short.starts_with('-') && !short.starts_with("--") => {
                for (offset, short_option) in short[1..].char_indices() {
                    match short_option {
                        's' => options.debug_info = DebugInfoMode::StripDebug,
                        'm' => source_goal = SourceGoal::Module,
                        'q' => quit = true,
                        'v' => return Ok(version()),
                        'h' => {
                            println!("{USAGE}");
                            return Ok(ExitCode::SUCCESS);
                        }
                        'e' => {
                            let source_offset = 1 + offset + short_option.len_utf8();
                            if source_offset < short.len() {
                                expression = Some(short[source_offset..].to_owned());
                            } else {
                                let source = args.get(index).ok_or("-e requires an expression")?;
                                expression = Some(source.clone());
                                index += 1;
                            }
                            break;
                        }
                        _ => return Err(format!("unknown option: -{short_option}")),
                    }
                }
            }
            _ => return Err(format!("unknown option: {option}")),
        }
    }
    if options.gc_stress.is_some() && !Runtime::GC_STRESS_AVAILABLE {
        return Err("--gc-stress needs a build with the gc-stress feature".into());
    }

    if quit {
        let runtime = Runtime::with_host(SystemHost::default(), options.runtime_options());
        drop(runtime.context());
        return Ok(ExitCode::SUCCESS);
    }
    let request = if let Some(source) = expression {
        let source_goal = match source_goal {
            SourceGoal::Module => SourceGoal::Module,
            SourceGoal::Auto | SourceGoal::Script => SourceGoal::Script,
        };
        // On Unix, pinned qjs ignores js_module_set_import_meta's failed
        // realpath("<cmdline>") and leaves import.meta empty. Its Windows path
        // has no realpath call and initializes file://<cmdline> normally.
        #[cfg(windows)]
        let main_module_path =
            (source_goal == SourceGoal::Module).then_some("<cmdline>".to_owned());
        #[cfg(not(windows))]
        let main_module_path = None;
        EvaluationRequest {
            source: source.into_bytes(),
            filename: "<cmdline>".to_owned(),
            source_goal,
            main_module_path,
            script_args: args[index..].to_vec(),
            options,
        }
    } else {
        let Some(file) = args.get(index) else {
            println!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        };
        let source = match std::fs::read(file) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("{file}: {error}");
                return Ok(ExitCode::from(1));
            }
        };
        let source_goal = match source_goal {
            SourceGoal::Auto if is_module_file(file, &source) => SourceGoal::Module,
            SourceGoal::Auto => SourceGoal::Script,
            source_goal => source_goal,
        };
        EvaluationRequest {
            source,
            filename: file.clone(),
            source_goal,
            main_module_path: Some(file.clone()),
            script_args: args[index..].to_vec(),
            options,
        }
    };
    Ok(run_on_evaluation_stack(move || evaluate(&request)))
}

/// Owned evaluation inputs, safe to move onto the evaluation thread.
struct EvaluationRequest {
    source: Vec<u8>,
    filename: String,
    source_goal: SourceGoal,
    main_module_path: Option<String>,
    script_args: Vec<String>,
    options: Options,
}

#[derive(Debug)]
struct FileModuleLoader;

impl ModuleLoader for FileModuleLoader {
    fn load(
        &mut self,
        scope: &mut Scope<'_>,
        name: &str,
        attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value> {
        let throw = |scope: &mut Scope<'_>, kind: NativeErrorKind, message: String| {
            scope
                .new_native_error(kind, &message)
                .unwrap_or(Value::Undefined)
        };
        if let Some(attributes) = attributes.effective() {
            for attribute in attributes {
                if !attribute.key.utf16_units().eq("type".encode_utf16()) {
                    let key = attribute.key.to_utf8_lossy();
                    return Err(throw(
                        scope,
                        NativeErrorKind::Type,
                        format!("import attribute '{key}' is not supported"),
                    ));
                }
            }
        }
        // QuickJS's `js_module_loader` (`quickjs-libc.c`): "could not load
        // module filename '%s'".
        let source = std::fs::read(name).map_err(|_| {
            throw(
                scope,
                NativeErrorKind::Reference,
                format!("could not load module filename '{name}'"),
            )
        })?;
        if import_type_is(attributes, "json5") {
            return Ok(ModuleSource::Json5(source));
        }
        if name.ends_with(".json") || import_type_is(attributes, "json") {
            return Ok(ModuleSource::Json(source));
        }
        let url = canonical_file_url(name)
            .map_err(|message| throw(scope, NativeErrorKind::Type, message))?;
        Ok(ModuleSource::Source {
            text: source,
            import_meta: import_meta(&url, false),
        })
    }
}

fn import_type_is(attributes: &ModuleImportAttributes, expected: &str) -> bool {
    attributes.effective().is_some_and(|attributes| {
        attributes.iter().any(|attribute| {
            attribute.key.utf16_units().eq("type".encode_utf16())
                && attribute.value.utf16_units().eq(expected.encode_utf16())
        })
    })
}

fn canonical_file_url(filename: &str) -> Result<String, String> {
    if filename.contains(':') {
        return Ok(filename.to_owned());
    }
    #[cfg(windows)]
    {
        return Ok(format!("file://{filename}"));
    }
    #[cfg(not(windows))]
    let canonical = std::fs::canonicalize(filename).map_err(|_| "realpath failure".to_owned())?;
    #[cfg(not(windows))]
    let canonical = canonical
        .to_str()
        .ok_or_else(|| "module filename is not valid Unicode".to_owned())?;
    #[cfg(not(windows))]
    Ok(format!("file://{canonical}"))
}

/// QuickJS's `js_module_set_import_meta`: `url` and `main`.
fn import_meta(url: &str, is_main: bool) -> Vec<(String, Value)> {
    let url = JsString::try_from_utf8(url).map_or(Value::Undefined, Value::String);
    vec![
        ("url".to_owned(), url),
        ("main".to_owned(), Value::Bool(is_main)),
    ]
}

fn is_module_file(filename: &str, source: &[u8]) -> bool {
    filename.ends_with(".mjs") || quickjs_detect_module_bytes(source)
}

fn evaluate(request: &EvaluationRequest) -> ExitCode {
    let options = &request.options;
    let runtime = Runtime::with_host(SystemHost::default(), options.runtime_options());
    let mut context = runtime.context();
    let setup = (|| -> Result<(), RuntimeError> {
        runtime.set_debug_info_mode(options.debug_info)?;
        runtime.set_gc_stress(options.gc_stress)?;
        runtime.set_record_gc_pauses(options.gc_stats)?;
        // QuickJS's `qjs` main thread may not block (`Atomics.wait` throws);
        // `--cannot-block` keeps that explicit for the Test262 shells.
        let _ = options.cannot_block;
        runtime.set_can_block(false)?;
        // Upstream qjs installs its filesystem loader for every process,
        // including Script-goal `-e`, so dynamic import has the same host
        // boundary everywhere.
        runtime.set_module_loader(FileModuleLoader)?;
        // `js_std_promise_rejection_tracker`.
        runtime.set_track_rejections(options.dump_unhandled_rejection)?;
        context.install_qjs_helpers(&QjsHelpers {
            std: options.std,
            script_args: Some(request.script_args.clone()),
        })?;
        #[cfg(feature = "test262-host")]
        if options.test262 {
            context.install_test262_host()?;
        }
        Ok(())
    })();
    if let Err(error) = setup {
        eprintln!("qjs: {error}");
        return ExitCode::FAILURE;
    }
    if options.disassemble {
        return match runtime.disassemble(&request.source, &request.filename) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        };
    }
    let evaluation = match request.source_goal {
        SourceGoal::Module => evaluate_module(&runtime, &mut context, request),
        _ => context
            .eval_bytes_with_filename(&request.source, &request.filename)
            .map_err(Failure::Runtime),
    };
    let code = match evaluation {
        Ok(value) => {
            if !drain_pending_jobs(&runtime, &mut context) || report_unhandled_rejections(&runtime)
            {
                ExitCode::from(1)
            } else {
                if options.print_result {
                    println!("{}", completion_text(&value));
                }
                ExitCode::SUCCESS
            }
        }
        Err(Failure::Rejected(exception)) => {
            report_exception(&runtime, &exception);
            ExitCode::from(1)
        }
        Err(Failure::Runtime(RuntimeError::Exception)) => {
            report_pending_exception(&runtime, &mut context);
            ExitCode::from(1)
        }
        Err(Failure::Runtime(error)) => {
            report_runtime_error(&error);
            ExitCode::from(1)
        }
    };
    report_statistics(&runtime, options);
    code
}

/// Why an evaluation failed.
enum Failure {
    Runtime(RuntimeError),
    /// The main module's evaluation was rejected with this reason.
    Rejected(Value),
}

/// Drain the job queue the way `js_std_loop` does: a job that leaves a
/// pending exception is reported and the queue keeps draining. Returns
/// `false` when an internal error (not a JavaScript exception) stopped it.
fn drain_pending_jobs(runtime: &Runtime, context: &mut Context) -> bool {
    loop {
        match runtime.run_pending_job() {
            Ok(outcome) if outcome.executed() => {}
            Ok(_) => return true,
            Err(RuntimeError::Exception) => report_pending_exception(runtime, context),
            Err(error) => {
                report_runtime_error(&error);
                return false;
            }
        }
    }
}

/// `js_std_promise_rejection_check`: report every rejection that remained
/// unhandled once the job queue drained; whether there was one.
fn report_unhandled_rejections(runtime: &Runtime) -> bool {
    let rejections = runtime.take_unhandled_rejections().unwrap_or_default();
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    for (_, reason) in &rejections {
        let _ = stderr.write_all(b"Possibly unhandled promise rejection: ");
        match runtime.qjs_print_value_bytes(reason) {
            Ok(text) => {
                let _ = stderr.write_all(&text);
            }
            Err(_) => {
                let _ = stderr.write_all(b"[unknown]");
            }
        }
        let _ = stderr.write_all(b"\n");
    }
    !rejections.is_empty()
}

fn evaluate_module(
    runtime: &Runtime,
    context: &mut Context,
    request: &EvaluationRequest,
) -> Result<Value, Failure> {
    let import_meta = match &request.main_module_path {
        Some(path) => match canonical_file_url(path) {
            Ok(url) => import_meta(&url, true),
            Err(message) => {
                eprintln!("{message}");
                return Err(Failure::Runtime(RuntimeError::Invariant(
                    "the main module's path has no URL",
                )));
            }
        },
        None => Vec::new(),
    };
    let module = context
        .compile_module_bytes_with_import_meta(&request.source, &request.filename, import_meta)
        .map_err(Failure::Runtime)?;
    let Value::Object(promise) = context.execute_module(&module).map_err(Failure::Runtime)? else {
        return Err(Failure::Runtime(RuntimeError::Invariant(
            "module evaluation did not return a Promise",
        )));
    };
    loop {
        let snapshot = runtime
            .promise_snapshot(&promise)
            .map_err(Failure::Runtime)?
            .ok_or(Failure::Runtime(RuntimeError::Invariant(
                "module evaluation returned a non-Promise object",
            )))?;
        match snapshot.state() {
            PromiseState::Fulfilled => return Ok(snapshot.result().clone()),
            PromiseState::Rejected => return Err(Failure::Rejected(snapshot.result().clone())),
            PromiseState::Pending => match runtime.run_pending_job() {
                Ok(outcome) if outcome.executed() => {}
                Ok(_) => std::thread::yield_now(),
                Err(RuntimeError::Exception) => report_pending_exception(runtime, context),
                Err(error) => return Err(Failure::Runtime(error)),
            },
        }
    }
}

fn completion_text(value: &Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => number_to_string(*value),
        Value::BigInt(value) => value.to_string(),
        Value::String(value) => value.to_utf8_lossy(),
        Value::Object(_) => "[object Object]".to_owned(),
        Value::Symbol(_) => "Symbol()".to_owned(),
    }
}

/// An engine failure that is not a JavaScript exception. The heap cap is
/// reported as QuickJS reports its uncaught out-of-memory error.
fn report_runtime_error(error: &RuntimeError) {
    match error {
        RuntimeError::OutOfMemory => eprintln!("InternalError: out of memory"),
        other => eprintln!("{other}"),
    }
}

fn report_pending_exception(runtime: &Runtime, context: &mut Context) {
    match context.take_exception().ok().flatten() {
        Some(exception) => report_exception(runtime, &exception),
        None => eprintln!("JavaScript exception"),
    }
}

/// An uncaught exception as QuickJS dumps it: `JS_PrintValue`.
fn report_exception(runtime: &Runtime, exception: &Value) {
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    match runtime.qjs_print_value_bytes(exception).ok() {
        Some(text) => {
            let _ = stderr.write_all(&text);
            let _ = stderr.write_all(b"\n");
        }
        None => {
            let _ = stderr.write_all(b"JavaScript exception\n");
        }
    }
}

#[cfg(test)]
fn format_exception(runtime: &Runtime, exception: &Value) -> Option<String> {
    runtime.print_value(exception).ok()
}

/// `--gc-stats`, `--ic-stats` and `--memory-report`, to stderr at exit.
fn report_statistics(runtime: &Runtime, options: &Options) {
    if options.ic_stats
        && let Ok(Some(report)) = runtime.ic_report(10)
    {
        eprint!("{report}");
    }
    if options.gc_stats
        && let Ok(report) = runtime.gc_report()
    {
        eprint!("{report}");
    }
    if options.memory_report
        && let Ok(report) = runtime.memory_report()
    {
        eprint!("{report}");
    }
}

#[cfg(test)]
mod tests {
    use super::format_exception;
    use quickjs_oxide::engine::api::core::{
        AccessorValue, CallableRef, DescriptorField, JsString, OrdinaryPropertyDescriptor, Runtime,
        Value,
    };

    fn data_descriptor(value: Value) -> OrdinaryPropertyDescriptor {
        OrdinaryPropertyDescriptor {
            value: DescriptorField::Present(value),
            writable: DescriptorField::Present(true),
            enumerable: DescriptorField::Present(false),
            configurable: DescriptorField::Present(true),
            ..OrdinaryPropertyDescriptor::new()
        }
    }

    fn accessor_descriptor(getter: CallableRef) -> OrdinaryPropertyDescriptor {
        OrdinaryPropertyDescriptor {
            get: DescriptorField::Present(AccessorValue::Callable(getter)),
            set: DescriptorField::Present(AccessorValue::Undefined),
            enumerable: DescriptorField::Present(false),
            configurable: DescriptorField::Present(true),
            ..OrdinaryPropertyDescriptor::new()
        }
    }

    #[test]
    fn error_dump_uses_raw_shadowing_and_never_executes_getters() {
        let runtime = Runtime::new_with_host_services(quickjs_oxide_host::SystemHost::default());
        let mut context = runtime.context();
        let Value::Object(error) = context.eval("new Error(\"boom\")").unwrap() else {
            panic!("Error constructor did not return an object");
        };
        let prototype = runtime
            .get_prototype_of(&error)
            .unwrap()
            .expect("Error has a prototype");
        let Value::Object(getter_object) = context
            .eval("(function diagnosticGetter(){ throw \"getter ran\"; })")
            .unwrap()
        else {
            panic!("getter expression did not return an object");
        };
        let getter = runtime
            .as_callable(&getter_object)
            .unwrap()
            .expect("getter is callable");

        for (name, prototype_value) in [
            ("name", "PrototypeName"),
            ("message", "PrototypeMessage"),
            ("stack", "prototype stack\n"),
        ] {
            let key = runtime.intern_property_key(name).unwrap();
            assert!(
                runtime
                    .define_own_property(
                        &prototype,
                        &key,
                        &data_descriptor(Value::String(
                            JsString::try_from_utf8(prototype_value).unwrap(),
                        )),
                    )
                    .unwrap()
            );
            assert!(
                runtime
                    .define_own_property(&error, &key, &accessor_descriptor(getter.clone()))
                    .unwrap()
            );
        }

        assert!(!context.has_exception().expect("runtime state"));
        assert_eq!(
            format_exception(&runtime, &Value::Object(error)),
            Some("Error".to_owned())
        );
        assert!(
            !context.has_exception().expect("runtime state"),
            "diagnostic getter was executed"
        );
    }

    #[test]
    fn error_dump_reads_exactly_one_raw_prototype_level() {
        let runtime = Runtime::new_with_host_services(quickjs_oxide_host::SystemHost::default());
        let mut context = runtime.context();
        let Value::Object(error) = context.eval("new Error()").unwrap() else {
            panic!("Error constructor did not return an object");
        };
        let prototype = runtime
            .get_prototype_of(&error)
            .unwrap()
            .expect("Error has a prototype");

        for (name, value) in [
            ("name", "PrototypeName"),
            ("message", "PrototypeMessage"),
            ("stack", "prototype stack\n"),
        ] {
            let key = runtime.intern_property_key(name).unwrap();
            assert!(
                runtime
                    .define_own_property(
                        &prototype,
                        &key,
                        &data_descriptor(Value::String(JsString::try_from_utf8(value).unwrap())),
                    )
                    .unwrap()
            );
        }
        let stack = runtime.intern_property_key("stack").unwrap();
        assert!(runtime.delete_property(&error, &stack).unwrap());

        assert_eq!(
            format_exception(&runtime, &Value::Object(error)),
            Some("PrototypeName: PrototypeMessage\nprototype stack".to_owned())
        );
    }
}
