//! The parser against the stack-code front end: every source both accept,
//! or both reject with the same message at the same position.

use super::Parser;
use crate::engine::api::error::Error;
use crate::engine::ast::ProgramKind;
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::compiler::{
    compile_unlinked_module_bytes_with_name, compile_unlinked_script_with_filename,
};
use crate::engine::value::JsString;

/// The old front end's verdict: `Ok` or the error's message and offset.
fn old(source: &str, kind: ProgramKind) -> Result<(), (String, Option<usize>)> {
    let result = match kind {
        ProgramKind::Module => compile_unlinked_module_bytes_with_name(
            source.as_bytes(),
            JsString::from_static("m.js"),
            DebugInfoMode::Full,
        )
        .map(drop),
        _ => compile_unlinked_script_with_filename(source, "s.js", DebugInfoMode::Full).map(drop),
    };
    result.map_err(|error| describe(&error))
}

fn new(source: &str, kind: ProgramKind) -> Result<(), (String, Option<usize>)> {
    Parser::parse_str(source, kind)
        .map(drop)
        .map_err(|error| describe(&error))
}

fn describe(error: &Error) -> (String, Option<usize>) {
    (
        error.message().to_string(),
        error.span().map(|span| span.start.byte_offset),
    )
}

fn agree(source: &str, kind: ProgramKind) -> Result<(), String> {
    let (old, new) = (old(source, kind), new(source, kind));
    if old == new {
        return Ok(());
    }
    Err(format!("old {old:?}\nnew {new:?}"))
}

#[track_caller]
fn assert_agree(source: &str) {
    if let Err(difference) = agree(source, ProgramKind::Script) {
        panic!("{source:?}\n{difference}");
    }
}

#[test]
fn ordinary_scripts_parse() {
    for source in [
        "var a = 1, b; let c = a + b * 2; const d = c ?? 0;",
        "function f(a, b = 1, ...c) { 'use strict'; return a + b + c.length; }",
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
        "new A; new A(1); new new A()(); new.target;",
        "(a, b) => a + b; async (x) => await x; x => ({});",
        "if (a) function q() {} else function r() {}",
        "o = { get a() { return 1; }, set a(v) {}, async *m() {}, [k]: 1, __proto__: null, 'x': 1, 2: 3 };",
        "with (o) { x; }",
        "delete o.p; typeof missing; void 0; -x ** 2;",
    ] {
        assert_agree(source);
    }
}

#[test]
fn early_errors_match() {
    for source in [
        "let a; let a;",
        "let a; var a;",
        "var a; let a;",
        "const a;",
        "function f(a, a) { 'use strict'; }",
        "(a, a) => 1;",
        "'use strict'; with (o) {}",
        "break;",
        "continue;",
        "a: a: ;",
        "x = a?.b = 1;",
        "1 = 2;",
        "a ?? b || c;",
        "-x ** 2;",
        "class A { constructor() {} constructor() {} }",
        "class A { #x; #x; }",
        "function f() { super(); }",
        "({ a = 1 });",
        "for (let x = 1 of y) ;",
        "for await (x of y) ;",
        "'use strict'; delete x;",
        "'use strict'; eval = 1;",
        "new.target;",
        "return;",
        "yield 1; function* g() { yield = 1 }",
        "let [a] ;",
        "class { }",
        "if (1) let x = 1;",
        "while (1) function f() {}",
        "`\\u{`",
        "a\n++b",
        "({ __proto__: 1, __proto__: 2 });",
        "async function f() { function g(a = await 1) {} }",
        "try {} catch (e) { let e; }",
        "function f(a = 1) { 'use strict'; }",
        "label: { label: ; }",
        "for (;;) { break missing; }",
    ] {
        assert_agree(source);
    }
}

/// Walks `OXIDE_PARSE_CORPUS` (a directory of `.js` files, Test262's
/// frontmatter honored) and reports every disagreement.
#[test]
#[ignore = "set OXIDE_PARSE_CORPUS to a source tree"]
fn corpus_agrees() {
    let Ok(root) = std::env::var("OXIDE_PARSE_CORPUS") else {
        return;
    };
    let mut files = Vec::new();
    let mut pending = vec![std::path::PathBuf::from(root)];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("readable directory") {
                pending.push(entry.expect("directory entry").path());
            }
        } else if path.extension().is_some_and(|ext| ext == "js")
            && !path.to_string_lossy().contains("_FIXTURE")
        {
            files.push(path);
        }
    }
    files.sort();
    let limit: usize = std::env::var("OXIDE_PARSE_LIMIT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(40);
    let mut checked = 0;
    let mut failures = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let module = text.contains("flags: [module]")
            || text.contains("flags: [module,")
            || text.contains(", module]");
        let strict = text.contains("onlyStrict");
        let raw = text.contains("flags: [raw]");
        let source = if strict && !raw && !module {
            format!("'use strict';\n{text}")
        } else {
            text
        };
        let kind = if module {
            ProgramKind::Module
        } else {
            ProgramKind::Script
        };
        let outcome = std::panic::catch_unwind(|| agree(&source, kind));
        checked += 1;
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(difference)) => failures.push(format!("{}\n{difference}", path.display())),
            Err(_) => failures.push(format!("{}\npanicked", path.display())),
        }
    }
    for failure in failures.iter().take(limit) {
        eprintln!("{failure}\n");
    }
    eprintln!("{checked} files, {} disagreements", failures.len());
    assert!(failures.is_empty());
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
        std::mem::size_of::<crate::engine::compiler::lexer::Span>(),
        std::mem::size_of::<Result<Expr, Error>>(),
    );
}
