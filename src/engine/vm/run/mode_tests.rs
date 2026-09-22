//! Full compiler/driver comparisons, with no synthetic executable snapshots.
use crate::engine::api::{Context, Runtime, Value};

// The first four are the independent canonical/cache × canonical/quick matrix.
const MODES: [u8; 7] = [0, 1, 4, 5, 2, 8, 15];

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    result: String,
    effects: String,
    jobs: usize,
}

fn string(value: Value) -> String {
    let Value::String(value) = value else {
        panic!("fixture must return a stable string, got {value:?}");
    };
    value.to_string()
}

fn observe(mode: u8, source: &str) -> Observation {
    observe_with(mode, source, |_, _| {})
}

fn observe_with(mode: u8, source: &str, setup: impl FnOnce(&Runtime, &mut Context)) -> Observation {
    let runtime = Runtime::new();
    assert_eq!(runtime.0.execution_mode_override.get(), None);
    runtime.0.execution_mode_override.set(Some(mode));
    let weak = std::rc::Rc::downgrade(&runtime.0);
    let mut context = runtime.new_context();
    setup(&runtime, &mut context);
    drop(
        context
            .eval("var modeLog=[];var modeAsyncResult='unset';")
            .unwrap(),
    );
    let result = string(
        context
            .eval_with_filename(source, "mode-diff.js")
            .unwrap_or_else(|error| panic!("mode {mode}: {error:?}; source {source}")),
    );
    let mut jobs = 0;
    while runtime.is_job_pending() {
        // Exercise detached/resumed owning state between each queued callback.
        runtime.run_gc().unwrap();
        runtime.execute_pending_job().unwrap();
        jobs += 1;
        assert!(jobs < 1000, "mode {mode}: unexpected pending-job loop");
    }
    runtime.run_gc().unwrap();
    let effects = string(
        context
            .eval("JSON.stringify([modeLog,modeAsyncResult])")
            .unwrap(),
    );
    assert!(
        runtime.0.state.borrow().active_frames.is_empty(),
        "mode {mode}"
    );
    assert!(!context.has_exception(), "mode {mode}");
    assert_eq!(runtime.0.execution_mode_override.get(), Some(mode));
    drop(context);
    runtime.run_gc().unwrap();
    drop(runtime);
    assert!(
        weak.upgrade().is_none(),
        "mode {mode}: driver retained Runtime"
    );
    Observation {
        result,
        effects,
        jobs,
    }
}

#[test]
fn execution_modes_full_driver_match_results_effects_jobs_and_error_pcs() {
    let cases = [
        (
            "hot_generic_hot",
            r#"(function(){
            function f(a){var n=a+1;var o={get x(){modeLog.push('get');return n}};
                n=o.x;n=n*2;n=n-1;return n}
            return JSON.stringify([f(20),f(2)]);
        })()"#,
        ),
        (
            "method_literal_arguments",
            r#"(function(){
            var o={base:39,m(a,b){modeLog.push([this.base,a,b]);return this.base+a+b}};
            var n=0;for(var i=0;i<4;i++)n+=o.m(1,2);
            return JSON.stringify(n);
        })()"#,
        ),
        (
            "branches_and_number_edges",
            r#"(function(){
            var n=0;for(var i=0;i<12;i++){if(i&1)n=n+i;else n=n-i;}
            return JSON.stringify([n,Object.is(-0,-1*0),Number.isNaN(0/0),2147483647+1,
                (4294967295>>>1),NaN<0,NaN===NaN]);
        })()"#,
        ),
        (
            "coercion_once",
            r#"(function(){
            var a={valueOf(){modeLog.push('left');return 8}},
                b={valueOf(){modeLog.push('right');return 2}};
            var n=a-b;n=n*3;
            var p=new Proxy({x:4},{get(t,k){modeLog.push(k);return t[k]}});
            n+=p.x;return JSON.stringify(n);
        })()"#,
        ),
        (
            "local_argument_captured_mapped",
            r#"(function(){
            function direct(a){'use strict';var b=1;b=a+2;a=b*2;return a+b}
            function mapped(a){arguments[0]=4;a=a+1;modeLog.push(arguments[0]);
                var b=10;function read(){return b}b=b+read();return a+b}
            return JSON.stringify([direct(3),mapped(1)]);
        })()"#,
        ),
        (
            "tdz_const_declines",
            r#"(function(){
            var n=2;try{let x=x;}catch(e){modeLog.push(e.name)}
            try{const c=1;c=2;}catch(e){modeLog.push(e.name)}
            let x=4;function write(){x=7}write();n=n+x;return JSON.stringify(n);
        })()"#,
        ),
        (
            "owning_numeric_stores",
            r#"(function(){
            function big(a){var n=a*3n;n=n-7n;a=n*5n;return a+n}
            var s='a long heap backed string';s=s+' suffix';s=s+'!';
            var n=big(1361129467683753853853498429727072845824n);
            modeLog.push(typeof n,s.length);return JSON.stringify([s,n.toString()]);
        })()"#,
        ),
        (
            "try_finally_prefix",
            r#"(function(){
            var n=1;try{n=n+2;throw new Error('ordered')}
            catch(e){modeLog.push(e.message);n=n*3}
            finally{modeLog.push(n);n=n+1}
            return JSON.stringify(n);
        })()"#,
        ),
        (
            "generator_next_throw_return",
            r#"(function(){
            function* g(){var n=40;try{n+=yield 1;yield n}
                catch(e){modeLog.push(e);yield 7}finally{modeLog.push('finally')}return 9}
            var a=g(),b=g();return JSON.stringify([a.next(),a.throw(6),a.return(8),
                b.next(),b.next(2),b.return(5)]);
        })()"#,
        ),
        (
            "async_pending_jobs",
            r#"(function(){
            modeAsyncResult='pending';
            (async function(){var n=40;n+=await Promise.resolve(2);
                try{await Promise.reject('reject')}catch(e){modeLog.push(e)}
                finally{modeLog.push('async-finally')}return n})()
                .then(n=>{modeAsyncResult=n;modeLog.push('done')});
            return 'scheduled';
        })()"#,
        ),
        (
            "native_callback_reentry",
            r#"(function(){
            var a=[1,2,3].map(function(n){modeLog.push(n);return n*2});
            var n=Math.min({valueOf(){modeLog.push('native');return 42}},50);
            return JSON.stringify([a,n]);
        })()"#,
        ),
        // The throw is deliberately on line 4 of the supplied source.
        (
            "error_line",
            "(function modeErrorSite(){\nvar n=1;\ntry{\nthrow new Error('mode-fault');\n}catch(e){modeLog.push(e.name,e.message);return e.stack}\n})()",
        ),
    ];
    for (name, source) in cases {
        let canonical = observe(0, source);
        if name == "error_line" {
            assert!(canonical.result.contains("modeErrorSite"), "{canonical:?}");
            assert!(canonical.result.contains("mode-diff.js:4"), "{canonical:?}");
        }
        if name == "coercion_once" {
            assert_eq!(canonical.effects, r#"[["left","right","x"],"unset"]"#);
        }
        if name == "async_pending_jobs" {
            assert!(canonical.jobs > 0);
            assert_eq!(
                canonical.effects,
                r#"[["reject","async-finally","done"],42]"#
            );
        }
        for mode in MODES.into_iter().skip(1) {
            assert_eq!(observe(mode, source), canonical, "case {name}, mode {mode}");
        }
    }
}

#[test]
fn execution_mode_override_is_runtime_local_and_shared_only_by_clones() {
    let first = Runtime::new();
    let cloned = first.clone();
    let second = Runtime::new();
    first.0.execution_mode_override.set(Some(15));
    assert_eq!(cloned.0.execution_mode_override.get(), Some(15));
    assert_eq!(second.0.execution_mode_override.get(), None);
    second.0.execution_mode_override.set(Some(0));
    cloned.0.execution_mode_override.set(None);
    assert_eq!(first.0.execution_mode_override.get(), None);
    assert_eq!(second.0.execution_mode_override.get(), Some(0));
}

#[cfg(feature = "profiling")]
#[test]
fn execution_modes_full_driver_reach_the_selected_experimental_paths() {
    use crate::engine::api::profiling::CostProfile;
    for mode in MODES {
        let runtime = Runtime::new();
        runtime.0.execution_mode_override.set(Some(mode));
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"(function(a){'use strict';
            var n=0;
            for(var i=0;i<3;i++){
                n=a*2;a=n+1;
                var owned=1361129467683753853853498429727072845824n*(BigInt(i)+2n);
                var text='heap numeric output '+i;
            }
            return n===38&&a===39&&typeof owned==='bigint'&&text==='heap numeric output 2';
        })(4)"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        let costs = profile.snapshot();
        let event = |name| costs.owned_execution_events.get(name).copied().unwrap_or(0);
        if mode & 1 != 0 {
            assert!(
                event("tos.hit") > 0,
                "mode {mode}: {:?}",
                costs.owned_execution_events
            );
        }
        assert_eq!(
            event("tos.owned_numeric_output") > 0,
            mode & 2 != 0,
            "mode {mode}: {:?}",
            costs.owned_execution_events
        );
        assert_eq!(
            event("quick.dispatch.hot") > 0,
            mode & 4 != 0,
            "mode {mode}: {:?}",
            costs.owned_execution_events
        );
        for name in ["fusion.StoreDropLocal", "fusion.StoreDropArgument"] {
            assert_eq!(
                event(name) > 0,
                mode & 8 != 0,
                "mode {mode}: {:?}",
                costs.owned_execution_events
            );
        }
    }
}

#[cfg(feature = "profiling")]
#[test]
fn execution_modes_four_way_preserve_fallback_fusion_and_logical_counts() {
    use crate::engine::api::profiling::CostProfile;
    let cases = [
        (
            "hot_generic_hot",
            "",
            "(function(a){var n=a+1;var o={get x(){return n}};n=o.x;return n*2})(20)",
            Value::Int(42),
        ),
        (
            "numeric_guard_decline",
            "",
            "(function(){var log='';var a={valueOf(){log+='l';return 8}},b={valueOf(){log+='r';return 2}};var n=a-b;n=n*7;return n===42&&log==='lr'})()",
            Value::Bool(true),
        ),
        (
            "method_literal_span",
            // Warm the same site independently in each fresh Runtime. Both
            // PushI32 argument words must remain skipped by the Generic span.
            "var holder={min:Math.min};function method(n){var r;for(var i=0;i<n;i++)r=holder.min(42,43);return r}method(2)",
            "method(20)",
            Value::Int(42),
        ),
    ];
    for (name, setup, source, expected) in cases {
        let mut baseline = None;
        for mode in [0, 1, 4, 5] {
            let runtime = Runtime::new();
            runtime.0.execution_mode_override.set(Some(mode));
            let mut context = runtime.new_context();
            if !setup.is_empty() {
                drop(context.eval(setup).unwrap());
            }
            let profile = CostProfile::start();
            assert_eq!(
                context.eval(source).unwrap(),
                expected,
                "{name}, mode {mode}"
            );
            let costs = profile.snapshot();
            let event = |key| costs.owned_execution_events.get(key).copied().unwrap_or(0);
            let logical = (costs.owned_instructions, costs.owned_max_operand_depth);
            assert!(logical.0 > 0, "{name}, mode {mode}");
            if let Some(canonical) = baseline {
                assert_eq!(
                    logical, canonical,
                    "{name}, mode {mode}: logical accounting"
                );
            } else {
                baseline = Some(logical);
            }
            for key in [
                "quick.dispatch.hot",
                "quick.dispatch.generic",
                "quick.canonical_fetch",
            ] {
                assert_eq!(
                    event(key) > 0,
                    mode & 4 != 0,
                    "{name}, mode {mode}: {key}: {:?}",
                    costs.owned_execution_events
                );
            }
            if name == "numeric_guard_decline" {
                for key in ["quick.dispatch.declined", "quick.dispatch.numeric"] {
                    assert_eq!(
                        event(key) > 0,
                        mode & 4 != 0,
                        "{name}, mode {mode}: {key}: {:?}",
                        costs.owned_execution_events
                    );
                }
            }
            assert_eq!(
                event("quick.canonical_fetch"),
                event("quick.dispatch.generic") + event("quick.dispatch.declined"),
                "{name}, mode {mode}: canonical fetch classification"
            );
            if name == "method_literal_span" {
                assert_eq!(event("method_call_span"), 20, "{name}, mode {mode}");
            }
        }
    }
}

#[cfg(feature = "test262-host")]
#[test]
fn execution_modes_reentrant_host_gc_preserves_live_scalar_and_heap_owners() {
    use crate::engine::api::{DescriptorField, OrdinaryPropertyDescriptor};
    let source = r#"(function activeModeGc(){
        var a={answer:40},s='heap string',n=1361129467683753853853498429727072845824n;
        n=n*3n;s=s+' result';var result=a.answer+2;
        hostGc.call(a,a,s,n);modeLog.push(result,s);
        return JSON.stringify([a.answer,result,n.toString()]);
    })()"#;
    let run = |mode| {
        observe_with(mode, source, |runtime, context| {
            let gc = context.new_test262_gc_function().unwrap();
            let global = context.global_object().unwrap();
            let key = runtime.intern_property_key("hostGc").unwrap();
            let mut descriptor = OrdinaryPropertyDescriptor::new();
            descriptor.value = DescriptorField::Present(Value::Object(gc.as_object().clone()));
            descriptor.writable = DescriptorField::Present(true);
            descriptor.configurable = DescriptorField::Present(true);
            assert!(
                context
                    .define_own_property(&global, &key, &descriptor)
                    .unwrap()
            );
        })
    };
    let canonical = run(0);
    for mode in MODES.into_iter().skip(1) {
        assert_eq!(run(mode), canonical, "host GC mode {mode}");
    }
}
