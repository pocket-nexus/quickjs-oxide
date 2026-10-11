use super::*;

/// Early errors whose message and position follow pinned QuickJS: the name
/// checks that QuickJS runs after advancing past a token report the token
/// that follows it.
#[test]
fn early_error_messages_and_positions_follow_quickjs() {
    for (module, source, message, line, column) in [
        (
            false,
            "\"use strict\";07;",
            "octal literals are deprecated in strict mode",
            1,
            14,
        ),
        (false, "/*/", "unexpected end of comment", 1, 1),
        (
            false,
            "for(\n    false\n) {\n  break;\n}",
            "invalid for in/of left hand-side",
            3,
            1,
        ),
        (
            false,
            "for(\n) {\n  break;\n}",
            "unexpected token in expression: ')'",
            2,
            1,
        ),
        (
            false,
            "for (var x o\\u0066 []) ;",
            "expected 'of' or 'in' in for control expression",
            1,
            12,
        ),
        (
            false,
            "for (const let in {}) {}",
            "invalid lexical variable name",
            1,
            16,
        ),
        (
            false,
            "var af = enum => 1;",
            "unexpected token in expression: 'enum'",
            1,
            10,
        ),
        (
            false,
            "\"use strict\";var af = arguments => 1;",
            "invalid argument name in strict code",
            1,
            36,
        ),
        (
            false,
            "var af = (eval) => { \"use strict\"; };",
            "invalid argument name in strict code",
            1,
            22,
        ),
        (
            false,
            "{ async function f() {} class f {} }",
            "invalid redefinition of lexical identifier",
            1,
            36,
        ),
        (
            false,
            "{ var f; class f {} }",
            "invalid redefinition of global identifier",
            1,
            21,
        ),
        (
            false,
            "var f = () => super.x;",
            "'super' is only valid in a method",
            1,
            20,
        ),
        (false, "var f = super;", "invalid use of 'super'", 1, 14),
        (
            false,
            "var af = (x, [x]) => 1;",
            "duplicate parameter names not allowed in this context",
            1,
            15,
        ),
        (
            false,
            "function g(x, {a: x}) {}",
            "duplicate parameter names not allowed in this context",
            1,
            19,
        ),
        (
            false,
            "function g(x, {...x}) {}",
            "duplicate parameter names not allowed in this context",
            1,
            19,
        ),
        (
            true,
            "let f; let f;",
            "invalid redefinition of global identifier",
            1,
            13,
        ),
        (
            true,
            "let f; class f {}",
            "invalid redefinition of global identifier",
            1,
            18,
        ),
    ] {
        let error = if module {
            compile_unlinked_module_with_filename(source, "t.mjs", DebugInfoMode::StripDebug)
                .map(|_| ())
                .unwrap_err()
        } else {
            compile_unlinked_script(source).map(|_| ()).unwrap_err()
        };
        assert_eq!(error.kind(), ErrorKind::Syntax, "{source}");
        assert_eq!(error.message(), message, "{source}");
        let span = error.span().expect("syntax error lost its source span");
        assert_eq!(
            (span.start.line, span.start.column),
            (line, column),
            "{source}"
        );
    }
}
