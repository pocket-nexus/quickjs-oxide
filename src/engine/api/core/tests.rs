//! The corpus (`core/tests/corpus`) through `Context::eval` prints what the
//! lowering's driver prints.

use std::cell::RefCell;
use std::rc::Rc;

use super::{EvalOptions, QjsHelpers, Runtime, RuntimeError, RuntimeOptions, Value};
use crate::engine::compiler::CompileOptions;
use crate::engine::compiler::register::run_tests::{corpus, run_on_core};

#[derive(Default)]
struct Capture(Rc<RefCell<String>>);

impl oxide_core::host::Host for Capture {
    fn write(&mut self, _: oxide_core::host::Stream, text: &[u8]) {
        self.0.borrow_mut().push_str(&String::from_utf8_lossy(text));
    }
}

/// `source` through the API: `print` captured, the jobs drained, an
/// uncaught exception reported as `uncaught: <ToString>`.
fn run_api(source: &str) -> String {
    let output = Rc::default();
    let runtime = Runtime::with_host(Capture(Rc::clone(&output)), RuntimeOptions::default());
    let context = runtime.context();
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();
    let mut result = context.eval(source).map(drop);
    if result.is_ok() {
        result = runtime.run_jobs();
    }
    let mut text = output.borrow().clone();
    if let Err(error) = result {
        let RuntimeError::Exception(exception) = error else {
            panic!("not an exception: {error}");
        };
        let shown = context
            .to_js_string(&exception)
            .map_or_else(|_| "<unprintable>".to_owned(), |text| text.to_utf8_lossy());
        text.push_str(&format!("uncaught: {shown}\n"));
    }
    text
}

#[test]
fn the_corpus_prints_through_the_api_what_the_driver_prints() {
    let programs = corpus();
    assert_eq!(programs.len(), 49);
    let mut failures = Vec::new();
    for (name, source) in programs {
        let expected = run_on_core(&source);
        let actual = run_api(&source);
        if expected != actual {
            failures.push(format!(
                "==== {name}\n-- driver\n{expected}-- API\n{actual}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn transient_code_is_freed_once_nothing_names_it() {
    let runtime = Runtime::new();
    let context = runtime.context();
    runtime.run_gc().unwrap();
    let before = runtime.memory_report().unwrap().code;
    let transient = EvalOptions::default().transient();
    for i in 0..20 {
        let source = format!("(function () {{ return {{ k{i}: {i} }}; }})().k{i}");
        assert_eq!(
            context.eval_with_options(&source, &transient).unwrap(),
            Value::Int(i)
        );
    }
    let kept = context
        .compile_with_options(
            "(function () { return 'kept'; })()",
            &CompileOptions::default().transient(),
        )
        .unwrap();
    runtime.run_gc().unwrap();
    let after = runtime.memory_report().unwrap().code;
    assert_eq!(after.static_units, before.static_units);
    // Only the compiled script the embedder holds is left: its two units.
    assert_eq!(after.dynamic_units, before.dynamic_units + 2);
    let result = context.execute(&kept).unwrap();
    assert_eq!(
        context.to_js_string(&result).unwrap().to_utf8_lossy(),
        "kept"
    );
    drop(kept);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.memory_report().unwrap().code.dynamic_units,
        before.dynamic_units
    );
    // Code compiled without the option lives as long as the runtime.
    context.eval("(function () { return 1; })()").unwrap();
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.memory_report().unwrap().code.static_units,
        before.static_units + 2
    );
}

#[test]
fn call_entry_keeps_static_and_dynamic_units_through_rewrites_and_construction() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let context = runtime.context();
    context
        .eval(
            "function staticStep(next, n) { return n ? next(staticStep, n - 1) + 1 : 1; }
             function StaticCtor(x) { this.x = x; this.target = new.target; }
             function sloppy() { return typeof this + ':' + this.valueOf(); }
             function strict() { 'use strict'; return this; }",
        )
        .unwrap();
    let transient = EvalOptions::default().transient();
    let result = context
        .eval_with_options(
            "(function () {
                function dynamicStep(next, n) { return n ? next(dynamicStep, n - 1) + 1 : 1; }
                function DynamicCtor(x) { this.x = x; this.target = new.target; }
                function collect(a, ...rest) { return a + rest.length + arguments.length; }
                class Derived extends StaticCtor {}
                var a = new StaticCtor(11);
                var b = new DynamicCtor(...[13]);
                var c = new Derived(...[17]);
                return staticStep(dynamicStep, 20) === 21
                    && dynamicStep(staticStep, 21) === 22
                    && sloppy.call(7) === 'object:7'
                    && (function temporary() { return temporary; }).call(7).name === 'temporary'
                    && strict.apply(null, []) === null
                    && Function.prototype.call.call(collect, null, 3, 4, 5) === 8
                    && collect(...[5, 6, 7, 8]) === 12
                    && a.x === 11 && a.target === StaticCtor
                    && b.x === 13 && b.target === DynamicCtor
                    && c.x === 17 && c.target === Derived;
             })()",
            &transient,
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn failed_call_entry_keeps_the_callers_handlers_and_code() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let context = runtime.context();
    context
        .eval("var notConstructor = () => 1; function staticThrow() { throw 23; }")
        .unwrap();
    let result = context
        .eval_with_options(
            "(function () {
                var caught = 0;
                try { staticThrow(); } catch (e) { caught += e; }
                try { new notConstructor(); } catch (e) { caught += e instanceof TypeError; }
                try { new notConstructor(...[]); } catch (e) { caught += e instanceof TypeError; }
                try { (0)(...[]); } catch (e) { caught += e instanceof TypeError; }
                return caught;
             })()",
            &EvalOptions::default().transient(),
        )
        .unwrap();
    assert_eq!(result, Value::Int(26));
}

#[test]
fn call_returns_restore_callers_after_native_calls_and_suspension() {
    assert_eq!(
        run_api(
            "function tailNative(n) { return Math.abs(n); }
             function tailConstruct(n) { return new Number(n); }
             function actual(a, b) { return [a, b, arguments.length].join(':'); }
             function* values(n) { yield n; return n + 1; }
             async function ready(n) { return n + 2; }
             function caller() {
                 var keep = { value: 19 };
                 var a = tailNative(-7);
                 var b = tailConstruct(11);
                 var c = actual(3, 5, 8, 13);
                 var it = values(17);
                 var d = it.next().value + it.next().value;
                 ready(23).then(function (e) { print(keep.value, a, b.valueOf(), c, d, e); });
             }
             caller();"
        ),
        "19 7 11 3:5:4 35 25\n"
    );
}

#[test]
fn element_hits_and_reentrant_misses_preserve_the_register_window() {
    assert_eq!(
        run_api(
            "function grow(n) { return n ? grow(n - 1) + 1 : 1; }
             var bump = (function () { var n = 0; return function () { return ++n; }; })();
             var written = 0;
             Object.defineProperty(Array.prototype, '1', {
                 configurable: true,
                 get: function () { bump(); return grow(600); },
                 set: function (v) { bump(); written = v + grow(400); }
             });
             var a = [2, , 4];
             a[1] = 5;
             var total = a[0] + a[1] + a[2];
             delete Array.prototype[1];
             var t = new Int16Array(2);
             t[0] = 7;
             t[1] = { valueOf: function () { return grow(700); } };
             function mapped(x) { arguments[0] = 3; return x; }
             print(total, written, t[0], t[1], mapped(9), bump());"
        ),
        "607 406 7 701 3 3\n"
    );
}

#[test]
fn short_calls_publish_at_the_first_unexecuted_operation() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime
        .context()
        .eval(
            "function grow(n) { return n ? grow(n - 1) + 1 : 1; }
         function read(o) { return o.x; }
         function step(o) { o.n++; return o.x; }
         function store(o, x) { o.n++; o.x = x; }
         function add(o, x) { o.n++; return x + 1; }
         function callback(o, f) { o.n++; return f(); }
         var o = { n: 0, x: 3 }, ok = true;
         for (var i = 0; i < 20; ++i) {
             ok = ok && read(o) === i + 3 && step(o) === i + 3;
             store(o, i + 4);
             ok = ok && add(o, i) === i + 1;
         }
         var seen = 0;
         Object.defineProperty(o, 'x', {
             configurable: true,
             get: function () { ++seen; return grow(400); },
             set: function (v) { seen += v + grow(300); }
         });
         ok = ok && step(o) === 401 && seen === 1;
         store(o, 7);
         ok = ok && seen === 309;
         ok = ok && add(o, { valueOf: function () { return grow(200); } }) === 202;
         ok = ok && callback(o, function () { return grow(100); }) === 101;
         Object.defineProperty(o, 'x', { get: function () { throw 17; } });
         try { step(o); } catch (e) { ok = ok && e === 17; }
         ok && o.n === 65;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn short_calls_keep_numeric_identity_and_receiver_semantics() {
    assert_eq!(
        run_api(
            "function id(x) { return x; }
             function sum(x, y) { return x + y; }
             function sub(x, y) { return x - y; }
             function lt(x, y) { return x < y; }
             function eq(x, y) { return x === y; }
             function choose(x, y) { return x ? x : y; }
             function receiver() { 'use strict'; return this; }
             function self() { return self; }
             var a = {}, b = {};
             print(1 / id(-0), sum(2147483647, 1), sub(-2147483648, 1),
                 sum('a', 2), lt(NaN, 1), eq(a, a), eq(a, b),
                 eq('ab', ['a', 'b'].join('')), eq(1n, BigInt('1')),
                 choose('', 7), choose(NaN, 9), receiver.call(null), self() === self);"
        ),
        "-Infinity 2147483648 -2147483649 a2 false true false true true 7 9 null true\n"
    );
}

#[test]
fn calls_follow_replaced_collected_and_dynamic_functions() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let context = runtime.context();
    let result = context
        .eval(
            "function apply(f, x) { return f(x); }
         function method(o, x) { return o.f(x); }
         function make(n) { return function (x) { return n + x; }; }
         var o = { f: make(0) }, ok = true;
         for (var n = 0; n < 60; ++n) {
             var f = make(n); o.f = f;
             for (var i = 0; i < 4; ++i) {
                 ok = ok && apply(f, i) === n + i && method(o, i) === n + i;
             }
             o.f = Math.abs;
             ok = ok && apply(Math.abs, -n) === n && method(o, -n) === n;
             o.f = Function('x', 'return x + 2');
             ok = ok && method(o, n) === n + 2;
             try { apply({}, n); ok = false; } catch (e) { ok = ok && e instanceof TypeError; }
         }
         ok;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn planned_reads_and_writes_miss_before_observable_callbacks() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime.context().eval(
        "var Mode = { none: 0 };
         function active(o) { return o.x !== Mode.none; }
         function clear(o) { o.x = Mode.none; }
         function mark(o) { o.x = false; }
         function set(o, v) { o.x = v; }
         function at(o, i) { return o.a[i]; }
         var o = { x: 1, a: [3, 5] }, ok = true, seen = 0;
         for (var i = 0; i < 20; ++i) {
             set(o, 1); ok = ok && active(o) && at(o, 1) === 5;
             clear(o); ok = ok && !active(o);
             mark(o); ok = ok && o.x === false;
         }
         Mode.none = 2; clear(o); ok = ok && o.x === 2;
         Object.defineProperty(o, 'x', { set: function (v) { ++seen; }, configurable: true });
         set(o, 7); clear(o); mark(o);
         ok = ok && seen === 3;
         Object.defineProperty(Mode, 'none', { get: function () { ++seen; return 9; } });
         clear(o); ok = ok && seen === 5;
         Object.defineProperty(Array.prototype, '1', { get: function () { ++seen; return 11; }, configurable: true });
         delete o.a[1]; ok = ok && at(o, 1) === 11 && seen === 6;
         delete Array.prototype[1]; o.a.length = 1;
         ok && at(o, 1) === undefined;"
    ).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn planned_field_choices_read_only_the_selected_branch() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime
        .context()
        .eval(
            "var Mode = { value: 1 }, o = { k: 0, a: 3, b: 5 }, seen = 0, ok = true;
         function pick(o) { return o.k == Mode.value ? o.a : o.b; }
         for (var i = 0; i < 20; ++i) {
             o.k = 0; ok = ok && pick(o) === 5;
             o.k = 1; ok = ok && pick(o) === 3;
         }
         Object.defineProperty(o, 'a', { get: function () { ++seen; return 17; } });
         o.k = 0; ok = ok && pick(o) === 5 && seen === 0;
         o.k = 1; ok = ok && pick(o) === 17 && seen === 1;
         Mode.value = { valueOf: function () { ++seen; return 1; } };
         ok = ok && pick(o) === 17 && seen === 3;
         Object.defineProperty(Mode, 'value', { get: function () { ++seen; return 0; } });
         ok && pick(o) === 5 && seen === 4;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn conditional_backedges_poll_numeric_and_truthiness_loops() {
    struct Stop;
    impl oxide_core::host::Host for Stop {
        fn interrupt(&mut self) -> bool {
            true
        }
    }
    for source in [
        "function f(n) { for(var i=0;i<n;i++) {} return i; } f(3000)",
        "function f(n,s) { for(var i=0;i<n;i+=s) {} return i; } f(3000,1)",
        "function f(n) { for(var i=0;i<n;i+=1) {} return i; } f(3000)",
        "function f(n) { var x; for(var i=0;i<n;) { i++; x=i*i; } return x; } f(3000)",
        "function f(n) { while(n) --n; return n; } f(3000)",
        "function f(n) { var i=0,done=false; while(!done) { i++; done=i>=n; } return i; } f(3000)",
    ] {
        let runtime = Runtime::with_host(Stop, RuntimeOptions::default());
        assert_eq!(
            runtime.context().eval(source),
            Err(RuntimeError::Interrupted),
            "{source}"
        );
    }
}

#[test]
fn planned_bit_tests_keep_short_circuit_reads_and_coercions() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime
        .context()
        .eval(
            "globalThis.BITS_MASK=4; globalThis.BITS_TARGET=2;
         var reads=0, conversions=0;
         function ready() { return (this.bits & BITS_MASK)!==0 || this.other===BITS_TARGET; }
         var o={bits:0,other:2,ready:ready},ok=true;
         for(var i=0;i<30;i++) { o.bits=i&7; ok=ok&&o.ready(); }
         Object.defineProperty(o,'other',{get:function(){ ++reads; return 2; }});
         Object.defineProperty(globalThis,'BITS_TARGET',{get:function(){ ++reads; return 2; }});
         o.bits=4; ok=ok&&o.ready()&&reads===0;
         o.bits=0; ok=ok&&o.ready()&&reads===2;
         BITS_MASK={valueOf:function(){ ++conversions; return 4; }};
         o.bits=4; ok=ok&&o.ready()&&conversions===1&&reads===2;
         o.bits=0; ok&&o.ready()&&conversions===2&&reads===4;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn planned_bit_updates_fall_back_before_any_observable_write() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime.context().eval(
        "globalThis.BITS_MASK=4;
         var reads=0,writes=0,conversions=0,stored=0;
         function update() { 'use strict'; this.bits=this.bits|BITS_MASK; }
         function grow(n) { return n?grow(n-1)+1:1; }
         var o={bits:0,update:update},ok=true;
         for(var i=0;i<30;i++) { o.bits=i; o.update(); ok=ok&&o.bits===(i|4); }
         Object.defineProperty(o,'bits',{get:function(){ ++reads; return stored; },set:function(v){ ++writes; stored=v; },configurable:true});
         o.update(); ok=ok&&reads===1&&writes===1&&stored===4;
         BITS_MASK={valueOf:function(){ ++conversions; grow(400); return 8; }};
         o.update(); ok=ok&&reads===2&&writes===2&&conversions===1&&stored===12;
         Object.defineProperty(o,'bits',{value:1,writable:false});
         var threw=false;
         try { o.update(); } catch(e) { threw=e instanceof TypeError; }
         ok&&threw&&conversions===2&&writes===2&&o.bits===1;"
    ).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn planned_bit_updates_preserve_unsigned_results_and_shift_counts() {
    let runtime = Runtime::new();
    let result = runtime
        .context()
        .eval(
            "globalThis.BITS_SHIFT=1;
         function shl() { this.bits=this.bits<<BITS_SHIFT; }
         function sar() { this.bits=this.bits>>BITS_SHIFT; }
         function shr() { this.bits=this.bits>>>BITS_SHIFT; }
         function xor() { this.bits=this.bits^BITS_SHIFT; }
         var o={bits:0,shl:shl,sar:sar,shr:shr,xor:xor},ok=true;
         for(var i=0;i<30;i++) {
             o.bits=-1; o.shr(); ok=ok&&o.bits===2147483647;
             o.bits=-2; o.sar(); ok=ok&&o.bits===-1;
             o.bits=1073741824; o.shl(); ok=ok&&o.bits===-2147483648;
             o.bits=7; o.xor(); ok=ok&&o.bits===6;
         }
         BITS_SHIFT=0; o.bits=-1; o.shr(); ok=ok&&o.bits===4294967295;
         BITS_SHIFT=33; o.shr(); ok=ok&&o.bits===2147483647;
         BITS_SHIFT=-1; o.bits=1; o.shl(); ok&&o.bits===-2147483648;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn arithmetic_comparisons_complete_each_conversion_once() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime
        .context()
        .eval(
            "function grow(n) { return n ? grow(n-1)+1 : 1; }
         function inc(n) { for(var i=0;i<n;i++) {} return i; }
         function add(n,s) { for(var i=0;i<n;i+=s) {} return i; }
         function imm(n) { for(var i=2147483646;i<n;i+=1) {} return i; }
         function text() { for(var i='';i<3;i+=1) {} return i; }
         function big() { for(var i=0n;i<3n;i++) {} return i; }
         var reads=0;
         var bound={valueOf:function(){ ++reads; grow(600); return 3; }};
         var step={valueOf:function(){ ++reads; grow(400); return 1; }};
         inc(bound)===3 && reads===4 && add(3,step)===3 && reads===7
             && imm(2147483650)===2147483650 && text()==='11' && big()===3n;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn read_only_call_plans_fall_back_on_accessor_invalidation_and_coercion() {
    let runtime = Runtime::new();
    if Runtime::GC_STRESS_AVAILABLE {
        runtime.set_gc_stress(Some(1)).unwrap();
    }
    let result = runtime
        .context()
        .eval(
            "function read(o) { return o.x; }
         function chain(o) { return o.x.y; }
         function less(a, b) { return a.x < b.x; }
         var proto = { x: 3 }, a = Object.create(proto), b = { x: 7 }, c = { x: { y: 11 } };
         var ok = true, reads = 0, conversions = 0;
         for (var i = 0; i < 20; ++i) { ok = ok && read(a) === 3 && chain(c) === 11 && less(a, b); }
         Object.defineProperty(proto, 'x', { get: function () { ++reads; return 5; } });
         ok = ok && read(a) === 5 && less(a, b) && reads === 2;
         b.x = { valueOf: function () { ++conversions; return 9; } };
         ok = ok && less(a, b) && reads === 3 && conversions === 1;
         Object.defineProperty(c.x, 'y', { get: function () { ++reads; throw 23; } });
         try { chain(c); ok = false; } catch (e) { ok = ok && e === 23; }
         ok && reads === 4;",
        )
        .unwrap();
    assert_eq!(result, Value::Bool(true));
}

/// Defines a core host function as the global `name`, as the API's host
/// functions will be.
fn define_core_host(
    runtime: &Runtime,
    name: &str,
    constructor: bool,
    body: impl Fn(
        &mut oxide_core::NativeCx<'_>,
        oxide_core::vm::native::NativeCall,
    ) -> Result<oxide_core::Value, oxide_core::Throw>
    + 'static,
) {
    let mut inner = runtime.shared.inner.borrow_mut();
    let core = &mut inner.core;
    let body: Rc<oxide_core::vm::native::HostFn> = Rc::new(body);
    let function = core
        .native_cx()
        .new_host_function(name, 0, constructor, &[], body)
        .unwrap();
    let handle = core.hold(oxide_core::Value::object(function));
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = core.key_from_units(&units).unwrap();
    let global = core.global_object();
    let value = oxide_core::Value::object(function);
    core.set(global, key, value, oxide_core::Value::object(global))
        .unwrap();
    core.release(handle);
}

#[test]
fn a_script_calls_a_host_function_and_catches_its_throw() {
    let runtime = Runtime::new();
    let context = runtime.context();
    define_core_host(&runtime, "twice", false, |cx, call| {
        let n = cx.to_number(call.arg(cx, 0))?;
        Ok(oxide_core::Value::number(n * 2.0))
    });
    define_core_host(&runtime, "fail", false, |cx, _| {
        let error = cx
            .new_error(oxide_core::ErrorKind::Type, "x is required")
            .unwrap();
        Err(oxide_core::Throw(error))
    });
    assert_eq!(context.eval("twice(21) === 42").unwrap(), Value::Bool(true));
    assert_eq!(
        context
            .eval("try { fail(); } catch (e) { e instanceof TypeError && e.message }")
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static("x is required"))
    );
    assert_eq!(
        context
            .eval("typeof twice + twice.name + twice.length")
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static(
            "functiontwice0"
        ))
    );
    assert!(matches!(
        context.eval("new twice()"),
        Err(RuntimeError::Exception(_))
    ));
}

#[test]
fn a_host_function_is_listed_in_a_backtrace() {
    let runtime = Runtime::new();
    let context = runtime.context();
    define_core_host(&runtime, "h", false, |cx, call| {
        cx.call(call.arg(cx, 0), oxide_core::Value::UNDEFINED, &[])
    });
    let stack = context
        .eval("try { h(function inner() { throw new Error('x'); }); } catch (e) { e.stack }")
        .unwrap();
    let Value::String(stack) = stack else {
        panic!("no stack");
    };
    let stack = stack.to_utf8_lossy();
    let lines: Vec<&str> = stack.lines().collect();
    assert!(lines[0].contains("at inner (<input>"), "{stack}");
    assert_eq!(lines[1].trim(), "at h (native)", "{stack}");
    assert!(lines[2].contains("at <eval> (<input>"), "{stack}");
}

#[test]
fn a_host_object_prints_as_an_object() {
    let runtime = Runtime::new();
    let context = runtime.context();
    define_core_host(&runtime, "make", false, |cx, _| {
        let prototype = cx.intrinsic(oxide_core::Intrinsic::ObjectPrototype);
        let object = cx.new_host_object(Some(prototype), Rc::new(7_u8)).unwrap();
        Ok(oxide_core::Value::object(object))
    });
    assert_eq!(
        context
            .eval("var o = make(); o.x = 1; typeof o + Object.prototype.toString.call(o) + o.x")
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static(
            "object[object Object]1"
        ))
    );
    let o = context.eval("o").unwrap();
    // The printer names the class `Object`, as it names every internal
    // record without a class of its own.
    assert_eq!(runtime.print_value(&o).unwrap(), "Object { x: 1 }");
}

#[test]
fn handles_a_host_function_drops_are_released_after_each_call() {
    use super::Scope;
    let runtime = Runtime::new();
    let context = runtime.context();
    let highest = Rc::new(std::cell::Cell::new(0usize));
    let seen = Rc::clone(&highest);
    let global = context.global_object().unwrap();
    context
        .define_function(&global, "touch", 1, move |scope: &mut Scope<'_>| {
            let argument = scope.arg(0);
            let live = scope.core.heap().handles_ref().live_count();
            seen.set(seen.get().max(live));
            drop(argument);
            Ok(Value::Undefined)
        })
        .unwrap();
    drop(global);
    context
        .eval("var o = {}; for (var i = 0; i < 10000000; i++) touch(o);")
        .unwrap();
    assert!(highest.get() < 16, "{} live handles", highest.get());
}
