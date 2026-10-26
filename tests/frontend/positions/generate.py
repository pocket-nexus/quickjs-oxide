#!/usr/bin/env python3
"""Writes the position test programs: each case throws from one construct
and prints the error's stack. Each `NAME.out` records the stack QuickJS
prints; `scripts/checks/check-outputs.py` compares every case with it, and
`--update` rewrites the records after a deliberate change.

    python3 tests/frontend/positions/generate.py
    python3 scripts/checks/check-outputs.py --qjs target/release/qjs tests/frontend/positions
"""

import os

HERE = os.path.dirname(os.path.abspath(__file__))

# (name, setup, body): `body` runs inside a function `f`, called from `g`.
CASES = [
    ("member-undefined", "var u;", "return u.x;"),
    ("member-chain", "var o = {a: {}};", "return o.a.b.c;"),
    ("computed-undefined", "var u, k = 'x';", "return u[k];"),
    ("computed-key-throws", "var o = {}; var k = {toString() { throw new Error('k'); }};", "return o[k];"),
    ("call-undefined", "var u;", "return u();"),
    ("method-missing", "var o = {};", "return o.m();"),
    ("method-chain", "var o = {m() { return 1; }};", "return o.m()();"),
    ("call-args-multiline", "var o = {};", "return o.m(\n  1,\n  2\n);"),
    ("new-undefined", "var U;", "return new U();"),
    ("new-member", "var o = {};", "return new o.C(1);"),
    ("new-no-args", "var o = {};", "return new o.C;"),
    ("spread-call", "var a = [1];", "return u(...a);"),
    ("spread-method", "var o = {}, a = [1];", "return o.m(...a);"),
    ("spread-not-iterable", "function h() {}", "return h(...1);"),
    ("template", "var u;", "return `a${u.x}b`;"),
    ("tagged", "var u;", "return u`x`;"),
    ("tagged-member", "var o = {};", "return o.t`x${1}`;"),
    ("undeclared-read", "", "return missing;"),
    ("undeclared-strict-write", "", "'use strict'; missing = 1;"),
    ("const-assign", "const c = 1;", "c = 2;"),
    ("local-const-assign", "", "const c = 1; c = 2;"),
    ("tdz-read", "", "x; let x = 1;"),
    ("tdz-closure", "", "function h() { return y; } h(); let y;"),
    ("bigint-mix", "", "return 1n + 1;"),
    ("in-primitive", "", "return 'x' in 1;"),
    ("instanceof", "", "return {} instanceof 1;"),
    ("unary-symbol", "", "return +Symbol();"),
    ("neg-symbol", "", "return -Symbol();"),
    ("bitnot-symbol", "", "return ~Symbol();"),
    ("concat-symbol", "", "return Symbol() + '';"),
    ("increment-symbol", "var s = Symbol();", "s++;"),
    ("compound-member", "var o = {};", "o.x.y += 1;"),
    ("update-member", "var u;", "u.x++;"),
    ("compound-computed", "var u, k = 'k';", "u[k] += 1;"),
    ("destructure-array-decl", "", "var [a] = 1;"),
    ("destructure-object-decl", "", "var {a} = null;"),
    ("destructure-let", "", "let [a, b] = {};"),
    ("destructure-assign", "var a;", "[a] = 1;"),
    ("destructure-object-assign", "var a;", "({a} = null);"),
    ("destructure-nested", "", "var {a: {b}} = {};"),
    ("destructure-default-throws", "", "var [a = u.x] = [];"),
    ("destructure-param", "function h({a}) {}", "h();"),
    ("destructure-param-array", "function h([a]) {}", "h(1);"),
    ("for-of-not-iterable", "", "for (var x of 1) {}"),
    ("for-of-body-throws", "var u;", "for (var x of [1]) { u.x; }"),
    ("for-in-body-throws", "var u;", "for (var k in {a: 1}) { u.x; }"),
    ("array-spread", "", "return [...1];"),
    ("class-no-new", "class C {}", "C();"),
    ("derived-this", "class B {} class C extends B { constructor() { this.x = 1; super(); } }", "new C();"),
    ("derived-no-super", "class B {} class C extends B { constructor() {} }", "new C();"),
    ("derived-return", "class B {} class C extends B { constructor() { super(); return 1; } }", "new C();"),
    ("super-twice", "class B {} class C extends B { constructor() { super(); super(); } }", "new C();"),
    ("extends-non-constructor", "", "class C extends 1 {}"),
    ("private-missing", "class C { #x; static get(o) { return o.#x; } }", "C.get({});"),
    ("private-method-missing", "class C { #m() {} static call(o) { return o.#m(); } }", "C.call({});"),
    ("field-init-throws", "var u; class C { x = u.y; }", "new C();"),
    ("static-init-throws", "var u;", "class C { static x = u.y; }"),
    ("static-block-throws", "var u;", "class C { static { u.y; } }"),
    ("getter-throws", "var o = { get x() { throw new Error('g'); } };", "return o.x;"),
    ("setter-throws", "var o = { set x(v) { throw new Error('s'); } };", "o.x = 1;"),
    ("generator-yield-star", "function* gen() { yield* 1; }", "gen().next();"),
    ("generator-throws", "var u; function* gen() { yield 1; u.x; }", "var it = gen(); it.next(); it.next();"),
    ("return-call", "var u;", "return u();"),
    ("return-method", "var o = {};", "return o.m();"),
    ("return-template-call", "var u;", "return `${u.x}`;"),
    ("optional-chain", "var o = {};", "return o?.a.b;"),
    ("optional-call", "var o = {a: {}};", "return o?.a.b();"),
    ("conditional", "var u;", "return u ? 1 : u.x;"),
    ("logical", "var u;", "return u || u.x;"),
    ("nullish", "var u;", "return u ?? u.x;"),
    ("comma", "var u;", "return (1, u.x);"),
    ("switch-disc", "var u;", "switch (u.x) { case 1: }"),
    ("switch-case", "var u;", "switch (1) { case u.x: }"),
    ("throw-statement", "", "throw new Error('t');"),
    ("throw-value", "", "throw 1;"),
    ("nested-calls", "function a() { return b(); } function b() { return c(); } function c() { return u.x; } var u;", "return a();"),
    ("arrow", "var u;", "return (() => u.x)();"),
    ("arrow-nested", "var u;", "return (() => () => u.x)()();"),
    ("closure-capture", "var u; function mk() { var v = u; return function () { return v.x; }; }", "return mk()();"),
    ("eval-code", "var u;", "return eval('u.x');"),
    ("indirect-eval", "var u;", "return (0, eval)('u.x');"),
    ("new-function", "", "return new Function('return u.x')();"),
    ("json-parse", "", "return JSON.parse('{');"),
    ("array-method-callback", "var u;", "return [1].map(function () { return u.x; });"),
    ("object-literal-computed", "var u;", "return { [u.x]: 1 };"),
    ("object-literal-value", "var u;", "return { a: u.x };"),
    ("array-literal-element", "var u;", "return [1, u.x];"),
    ("delete-strict", "", "'use strict'; delete Object.prototype;"),
    ("typeof-tdz", "", "typeof z; let z;"),
    ("call-in-args", "var u; function h() {}", "return h(u.x);"),
    ("multiline-member", "var u;", "return u\n  .x\n  .y;"),
    ("multiline-binary", "", "return 1n\n  +\n  1;"),
    ("multiline-call", "var u;", "return u\n  (\n  );"),
    ("labeled-loop", "var u;", "outer: for (;;) { for (;;) { u.x; break outer; } }"),
    ("do-while", "var u;", "do { u.x; } while (0);"),
    ("while-test", "var u;", "while (u.x) {}"),
    ("for-update", "var u;", "for (var i = 0; i < 1; u.x) {}"),
    ("for-let-closure", "var u, fs = [];", "for (let i = 0; i < 2; i++) fs.push(() => u.x); fs[0]();"),
    ("try-finally", "var u;", "try { u.x; } finally {}"),
    ("finally-throws", "var u;", "try {} finally { u.x; }"),
    ("catch-throws", "var u;", "try { throw 1; } catch (e) { u.x; }"),
    ("async-await", "var u; async function h() { await 0; u.x; }", "h().catch(function (e) { print(e.stack); });"),
    ("async-arrow", "var u;", "(async () => { u.x; })().catch(function (e) { print(e.stack); });"),
    ("async-generator", "var u; async function* h() { yield 1; u.x; }", "var it = h(); it.next(); it.next().catch(function (e) { print(e.stack); });"),
    ("default-param", "var u; function h(a = u.x) {}", "h();"),
    ("rest-param", "var u; function h(...r) { return u.x; }", "h(1);"),
    ("arguments-callee", "", "'use strict'; return arguments.callee;"),
    ("super-property", "class B { m() { return this.u.x; } } class C extends B { m() { return super.m(); } }", "return new C().m();"),
    ("super-call-args", "var u; class B {} class C extends B { constructor() { super(u.x); } }", "new C();"),
    ("regexp-exec", "var u;", "return /a/.exec(u.x);"),
    ("string-method", "", "return ''.repeat(-1);"),
    ("number-method", "", "return (1).toFixed(200);"),
    ("array-length", "", "return new Array(-1);"),
    ("stack-overflow-like", "function r(n) { if (n === 0) return u.x; return r(n - 1); } var u;", "return r(3);"),
]


def program(name, setup, body):
    return f"""// Position case: {name}.
{setup}
function f() {{
  {body}
}}
function g() {{
  return f();
}}
try {{
  g();
}} catch (e) {{
  print(e && e.stack !== undefined ? e.stack : String(e));
}}
"""


def main():
    for index, (name, setup, body) in enumerate(CASES):
        path = os.path.join(HERE, f"{index:03}-{name}.js")
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(program(name, setup, body))
    print(f"{len(CASES)} position cases")


if __name__ == "__main__":
    main()
