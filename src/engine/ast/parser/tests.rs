//! The parser's verdicts: sources it accepts, and early errors with
//! QuickJS's message at QuickJS's position.

use super::Parser;
use crate::engine::api::error::Error;
use crate::engine::ast::ProgramKind;

fn new(source: &str, kind: ProgramKind) -> Result<(), (String, Option<usize>)> {
    let program = Parser::parse_str(source, kind).map_err(|error| describe(&error))?;
    if program.has_with {
        return Ok(());
    }
    let built =
        crate::engine::hir::build::build(program, "s.js").map_err(|error| describe(&error))?;
    check_hir(&built.program);
    Ok(())
}

/// The HIR invariants: it validates, its binary form round-trips byte for
/// byte, and stripping type information changes nothing.
fn check_hir(program: &crate::engine::hir::schema::Program) {
    use crate::engine::hir::schema::{read, strip, write};
    if let Err(error) = crate::engine::hir::validate::validate(program) {
        panic!("{error}");
    }
    let bytes = write(program);
    let back = read(&bytes).expect("the HIR reads back");
    assert!(write(&back) == bytes, "the HIR does not round-trip");
    let mut stripped = back;
    strip(&mut stripped);
    assert!(write(&stripped) == bytes, "strip changed JavaScript HIR");
}

fn describe(error: &Error) -> (String, Option<usize>) {
    (
        error.message().to_string(),
        error.span().map(|span| span.start.byte_offset),
    )
}

#[test]
fn ordinary_scripts_parse() {
    for source in [
        "var a = 1, b; let c = a + b * 2; const d = c ?? 0;",
        "function f(a, b = 1, ...c) { return a + b + c.length; }",
        "label: for (let i = 0; i < 3; i++) { if (i) continue label; else break label; }",
        "for (const x of [1, 2]) ; for (var k in {}) ; for (x in y) ;",
        "class A extends B { #x = 1; static y; constructor() { super(); } get z() { return this.#x; } static { } }",
        "({ a, b: [c, d = 2], ...e } = f);",
        "async function g() { await 1; for await (const x of y) ; }",
        "function* h() { yield; yield* [1]; }",
        "x = a?.b.c?.(d)?.[e];",
        "try { throw 1 } catch ({ message }) { } finally { }",
        "switch (x) { case 1: case 2: f(); break; default: g(); }",
        "`a${b}c${d}`; tag`x${y}`;",
        "/re/g.test(s); a = b / c;",
        "new A; new A(1); new new A()(); function t() { return new.target; }",
        "(a, b) => a + b; async (x) => await x; x => ({});",
        "if (a) function q() {} else function r() {}",
        "o = { get a() { return 1; }, set a(v) {}, async *m() {}, [k]: 1, __proto__: null, 'x': 1, 2: 3 };",
        "with (o) { x; }",
        "delete o.p; typeof missing; void 0; (-x) ** 2;",
    ] {
        assert_eq!(new(source, ProgramKind::Script), Ok(()), "{source:?}");
    }
}

/// Each source with QuickJS's verdict: `None` when it parses, else the
/// early error's message and byte offset.
#[test]
fn early_errors_match_quickjs() {
    for (source, expected) in [
        (
            "let a; let a;",
            Some(("invalid redefinition of lexical identifier", Some(12))),
        ),
        (
            "let a; var a;",
            Some(("invalid redefinition of lexical identifier", Some(12))),
        ),
        (
            "var a; let a;",
            Some(("invalid redefinition of global identifier", Some(12))),
        ),
        (
            "const a;",
            Some(("missing initializer for const variable", Some(7))),
        ),
        (
            "function f(a, a) { 'use strict'; }",
            Some((
                "duplicate argument names not allowed in this context",
                Some(19),
            )),
        ),
        (
            "(a, a) => 1;",
            Some((
                "duplicate argument names not allowed in this context",
                Some(10),
            )),
        ),
        (
            "'use strict'; with (o) {}",
            Some(("invalid keyword: with", Some(14))),
        ),
        (
            "break;",
            Some(("break must be inside loop or switch", Some(5))),
        ),
        ("continue;", Some(("continue must be inside loop", Some(8)))),
        ("a: a: ;", Some(("duplicate label name", Some(3)))),
        (
            "x = a?.b = 1;",
            Some(("invalid assignment left-hand side", Some(11))),
        ),
        (
            "1 = 2;",
            Some(("invalid assignment left-hand side", Some(4))),
        ),
        ("a ?? b || c;", Some(("expecting ';'", Some(7)))),
        (
            "-x ** 2;",
            Some((
                "unparenthesized unary expression can't appear on the left-hand side of '**'",
                None,
            )),
        ),
        (
            "class A { constructor() {} constructor() {} }",
            Some(("property constructor appears more than once", Some(27))),
        ),
        (
            "class A { #x; #x; }",
            Some(("private class field is already defined", Some(16))),
        ),
        (
            "function f() { super(); }",
            Some((
                "super() is only valid in a derived class constructor",
                Some(20),
            )),
        ),
        ("({ a = 1 });", Some(("expecting '}'", Some(5)))),
        (
            "for (let x = 1 of y) ;",
            Some((
                "a declaration in the head of a for-of loop can't have an initializer",
                Some(15),
            )),
        ),
        ("for await (x of y) ;", Some(("expecting '('", Some(4)))),
        (
            "'use strict'; delete x;",
            Some(("cannot delete a direct reference in strict mode", Some(22))),
        ),
        (
            "'use strict'; eval = 1;",
            Some(("invalid lvalue in strict mode", Some(21))),
        ),
        (
            "new.target;",
            Some(("new.target only allowed within functions", Some(0))),
        ),
        ("return;", Some(("return not in a function", Some(0)))),
        (
            "yield 1; function* g() { yield = 1 }",
            Some(("expecting ';'", Some(6))),
        ),
        ("let [a] ;", Some(("variable name expected", Some(4)))),
        (
            "class { }",
            Some(("class statement requires a name", Some(6))),
        ),
        (
            "if (1) let x = 1;",
            Some((
                "lexical declarations can't appear in single-statement context",
                Some(7),
            )),
        ),
        (
            "while (1) function f() {}",
            Some((
                "function declarations can't appear in single-statement context",
                Some(10),
            )),
        ),
        (
            "`\\u{`",
            Some(("malformed escape sequence in string literal", Some(1))),
        ),
        ("a\n++b", None),
        (
            "({ __proto__: 1, __proto__: 2 });",
            Some(("duplicate __proto__ property name", Some(17))),
        ),
        (
            "async function f() { function g(a = await 1) {} }",
            Some(("expecting ','", Some(42))),
        ),
        (
            "try {} catch (e) { let e; }",
            Some(("invalid redefinition of lexical identifier", Some(24))),
        ),
        (
            "function f(a = 1) { 'use strict'; }",
            Some((
                "\"use strict\" not allowed in function with default or destructuring parameter",
                Some(20),
            )),
        ),
        (
            "label: { label: ; }",
            Some(("duplicate label name", Some(9))),
        ),
        (
            "for (;;) { break missing; }",
            Some(("break/continue label not found", Some(17))),
        ),
    ] {
        let expected =
            expected.map(|(message, offset): (&str, Option<usize>)| (message.to_owned(), offset));
        assert_eq!(
            new(source, ProgramKind::Script).err(),
            expected,
            "{source:?}"
        );
    }
}

#[test]
fn node_sizes() {
    use crate::engine::ast::*;
    eprintln!(
        "Expr {} ExprKind {} Stmt {} StmtKind {} Pattern {} Function {} PropKey {} Span {} Result<Expr> {}",
        std::mem::size_of::<Expr>(),
        std::mem::size_of::<ExprKind>(),
        std::mem::size_of::<Stmt>(),
        std::mem::size_of::<StmtKind>(),
        std::mem::size_of::<Pattern>(),
        std::mem::size_of::<Function>(),
        std::mem::size_of::<PropKey>(),
        std::mem::size_of::<crate::engine::ast::lexer::Span>(),
        std::mem::size_of::<Result<Expr, Error>>(),
    );
}
