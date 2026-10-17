use crate::runtime_completion_oracle::ORACLE_NORMALIZER;
use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::normalize_rust_value_units as normalize_rust_value;
use quickjs_oxide::engine::api::core::{
    AccessorValue, Context, DescriptorField, JsString, OrdinaryPropertyDescriptor, Runtime,
    RuntimeError, Value,
};
use std::ffi::OsStr;
use std::process::Command;

#[test]
fn template_concat_lookup_order_matches_pinned_quickjs() {
    let oracle = require_qjs_oracle!("template getter differential");

    for (description, source) in [
        (
            "getter precedes expressions and hook follows them",
            "Function.trace='';(function(){var one=function(){Function.trace=Function.trace+'1';return 10;};var two=function(){Function.trace=Function.trace+'2';return 20;};var value=`a${one()}b${two()}c`;return value+'|'+Function.trace;})()",
        ),
        (
            "no-substitution template skips getter",
            "Function.trace='';(function(){var value=`plain`;return value+'|'+Function.trace;})()",
        ),
    ] {
        let (runtime, mut context) = host_runtime_context();
        install_concat_getter(&runtime, &mut context);
        let value = context
            .eval(source)
            .unwrap_or_else(|error| panic!("Rust rejected {description:?}: {error}"));
        assert_eq!(
            normalize_rust_value(&value),
            oracle_getter_value_observation(&oracle, source, description),
            "concat getter order drifted for {description:?}"
        );
    }
}

#[test]
fn template_concat_getter_fault_sites_match_pinned_quickjs() {
    let oracle = require_qjs_oracle!("template getter stack differential");

    for (description, source) in [
        (
            "new statement replaces a previous operator marker",
            "1+2;\n`a${3}b`",
        ),
        (
            "composite template inherits expression entry",
            "1 + `a${2}b`",
        ),
        (
            "function-body expression entry",
            "(function(){ 1+2;\n`a${3}b`; })()",
        ),
        (
            "moved for update inherits the body marker",
            "Function.i=0;\nfor(;Function.i++<1;`a${3}b`){4+5;}",
        ),
        (
            "moved for update after an empty body inherits the test marker",
            "Function.i=0;\nfor(;Function.i++<1;`a${3}b`);",
        ),
        (
            "unmoved no-test for update inherits the initializer marker",
            "Function.i=0;\nfor(;;`a${3}b`);",
        ),
        (
            "for initializer inherits the preceding marker",
            "1+2;\nfor(`a${3}b`;false;);",
        ),
        (
            "for test inherits the preceding marker",
            "Function.i=0;\nfor(;`a${3}b`;);",
        ),
        (
            "regular label does not replace the body expression marker",
            "1+2;\nouter: `a${3}b`;",
        ),
        (
            "labeled continue does not replace the relocated update marker",
            "Function.i=0;\nouter: for(;Function.i++<1;`a${3}b`){4+5; continue outer;}",
        ),
    ] {
        assert_eq!(
            rust_getter_fault_location(source),
            oracle_getter_fault_location(&oracle, source, description),
            "concat getter fault site drifted for {description:?}"
        );
    }
}

#[test]
fn template_stack_limit_uses_reachable_bytecode_like_pinned_quickjs() {
    let oracle = require_qjs_oracle!("template stack differential");

    let accepted_boundary = format!("`{}`", "${0}".repeat(65_532));
    Runtime::new_with_host_services(quickjs_oxide_host::SystemHost::default())
        .new_context()
        .expect("create context")
        .compile(&accepted_boundary)
        .expect("Rust rejected the reachable QuickJS template stack boundary");
    let upstream = run_oracle_file(&oracle, &accepted_boundary, "reachable-boundary");
    assert!(
        upstream.status.success(),
        "QuickJS rejected its reachable template stack boundary: {}",
        String::from_utf8_lossy(&upstream.stderr)
    );

    for (count, suffix) in [(65_533, "limit"), (65_536, "wrapped-argc")] {
        let substitutions = "${0}".repeat(count);
        let reachable = format!("`{substitutions}`");
        let (runtime, mut context) = host_runtime_context();
        assert_eq!(context.compile(&reachable), Err(RuntimeError::Exception));
        assert_eq!(
            take_rust_error_name_message(&runtime, &mut context),
            "InternalError|stack overflow"
        );
        let upstream = run_oracle_file(&oracle, &reachable, &format!("{suffix}-reachable"));
        assert!(
            !upstream.status.success(),
            "QuickJS accepted stack overflow at {count} substitutions"
        );
        assert!(
            String::from_utf8(upstream.stderr)
                .unwrap()
                .starts_with("InternalError: stack overflow"),
            "QuickJS stack-overflow category drifted at {count} substitutions"
        );

        let unreachable = format!("(function(){{return 1;{reachable};}})");
        context
            .compile(&unreachable)
            .expect("Rust rejected unreachable oversized template");
        let upstream = run_oracle_file(&oracle, &unreachable, &format!("{suffix}-unreachable"));
        assert!(
            upstream.status.success(),
            "QuickJS rejected unreachable oversized template: {}",
            String::from_utf8_lossy(&upstream.stderr)
        );
    }

    let huge_template = format!("`{}`", "${0}".repeat(65_533));
    let switch_body_boundary = format!("`{}`", "${0}".repeat(65_531));
    let switch_body_overflow = format!("`{}`", "${0}".repeat(65_532));
    let switch_case_boundary = format!("`{}`", "${0}".repeat(65_530));
    let switch_case_overflow = format!("`{}`", "${0}".repeat(65_531));
    for (suffix, source, accepted) in [
        (
            "switch-selector-stack-boundary",
            format!("switch({accepted_boundary}){{}}"),
            true,
        ),
        (
            "switch-selector-stack-overflow",
            format!("switch({huge_template}){{}}"),
            false,
        ),
        (
            "switch-body-retained-selector-boundary",
            format!("switch(0){{default:{switch_body_boundary};}}"),
            true,
        ),
        (
            "switch-body-retained-selector-overflow",
            format!("switch(0){{default:{switch_body_overflow};}}"),
            false,
        ),
        (
            "switch-case-retained-selector-and-dup-boundary",
            format!("switch(0){{case {switch_case_boundary}:;}}"),
            true,
        ),
        (
            "switch-case-retained-selector-and-dup-overflow",
            format!("switch(0){{case {switch_case_overflow}:;}}"),
            false,
        ),
        (
            "switch-break-makes-body-tail-unreachable",
            format!("switch(0){{case 0:break;{huge_template};}}"),
            true,
        ),
        (
            "switch-body-before-break-is-reachable",
            format!("switch(0){{case 0:{huge_template};break;}}"),
            false,
        ),
        (
            "switch-constant-unmatched-body-is-not-folded",
            format!("switch(0){{case 1:{huge_template};}}"),
            false,
        ),
        (
            "switch-case-test-is-reachable",
            format!("switch(0){{case {huge_template}:;}}"),
            false,
        ),
        (
            "if-false-folded",
            format!("if(false){{{huge_template};}}"),
            true,
        ),
        (
            "if-true-else-folded",
            format!("if(true){{}}else{{{huge_template};}}"),
            true,
        ),
        (
            "empty-string-not-folded",
            format!("if(''){{{huge_template};}}"),
            false,
        ),
        (
            "float-not-folded",
            format!("if(0.5){{{huge_template};}}"),
            false,
        ),
        (
            "while-false-folded",
            format!("while(false){{{huge_template};}}"),
            true,
        ),
        (
            "while-null-folded",
            format!("while(null){{{huge_template};}}"),
            true,
        ),
        (
            "while-int-zero-folded",
            format!("while(0){{{huge_template};}}"),
            true,
        ),
        (
            "while-void-zero-folded",
            format!("while(void 0){{{huge_template};}}"),
            true,
        ),
        (
            "while-empty-string-not-folded",
            format!("while(''){{{huge_template};}}"),
            false,
        ),
        (
            "while-float-not-folded",
            format!("while(0.5){{{huge_template};}}"),
            false,
        ),
        (
            "while-bigint-not-folded",
            format!("while(0n){{{huge_template};}}"),
            false,
        ),
        (
            "while-identifier-not-folded",
            format!("while(undefined){{{huge_template};}}"),
            false,
        ),
        (
            "while-negative-zero-not-folded",
            format!("while(-0){{{huge_template};}}"),
            false,
        ),
        (
            "while-not-expression-not-folded",
            format!("while(!1){{{huge_template};}}"),
            false,
        ),
        (
            "do-body-is-reachable",
            format!("do{{{huge_template};}}while(false)"),
            false,
        ),
        (
            "while-break-makes-tail-unreachable",
            format!("while(true){{break;{huge_template};}}"),
            true,
        ),
        (
            "while-dynamic-condition-break-makes-tail-unreachable",
            format!("while(''){{break;{huge_template};}}"),
            true,
        ),
        (
            "while-body-before-break-is-reachable",
            format!("while(true){{{huge_template};break;}}"),
            false,
        ),
        (
            "do-break-makes-tail-unreachable",
            format!("do{{break;{huge_template};}}while(false)"),
            true,
        ),
        (
            "do-continue-makes-tail-unreachable",
            format!("do{{continue;{huge_template};}}while(false)"),
            true,
        ),
        (
            "outer-folded-loop-makes-nested-do-unreachable",
            format!("while(false){{do{{{huge_template};}}while(false)}}"),
            true,
        ),
        (
            "for-false-makes-body-unreachable",
            format!("for(;false;){{{huge_template};}}"),
            true,
        ),
        (
            "for-string-test-is-not-folded",
            format!("for(;' ';){{{huge_template};}}"),
            false,
        ),
        (
            "for-initializer-is-reachable",
            format!("for({huge_template};false;){{}}"),
            false,
        ),
        (
            "for-false-makes-update-unreachable",
            format!("for(;false;{huge_template}){{}}"),
            true,
        ),
        (
            "for-break-makes-update-unreachable",
            format!("for(;true;{huge_template}){{break;}}"),
            true,
        ),
        (
            "for-continue-makes-update-reachable",
            format!("for(;true;{huge_template}){{continue;}}"),
            false,
        ),
        (
            "for-break-makes-body-tail-unreachable",
            format!("for(;;){{break;{huge_template};}}"),
            true,
        ),
        (
            "for-body-before-break-is-reachable",
            format!("for(;;){{{huge_template};break;}}"),
            false,
        ),
        (
            "labeled-break-makes-regular-tail-unreachable",
            format!("outer:{{break outer;{huge_template};}}"),
            true,
        ),
        (
            "labeled-body-before-break-is-reachable",
            format!("outer:{{{huge_template};break outer;}}"),
            false,
        ),
        (
            "outer-labeled-break-makes-for-update-unreachable",
            format!("outer:for(;true;{huge_template}){{while(true){{break outer;}}}}"),
            true,
        ),
        (
            "outer-labeled-continue-makes-for-update-reachable",
            format!("outer:for(;true;{huge_template}){{while(true){{continue outer;}}}}"),
            false,
        ),
        (
            "reachable-in-key-contributes-to-bytecode-stack",
            format!("{huge_template} in Function"),
            false,
        ),
        (
            "logical-short-circuit-in-is-not-folded-for-stack-limit",
            format!("false&&({huge_template} in Function)"),
            false,
        ),
        (
            "reachable-instanceof-candidate-contributes-to-bytecode-stack",
            format!("{huge_template} instanceof Function"),
            false,
        ),
        (
            "logical-short-circuit-instanceof-is-not-folded-for-stack-limit",
            format!("false&&({huge_template} instanceof Function)"),
            false,
        ),
    ] {
        let (runtime, mut context) = host_runtime_context();
        let rust = context.compile(&source);
        let upstream = run_oracle_file(&oracle, &source, suffix);
        if accepted {
            rust.unwrap_or_else(|error| panic!("Rust rejected {suffix}: {error}"));
            assert!(
                upstream.status.success(),
                "QuickJS rejected {suffix}: {}",
                String::from_utf8_lossy(&upstream.stderr)
            );
        } else {
            assert_eq!(rust, Err(RuntimeError::Exception));
            assert_eq!(
                take_rust_error_name_message(&runtime, &mut context),
                "InternalError|stack overflow"
            );
            assert!(
                String::from_utf8(upstream.stderr)
                    .unwrap()
                    .starts_with("InternalError: stack overflow"),
                "QuickJS stack-overflow category drifted for {suffix}"
            );
        }
    }
}

#[test]
fn tagged_templates_publish_realm_local_template_objects() {
    let (_runtime, mut context) = host_runtime_context();
    assert_eq!(
        context
            .eval(
                "(function tag(strings, value) { return strings[0] + value + strings[1]; })`a${40 + 2}b`",
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("a42b").unwrap())
    );
    assert_eq!(
        context
            .eval("(function tag(strings) { return strings.raw[0]; })`\\x`")
            .unwrap(),
        Value::String(JsString::try_from_utf8("\\x").unwrap())
    );
    assert_eq!(
        context
            .eval("(function tag(strings) { return strings[0]; })\n`x`")
            .unwrap(),
        Value::String(JsString::try_from_utf8("x").unwrap())
    );
}

fn install_concat_getter(runtime: &Runtime, context: &mut Context) {
    install_concat_getter_source(
        runtime,
        context,
        "(function(){Function.trace=Function.trace+'g';return function(a,b,c,d){'use strict';Function.trace=Function.trace+'h';return 'hook:'+this+':'+a+':'+b+':'+c+':'+d;};})",
    );
}

fn install_throwing_concat_getter(runtime: &Runtime, context: &mut Context) {
    install_concat_getter_source(
        runtime,
        context,
        "(function(){throw new Error('template getter')})",
    );
}

fn install_concat_getter_source(runtime: &Runtime, context: &mut Context, source: &str) {
    let getter = context.eval(source).unwrap();
    let Value::Object(getter) = getter else {
        panic!("concat getter source did not produce a function object");
    };
    let getter = runtime.as_callable(&getter).unwrap().unwrap();
    let prototype = context.string_prototype().unwrap();
    let concat = runtime.intern_property_key("concat").unwrap();
    assert!(
        context
            .define_own_property(
                &prototype,
                &concat,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

fn rust_getter_fault_location(source: &str) -> String {
    let (runtime, mut context) = host_runtime_context();
    install_throwing_concat_getter(&runtime, &mut context);
    assert_eq!(
        context.eval_with_filename(source, "<evalScript>"),
        Err(RuntimeError::Exception)
    );
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("template getter did not throw an Error object");
    };
    let stack = runtime.intern_property_key("stack").unwrap();
    let Value::String(stack) = context.get_property(&error, &stack).unwrap() else {
        panic!("template getter Error.stack was not a string");
    };
    eval_script_location(&stack.to_utf8_lossy())
}

fn oracle_getter_value_observation(oracle: &OsStr, source: &str, description: &str) -> String {
    let script = format!(
        "Object.defineProperty(String.prototype, 'concat', {{get:function(){{Function.trace=Function.trace+'g';return function(a,b,c,d){{'use strict';Function.trace=Function.trace+'h';return 'hook:'+this+':'+a+':'+b+':'+c+':'+d;}};}}, configurable:true}});\nvar __qjo_value = std.evalScript(scriptArgs[0]);\n{ORACLE_NORMALIZER}"
    );
    let output = Command::new(oracle)
        .args(["--std", "-e", &script, source])
        .output()
        .unwrap_or_else(|error| panic!("could not run QuickJS for {description:?}: {error}"));
    assert!(
        output.status.success(),
        "QuickJS rejected {description:?} ({source:?}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("QuickJS getter output was not UTF-8")
        .trim_end()
        .to_owned()
}

fn oracle_getter_fault_location(oracle: &OsStr, source: &str, description: &str) -> String {
    let setup = "Object.defineProperty(String.prototype,'concat',{get:function(){throw new Error('template getter')},configurable:true});std.evalScript(scriptArgs[0]);";
    let output = Command::new(oracle)
        .args(["--std", "-e", setup, source])
        .output()
        .unwrap_or_else(|error| panic!("could not run QuickJS for {description:?}: {error}"));
    assert!(
        !output.status.success(),
        "QuickJS getter unexpectedly completed {description:?}"
    );
    let stderr = String::from_utf8(output.stderr).expect("QuickJS getter stack was not UTF-8");
    eval_script_location(&stderr)
}

fn eval_script_location(stack: &str) -> String {
    stack
        .lines()
        .find_map(|line| {
            line.trim()
                .split_once("(<evalScript>:")
                .and_then(|(_, location)| location.strip_suffix(')'))
        })
        .unwrap_or_else(|| panic!("stack had no evalScript frame: {stack:?}"))
        .to_owned()
}

fn take_rust_error_name_message(runtime: &Runtime, context: &mut Context) -> String {
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("Rust compiler did not materialize an Error object");
    };
    let read = |context: &mut Context, name: &str| {
        let key = runtime.intern_property_key(name).unwrap();
        context.get_property(&error, &key).unwrap()
    };
    let Value::String(name) = read(context, "name") else {
        panic!("Rust Error.name was not a string");
    };
    let Value::String(message) = read(context, "message") else {
        panic!("Rust Error.message was not a string");
    };
    format!("{}|{}", name.to_utf8_lossy(), message.to_utf8_lossy())
}

fn run_oracle_file(oracle: &OsStr, source: &str, suffix: &str) -> std::process::Output {
    let path = std::env::temp_dir().join(format!(
        "quickjs-oxide-template-stack-{}-{suffix}.js",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("write temporary QuickJS source");
    let output = Command::new(oracle)
        .args(["--stack-size", "32M"])
        .arg(&path)
        .output()
        .expect("run QuickJS stack oracle");
    std::fs::remove_file(path).expect("remove temporary QuickJS source");
    output
}
