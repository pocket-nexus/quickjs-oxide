//! The embedding API over the new core (new-core phase 6, section 2.6):
//! every method of sections 2.1–2.4 through the public surface.

use std::cell::RefCell;
use std::rc::Rc;

use quickjs_oxide::engine::api::core::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, Context, CoreValue, DescriptorField,
    GcPolicy, HeapLimits, Host, JsBigInt, JsString, ModuleImportAttributes, ModuleLoader,
    ModuleSource, NativeErrorKind, OrdinaryPropertyDescriptor, PendingJobOutcome, PromiseState,
    QjsHelpers, RejectionEvent, Runtime, RuntimeError, RuntimeOptions, Scope, Stream, Value,
    WellKnownSymbol,
};

fn runtime() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.context();
    (runtime, context)
}

fn thrown(context: &mut Context) -> String {
    let exception = context
        .take_exception()
        .unwrap()
        .expect("a pending exception");
    context.runtime().print_value(&exception).unwrap()
}

#[test]
fn eval_returns_completion_values_of_every_kind() {
    let (_, mut context) = runtime();
    assert_eq!(context.eval("1 + 2").unwrap(), Value::Int(3));
    assert_eq!(context.eval("0.5").unwrap(), Value::Float(0.5));
    assert_eq!(
        context.eval("'a' + 'b'").unwrap(),
        Value::String(JsString::from_static("ab"))
    );
    assert_eq!(context.eval("true").unwrap(), Value::Bool(true));
    assert_eq!(context.eval("null").unwrap(), Value::Null);
    assert_eq!(context.eval("void 0").unwrap(), Value::Undefined);
    assert_eq!(
        context.eval("2n ** 70n").unwrap(),
        Value::BigInt(JsBigInt::parse_js_string("1180591620717411303424").unwrap())
    );
    assert!(matches!(
        context.eval("Symbol('s')").unwrap(),
        Value::Symbol(_)
    ));
    assert!(matches!(context.eval("({})").unwrap(), Value::Object(_)));
    // Declarations persist between scripts of the one realm.
    context.eval("var x = 41; let y = 1;").unwrap();
    assert_eq!(context.eval("x + y").unwrap(), Value::Int(42));
}

#[test]
fn errors_are_pending_exceptions() {
    let (_, mut context) = runtime();
    assert_eq!(context.eval("null.x"), Err(RuntimeError::Exception));
    assert!(context.has_exception().unwrap());
    assert!(
        thrown(&mut context)
            .starts_with("TypeError: cannot read property 'x' of null\n    at <eval> (<input>")
    );
    assert!(!context.has_exception().unwrap());
    // A syntax error is thrown with its location, as `JS_Eval` throws it.
    assert_eq!(
        context.eval_with_filename("var x = ;", "f.js"),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        thrown(&mut context),
        "SyntaxError: unexpected token in expression: ';'\n    at f.js:1:9"
    );
    // A dropped feature (phase 0) is an early SyntaxError.
    assert_eq!(context.eval("with ({}) {}"), Err(RuntimeError::Exception));
    assert!(thrown(&mut context).starts_with("SyntaxError: with statements are not supported"));
}

#[test]
fn one_realm_per_runtime() {
    let runtime = Runtime::new();
    let mut first = runtime.new_context().unwrap();
    let mut second = Context::new(&runtime);
    first.eval("var shared = 7").unwrap();
    assert_eq!(second.eval("shared").unwrap(), Value::Int(7));
    assert_eq!(
        first.global_object().unwrap(),
        second.global_object().unwrap()
    );
    // Separate runtimes are isolated.
    let mut other = Runtime::new().context();
    assert_eq!(
        other.eval("typeof shared").unwrap(),
        Value::String(JsString::from_static("undefined"))
    );
}

#[test]
fn calls_construct_and_properties() {
    let (runtime, mut context) = runtime();
    let Value::Object(add) = context
        .eval("(function (a, b) { return this.base + a + b; })")
        .unwrap()
    else {
        panic!("a function");
    };
    let add = runtime.as_callable(&add).unwrap().expect("callable");
    let receiver = context.new_object().unwrap();
    let base = runtime.intern_property_key("base").unwrap();
    assert!(
        context
            .set_property(&receiver, &base, Value::Int(10))
            .unwrap()
    );
    assert_eq!(
        context
            .call(
                &add,
                Value::Object(receiver.clone()),
                &[Value::Int(1), Value::Int(2)]
            )
            .unwrap(),
        Value::Int(13)
    );
    let Value::Object(point) = context
        .eval("(class Point { constructor(x) { this.x = x; } })")
        .unwrap()
    else {
        panic!("a class");
    };
    let point = runtime.as_callable(&point).unwrap().unwrap();
    assert!(runtime.is_constructor(&point).unwrap());
    let Value::Object(instance) = context.construct(&point, &[Value::Int(5)]).unwrap() else {
        panic!("an instance");
    };
    let x = runtime.intern_property_key("x").unwrap();
    assert_eq!(context.get_property(&instance, &x).unwrap(), Value::Int(5));
    assert_eq!(
        runtime.get_prototype_of(&instance).unwrap(),
        Some(
            match context
                .get_property(&point, &runtime.intern_property_key("prototype").unwrap())
                .unwrap()
            {
                Value::Object(prototype) => prototype,
                _ => panic!("a prototype"),
            }
        )
    );
    // Not a constructor: a TypeError, as `new` throws.
    let Value::Object(arrow) = context.eval("() => 1").unwrap() else {
        panic!("an arrow function");
    };
    let arrow = runtime.as_callable(&arrow).unwrap().unwrap();
    assert!(!runtime.is_constructor(&arrow).unwrap());
    assert_eq!(context.construct(&arrow, &[]), Err(RuntimeError::Exception));
    assert!(thrown(&mut context).starts_with("TypeError"));
    // Descriptors.
    let object = context.new_object().unwrap();
    let key = runtime.intern_property_key("k").unwrap();
    assert!(
        context
            .define_own_property(
                &object,
                &key,
                &OrdinaryPropertyDescriptor::data(Value::Int(1), false, true, false)
            )
            .unwrap()
    );
    assert_eq!(
        context.get_own_property(&object, &key).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(1),
            writable: false,
            enumerable: true,
            configurable: false,
        })
    );
    assert!(!runtime.delete_property(&object, &key).unwrap());
    let getter = match context.eval("(function () { return 9; })").unwrap() {
        Value::Object(getter) => runtime.as_callable(&getter).unwrap().unwrap(),
        _ => panic!("a function"),
    };
    let accessor = runtime.intern_property_key("a").unwrap();
    assert!(
        context
            .define_own_property(
                &object,
                &accessor,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter.clone())),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context.get_property(&object, &accessor).unwrap(),
        Value::Int(9)
    );
    assert!(matches!(
        context.get_own_property(&object, &accessor).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Accessor {
            get: Some(_),
            set: None,
            ..
        })
    ));
    let keys = runtime.own_property_keys(&object).unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(
        runtime.property_key_to_js_string(&keys[0]).unwrap(),
        JsString::from_static("k")
    );
    assert!(runtime.has_own_property(&object, &accessor).unwrap());
    assert!(runtime.delete_property(&object, &accessor).unwrap());
    // Arrays and indices.
    let array = context
        .new_array_from_values(vec![Value::Int(1), Value::Int(2)])
        .unwrap();
    assert!(runtime.is_array_object(&array).unwrap());
    let length = runtime.intern_property_key("length").unwrap();
    assert_eq!(
        context.get_property(&array, &length).unwrap(),
        Value::Int(2)
    );
    let one = runtime.intern_property_key("1").unwrap();
    assert_eq!(context.get_property(&array, &one).unwrap(), Value::Int(2));
}

#[test]
fn symbols_and_conversions() {
    let (runtime, mut context) = runtime();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("tag")))
        .unwrap();
    assert_eq!(
        runtime.symbol_description(&symbol).unwrap(),
        Some(JsString::from_static("tag"))
    );
    let iterator = runtime
        .well_known_symbol(WellKnownSymbol::Iterator)
        .unwrap();
    assert_eq!(
        context.eval("Symbol.iterator").unwrap(),
        Value::Symbol(iterator.clone())
    );
    let array = context.new_array().unwrap();
    assert!(matches!(
        context.get_property(&array, &iterator.into()).unwrap(),
        Value::Object(_)
    ));
    assert_eq!(
        context.to_js_string(&Value::Float(1.5)).unwrap(),
        JsString::from_static("1.5")
    );
    assert_eq!(
        context
            .to_number(&Value::String(JsString::from_static(" 12 ")))
            .unwrap(),
        12.0
    );
    assert_eq!(
        runtime
            .type_of(&context.eval("(function(){})").unwrap())
            .unwrap(),
        "function"
    );
    assert_eq!(
        runtime
            .print_value(
                &context
                    .eval("({a: [1, 'b'], m: new Map([[1, 2]])})")
                    .unwrap()
            )
            .unwrap(),
        "{ a: [ 1, \"b\" ], m: Map(1) { 1 => 2 } }"
    );
    assert!(Value::Float(f64::NAN).same_value(&Value::Float(f64::NAN)));
    assert!(!Value::Float(0.0).same_value(&Value::Float(-0.0)));
    assert!(Value::Int(1).strict_equal(&Value::Float(1.0)));
    let error = context
        .new_native_error(NativeErrorKind::Range, "bad")
        .unwrap();
    let Value::Object(error) = error else {
        panic!("an error")
    };
    assert!(runtime.is_error_object(&error).unwrap());
}

#[test]
fn jobs_promises_and_rejections() {
    let (runtime, mut context) = runtime();
    runtime.set_track_rejections(true).unwrap();
    let Value::Object(promise) = context.eval("var log = []; Promise.resolve(1).then(v => log.push(v)); Promise.reject(new Error('lost')); Promise.resolve(2)").unwrap() else {
        panic!("a promise");
    };
    assert!(runtime.is_job_pending().unwrap());
    let snapshot = runtime.promise_snapshot(&promise).unwrap().unwrap();
    assert_eq!(snapshot.state(), PromiseState::Fulfilled);
    assert_eq!(snapshot.result(), &Value::Int(2));
    assert_eq!(
        runtime.run_pending_job().unwrap(),
        PendingJobOutcome::Executed
    );
    runtime.run_jobs().unwrap();
    assert_eq!(runtime.run_pending_job().unwrap(), PendingJobOutcome::NoJob);
    assert_eq!(
        context.eval("log.join()").unwrap(),
        Value::String(JsString::from_static("1"))
    );
    let rejections = runtime.take_unhandled_rejections().unwrap();
    assert_eq!(rejections.len(), 1);
    assert!(
        runtime
            .print_value(&rejections[0].1)
            .unwrap()
            .starts_with("Error: lost")
    );
    // A handler attached later removes the rejection.
    context.eval("var p = Promise.reject(1)").unwrap();
    context.eval("p.catch(() => {})").unwrap();
    runtime.run_jobs().unwrap();
    assert!(runtime.take_unhandled_rejections().unwrap().is_empty());
    // A reaction that throws rejects its derived promise; the drain goes on.
    context
        .eval("Promise.resolve().then(() => { throw 0; })")
        .unwrap();
    runtime.run_jobs().unwrap();
    assert_eq!(runtime.take_unhandled_rejections().unwrap().len(), 1);
}

#[derive(Default)]
struct Recorder {
    output: Rc<RefCell<String>>,
    events: Rc<RefCell<Vec<RejectionEvent>>>,
}

impl Host for Recorder {
    fn write(&mut self, _: Stream, text: &[u8]) {
        self.output
            .borrow_mut()
            .push_str(&String::from_utf8_lossy(text));
    }

    fn now_ms(&mut self) -> f64 {
        86_400_000.0
    }

    fn random_seed(&mut self) -> u64 {
        1
    }

    fn promise_rejection(&mut self, event: RejectionEvent, _: CoreValue) {
        self.events.borrow_mut().push(event);
    }
}

#[test]
fn the_host_receives_output_time_and_rejections() {
    let host = Recorder::default();
    let output = Rc::clone(&host.output);
    let events = Rc::clone(&host.events);
    let runtime = Runtime::with_host(host, RuntimeOptions::default());
    let mut context = runtime.context();
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();
    context
        .eval("print('hi', {a: 1}); console.log(Date.now())")
        .unwrap();
    assert_eq!(*output.borrow(), "hi { a: 1 }\n86400000\n");
    context
        .eval("var q = Promise.reject(0); q.then(undefined, () => {})")
        .unwrap();
    assert_eq!(
        *events.borrow(),
        [RejectionEvent::Rejected, RejectionEvent::Handled]
    );
    // The seed comes from the host: the same sequence twice.
    let first = Runtime::with_host(Recorder::default(), RuntimeOptions::default())
        .context()
        .eval("Math.random()")
        .unwrap();
    let second = Runtime::with_host(Recorder::default(), RuntimeOptions::default())
        .context()
        .eval("Math.random()")
        .unwrap();
    assert_eq!(first, second);
}

struct Files;

impl ModuleLoader for Files {
    fn load(
        &mut self,
        scope: &mut Scope<'_>,
        name: &str,
        attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value> {
        let json = attributes.effective().is_some();
        match name {
            "dep.js" => Ok(ModuleSource::source("export const answer = 42;")),
            "meta.js" => Ok(ModuleSource::Source {
                text: b"export const url = import.meta.url;".to_vec(),
                import_meta: vec![(
                    "url".into(),
                    Value::String(JsString::from_static("file:///meta.js")),
                )],
            }),
            "data.json" if json => Ok(ModuleSource::Json(b"{\"n\": 1}".to_vec())),
            "data.json5" => Ok(ModuleSource::Json5(
                b"{n: +2, /* c */ 'm': [0x10,],}".to_vec(),
            )),
            "compiled.js" => scope
                .compile_module("compiled.js", b"export default 'via scope';", Vec::new())
                .map(ModuleSource::Compiled),
            "broken.js" => scope
                .compile_module("broken.js", b"export let = ;", Vec::new())
                .map(ModuleSource::Compiled),
            _ => Err(scope
                .new_native_error(
                    NativeErrorKind::Reference,
                    &format!("could not load module filename '{name}'"),
                )
                .unwrap()),
        }
    }
}

fn run_module(context: &mut Context, source: &str) -> Result<Value, String> {
    let runtime = context.runtime().clone();
    let module = context
        .compile_module_bytes_with_filename(source.as_bytes(), "main.js")
        .map_err(|_| thrown(context))?;
    let promise = match context.execute_module(&module) {
        Ok(Value::Object(promise)) => promise,
        Ok(other) => panic!("not a promise: {other:?}"),
        Err(_) => return Err(thrown(context)),
    };
    runtime.run_jobs().unwrap();
    let snapshot = runtime.promise_snapshot(&promise).unwrap().unwrap();
    match snapshot.state() {
        PromiseState::Fulfilled => Ok(snapshot.result().clone()),
        PromiseState::Rejected => Err(runtime.print_value(snapshot.result()).unwrap()),
        PromiseState::Pending => Err("pending".into()),
    }
}

#[test]
fn modules_load_through_the_loader_and_its_scope() {
    let (runtime, mut context) = runtime();
    runtime.set_module_loader(Files).unwrap();
    run_module(
        &mut context,
        "import { answer } from './dep.js'; import { url } from 'meta.js'; \
         import data from 'data.json' with { type: 'json' }; import five from 'data.json5'; \
         import compiled from 'compiled.js'; \
         globalThis.result = [answer, url, data.n, five.n, five.m[0], compiled].join();",
    )
    .unwrap();
    assert_eq!(
        context.eval("result").unwrap(),
        Value::String(JsString::from_static("42,file:///meta.js,1,2,16,via scope"))
    );
    // The loader's error is thrown as it made it.
    let error = run_module(&mut context, "import 'missing.js';").unwrap_err();
    assert!(
        error.starts_with("ReferenceError: could not load module filename 'missing.js'"),
        "{error}"
    );
    let error = run_module(&mut context, "import 'broken.js';").unwrap_err();
    assert!(error.starts_with("SyntaxError"), "{error}");
    // `import()` goes through the same loader.
    context
        .eval("import('./dep.js').then(m => globalThis.dynamic = m.answer)")
        .unwrap();
    runtime.run_jobs().unwrap();
    assert_eq!(context.eval("dynamic").unwrap(), Value::Int(42));
}

struct Reentrant(Runtime);

impl ModuleLoader for Reentrant {
    fn load(
        &mut self,
        scope: &mut Scope<'_>,
        _: &str,
        _: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value> {
        // Re-entering the API without the scope is an error, not a panic.
        assert_eq!(self.0.context().eval("1"), Err(RuntimeError::Reentered));
        // A value cloned and dropped inside the callback.
        let object = Value::Object(scope.new_object().unwrap());
        let copy = object.clone();
        drop(object);
        drop(copy);
        Ok(ModuleSource::source("export default 1;"))
    }
}

#[test]
fn reentry_from_a_callback_is_an_error_and_values_drop_later() {
    let (runtime, mut context) = runtime();
    runtime
        .set_module_loader(Reentrant(runtime.clone()))
        .unwrap();
    assert_eq!(
        run_module(&mut context, "import x from 'x.js'; globalThis.x = x;").unwrap(),
        Value::Undefined
    );
    assert_eq!(context.eval("x").unwrap(), Value::Int(1));
}

#[test]
fn a_heap_cap_surfaces_as_out_of_memory() {
    let runtime = Runtime::with_options(RuntimeOptions {
        heap: HeapLimits { max_bytes: 8 << 20 },
        ..RuntimeOptions::default()
    });
    let mut context = runtime.context();
    assert_eq!(
        context.eval("var keep = []; for (;;) keep.push({ a: keep.length });"),
        Err(RuntimeError::OutOfMemory)
    );
}

#[test]
fn memory_report_counts_a_known_graph() {
    let (runtime, mut context) = runtime();
    runtime.run_gc().unwrap();
    let before = runtime.memory_report().unwrap();
    context
        .eval("var graph = []; for (var i = 0; i < 1000; i++) graph.push({});")
        .unwrap();
    runtime.run_gc().unwrap();
    let after = runtime.memory_report().unwrap();
    assert_eq!(after.objects.live - before.objects.live, 1001);
    context.eval("graph = null").unwrap();
    runtime.run_gc().unwrap();
    let freed = runtime.memory_report().unwrap();
    assert_eq!(freed.objects.live, before.objects.live);
    assert!(freed.reserved_bytes() > 0);
    assert_eq!(freed.cap, usize::MAX);
    assert!(freed.code.units > 0);
    let text = freed.to_string();
    assert!(text.contains("objects"), "{text}");
}

#[test]
fn execution_turns_and_the_stats_entries() {
    let (runtime, mut context) = runtime();
    context.eval("Promise.resolve().then(() => {})").unwrap();
    let inside = runtime.with_execution_turn(|| runtime.run_pending_job());
    assert!(matches!(inside, Err(RuntimeError::Invariant(_))));
    assert_eq!(
        runtime.run_pending_job().unwrap(),
        PendingJobOutcome::Executed
    );
    assert!(runtime.gc_report().unwrap().starts_with("gc: "));
    let disassembly = runtime
        .disassemble(b"function f(a) { return a + 1; }", "d.js")
        .unwrap();
    assert!(disassembly.contains("AddI"), "{disassembly}");
    assert_eq!(
        runtime.run_gc().unwrap().collections,
        runtime.gc_stats().unwrap().collections
    );
    // A manual policy collects only when asked.
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    assert_eq!(runtime.gc_policy().unwrap(), GcPolicy::Manual);
    let before = runtime.gc_stats().unwrap().collections;
    context
        .eval("for (var i = 0; i < 200000; i++) ({ i });")
        .unwrap();
    assert_eq!(runtime.gc_stats().unwrap().collections, before);
    runtime.set_gc_policy(GcPolicy::Automatic).unwrap();
    context
        .eval("for (var i = 0; i < 200000; i++) ({ i });")
        .unwrap();
    assert!(runtime.gc_stats().unwrap().collections > before);
}
