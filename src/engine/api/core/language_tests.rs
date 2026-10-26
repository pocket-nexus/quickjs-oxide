//! The language's behavior through the embedding API: bindings, calls,
//! parameters, destructuring, generators, `eval` and globals, each checked by
//! what a program observes.

use crate::engine::api::core::compile::DebugInfoMode;
use crate::engine::api::core::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, Context, DescriptorField,
    OrdinaryPropertyDescriptor, PropertyKey, Runtime, RuntimeError, Value, WellKnownSymbol,
};
use crate::engine::value::JsString;

/// A fresh runtime and its context.
fn test_context() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    (runtime, context)
}

/// The run-time code still alive after a collection: dynamic units, live
/// inline-cache entries and atoms.
fn collected_code(runtime: &Runtime) -> (usize, usize, usize) {
    runtime.run_gc().unwrap();
    let report = runtime.memory_report().unwrap();
    (
        report.code.dynamic_units,
        report.ic_entries - report.ic_free,
        report.atoms,
    )
}

/// Evaluate a self-checking script that returns `true` on success.
#[track_caller]
fn assert_script_true(context: &mut Context, source: &str) {
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
}

fn evaluate(source: &str) -> Value {
    Runtime::new()
        .new_context()
        .expect("create context")
        .eval(source)
        .unwrap()
}

fn evaluate_in_context(source: &str) -> Value {
    Runtime::new()
        .new_context()
        .expect("create context")
        .eval(source)
        .unwrap()
}

fn evaluate_error(runtime: &Runtime, context: &mut Context, source: &str) -> (JsString, JsString) {
    let Err(RuntimeError::Exception(Value::Object(error))) = context.eval(source) else {
        panic!("source did not throw an Error object: {source}");
    };
    let name = runtime.intern_property_key("name").unwrap();
    let message = runtime.intern_property_key("message").unwrap();
    let Value::String(name) = context.get_property(&error, &name).unwrap() else {
        panic!("Error.name was not a string: {source}");
    };
    let Value::String(message) = context.get_property(&error, &message).unwrap() else {
        panic!("Error.message was not a string: {source}");
    };
    (name, message)
}

fn evaluate_function_name(source: &str) -> (JsString, bool, bool, bool) {
    let (runtime, context) = test_context();
    let Value::Object(function) = context.eval(source).unwrap() else {
        panic!("source did not evaluate to a function object");
    };
    let name = runtime.intern_property_key("name").unwrap();
    let CompleteOrdinaryPropertyDescriptor::Data {
        value: Value::String(value),
        writable,
        enumerable,
        configurable,
    } = runtime.get_own_property(&function, &name).unwrap().unwrap()
    else {
        panic!("function name did not have the ordinary data descriptor");
    };
    (value, writable, enumerable, configurable)
}

#[test]
fn assignment_source_members_preserve_quickjs_reads_keys_references_and_method_receivers() {
    let (runtime, mut context) = test_context();
    let global = context.global_object().unwrap();
    let data = |value| OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(value),
        writable: DescriptorField::Present(true),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
    };

    let base = context.new_object().unwrap();
    for (name, value) in [("x", Value::Int(7)), ("default", Value::Int(8))] {
        let key = runtime.intern_property_key(name).unwrap();
        assert!(
            context
                .define_own_property(&base, &key, &data(value))
                .unwrap()
        );
    }
    let base_name = runtime.intern_property_key("base").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &base_name,
                &data(Value::Object(base.try_clone().expect("duplicate root")))
            )
            .unwrap()
    );

    let Value::Object(method) = context
        .eval("(function(){ return this === base; })")
        .unwrap()
    else {
        panic!("method source did not produce a function");
    };
    let method_key = runtime.intern_property_key("m").unwrap();
    assert!(
        context
            .define_own_property(&base, &method_key, &data(Value::Object(method)))
            .unwrap()
    );

    let Value::Object(getter) = context.eval("(function(){ return this; })").unwrap() else {
        panic!("getter source did not produce a function");
    };
    let getter = runtime.as_callable(&getter).unwrap().unwrap();
    let getter_key = runtime.intern_property_key("receiver").unwrap();
    assert!(
        context
            .define_own_property(
                &base,
                &getter_key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );

    assert_eq!(context.eval("base.x").unwrap(), Value::Int(7));
    assert_eq!(context.eval("base['x']").unwrap(), Value::Int(7));
    assert_eq!(context.eval("base.default").unwrap(), Value::Int(8));
    assert_eq!(context.eval("base\n.x").unwrap(), Value::Int(7));
    assert_eq!(context.eval("base\n['x']").unwrap(), Value::Int(7));
    assert_script_true(&mut context, "base.receiver === base");
    assert_script_true(&mut context, "base.m()");
    assert_script_true(&mut context, "base['m']()");
    assert_script_true(&mut context, "((base.m))()");
    assert_eq!(context.eval("(0, base.m)()").unwrap(), Value::Bool(false));
    assert_eq!(
        context.eval("(true ? base.m : base.m)()").unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        context.eval("(true && base.m)()").unwrap(),
        Value::Bool(false)
    );

    let hint = runtime.intern_property_key("keyHint").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &hint,
                &data(Value::String(JsString::from_static("none"))),
            )
            .unwrap()
    );
    let Value::Object(to_key) = context
        .eval("(function(hint){ keyHint = hint; return 'x'; })")
        .unwrap()
    else {
        panic!("ToPropertyKey source did not produce a function");
    };
    let key_object = context.new_object().unwrap();
    let to_primitive = PropertyKey::from(
        runtime
            .well_known_symbol(WellKnownSymbol::ToPrimitive)
            .expect("well-known symbol"),
    );
    assert!(
        context
            .define_own_property(&key_object, &to_primitive, &data(Value::Object(to_key)))
            .unwrap()
    );
    let key_name = runtime.intern_property_key("keyObject").unwrap();
    assert!(
        context
            .define_own_property(&global, &key_name, &data(Value::Object(key_object)),)
            .unwrap()
    );
    assert_eq!(context.eval("base[keyObject]").unwrap(), Value::Int(7));
    assert_eq!(
        context.eval("keyHint").unwrap(),
        Value::String(JsString::from_static("string"))
    );
    assert_eq!(
        context.eval("keyHint = 'none'").unwrap(),
        Value::String(JsString::from_static("none"))
    );
    assert!(matches!(
        context.eval("null[keyObject]"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.eval("keyHint").unwrap(),
        Value::String(JsString::from_static("none"))
    );

    assert_eq!(context.eval("'abc'.length").unwrap(), Value::Int(3));
    assert_eq!(
        context.eval("'abc'[1]").unwrap(),
        Value::String(JsString::from_static("b"))
    );
    assert!(matches!(
        context.eval("Function().toString()"),
        Ok(Value::String(_))
    ));
}

#[test]
fn assignment_discarded_binding_assignments_keep_their_semantics() {
    let (_, context) = test_context();
    let source = r#"
        (function () {
            var out = [];
            var o = {}, s, t;
            for (var i = 0; i < 3; i++) { s = o; t = s; }
            out.push(s === o && t === o);
            // Sloppy mapped arguments observe a discarded argument write.
            out.push((function (a) { a = 2; return arguments[0]; })(1));
            out.push((function (a) { 'use strict'; a = 2; return arguments[0]; })(1));
            out.push((function (a) { arguments[0] = 3; a = 4; return arguments[0] + a; })(1));
            // Captured locals and closure variables.
            var c, read = function () { return c; };
            c = o;
            out.push(read() === o);
            var write = function (v) { c = v; };
            write(5);
            out.push(c);
            // TDZ, const and immutable function-name writes still throw.
            try { (function () { x = 1; let x; })(); out.push('no'); }
            catch (e) { out.push(e.constructor.name); }
            try { (function () { const k = 1; k = 2; })(); out.push('no'); }
            catch (e) { out.push(e.constructor.name); }
            try { (function f() { 'use strict'; f = 1; })(); out.push('no'); }
            catch (e) { out.push(e.constructor.name); }
            out.push((function f() { f = 1; return typeof f; })());
            // An exception in the right-hand side leaves the old value.
            var kept = 'old';
            try { kept = (function () { throw 1; })(); } catch (e) {}
            out.push(kept);
            // Conditional right-hand sides join on the store.
            var r;
            for (var j = 0; j < 2; j++) { r = j ? 'b' : 'a'; }
            out.push(r);
            // Completion values of eval code.
            out.push(eval('let q; q = 7;'));
            out.push(eval('var e1 = 1; e1 = 8;'));
            return out.join(',');
        })()
    "#;
    assert_eq!(
        context.eval(source).unwrap(),
        Value::String(JsString::from_static(
            "true,2,1,8,true,5,ReferenceError,TypeError,TypeError,function,old,b,7,8"
        ))
    );
}

#[test]
fn calls_runtime_executes_call_method_and_construct_spread() {
    assert_eq!(
        evaluate_in_context(
            r#"
            function list() { return Array.prototype.join.call(arguments, ","); }
            var object = { value: 7, add: function (amount) { return this.value + amount; } };
            function C(a, b) { this.value = a + b; }
            [
                list(0, ...[1, 2], 3, ...[], 4,),
                object.add(...[5]),
                new C(...[20, 22]).value
            ].join("|")
            "#,
        ),
        Value::String(JsString::from_static("0,1,2,3,4|12|42"))
    );
}

#[test]
fn calls_runtime_append_reads_the_iterator_method_once() {
    // ArgumentListEvaluation's GetIterator: one `@@iterator` read
    // (QuickJS reads it twice, QUICKJS-SPEC-014).
    assert_eq!(
        evaluate_in_context(
            r#"
            (function () {
                var reads = 0;
                var source = [1, 2];
                Object.defineProperty(source, Symbol.iterator, {
                    get: function () {
                        reads++;
                        if (reads === 1) return Array.prototype.values;
                        return function () { return [9].values(); };
                    }
                });
                function list() { return Array.prototype.join.call(arguments, ","); }
                var copied = list(...source);
                return copied + "|" + reads;
            })()
            "#,
        ),
        Value::String(JsString::from_static("1,2|1"))
    );
}

#[test]
fn calls_var_initializer_named_evaluation_follows_quickjs_set_name_marker() {
    for source in [
        "(function() { var f = function() {}; return f; })()",
        "(function() { var f = (((function() {}))); return f; })()",
        "(function() { var \\u0066 = function() {}; return f; })()",
        "(function() { var f; f = function() {}; return f; })()",
    ] {
        assert_eq!(
            evaluate_function_name(source),
            (JsString::from_static("f"), false, false, true),
            "direct anonymous initializer should inherit the binding name: {source}"
        );
    }

    for source in [
        "(function() { return function() {}; })()",
        "(function() { var f = (0, function() {}); return f; })()",
        "(function() { var f = true ? function() {} : function() {}; return f; })()",
        "(function() { var f = 0 || function() {}; return f; })()",
    ] {
        assert_eq!(
            evaluate_function_name(source),
            (JsString::from_static(""), false, false, true),
            "non-AnonymousFunctionDefinition expression must keep an empty name: {source}"
        );
    }
}

#[test]
fn closures_ordinary_function_fallthrough_returns_undefined() {
    assert_eq!(
        evaluate_in_context("(function(a) { a; })(42)"),
        Value::Undefined
    );
}

#[test]
fn destructuring_object_binding_declarations_cover_direct_and_loop_surfaces() {
    for (source, expected) in [
        (
            "(function(){var {fixed}={fixed:'v'};let {['computed']:computed}={computed:'l'};const {nested:{value}}={nested:{value:'c'}};return fixed+computed+value})()",
            "vlc",
        ),
        (
            "(function(){var result='';for(var {fixed}={fixed:'v'};;){result+=fixed;break}for(let {['computed']:computed}={computed:'l'};;){result+=computed;break}for(const {nested:{value}}={nested:{value:'c'}};;){result+=value;break}return result})()",
            "vlc",
        ),
        (
            "(function(){var result='';for(var {0:first} in {ab:1})result+=first;for(let {[1]:second} in {ab:1})result+=second;for(const {constructor:{name}} in {ab:1})result+=name;return result})()",
            "abString",
        ),
        (
            "(function(){var result='';for(var {fixed} of [{fixed:'v'}])result+=fixed;for(let {['computed']:computed} of [{computed:'l'}])result+=computed;for(const {nested:{value}} of [{nested:{value:'c'}}])result+=value;return result})()",
            "vlc",
        ),
    ] {
        assert_eq!(
            evaluate_in_context(source),
            Value::String(JsString::from_static(expected)),
            "{source}"
        );
    }
}

#[test]
fn eval_binding_pattern_initializers_can_read_and_capture_parameter_arguments() {
    assert_eq!(
        evaluate_in_context(
            "(function({x=arguments[0].answer}={},a=eval('0')){return x})({x:undefined,answer:42})",
        ),
        Value::Int(42)
    );
    assert_eq!(
        evaluate_in_context(
            "(function({x=()=>arguments[0].answer}={},a=eval('0')){return x()})({x:undefined,answer:42})",
        ),
        Value::Int(42)
    );
}

#[test]
fn eval_sloppy_direct_eval_initializes_novel_destructured_var_bindings() {
    assert_eq!(
        evaluate_in_context("(function(){eval('var {x=41}={}');return x})()"),
        Value::Int(41)
    );
    assert_eq!(
        evaluate_in_context("(function(){eval('var [x=42]=[]');return x})()"),
        Value::Int(42)
    );
}

#[test]
fn eval_code_compiled_at_run_time_is_freed_with_its_last_function() {
    let (runtime, mut context) = test_context();
    assert_script_true(
        &mut context,
        r#"
        function round() {
            for (var i = 0; i < 2000; i++) {
                var f = new Function("o", "return o.k" + i + " + " + i);
                if (f({}) === f({}) || f({ ["k" + i]: 1 }) !== i + 1) throw new Error("" + i);
                eval("var e = { p" + i + ": " + i + " }; e.p" + i);
            }
            return true;
        }
        true
        "#,
    );
    // Each round's own script is a static unit (one global cache entry);
    // the code it compiles leaves nothing behind.
    let rounds: Vec<_> = (0..3)
        .map(|_| {
            assert_script_true(&mut context, "round()");
            collected_code(&runtime)
        })
        .collect();
    assert_eq!(rounds[0].0, 0, "no dynamic unit survives");
    assert_eq!(rounds[1].0, 0);
    assert_eq!(rounds[1].1 - rounds[0].1, rounds[2].1 - rounds[1].1);
    assert!(rounds[2].1 - rounds[1].1 <= 2, "{rounds:?}");
    assert_eq!(rounds[0].2, rounds[2].2, "atoms");
}

#[test]
fn eval_a_kept_function_keeps_exactly_its_unit() {
    let (runtime, mut context) = test_context();
    let (before, _, _) = collected_code(&runtime);
    assert_script_true(
        &mut context,
        r#"
        var kept = [];
        for (var i = 0; i < 300; i++) {
            var f = new Function("return 'r" + i + "'");
            if (i % 10 === 0) kept.push(f);
        }
        f = null;
        true
        "#,
    );
    let (units, _, _) = collected_code(&runtime);
    assert_eq!(units - before, 30);
    assert_script_true(
        &mut context,
        r#"
        kept.every(function (f, n) {
            return f() === "r" + n * 10 && f.name === "anonymous" &&
                f.toString().indexOf("return 'r" + n * 10 + "'") > 0;
        })
        "#,
    );
}

#[test]
fn eval_eval_code_survives_collections_while_it_runs_or_waits() {
    let (runtime, mut context) = test_context();
    assert_script_true(
        &mut context,
        r#"
        var g = eval("(function* gen(n) { var a = yield n; yield a + n; })")(1);
        var e = eval("\n\n(function named() { return new Error('at three'); })");
        true
        "#,
    );
    // Fresh code around them, then a collection: the generator's and the
    // function's units stay.
    assert_script_true(
        &mut context,
        r#"
        for (var i = 0; i < 200; i++) eval("(function () { return " + i + "; })")();
        true
        "#,
    );
    runtime.run_gc().unwrap();
    assert_script_true(
        &mut context,
        r#"
        var first = g.next().value;
        for (var i = 0; i < 200; i++) eval("(function () { return " + i + "; })")();
        var second = g.next(10).value;
        var error = e();
        first === 1 && second === 11 && e.name === "named" &&
            error.stack.indexOf(":3:") > 0
        "#,
    );
    runtime.run_gc().unwrap();
    assert_script_true(&mut context, "g.next().done && e().message === 'at three'");
}

#[test]
fn eval_atoms_of_collected_code_leave_the_table() {
    let (runtime, mut context) = test_context();
    let source = r#"
        for (var i = 0; i < 20000; i++) {
            var o = eval("({k" + i + ": 1, ['c' + " + i + "]: 2})");
            o["x" + i] = 3;
        }
        true
    "#;
    assert_script_true(&mut context, source);
    let (_, _, first) = collected_code(&runtime);
    assert_script_true(&mut context, source);
    let (_, _, second) = collected_code(&runtime);
    assert_eq!(first, second);
    assert_script_true(&mut context, "o.k19999 + o.c19999 + o.x19999 === 6");
}

#[test]
fn generators_generator_yield_in_array_destructuring_unwinds_transient_iterators() {
    let source = r#"
        (function(){
            var log="";
            function make(name,first){
                var step=0;
                return {
                    [Symbol.iterator]:function(){log+="I"+name;return this},
                    next:function(){
                        log+="N"+name;
                        if(step++===0)return{value:first,done:false};
                        return{done:true}
                    },
                    return:function(){log+="R"+name;return{done:true}}
                }
            }

            function* binding(){
                var[a=yield 1]=make("b",undefined);
                return a
            }
            var b=binding(),b0=b.next(),b1=b.return(40);

            function* assignment(){
                var a;
                [a=yield 2]=make("a",undefined);
                return a
            }
            var a=assignment(),a0=a.next(),a1=a.return(41);

            function* nested(){
                var[[x=yield 3]]=make("o",make("n",undefined));
                return x
            }
            var n=nested(),n0=n.next(),n1=n.return(42);

            function* thrown(){
                var[x=yield 4]=make("t",undefined)
            }
            var t=thrown(),t0=t.next(),caught;
            try{t.throw("x")}catch(e){caught=e}

            function* normal(){
                var[x=yield 5]=make("c",undefined);
                return x
            }
            var c=normal(),c0=c.next(),c1=c.next(43);

            return [
                b0.value,b0.done,b1.value,b1.done,
                a0.value,a1.value,n0.value,n1.value,
                t0.value,caught,c0.value,c1.value,c1.done,log
            ].join("|")
        })()
    "#;
    assert_eq!(
        evaluate_in_context(source),
        Value::String(JsString::from_static(
            "1|false|40|true|2|41|3|42|4|x|5|43|true|\
             IbNbRbIaNaRaIoNoInNnRnRoItNtRtIcNcRc"
        )),
    );
}

#[test]
fn generators_generator_destructuring_return_orders_iterator_finally_and_yield_star() {
    let source = r#"
        (function(){
            var log="";
            function source(name){
                var step=0;
                return {
                    [Symbol.iterator]:function(){log+="I"+name+"|";return this},
                    next:function(){
                        log+="N"+name+"|";
                        if(step++===0)return{value:undefined,done:false};
                        return{done:true}
                    },
                    return:function(){log+="R"+name+"|";return{done:true}}
                }
            }

            function* withFinally(){
                try{
                    var[x=yield 1]=source("outer")
                }finally{
                    log+="F|"
                }
            }
            var a=withFinally(),a0=a.next(),a1=a.return(40);

            var delegate={
                [Symbol.iterator]:function(){log+="Id|";return this},
                next:function(){log+="Nd|";return{value:2,done:false}},
                return:function(value){
                    log+="Rd:"+value+"|";
                    return{value:value,done:true}
                }
            };
            function* delegated(){
                var[x=yield* delegate]=source("host")
            }
            var b=delegated(),b0=b.next(),b1=b.return(41);

            return [
                a0.value,a1.value,a1.done,
                b0.value,b1.value,b1.done,log
            ].join("|")
        })()
    "#;
    assert_eq!(
        evaluate_in_context(source),
        Value::String(JsString::from_static(
            "1|40|true|2|41|true|Iouter|Nouter|Router|F|\
             Ihost|Nhost|Id|Nd|Rd:41|Rhost|"
        )),
    );
}

#[test]
fn generators_generator_for_of_head_return_drops_outer_iterator_like_quickjs() {
    let source = r#"
        (function(){
            var log="",out=[],holder={};
            function make(name,values){
                var index=0;
                return {
                    [Symbol.iterator]:function(){log+="I"+name;return this},
                    next:function(){
                        log+="N"+name;
                        return index<values.length
                            ? {value:values[index++],done:false}
                            : {done:true}
                    },
                    return:function(){log+="R"+name;return{done:true}}
                }
            }
            function take(name,run){
                log="";
                out.push(name+":"+run()+":"+log)
            }

            take("binding",function(){
                function* g(){
                    for(var[x=yield 1]of make("o",[make("i",[undefined])])){}
                }
                var iterator=g(),first=iterator.next(),last=iterator.return(40);
                return[first.value,first.done,last.value,last.done].join(",")
            });
            take("assignment",function(){
                var x;
                function* g(){
                    for([x=yield 2]of make("o",[make("i",[undefined])])){}
                }
                var iterator=g(),first=iterator.next(),last=iterator.return(41);
                return[first.value,last.value,last.done].join(",")
            });
            take("computed-return",function(){
                function* g(){for(holder[yield 3]of make("o",[9])){}}
                var iterator=g(),first=iterator.next(),last=iterator.return(42);
                return[first.value,last.value,last.done].join(",")
            });
            take("computed-next",function(){
                function* g(){
                    for(holder[yield 4]of make("o",[9])){log+="B"+holder.k}
                }
                var iterator=g(),first=iterator.next(),last=iterator.next("k");
                return[first.value,last.value,last.done,holder.k].join(",")
            });
            take("nested",function(){
                function* g(){
                    for(var[[x=yield 5]]of
                        make("o",[make("m",[make("i",[undefined])])])){}
                }
                var iterator=g(),first=iterator.next(),last=iterator.return(43);
                return[first.value,last.value,last.done].join(",")
            });
            take("yield-star",function(){
                var delegate={
                    [Symbol.iterator]:function(){log+="Id";return this},
                    next:function(){log+="Nd";return{value:6,done:false}},
                    return:function(value){
                        log+="Rd"+value;
                        return{value:value,done:true}
                    }
                };
                function* g(){
                    for(var[x=yield* delegate]of
                        make("o",[make("i",[undefined])])){}
                }
                var iterator=g(),first=iterator.next(),last=iterator.return(44);
                return[first.value,last.value,last.done].join(",")
            });
            take("body",function(){
                function* g(){for(var x of make("o",[9]))yield 7}
                var iterator=g(),first=iterator.next(),last=iterator.return(45);
                return[first.value,last.value,last.done].join(",")
            });
            take("rhs",function(){
                function* g(){
                    for(var x of(log+="Q",yield 8,make("o",[9]))){}
                }
                var iterator=g(),first=iterator.next(),last=iterator.return(46);
                return[first.value,last.value,last.done].join(",")
            });
            take("for-in-assignment",function(){
                function* g(){for(holder[yield 9]in{a:1}){}}
                var iterator=g(),first=iterator.next(),last=iterator.return(47);
                return[first.value,last.value,last.done].join(",")
            });
            take("for-in-binding",function(){
                function* g(){for(var{[yield 10]:x}in{a:1}){}}
                var iterator=g(),first=iterator.next(),last=iterator.return(48);
                return[first.value,last.value,last.done].join(",")
            });
            return out.join("|")
        })()
    "#;
    assert_eq!(
        evaluate_in_context(source),
        Value::String(JsString::from_static(
            "binding:1,false,40,true:IoNoIiNiRi|\
             assignment:2,41,true:IoNoIiNiRi|\
             computed-return:3,42,true:IoNo|\
             computed-next:4,,true,9:IoNoB9No|\
             nested:5,43,true:IoNoImNmIiNiRiRm|\
             yield-star:6,44,true:IoNoIiNiIdNdRd44Ri|\
             body:7,45,true:IoNoRo|\
             rhs:8,46,true:Q|\
             for-in-assignment:9,47,true:|\
             for-in-binding:10,48,true:"
        )),
    );
}

#[test]
fn generators_generator_for_of_head_inner_close_throw_pending_closes_outer() {
    let source = r#"
        (function(){
            var log="",out=[];
            function make(name,values,throwValue){
                var index=0;
                return {
                    [Symbol.iterator]:function(){log+="I"+name;return this},
                    next:function(){
                        log+="N"+name;
                        return index<values.length
                            ? {value:values[index++],done:false}
                            : {done:true}
                    },
                    return:function(){
                        log+="R"+name;
                        if(throwValue!==undefined)throw throwValue;
                        return{done:true}
                    }
                }
            }
            function run(name,outerThrow,throwResume){
                log="";
                function* g(){
                    for(var[x=yield 1]of
                        make("o",[make("i",[undefined],"inner")],outerThrow)){}
                }
                var iterator=g(),caught;
                iterator.next();
                try{
                    if(throwResume)iterator.throw("pending");
                    else iterator.return(42)
                }catch(error){
                    caught=error
                }
                out.push(name+":"+caught+":"+log)
            }
            run("outer-normal",undefined,false);
            run("outer-throw","outer",false);
            run("resume-throw","outer",true);
            return out.join("|")
        })()
    "#;
    assert_eq!(
        evaluate_in_context(source),
        Value::String(JsString::from_static(
            "outer-normal:inner:IoNoIiNiRiRo|\
             outer-throw:inner:IoNoIiNiRiRo|\
             resume-throw:pending:IoNoIiNiRiRo"
        )),
    );
}

#[test]
fn global_bindings_global_cell_execution_observes_updates_and_property_fallback_once() {
    let (_runtime, mut context) = test_context();
    assert_eq!(
        context
            .eval("var changingCell=1; function readChanging(){return changingCell} readChanging()")
            .unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        context.eval("changingCell=2; readChanging()").unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        context
            .eval("changingCell={x:3}; readChanging().x")
            .unwrap(),
        Value::Int(3)
    );
    assert_eq!(context.eval("var reads=0; Object.defineProperty(globalThis,'propertyOnly',{configurable:true,get:function(){reads++;return 17}}); function readProperty(){return propertyOnly} readProperty()+reads").unwrap(), Value::Int(18));
    assert_eq!(
        context
            .eval("delete globalThis.propertyOnly; typeof propertyOnly")
            .unwrap(),
        Value::String(JsString::from_static("undefined"))
    );
    assert_eq!(context.eval("Object.defineProperty(Object.getPrototypeOf(globalThis),'propertyOnly',{configurable:true,get:function(){reads++;return 23}}); readProperty()+reads").unwrap(), Value::Int(25));
    assert_script_true(
        &mut context,
        "delete Object.getPrototypeOf(globalThis).propertyOnly; try { readProperty(); } catch(e) { e instanceof ReferenceError }",
    );
    assert_script_true(
        &mut context,
        "try { eval('typeof tdzCell; let tdzCell=1'); } catch(e) { e instanceof ReferenceError }",
    );
}

#[test]
fn global_bindings_global_put_matches_strict_sloppy_readonly_and_setter_semantics() {
    let (runtime, context) = test_context();
    let global = context.global_object().unwrap();
    let descriptor = |value, writable| OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(value),
        writable: DescriptorField::Present(writable),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
    };

    assert_eq!(context.eval("created = 7").unwrap(), Value::Int(7));
    assert_eq!(context.eval("created").unwrap(), Value::Int(7));

    let readonly = runtime.intern_property_key("readonly").unwrap();
    assert!(
        context
            .define_own_property(&global, &readonly, &descriptor(Value::Int(1), false))
            .unwrap()
    );
    assert_eq!(context.eval("readonly = 2").unwrap(), Value::Int(2));
    assert_eq!(context.eval("readonly").unwrap(), Value::Int(1));
    let Err(RuntimeError::Exception(Value::Object(exception))) =
        context.eval("'use strict'; readonly = 2")
    else {
        panic!("strict read-only global assignment did not throw an object");
    };
    let message = runtime.intern_property_key("message").unwrap();
    assert_eq!(
        context.get_property(&exception, &message).unwrap(),
        Value::String(JsString::from_static("'readonly' is read-only"))
    );
    assert_eq!(context.eval("readonly += 2").unwrap(), Value::Int(3));
    assert_eq!(context.eval("readonly").unwrap(), Value::Int(1));
    assert_eq!(
        context.eval("'use strict'; readonly ||= 9").unwrap(),
        Value::Int(1)
    );
    assert!(matches!(
        context.eval("'use strict'; readonly += 2"),
        Err(RuntimeError::Exception(_))
    ));
    assert!(matches!(
        context.eval("'use strict'; readonly &&= 2"),
        Err(RuntimeError::Exception(_))
    ));

    let inherited = runtime.intern_property_key("inheritedReadOnly").unwrap();
    assert!(
        context
            .define_own_property(
                &context.object_prototype().unwrap(),
                &inherited,
                &descriptor(Value::Int(5), false),
            )
            .unwrap()
    );
    assert_eq!(
        context.eval("inheritedReadOnly = 6").unwrap(),
        Value::Int(6)
    );
    assert_eq!(context.eval("inheritedReadOnly").unwrap(), Value::Int(5));
    let Err(RuntimeError::Exception(Value::Object(exception))) =
        context.eval("'use strict'; inheritedReadOnly = 6")
    else {
        panic!("strict inherited read-only assignment did not throw an object");
    };
    assert_eq!(
        context.get_property(&exception, &message).unwrap(),
        Value::String(JsString::from_static("'inheritedReadOnly' is read-only"))
    );

    let no_setter = runtime.intern_property_key("noSetter").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &no_setter,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Undefined),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(context.eval("noSetter = 8").unwrap(), Value::Int(8));
    let Err(RuntimeError::Exception(Value::Object(exception))) =
        context.eval("'use strict'; noSetter = 8")
    else {
        panic!("strict setter-less assignment did not throw an object");
    };
    assert_eq!(
        context.get_property(&exception, &message).unwrap(),
        Value::String(JsString::from_static("no setter for property"))
    );
    assert_eq!(context.eval("noSetter ||= 8").unwrap(), Value::Int(8));
    assert!(matches!(
        context.eval("'use strict'; noSetter ||= 8"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.eval("'use strict'; noSetter &&= 8").unwrap(),
        Value::Undefined
    );

    assert!(matches!(
        context.eval("'use strict'; trulyMissing = 1"),
        Err(RuntimeError::Exception(_))
    ));
    let truly_missing = runtime.intern_property_key("trulyMissing").unwrap();
    assert!(!runtime.has_own_property(&global, &truly_missing).unwrap());

    let sink = runtime.intern_property_key("sink").unwrap();
    assert!(
        context
            .define_own_property(&global, &sink, &descriptor(Value::Int(0), true))
            .unwrap()
    );
    let Value::Object(setter) = context
        .eval("(function(v) { sink = v; return 99; })")
        .unwrap()
    else {
        panic!("setter source did not produce a function");
    };
    let setter = runtime.as_callable(&setter).unwrap().unwrap();
    let target = runtime.intern_property_key("setterTarget").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &target,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Undefined),
                    set: DescriptorField::Present(AccessorValue::Callable(setter)),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(context.eval("setterTarget = 42").unwrap(), Value::Int(42));
    assert_eq!(context.eval("sink").unwrap(), Value::Int(42));
    assert_eq!(context.eval("setterTarget ||= 17").unwrap(), Value::Int(17));
    assert_eq!(context.eval("sink").unwrap(), Value::Int(17));

    let Value::Object(getter) = context.eval("(function() { return this; })").unwrap() else {
        panic!("getter source did not produce a function");
    };
    let getter = runtime.as_callable(&getter).unwrap().unwrap();
    let getter_target = runtime.intern_property_key("getterTarget").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &getter_target,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context.eval("getterTarget").unwrap(),
        Value::Object(global.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        context.eval("typeof getterTarget").unwrap(),
        Value::String(JsString::from_static("object"))
    );

    let Value::Object(throwing_getter) = context.eval("(function() { throw 17; })").unwrap() else {
        panic!("throwing getter source did not produce a function");
    };
    let throwing_getter = runtime.as_callable(&throwing_getter).unwrap().unwrap();
    let throwing_target = runtime.intern_property_key("throwingGetter").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &throwing_target,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(throwing_getter)),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context.eval("typeof throwingGetter").err(),
        Some(RuntimeError::Exception(Value::Int(17)))
    );

    assert_eq!(context.eval("compoundSide = 0").unwrap(), Value::Int(0));
    assert!(matches!(
        context.eval("missingCompound += (compoundSide = 1)"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(context.eval("compoundSide").unwrap(), Value::Int(0));
    assert!(matches!(
        context.eval("missingLogical ||= (compoundSide = 2)"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(context.eval("compoundSide").unwrap(), Value::Int(0));
}

#[test]
fn global_bindings_late_global_property_delete_reconnect_and_cross_realm_use_the_defining_realm() {
    let runtime = Runtime::new();
    let defining = runtime.new_context().expect("create context");
    let caller = runtime.new_context().expect("create context");
    let Value::Object(reader) = defining
        .eval("(function() { return lateRealmValue; })")
        .unwrap()
    else {
        panic!("late global reader source did not produce a function");
    };
    let reader = runtime.as_callable(&reader).unwrap().unwrap();
    let key = runtime.intern_property_key("lateRealmValue").unwrap();
    let descriptor = |value| OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(Value::Int(value)),
        writable: DescriptorField::Present(true),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
    };
    assert!(
        caller
            .define_own_property(&caller.global_object().unwrap(), &key, &descriptor(9),)
            .unwrap()
    );
    let defining_global = defining.global_object().unwrap();
    assert!(
        defining
            .define_own_property(&defining_global, &key, &descriptor(1))
            .unwrap()
    );
    runtime.run_gc().unwrap();
    assert_eq!(
        caller.call(&reader, Value::Undefined, &[]).unwrap(),
        Value::Int(1)
    );

    assert!(runtime.delete_property(&defining_global, &key).unwrap());
    runtime.run_gc().unwrap();
    let Err(RuntimeError::Exception(Value::Object(exception))) =
        caller.call(&reader, Value::Undefined, &[])
    else {
        panic!("missing defining-realm global did not throw an object");
    };
    let reference_error = runtime.intern_property_key("ReferenceError").unwrap();
    let prototype = runtime.intern_property_key("prototype").unwrap();
    let Value::Object(reference_error) = defining
        .get_property(&defining_global, &reference_error)
        .unwrap()
    else {
        panic!("defining realm ReferenceError was not an object");
    };
    let Value::Object(reference_error_prototype) =
        defining.get_property(&reference_error, &prototype).unwrap()
    else {
        panic!("defining realm ReferenceError.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&exception).unwrap(),
        Some(reference_error_prototype)
    );
    assert!(
        defining
            .define_own_property(&defining_global, &key, &descriptor(2))
            .unwrap()
    );
    runtime.run_gc().unwrap();
    assert_eq!(
        caller.call(&reader, Value::Undefined, &[]).unwrap(),
        Value::Int(2)
    );
}

#[test]
fn lexical_bindings_ordinary_function_body_lexicals_execute_local_capture_and_constructor_paths() {
    assert_eq!(
        evaluate_in_context(
            "(function(){let x=1,y=x+1,z;return x*10+y+(typeof z==='undefined'?0:100)})()"
        ),
        Value::Int(12)
    );
    assert_eq!(
        evaluate_in_context("(function(){let arguments=3;let eval=4;return arguments+eval})()"),
        Value::Int(7)
    );
    assert_eq!(
        evaluate_in_context("(function(){var read=function(){return x};let x=7;return read()})()"),
        Value::Int(7)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){let x=function(){return x};return x()===x&&x.name==='x'})()"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){let x=1;var next=function(){x+=1;return x};return next()*10+next()})()"
        ),
        Value::Int(23)
    );
    assert_eq!(
        evaluate_in_context("(function(){const x=4;return function(){return x}()})()"),
        Value::Int(4)
    );
    assert_eq!(
        evaluate_in_context("Function('let x=1;const y=2;return x+y')()"),
        Value::Int(3)
    );
    assert_eq!(
        evaluate_in_context("(function(){var result=delete x;let x=1;return result})()"),
        Value::Bool(false)
    );
    assert_eq!(
        evaluate_in_context("(function(){const x=0;return x&&=missing})()"),
        Value::Int(0)
    );
}

#[test]
fn lexical_bindings_lexical_tdz_and_readonly_errors_follow_checked_local_and_capture_order() {
    let (runtime, mut context) = test_context();
    for source in [
        "(function(){return x;let x=1})()",
        "(function(){return typeof x;let x=1})()",
        "(function(){x=1;let x})()",
        "(function(){return function(){return x};let x=1})()()",
        "(function(){var set=function(){x=1};set();let x})()",
        "(function(){var add=function(){x+=missing};add();const x=1})()",
    ] {
        assert_eq!(
            evaluate_error(&runtime, &mut context, source),
            (
                JsString::from_static("ReferenceError"),
                JsString::from_static("x is not initialized")
            ),
            "{source}"
        );
    }
    for source in [
        "(function(){const x=1;x=2})()",
        "(function(){const x=1;return function(){x=2}})()()",
        "(function(){var set=function(){x=1};set();const x=2})()",
    ] {
        assert_eq!(
            evaluate_error(&runtime, &mut context, source),
            (
                JsString::from_static("TypeError"),
                JsString::from_static("'x' is read-only")
            ),
            "{source}"
        );
    }
}

#[test]
fn lexical_bindings_strip_debug_removes_lexical_tdz_names_but_not_readonly_atoms() {
    let runtime = Runtime::new();
    runtime
        .set_debug_info_mode(DebugInfoMode::StripDebug)
        .expect("set runtime configuration");
    let mut context = runtime.new_context().expect("create context");
    for source in [
        "(function(){return localName;let localName=1})()",
        "(function(){return function probe(){return capturedName};let capturedName=1})()()",
        "(function(){var read;outer:{read=function(){return blockName};break outer;let blockName=1;}return read();})()",
        "(function(){var read;switch(1){case 0:let switchName=1;case 1:read=function(){return switchName};break;}return read();})()",
    ] {
        assert_eq!(
            evaluate_error(&runtime, &mut context, source),
            (
                JsString::from_static("ReferenceError"),
                JsString::from_static("lexical variable is not initialized")
            )
        );
    }
    assert_eq!(
        evaluate_error(
            &runtime,
            &mut context,
            "(function(){var write;{const retainedName=1;write=function(){retainedName=2};}write();})()"
        ),
        (
            JsString::from_static("TypeError"),
            JsString::from_static("'retainedName' is read-only")
        )
    );
}

#[test]
fn operators_compiles_primitive_coercion_and_equality() {
    assert_eq!(
        evaluate("'answer: ' + 42"),
        Value::String(JsString::from_static("answer: 42"))
    );
    assert_eq!(evaluate("'42' == 42"), Value::Bool(true));
    assert_eq!(evaluate("'42' === 42"), Value::Bool(false));
}

#[test]
fn operators_relational_membership_uses_runtime_object_protocols() {
    assert_eq!(
        evaluate_in_context("'prototype' in Function"),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context("'missingMembershipKey' in Function"),
        Value::Bool(false)
    );
    assert_eq!(
        evaluate_in_context("'toString' in Function"),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context("Function instanceof Function"),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context("(function(){}) instanceof Function"),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context("1 instanceof Function"),
        Value::Bool(false)
    );
    assert_eq!(
        evaluate_in_context("(function(){}).bind(null) instanceof Function"),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){ var target=function DeepTarget(){}; var bound=target; for(var i=0;i<512;i++) bound=bound.bind(null); return 1 instanceof bound; })()"
        ),
        Value::Bool(false)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){ var result=false; for((result='prototype' in Function);false;); return result; })()"
        ),
        Value::Bool(true)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){ var result=false; for(result=Function instanceof Function;false;); return result; })()"
        ),
        Value::Bool(true)
    );

    let (_, mut context) = test_context();
    drop(context
        .eval(
            "Function.membershipTrace=''; Function[Symbol.toPrimitive]=function(hint){ Function.membershipTrace+=hint; return 'prototype'; };",
        )
        .unwrap());
    assert!(matches!(
        context.eval("Function in (Function.membershipTrace+='R',1)"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.eval("Function.membershipTrace").unwrap(),
        Value::String(JsString::from_static("R"))
    );
    assert_script_true(&mut context, "Function in Function");
    assert_eq!(
        context.eval("Function.membershipTrace").unwrap(),
        Value::String(JsString::from_static("Rstring"))
    );
}

#[test]
fn parameters_parameter_binding_patterns_execute_across_sync_function_forms() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var out=[];
                function declaration([a],{b}){return a+b}
                out.push(declaration([40],{b:2}));
                out.push((function([a]){return a+2})([40]));
                out.push((([a])=>a+2)([40]));
                out.push(({base:40,method([a]){return this.base+a}}).method([2]));
                out.push(Function('[a]','return a+2')([40]));
                var assigned;
                var setter={set value([a]){assigned=a}};
                setter.value=[42];
                out.push(assigned);
                return out.join('|');
            })()"#,
        ),
        Value::String(JsString::from_static("42|42|42|42|42|42"))
    );

    assert_eq!(
        evaluate_in_context(
            r#"(function([a,,[b,...tail]],{x:y,[String('z')]:z,...rest}){
                return a+'|'+b+'|'+tail.join(',')+'|'+y+'|'+z+'|'+rest.extra;
            })([1,0,[2,3,4]],{x:5,z:6,extra:7})"#,
        ),
        Value::String(JsString::from_static("1|2|3,4|5|6|7"))
    );

    assert_eq!(
        evaluate_in_context(
            r#"(function(...[a,b]){
                return a+b+'|'+arguments.length+'|'+(function(...[]){}).length+'|'+
                    (function(...{}){}).length+'|'+(function(...[[]]){}).length+'|'+
                    (function(...[,]){}).length+'|'+(function(...[x]){}).length+'|'+
                    (function(...{x}){}).length+'|'+(function(...[]){var x}).length+'|'+
                    (function(...[]){return arguments}).length+'|'+
                    (function(...[]){return this}).length+'|'+
                    (function(...[]){return new.target}).length+'|'+
                    (function(...[]){return function(){return this}}).length+'|'+
                    (function(...[]){return ()=>this}).length+'|'+
                    (function named(...[]){return named}).length+'|'+
                    (function(x,...[]){}).length+'|'+(function(x,...[y]){}).length+'|'+
                    (function mixed([x],...rest){return x+rest[0]+'|'+mixed.length})([40],2);
            })(40,2)"#,
        ),
        Value::String(JsString::from_static(
            "42|2|0|0|0|0|1|1|1|1|1|1|0|1|1|2|2|42|1"
        ))
    );
}

#[test]
fn parameters_parameter_binding_patterns_follow_quickjs_scope_arguments_and_hoist_order() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var results=[];
                results.push((function([a]){
                    arguments[0]=[9];
                    a=7;
                    return a+'|'+arguments[0][0];
                })([1]));
                results.push((function([a]){var a;return a})([42]));
                results.push((function([a]){
                    function a(){return 42}
                    return typeof a+'|'+a();
                })([1]));
                var key='outer';
                results.push((function({[String(key)]:value}){
                    var key='body';
                    return value;
                })({undefined:42,outer:1}));
                var lexical='outer';
                results.push((function({[lexical]:value}){
                    let lexical='body';
                    return value;
                })({outer:42}));
                results.push((function({[eval('"key"')]:value}){
                    return value;
                })({key:42}));
                results.push((function({[arguments]:arguments}){
                    return arguments;
                })({undefined:1,"[object Arguments]":42}));
                results.push((function({[(()=>eval('typeof key'))()]:value}){
                    var key='body';
                    return value;
                })({undefined:42}));
                results.push((function(){
                    return (([a])=>a+arguments[1])([40]);
                })(0,2));
                return results.join(';');
            })()"#,
        ),
        Value::String(JsString::from_static(
            "7|9;42;function|42;42;42;42;42;42;42"
        ))
    );

    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var saved;
                function capture(fn){saved=fn;return 'undefined'}
                return (function({[capture(()=>key)]:value}){
                    var key='body';
                    return value+'|'+saved();
                })({undefined:42});
            })()"#,
        ),
        Value::String(JsString::from_static("42|body"))
    );
}

#[test]
fn parameters_parameter_expression_binding_patterns_match_quickjs_scope_copy_and_length_quirks() {
    assert_eq!(
        evaluate_in_context(
            "(function(){var key='outer';return (function({[String(key)]:value},[x=1]){var key='body';return value})({outer:42,undefined:1},[])})()",
        ),
        Value::Int(42)
    );
    assert_eq!(
        evaluate_in_context("(function([a],b=(a=5)){return a})([1])"),
        Value::Int(5)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){var saved;var body=(function([a],f=(saved=()=>a)){var read=()=>a;a=2;return read})([1]);return body()+'|'+saved()})()",
        ),
        Value::String(JsString::from_static("2|1"))
    );
    assert_eq!(
        evaluate_in_context(
            "[(function([a=1],b){}).length,(function([a]=[1],b){}).length,(function(a,[b=1],c){}).length,(function(a,[b]=[1],c){}).length,(function(a,...[b=1]){}).length,(function(a=1,...[b]){}).length].join('|')",
        ),
        Value::String(JsString::from_static("2|0|3|1|2|0"))
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){var source={};return (function({}=source){var source=null;return 42})()})()",
        ),
        Value::Int(42)
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){return (function(...[a]=[99]){return String(a)} )()+'|'+(function(...[a]=[99]){return a})(42)+'|'+(function(...[a]=[99]){}).length})()",
        ),
        Value::String(JsString::from_static("undefined|42|0"))
    );
    assert_eq!(
        evaluate_in_context(
            "(function(){var out;({set value([a]=[42]){out=a}}).value=undefined;return out})()",
        ),
        Value::Int(42)
    );
}

#[test]
fn parameters_parameter_expression_binding_patterns_compose_identifier_rest_across_surfaces() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                function ordinary([a=1],...rest){return a+'|'+rest.join(',')}
                var arrow=([a=1],...rest)=>a+'|'+rest.join(',');
                var object={method([a=1],...rest){return a+'|'+rest.join(',')}};
                return ordinary([],2,3)+';'+arrow([],2,3)+';'+object.method([],2,3);
            })()"#,
        ),
        Value::String(JsString::from_static("1|2,3;1|2,3;1|2,3"))
    );
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                function ordinary([a]=[1],...rest){return a+'|'+rest.join(',')}
                var arrow=([a]=[1],...rest)=>a+'|'+rest.join(',');
                var object={method([a]=[1],...rest){return a+'|'+rest.join(',')}};
                return ordinary(undefined,2,3)+';'+arrow(undefined,2,3)+';'+
                    object.method(undefined,2,3);
            })()"#,
        ),
        Value::String(JsString::from_static("1|2,3;1|2,3;1|2,3"))
    );
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                function later([a],b=1,...rest){return a+'|'+b+'|'+rest.join(',')}
                function earlier(a=0,[b],...rest){return a+'|'+b+'|'+rest.join(',')}
                function empty({},b=1,...rest){return b+'|'+rest.join(',')}
                return later([40],undefined,2,3)+';'+earlier(undefined,[40],2,3)+';'+
                    empty({},undefined,2,3);
            })()"#,
        ),
        Value::String(JsString::from_static("40|1|2,3;0|40|2,3;1|2,3"))
    );
}

#[test]
fn parameters_parameter_expression_binding_cells_and_body_copies_survive_gc_independently() {
    let (runtime, context) = test_context();
    drop(context
        .eval(
            "var initializerRead,bodyRead;bodyRead=(function([a],f=(initializerRead=()=>a)){var read=()=>a;a=2;return read})([1])",
        )
        .unwrap());
    runtime.run_gc().unwrap();
    assert_eq!(
        context.eval("bodyRead()+'|'+initializerRead()").unwrap(),
        Value::String(JsString::from_static("2|1"))
    );
}

#[test]
fn parameters_identifier_rest_parameters_execute_across_sync_function_forms() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                function ordinary(left,...rest){
                    return left+'|'+rest.length+'|'+rest[0]+'|'+rest[1]+'|'+Array.isArray(rest);
                }
                var arrow=(left,...rest)=>left+'|'+rest.length+'|'+rest[0]+'|'+rest[1];
                var object={base:40,method(left,...rest){
                    return this.base+'|'+left+'|'+rest.length+'|'+rest[0]+'|'+rest[1];
                }};
                var dynamic=Function('left','...rest','return left+rest[0]+rest[1]');
                return ordinary(40,1,2)+';'+arrow(40,1,2)+';'+object.method(1,1,2)+';'+
                    dynamic(40,1,1)+';'+ordinary.length+'|'+arrow.length+'|'+
                    object.method.length+'|'+dynamic.length;
            })()"#,
        ),
        Value::String(JsString::from_static(
            "40|2|1|2|true;40|2|1|2;40|1|2|1|2;42;1|1|1|1"
        ))
    );

    assert_eq!(
        evaluate_in_context(
            r#"(function(left,...rest){
                arguments[0]=7;
                arguments[1]=8;
                var first=left+'|'+rest[0];
                left=9;
                rest[0]=10;
                return first+'|'+left+'|'+rest[0]+'|'+arguments[0]+'|'+arguments[1];
            })(1,2)"#,
        ),
        Value::String(JsString::from_static("1|2|9|10|7|8"))
    );

    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var retained=(function(...rest){var rest;return Array.isArray(rest)+'|'+rest.length})();
                var hoisted=(function(...rest){function rest(){return 42}return typeof rest+'|'+rest()})();
                var capture=(function(...rest){return function(){return rest[0]+rest[1]}})(40,2);
                return retained+';'+hoisted+';'+capture();
            })()"#,
        ),
        Value::String(JsString::from_static("true|0;function|42;42"))
    );
}

#[test]
fn parameters_identifier_default_parameters_execute_across_sync_function_forms() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                function ordinary(a=40,b=a+2){return b}
                var arrow=(a=40,b=a+2)=>b;
                var object={base:40,method(a=this.base,b=a+2){return b}};
                var dynamic=Function('a=40','b=a+2','return b');
                return ordinary()+'|'+arrow()+'|'+object.method()+'|'+dynamic()+'|'+
                    ordinary.length+'|'+arrow.length+'|'+object.method.length+'|'+dynamic.length;
            })()"#,
        ),
        Value::String(JsString::from_static("42|42|42|42|0|0|0|0"))
    );
}

#[test]
fn parameters_identifier_default_parameter_environment_matches_quickjs_tdz_and_body_split() {
    let (runtime, mut context) = test_context();
    for source in [
        "(function(a=b,b=2){return a})()",
        "(function(a=a){return a})()",
        "((a=b,b=2)=>a)()",
        "({method(a=a){return a}}).method()",
    ] {
        let (name, _) = evaluate_error(&runtime, &mut context, source);
        assert_eq!(name, JsString::from_static("ReferenceError"), "{source}");
    }

    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var read;
                function f(a=1,b=(read=function(){return a},0)){
                    a=2;
                    return a+'|'+read();
                }
                return f();
            })()"#,
        ),
        // Pinned QuickJS keeps body reads on the raw argument slot while the
        // initializer closure retains the lexical parameter cell.
        Value::String(JsString::from_static("2|1"))
    );
}

#[test]
fn parameters_identifier_default_parameter_uses_private_function_name_before_body_shadowing() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var read=(function f(a=f){var f;return typeof a+'|'+(a===undefined)+'|'+typeof f})();
                var closure=(function f(a=()=>f){var f=1;return typeof a()+'|'+(a()===f)})();
                var write=(function f(a=(f=1)){var f;return typeof f+'|'+(f===1)})();
                return read+';'+closure+';'+write;
            })()"#,
        ),
        Value::String(JsString::from_static(
            "function|false|undefined;function|false;undefined|false"
        ))
    );

    let (runtime, mut context) = test_context();
    let (name, message) = evaluate_error(
        &runtime,
        &mut context,
        "(function(){'use strict';return (function f(a=(f=1)){var f;return typeof f})()})()",
    );
    assert_eq!(name, JsString::from_static("TypeError"));
    assert_eq!(message, JsString::from_static("'f' is read-only"));
}

#[test]
fn parameters_identifier_default_parameters_run_before_body_hoists_and_use_unmapped_arguments() {
    assert_eq!(
        evaluate_in_context(
            r#"(function(){
                var outer=40;
                function f(a=outer,b=arguments[0]){
                    var outer=1;
                    function a(){return 42}
                    arguments[0]=7;
                    return a()+'|'+b+'|'+arguments.length;
                }
                return f(undefined);
            })()"#,
        ),
        Value::String(JsString::from_static("42|undefined|1"))
    );
}

#[test]
fn parameters_identifier_rest_array_is_allocated_in_the_callee_realm() {
    let runtime = Runtime::new();
    let defining = runtime.new_context().expect("create context");
    let caller = runtime.new_context().expect("create context");
    let Value::Object(function) = defining
        .eval("(function(...rest){return Object.getPrototypeOf(rest)===Array.prototype})")
        .unwrap()
    else {
        panic!("rest source did not produce a function");
    };
    let function = runtime.as_callable(&function).unwrap().unwrap();
    assert_eq!(
        caller
            .call(
                &function,
                Value::Undefined,
                &[Value::Int(40), Value::Int(2)]
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn program_declarations_program_vars_instantiate_persist_and_preserve_existing_properties() {
    let (runtime, mut context) = test_context();
    assert_eq!(
        context
            .eval("var value;var value=2;if(false){var dormant=3}for(var loop=0;loop<2;loop++){};value+'|'+typeof dormant+'|'+loop")
            .unwrap(),
        Value::String(JsString::from_static("2|undefined|2"))
    );
    let global = context.global_object().unwrap();
    for (name, value) in [
        ("value", Value::Int(2)),
        ("dormant", Value::Undefined),
        ("loop", Value::Int(2)),
    ] {
        let key = runtime.intern_property_key(name).unwrap();
        assert_eq!(
            context.get_own_property(&global, &key).unwrap(),
            Some(CompleteOrdinaryPropertyDescriptor::Data {
                value,
                writable: true,
                enumerable: true,
                configurable: false,
            })
        );
    }
    assert_eq!(context.eval("var value;value").unwrap(), Value::Int(2));
    assert_eq!(
        context
            .eval("var captured=1,read=function(){return captured};captured=4;read()")
            .unwrap(),
        Value::Int(4)
    );
    assert_eq!(context.eval("delete value").unwrap(), Value::Bool(false));

    assert_eq!(context.eval("hostValue=7").unwrap(), Value::Int(7));
    let host = runtime.intern_property_key("hostValue").unwrap();
    assert_eq!(
        context.eval("var hostValue;hostValue").unwrap(),
        Value::Int(7)
    );
    assert_eq!(
        context.get_own_property(&global, &host).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(7),
            writable: true,
            enumerable: true,
            configurable: true,
        })
    );
    assert_script_true(
        &mut context,
        "Function.hostRead=function(){return hostValue};var hostValue=8;delete hostValue",
    );
    assert!(matches!(
        context.eval("Function.hostRead()"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.eval("var hostValue;hostValue").unwrap(),
        Value::Undefined
    );
    assert_eq!(
        context.eval("Function.hostRead()").unwrap(),
        Value::Undefined
    );
    assert_eq!(
        context.get_own_property(&global, &host).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: false,
        })
    );

    let fixed = runtime.intern_property_key("fixedVar").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &fixed,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Int(1)),
                    writable: DescriptorField::Present(false),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context.eval("var fixedVar;fixedVar").unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        context.eval("var fixedVar=2;fixedVar").unwrap(),
        Value::Int(1)
    );
    assert!(matches!(
        context.eval("'use strict';var fixedVar=2"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.get_own_property(&global, &fixed).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(1),
            writable: false,
            enumerable: false,
            configurable: true,
        })
    );

    assert_eq!(
        context.eval("Function.varSetterHits=0").unwrap(),
        Value::Int(0)
    );
    let Value::Object(setter) = context
        .eval("(function(value){Function.varSetterHits=value})")
        .unwrap()
    else {
        panic!("global var accessor probe did not create a setter");
    };
    let setter = runtime.as_callable(&setter).unwrap().unwrap();
    let accessor = runtime.intern_property_key("accessorVar").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &accessor,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Undefined),
                    set: DescriptorField::Present(AccessorValue::Callable(
                        setter.try_clone().expect("duplicate root")
                    )),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context
            .eval("var accessorVar;Function.varSetterHits")
            .unwrap(),
        Value::Int(0),
        "a var without initializer must not invoke an existing setter"
    );
    assert_eq!(
        context
            .eval("var accessorVar=9;Function.varSetterHits")
            .unwrap(),
        Value::Int(9)
    );
    assert_eq!(
        context.get_own_property(&global, &accessor).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Accessor {
            get: None,
            set: Some(setter),
            enumerable: false,
            configurable: true,
        })
    );

    let inherited = runtime.new_context().expect("create context");
    let inherited_key = runtime.intern_property_key("inheritedVar").unwrap();
    let prototype = inherited.object_prototype().unwrap();
    assert!(
        inherited
            .define_own_property(
                &prototype,
                &inherited_key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Int(5)),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        inherited.eval("var inheritedVar;inheritedVar").unwrap(),
        Value::Undefined
    );
    let inherited_global = inherited.global_object().unwrap();
    assert_eq!(
        inherited
            .get_own_property(&inherited_global, &inherited_key)
            .unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: false,
        })
    );

    let auto_init = runtime.new_context().expect("create context");
    assert_eq!(
        auto_init.eval("var Number;typeof Number").unwrap(),
        Value::String(JsString::from_static("function"))
    );
    assert_eq!(
        auto_init.eval("var Number=9;Number").unwrap(),
        Value::Int(9)
    );
    let number = runtime.intern_property_key("Number").unwrap();
    let auto_init_global = auto_init.global_object().unwrap();
    assert_eq!(
        auto_init
            .get_own_property(&auto_init_global, &number)
            .unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(9),
            writable: true,
            enumerable: false,
            configurable: true,
        })
    );
}

#[test]
fn program_declarations_program_global_lexical_preflight_and_failed_initializers() {
    let (runtime, context) = test_context();
    let name = runtime.intern_property_key("name").unwrap();
    let message = runtime.intern_property_key("message").unwrap();

    let delayed = context
        .compile("let delayedGlobal=7;delayedGlobal")
        .unwrap();
    assert_eq!(
        context.eval("typeof delayedGlobal").unwrap(),
        Value::String(JsString::from_static("undefined"))
    );
    assert_eq!(context.execute(&delayed).unwrap(), Value::Int(7));
    assert!(matches!(
        context.execute(&delayed),
        Err(RuntimeError::Exception(_))
    ));

    let atomic = context
        .compile("let untouched=function(){return Infinity},NaN=1,Infinity=2")
        .unwrap();
    let Err(RuntimeError::Exception(Value::Object(error))) = context.execute(&atomic) else {
        panic!("global declaration preflight did not throw an Error object");
    };
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("redeclaration of 'NaN'"))
    );
    assert_eq!(
        context.eval("typeof untouched").unwrap(),
        Value::String(JsString::from_static("undefined"))
    );
    assert_eq!(
        context.eval("let untouched=4;untouched").unwrap(),
        Value::Int(4)
    );

    assert_eq!(
        context
            .eval(
                "Function.saved=function(){return captured};let captured=(function(){throw 17})()"
            )
            .err(),
        Some(RuntimeError::Exception(Value::Int(17)))
    );
    let Err(RuntimeError::Exception(Value::Object(error))) = context.eval("Function.saved()")
    else {
        panic!("declaring-script capture did not preserve the global lexical TDZ");
    };
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("captured is not initialized"))
    );
    let Err(RuntimeError::Exception(exception)) = context.eval("captured") else {
        panic!("expected a JavaScript exception");
    };
    // The binding stays uninitialized: every reference to it is in its TDZ
    // (9.1.1.4.6 GetBindingValue), `typeof` too.
    let Value::Object(error) = exception else {
        panic!("later eval did not throw the global lexical's TDZ error");
    };
    assert_eq!(
        context.get_property(&error, &name).unwrap(),
        Value::String(JsString::from_static("ReferenceError"))
    );
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("captured is not initialized"))
    );
    assert!(matches!(
        context.eval("typeof captured"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(context.eval("delete captured").unwrap(), Value::Bool(false));
    assert!(matches!(
        context.eval("let captured=1"),
        Err(RuntimeError::Exception(_))
    ));
}

#[test]
fn statements_indirect_eval_roots_enable_annex_b_html_comments() {
    assert_eq!(
        evaluate_in_context(r#"(0, eval)("<!-- indirect eval\n40 + 2")"#),
        Value::Int(42)
    );
    assert_eq!(
        evaluate_in_context(r#""use strict"; (0, eval)("--> strict indirect eval\n40 + 2")"#),
        Value::Int(42)
    );
}

#[test]
fn statements_debugger_statement_is_an_asi_noop_that_preserves_eval_completion() {
    for (source, expected) in [
        ("40; debugger;", 40),
        ("40; debugger", 40),
        ("40; { debugger }", 40),
        ("debugger\n42", 42),
        ("debugger\r42", 42),
        ("debugger\u{2028}42", 42),
        ("debugger\u{2029}42", 42),
        ("while (false) debugger; 42", 42),
        ("if (true) debugger\n42", 42),
        ("label: debugger\n42", 42),
        (r#""use strict"; 40; debugger"#, 40),
        ("\"use strict\"\ndebugger\n42", 42),
    ] {
        assert_eq!(
            evaluate_in_context(source),
            Value::Int(expected),
            "{source:?}"
        );
    }

    assert_eq!(
        evaluate_in_context("(function () { debugger; return 42; })()"),
        Value::Int(42)
    );
}
