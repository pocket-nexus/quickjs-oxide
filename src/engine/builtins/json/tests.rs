use crate::engine::builtins::native::NativeCProto;
use crate::engine::value::conversion::NativeConversion;

use super::*;

#[test]
fn json_native_cproto_matches_pinned_function_table() {
    for kind in [
        JsonNativeKind::IsRawJson,
        JsonNativeKind::Parse,
        JsonNativeKind::RawJson,
        JsonNativeKind::Stringify,
    ] {
        let descriptor = NativeFunctionId::Json(kind).descriptor();
        assert_eq!(descriptor.cproto, NativeCProto::Generic, "{kind:?}");
        assert!(!descriptor.cproto.default_is_constructor(), "{kind:?}");
    }
}

#[test]
fn global_json_is_realm_aware_lazy_and_reserves_the_pinned_table_order() {
    let runtime = Runtime::new();
    let mut first = runtime.new_context();
    let second = runtime.new_context();
    let first_global = first.global_object().unwrap();
    let second_global = second.global_object().unwrap();
    let key = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Json)
        .unwrap();

    for (global, realm) in [(&first_global, first.realm), (&second_global, second.realm)] {
        let state = runtime.0.state.borrow();
        let object = state.heap.object(global.object_id()).unwrap();
        let shape = state.heap.shape(object.shape).unwrap();
        let slot = usize::try_from(shape.find(key.atom()).unwrap()).unwrap();
        assert_eq!(
            shape.entries()[slot].flags,
            PropertyFlags::data(true, false, true),
        );
        assert!(matches!(
            object.slots.get(slot),
            Some(PropertySlot::AutoInit(AutoInitProperty::Json {
                realm: defining_realm,
            })) if *defining_realm == realm
        ));
    }

    let Value::Object(json) = first.get_property(&first_global, &key).unwrap() else {
        panic!("JSON did not materialize to an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&json).unwrap(),
        Some(first.object_prototype().unwrap()),
    );
    let expected = [
        (JsonNativeKind::IsRawJson, "isRawJSON", 1),
        (JsonNativeKind::Parse, "parse", 2),
        (JsonNativeKind::RawJson, "rawJSON", 1),
        (JsonNativeKind::Stringify, "stringify", 3),
    ];
    for (kind, name, length) in expected {
        let method = runtime.intern_property_key(name).unwrap();
        let state = runtime.0.state.borrow();
        let object = state.heap.object(json.object_id()).unwrap();
        let shape = state.heap.shape(object.shape).unwrap();
        let slot = usize::try_from(shape.find(method.atom()).unwrap()).unwrap();
        assert_eq!(
            shape.entries()[slot].flags,
            PropertyFlags::data(true, false, true),
        );
        assert!(matches!(
            object.slots.get(slot),
            Some(PropertySlot::AutoInit(AutoInitProperty::NativeBuiltin {
                realm,
                target: NativeFunctionId::Json(target),
                name: target_name,
                length: target_length,
                min_readable_args,
            })) if *realm == first.realm
                && *target == kind
                && *target_name == name
                && *target_length == length
                && *min_readable_args == length
        ));
    }
}

#[test]
fn deleting_lazy_global_json_releases_its_realm_edge() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let global = context.global_object().unwrap();
    let key = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Json)
        .unwrap();
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .context_strong_count(context.realm)
        .unwrap();

    assert!(runtime.delete_property(&global, &key).unwrap());
    assert!(!runtime.has_own_property(&global, &key).unwrap());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .context_strong_count(context.realm)
            .unwrap(),
        before - 1,
    );
}

#[test]
fn json_module_parser_returns_the_strict_json_value() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let source = JsString::from_static("{\"answer\":42}");
    let filename = JsString::from_static("answer.json");
    let NativeConversion::Value(Value::Object(value)) = runtime
        .parse_json_module_text(context.realm, &source, &filename)
        .unwrap()
    else {
        panic!("strict JSON module text did not return its object value");
    };
    let answer = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Answer)
        .unwrap();
    assert_eq!(
        context.get_property(&value, &answer).unwrap(),
        Value::Int(42)
    );
}

#[test]
fn quickjs_extended_json_module_parser_is_host_selected_and_keeps_strict_json_strict() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let source = JsString::try_from_utf8(
        "/* leading */\n{\n\
         // comment\n\
         bare: 'single quoted',\n\
         verticalEscape: 'a\\vb',\n\
         continued: 'left\\\nright',\n\
         trailing: [1, 2,],\n\
         \u{000c}\u{000b}plus: +.5,\n\
         leadingDot: .25,\n\
         hexadecimal: 0x2a,\n\
         octal: 0o52,\n\
         binary: 0b101010,\n\
         notANumber: NaN,\n\
         positiveInfinity: +Infinity,\n\
         negativeInfinity: -Infinity,\n\
        }",
    )
    .unwrap();
    let filename = JsString::from_static("fixtures/value.data");
    let NativeConversion::Value(Value::Object(value)) = runtime
        .parse_json5_module_text(context.realm, &source, &filename)
        .unwrap()
    else {
        panic!("QuickJS extended JSON did not return its object value");
    };
    let global = context.global_object().unwrap();
    let key = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Json5Value)
        .unwrap();
    assert!(
        context
            .set_property(&global, &key, Value::Object(value))
            .unwrap()
    );
    assert_eq!(
        context
            .eval(
                "const value = __json5Value;\n\
                 value.bare === 'single quoted' &&\n\
                 value.verticalEscape.length === 3 &&\n\
                 value.verticalEscape.charCodeAt(1) === 11 &&\n\
                 value.continued === 'leftright' &&\n\
                 value.trailing.join(',') === '1,2' &&\n\
                 value.plus === 0.5 && value.leadingDot === 0.25 &&\n\
                 value.hexadecimal === 42 && value.octal === 42 &&\n\
                 value.binary === 42 && Number.isNaN(value.notANumber) &&\n\
                 value.positiveInfinity === Infinity &&\n\
                 value.negativeInfinity === -Infinity",
            )
            .unwrap(),
        Value::Bool(true)
    );

    let NativeConversion::Throw(Value::Object(error)) = runtime
        .parse_json_module_text(context.realm, &source, &filename)
        .unwrap()
    else {
        panic!("strict JSON unexpectedly accepted QuickJS extended JSON");
    };
    let message = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Message)
        .unwrap();
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("unexpected token: '/'"))
    );

    let line_separator = JsString::try_from_utf8("// comment\u{2028}{answer: 42}").unwrap();
    let NativeConversion::Value(Value::Object(value)) = runtime
        .parse_json5_module_text(context.realm, &line_separator, &filename)
        .unwrap()
    else {
        panic!("extended JSON line comment did not consume its Unicode terminator");
    };
    let answer = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Answer)
        .unwrap();
    assert_eq!(
        context.get_property(&value, &answer).unwrap(),
        Value::Int(42)
    );
}

#[test]
fn json_module_parser_reports_pinned_quickjs_token_locations() {
    let cases = [
        ("{\n  notJson: 0\n}\n", "expecting property name", 2, 3),
        (r#""a\q""#, "Bad escaped character", 1, 4),
        ("\"a\nb\"", "Bad control character in string literal", 1, 3),
        ("01", "Unexpected number", 1, 1),
        ("1.", "Unterminated fractional number", 1, 3),
        ("1e+", "Exponent part is missing a number", 1, 4),
        ("{\"a\":\0}", "unexpected token: ''", 1, 6),
        ("-é", "Unexpected token '�'", 1, 2),
        ("true false", "unexpected data at the end", 1, 6),
        ("{} \"unterminated", "Unexpected end of JSON input", 1, 4),
        ("\"😀\" x", "unexpected data at the end", 1, 5),
    ];

    for (source, expected_message, expected_line, expected_column) in cases {
        assert_json_module_syntax_location(
            source,
            expected_message,
            expected_line,
            expected_column,
        );
    }
}

#[test]
fn json_parse_prepends_pinned_input_location_to_the_active_backtrace() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let result = context
        .eval_with_filename(
            r#"
                (function authored() {
                    try {
                        JSON.parse('\n"  \\x"');
                    } catch (error) {
                        return [
                            error.name,
                            error.message,
                            error.fileName,
                            error.lineNumber,
                            error.columnNumber,
                            error.stack
                        ];
                    }
                })()
            "#,
            "json-callsite.js",
        )
        .unwrap();
    let Value::Object(result) = result else {
        panic!("JSON.parse diagnostic probe did not return its result array");
    };

    for (index, expected) in [
        Value::String(JsString::from_static("SyntaxError")),
        Value::String(JsString::from_static("Bad escaped character")),
        Value::String(JsString::from_static("<input>")),
        Value::Int(2),
        Value::Int(5),
    ]
    .into_iter()
    .enumerate()
    {
        let key = runtime.intern_property_key(&index.to_string()).unwrap();
        assert_eq!(context.get_property(&result, &key).unwrap(), expected);
    }

    let stack_key = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Literal5)
        .unwrap();
    let Value::String(stack) = context.get_property(&result, &stack_key).unwrap() else {
        panic!("JSON.parse SyntaxError stack was not a string");
    };
    let stack = stack.to_string();
    assert!(
        stack.starts_with("    at <input>:2:5\n    at parse (native)\n"),
        "JSON.parse stack lost its pinned synthetic source frame: {stack:?}",
    );
    assert!(
        stack.contains("    at authored (json-callsite.js:"),
        "JSON.parse stack lost its authored caller: {stack:?}",
    );
}

#[test]
fn quickjs_extended_json_module_parser_reports_pinned_negative_boundaries() {
    let cases = [
        ("{é: 1}", "unexpected character", 1, 2),
        ("{aé: 1}", "unexpected character", 1, 3),
        ("{a:\0}", "unexpected token: ''", 1, 4),
        ("{\"value\": 1.}", "Unterminated fractional number", 1, 13),
        (
            "{'value':'left\\\rright'}\n",
            "Bad escaped character",
            1,
            16,
        ),
        (
            "{'value':'left\\\r\nright'}\n",
            "Bad escaped character",
            1,
            16,
        ),
        ("0x", "Unexpected token '", 1, 3),
        ("+0o", "Unexpected token '", 1, 4),
        ("-0b", "Unexpected token '", 1, 4),
        ("{a:0xé}", "Unexpected token '�'", 1, 6),
        ("{a:+é}", "Unexpected token '�'", 1, 5),
        ("[1 2.]", "Unterminated fractional number", 1, 6),
    ];

    for (source, expected_message, expected_line, expected_column) in cases {
        assert_json5_module_syntax_location(
            source,
            expected_message,
            expected_line,
            expected_column,
        );
    }
}

fn assert_json_module_syntax_location(
    source: &str,
    expected_message: &str,
    expected_line: i32,
    expected_column: i32,
) {
    assert_json_module_syntax_location_with_mode(
        source,
        expected_message,
        expected_line,
        expected_column,
        false,
    );
}

#[test]
fn json_parse_nesting_cutoff_matches_the_pinned_stack_budget() {
    // Pinned QuickJS derives its JSON nesting ceiling from its one-MiB C
    // stack: this try-wrapped top-level calibration call accepts 10,893 nested
    // containers around a leaf and rejects the next value with a catchable
    // SyntaxError. Empty containers reach one less value-entry depth, so they
    // survive one level
    // further. These exact clean-top-level counts are pinned here; the
    // remaining call-depth difference is recorded in docs/deviations.md.
    let runtime = Runtime::new();
    let mut context = runtime.new_context();

    let mut outcome = |depth: usize, shape: &str| -> String {
        let script = match shape {
            "array-leaf" => format!(
                r#"
                    try {{
                        JSON.parse("[".repeat({depth}) + "1" + "]".repeat({depth}));
                        "ok";
                    }} catch (error) {{
                        error.name + ":" + error.message;
                    }}
                "#
            ),
            "array-empty" => format!(
                r#"
                    try {{
                        JSON.parse("[".repeat({depth}) + "]".repeat({depth}));
                        "ok";
                    }} catch (error) {{
                        error.name + ":" + error.message;
                    }}
                "#
            ),
            "object-leaf" => format!(
                r#"
                    try {{
                        JSON.parse('{{"a":'.repeat({depth}) + "1" + "}}".repeat({depth}));
                        "ok";
                    }} catch (error) {{
                        error.name + ":" + error.message;
                    }}
                "#
            ),
            other => panic!("unknown shape {other}"),
        };
        match context.eval(&script).unwrap() {
            Value::String(text) => text.to_utf8_lossy(),
            other => panic!("expected string outcome, got {other:?}"),
        }
    };

    assert_eq!(outcome(10_893, "array-leaf"), "ok");
    assert_eq!(outcome(10_894, "array-leaf"), "SyntaxError:stack overflow");
    assert_eq!(outcome(10_894, "array-empty"), "ok");
    assert_eq!(outcome(10_895, "array-empty"), "SyntaxError:stack overflow");

    // Objects share the array frame shape and therefore the same cutoff.
    assert_eq!(outcome(10_893, "object-leaf"), "ok");
    assert_eq!(outcome(10_894, "object-leaf"), "SyntaxError:stack overflow");

    // The descent is iterative: far beyond the ceiling it still throws a
    // catchable SyntaxError rather than overflowing the host thread stack
    // and aborting the process (the S3 robustness contract).
    for depth in [100_000_usize, 1_000_000] {
        assert_eq!(outcome(depth, "array-leaf"), "SyntaxError:stack overflow");
        assert_eq!(outcome(depth, "object-leaf"), "SyntaxError:stack overflow");
    }
}

#[test]
fn json_parse_caller_shapes_keep_the_documented_fixed_logical_budget() {
    // Pinned QuickJS's native stack budget shifts with these caller frames;
    // Oxide's fixed logical budget deliberately does not. Keep the Oxide side
    // of the open JSON-STACK-BUDGET-001 frontier stable and explicit.
    for (depth, accepted) in [(10_893, true), (10_894, false)] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let script = format!(
            r#"
                var text = "[".repeat({depth}) + "1" + "]".repeat({depth});
                JSON.parse(text);
            "#
        );
        let result = context.eval(&script);
        assert_eq!(
            result.is_ok(),
            accepted,
            "bare top-level call differed at depth {depth}",
        );
        if !accepted {
            assert!(matches!(result, Err(RuntimeError::Exception)));
            let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
                panic!("bare top-level failure was not an Error object");
            };
            assert_json_error_fields(
                &runtime,
                &mut context,
                &error,
                "stack overflow",
                1,
                10_895,
                "bare top-level call",
            );
        }
    }

    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    for (label, call) in [
        (
            "one JS frame",
            "(function () { return JSON.parse(text); })()",
        ),
        ("Function.prototype.call", "JSON.parse.call(JSON, text)"),
    ] {
        for (depth, expected) in [
            (10_886, "ok"),
            (10_887, "ok"),
            (10_893, "ok"),
            (10_894, "SyntaxError:stack overflow"),
        ] {
            let script = format!(
                r#"
                    var text = "[".repeat({depth}) + "1" + "]".repeat({depth});
                    try {{
                        {call};
                        "ok";
                    }} catch (error) {{
                        error.name + ":" + error.message;
                    }}
                "#
            );
            assert_eq!(
                context.eval(&script).unwrap(),
                Value::String(JsString::try_from_utf8(expected).unwrap()),
                "{label} differed at depth {depth}",
            );
        }
    }
}

#[test]
fn json_parse_reviver_nesting_cutoff_matches_pinned_and_stays_catchable() {
    // The pinned reviver walks post-order and checks its stack budget on
    // entering `internalize_json_property`, so the ceiling is shallower
    // (4,085 returns, 4,086 throws InternalError) and the exception kind is
    // InternalError rather than SyntaxError.
    let runtime = Runtime::new();
    let mut context = runtime.new_context();

    let mut outcome = |depth: usize| -> String {
        let script = format!(
            r#"
                try {{
                    JSON.parse(
                        "[".repeat({depth}) + "1" + "]".repeat({depth}),
                        function (key, value) {{ return value; }},
                    );
                    "ok";
                }} catch (error) {{
                    error.name + ":" + error.message;
                }}
            "#
        );
        match context.eval(&script).unwrap() {
            Value::String(text) => text.to_utf8_lossy(),
            other => panic!("expected string outcome, got {other:?}"),
        }
    };

    assert_eq!(outcome(4_085), "ok");
    assert_eq!(outcome(4_086), "InternalError:stack overflow");
    // Beyond the parse ceiling the parser trips first, still catchably and
    // without aborting on the host stack.
    for depth in [100_000_usize, 1_000_000] {
        assert_eq!(outcome(depth), "SyntaxError:stack overflow");
    }

    // A deep, in-budget walk stays post-order and hands the deepest leaf its
    // pinned `context.source` slice, proving the iterative frame's owned
    // parse records line up by array position.
    let source_check = context
        .eval(
            r#"
                (function () {
                    var deepest = "none";
                    JSON.parse(
                        "[".repeat(4000) + "1" + "]".repeat(4000),
                        function (key, value, context) {
                            if (Object.prototype.hasOwnProperty.call(context, "source")) {
                                deepest = context.source;
                            }
                            return value;
                        },
                    );
                    return deepest;
                })()
            "#,
        )
        .unwrap();
    assert_eq!(source_check, Value::String(JsString::from_static("1")));
}

#[test]
fn json_parse_reviver_kinds_keep_the_documented_fixed_logical_budget() {
    // QuickJS's remaining native stack differs by callable kind. Oxide's
    // resumable walk uses one documented logical ceiling for bytecode, native,
    // and bound revivers; lock down all three sides of that open frontier.
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    for (label, reviver, cases) in [
        (
            "bytecode",
            "function (key, value) { return value; }",
            &[(4_085, "ok"), (4_086, "InternalError:stack overflow")][..],
        ),
        (
            "native",
            "Boolean",
            &[
                (4_082, "ok"),
                (4_083, "ok"),
                (4_085, "ok"),
                (4_086, "InternalError:stack overflow"),
            ][..],
        ),
        (
            "bound",
            "(function (key, value) { return value; }).bind(null)",
            &[
                (4_081, "ok"),
                (4_082, "ok"),
                (4_085, "ok"),
                (4_086, "InternalError:stack overflow"),
            ][..],
        ),
    ] {
        for &(depth, expected) in cases {
            let script = format!(
                r#"
                    try {{
                        JSON.parse(
                            "[".repeat({depth}) + "1" + "]".repeat({depth}),
                            {reviver},
                        );
                        "ok";
                    }} catch (error) {{
                        error.name + ":" + error.message;
                    }}
                "#
            );
            assert_eq!(
                context.eval(&script).unwrap(),
                Value::String(JsString::try_from_utf8(expected).unwrap()),
                "{label} reviver differed at depth {depth}",
            );
        }
    }
}

#[test]
fn json_parse_stack_overflow_reports_the_pinned_token_column() {
    // The pinned check runs while advancing the nested value's token, so the
    // diagnostic column points at the leaf token (n + 1 on a one-line array
    // chain) rather than the end of input.
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let result = context.eval(
        r#"
            try {
                JSON.parse("[".repeat(10894) + "1" + "]".repeat(10894));
                "missing";
            } catch (error) {
                error.name + ":" + error.columnNumber;
            }
        "#,
    );
    assert_eq!(
        result.unwrap(),
        Value::String(JsString::from_static("SyntaxError:10895")),
    );
}

#[test]
fn json_parse_boundary_preserves_pinned_malformed_token_diagnostics() {
    // `json_next_token` lexes the nested token in the enclosing recursive
    // frame. At the first over-budget value depth, malformed input therefore
    // wins over the stack check that a successfully lexed leaf would hit.
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let cases = [
        (
            "array EOF",
            "[".repeat(10_894) + "\n",
            "Unexpected end of JSON input",
            2,
            1,
        ),
        (
            "identifier",
            "[".repeat(10_894) + "x",
            "unexpected token: 'x'",
            1,
            10_895,
        ),
        (
            "bad string",
            "[".repeat(10_894) + "\"ab\n",
            "Bad control character in string literal",
            1,
            10_898,
        ),
        (
            "closing brace",
            "[".repeat(10_894) + "}",
            "unexpected token: '}'",
            1,
            10_895,
        ),
        (
            "truncated true",
            "[".repeat(10_894) + "tru]",
            "unexpected token: 'tru'",
            1,
            10_895,
        ),
        (
            "truncated null",
            "[".repeat(10_894) + "nul",
            "unexpected token: 'nul'",
            1,
            10_895,
        ),
        (
            "minus",
            "[".repeat(10_894) + "-\n",
            "Unexpected token '\n'",
            1,
            10_896,
        ),
        (
            "fraction",
            "[".repeat(10_894) + "1.",
            "Unterminated fractional number",
            1,
            10_897,
        ),
        (
            "comma",
            "[".repeat(10_894) + ",",
            "unexpected token: ','",
            1,
            10_895,
        ),
        (
            "non-ASCII",
            "[".repeat(10_894) + "é",
            "unexpected character",
            1,
            10_895,
        ),
        (
            "object identifier",
            "{\"a\":".repeat(10_894) + "x",
            "unexpected token: 'x'",
            1,
            54_471,
        ),
        (
            "object EOF",
            "{\"a\":".repeat(10_894),
            "Unexpected end of JSON input",
            1,
            54_471,
        ),
    ];

    for (label, source, message, line, column) in cases {
        let source = JsString::try_from_utf8(&source).unwrap();
        let NativeConversion::Throw(Value::Object(error)) = runtime
            .parse_json_text(context.realm, &source, false)
            .unwrap()
        else {
            panic!("{label} did not throw a SyntaxError");
        };
        assert_json_error_fields(&runtime, &mut context, &error, message, line, column, label);
    }
}

#[test]
fn json_module_nesting_cutoff_matches_the_pinned_module_budget() {
    fn parse_module(
        runtime: &Runtime,
        realm: ContextId,
        depth: usize,
        bytes: bool,
        extended: bool,
    ) -> NativeConversion<Value> {
        let source = "[".repeat(depth) + "0" + &"]".repeat(depth);
        let filename = JsString::from_static("fixtures/deep.json");
        if bytes {
            if extended {
                runtime.parse_json5_module_bytes(realm, source.as_bytes(), &filename)
            } else {
                runtime.parse_json_module_bytes(realm, source.as_bytes(), &filename)
            }
        } else {
            let source = JsString::try_from_utf8(&source).unwrap();
            if extended {
                runtime.parse_json5_module_text(realm, &source, &filename)
            } else {
                runtime.parse_json_module_text(realm, &source, &filename)
            }
        }
        .unwrap()
    }

    // JSON modules reach `JS_ParseJSON` through a shallower pinned C call
    // path than JSON.parse. Pinned QuickJS therefore accepts 10,911 nested
    // containers and rejects 10,912, eighteen levels beyond JSON.parse.
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    for bytes in [false, true] {
        for extended in [false, true] {
            for depth in [10_894, 10_911] {
                assert!(
                    matches!(
                        parse_module(&runtime, context.realm, depth, bytes, extended),
                        NativeConversion::Value(_)
                    ),
                    "module rejected depth {depth} (bytes={bytes}, extended={extended})",
                );
            }

            let NativeConversion::Throw(Value::Object(error)) =
                parse_module(&runtime, context.realm, 10_912, bytes, extended)
            else {
                panic!("module accepted depth 10,912 (bytes={bytes}, extended={extended})");
            };
            for (name, expected) in [
                ("name", Value::String(JsString::from_static("SyntaxError"))),
                (
                    "message",
                    Value::String(JsString::from_static("stack overflow")),
                ),
                ("columnNumber", Value::Int(10_913)),
            ] {
                let key = runtime.intern_property_key(name).unwrap();
                assert_eq!(
                    context.get_property(&error, &key).unwrap(),
                    expected,
                    "{name} differed (bytes={bytes}, extended={extended})",
                );
            }
        }
    }
}

#[test]
fn json_module_boundary_preserves_pinned_malformed_token_diagnostics() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let filename = JsString::from_static("fixtures/deep.json");
    for (label, suffix, message) in [
        ("module identifier", "x", "unexpected token: 'x'"),
        ("module EOF", "", "Unexpected end of JSON input"),
    ] {
        let source = "[".repeat(10_912) + suffix;
        let source = JsString::try_from_utf8(&source).unwrap();
        let NativeConversion::Throw(Value::Object(error)) = runtime
            .parse_json_module_text(context.realm, &source, &filename)
            .unwrap()
        else {
            panic!("{label} did not throw a SyntaxError");
        };
        assert_json_error_fields(&runtime, &mut context, &error, message, 1, 10_913, label);
    }
}

fn assert_json5_module_syntax_location(
    source: &str,
    expected_message: &str,
    expected_line: i32,
    expected_column: i32,
) {
    assert_json_module_syntax_location_with_mode(
        source,
        expected_message,
        expected_line,
        expected_column,
        true,
    );
}

fn assert_json_module_syntax_location_with_mode(
    source: &str,
    expected_message: &str,
    expected_line: i32,
    expected_column: i32,
    extended: bool,
) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let source = JsString::try_from_utf8(source).unwrap();
    let filename = JsString::from_static("fixtures/value.json");
    let parsed = if extended {
        runtime.parse_json5_module_text(context.realm, &source, &filename)
    } else {
        runtime.parse_json_module_text(context.realm, &source, &filename)
    };
    let NativeConversion::Throw(Value::Object(error)) = parsed.unwrap() else {
        panic!("invalid JSON module text did not throw a SyntaxError");
    };

    for (name, expected) in [
        (
            "message",
            Value::String(JsString::try_from_utf8(expected_message).unwrap()),
        ),
        ("fileName", Value::String(filename.clone())),
        ("lineNumber", Value::Int(expected_line)),
        ("columnNumber", Value::Int(expected_column)),
        (
            "stack",
            Value::String(
                JsString::try_from_utf8(&format!(
                    "    at fixtures/value.json:{expected_line}:{expected_column}\n"
                ))
                .unwrap(),
            ),
        ),
    ] {
        let key = runtime.intern_property_key(name).unwrap();
        assert_eq!(
            context.get_property(&error, &key).unwrap(),
            expected,
            "{name} differed for {source:?}",
        );
    }
}

fn assert_json_error_fields(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    error: &ObjectRef,
    expected_message: &str,
    expected_line: i32,
    expected_column: i32,
    label: &str,
) {
    for (name, expected) in [
        ("name", Value::String(JsString::from_static("SyntaxError"))),
        (
            "message",
            Value::String(JsString::try_from_utf8(expected_message).unwrap()),
        ),
        ("lineNumber", Value::Int(expected_line)),
        ("columnNumber", Value::Int(expected_column)),
    ] {
        let key = runtime.intern_property_key(name).unwrap();
        assert_eq!(
            context.get_property(error, &key).unwrap(),
            expected,
            "{name} differed for {label}",
        );
    }
}
