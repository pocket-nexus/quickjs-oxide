//! Functions whose operand stack passes the register window
//! (`docs/performance/new-core-phase6-m4b.md`, section 4.3): their deeper
//! temporaries live in spill slots, a call or array past the window builds
//! on scratch registers or an argument array, and records, catches and
//! `finally` returns reached at those depths go through the slots.

use super::run_tests::run_new_core;

/// [`run_new_core`] on a thread with room for the front end's recursion
/// over sources this deep.
fn run_deep(source: String) -> String {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            crate::engine::compiler::stack_guard::declare_thread_stack(256 << 20);
            run_new_core(&source)
        })
        .unwrap()
        .join()
        .unwrap()
}

/// The front end's QuickJS stack bounds: a call of 65,533 arguments and a
/// template of 65,532 substitutions are the largest that compile. Under
/// the stress collector, which collects at every allocation, a shorter
/// length past the window stands for them.
const CALL_BOUND: usize = if cfg!(feature = "gc-stress") {
    2_000
} else {
    65_533
};
const TEMPLATE_BOUND: usize = if cfg!(feature = "gc-stress") {
    2_000
} else {
    65_532
};

/// `0,1,…,9,0,1,…` with `count` elements.
fn digits(count: usize) -> String {
    (0..count)
        .map(|index| (index % 10).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// The digits from 0 up to `count` without the trailing comma, as the
/// leading elements of an array literal that ends in something else.
fn prefix(count: usize) -> String {
    let mut text = digits(count);
    text.push(',');
    text
}

#[test]
fn calls_past_the_window_pass_every_argument() {
    let mut source = String::from(
        "function f() { var s = 0; for (var i = 0; i < arguments.length; i++) \
         s += arguments[i]; return arguments.length + ':' + s; }\n\
         var o = { m: f };\n\
         function C() { this.v = f.apply(null, arguments); }\n",
    );
    let mut expected = String::new();
    for (count, sum) in [(254, 1131), (255, 1135), (256, 1140), (1000, 4500)] {
        let arguments = digits(count);
        source.push_str(&format!(
            "print(f({arguments}), o.m({arguments}), new C({arguments}).v);\n"
        ));
        expected.push_str(&format!("{count}:{sum} {count}:{sum} {count}:{sum}\n"));
    }
    assert_eq!(run_deep(source), expected);
}

/// A call at the front end's bound passes every argument.
#[test]
fn a_call_at_the_front_end_bound_runs() {
    let last = CALL_BOUND - 1;
    let source = format!(
        "var n = 0, s = 0; function f() {{ n = arguments.length; s = arguments[{last}]; }}\n\
         f({});\nprint(n, s);",
        digits(CALL_BOUND)
    );
    assert_eq!(run_deep(source), format!("{CALL_BOUND} {}\n", last % 10));
}

#[test]
fn templates_past_the_window_concatenate_every_substitution() {
    let short = "${1}".repeat(300);
    let long = "${2}".repeat(TEMPLATE_BOUND);
    let last = TEMPLATE_BOUND - 1;
    let source =
        format!("print(`{short}`.length);\nvar s = `{long}`;\nprint(s.length, s[{last}]);");
    assert_eq!(run_deep(source), format!("300\n{TEMPLATE_BOUND} 2\n"));
}

#[test]
fn nesting_past_the_window_evaluates_inside_out() {
    let object = format!("{}1{}", "{a:".repeat(300), "}".repeat(300));
    let array = format!("{}1{}", "[0,".repeat(300), "]".repeat(300));
    let call = format!("{}7{}", "id(0,".repeat(300), ")".repeat(300));
    // Right-associative: every left operand waits on the stack.
    let power = format!("2**{}1", "1**".repeat(999));
    let source = format!(
        "function id(a, b) {{ return b; }}\n\
         print(JSON.stringify({object}).length, JSON.stringify({array}).length, {call}, {power});"
    );
    assert_eq!(run_deep(source), "1801 1201 7 2\n");
}

/// Constructs that write fixed register ranges (iterator records, a
/// destructuring's close on throw, a catch, spread and object spread,
/// classes, optional chains) at depths in the spill scope.
#[test]
fn deep_expressions_keep_records_and_catches_in_slots() {
    let deep = prefix(300);
    let source = format!(
        "var log = [];\n\
         function it(k) {{ var i = 0; return {{ [Symbol.iterator]() {{ return this; }}, \
           next() {{ log.push('n' + k); return {{ value: i++, done: i > 3 }}; }}, \
           return() {{ log.push('r' + k); return {{}}; }} }}; }}\n\
         function g() {{\n\
           var a, b;\n\
           var r = [{deep} ([a, b] = it(1), a + b), (() => {{ try {{ throw 7; }} catch (e) {{ return e; }} }})(), \
             [...'xyz'].length, ({{ ...{{ p: 1 }} }}).p, typeof class {{ static m() {{}} }}, ({{}})?.x?.y];\n\
           var o = {{ set t(v) {{ throw 'thrown'; }} }};\n\
           try {{ var q = [{deep} ([o.t] = it(2))]; }} catch (e) {{ log.push(e); }}\n\
           return r.length + ':' + r.slice(-6) + ':' + log;\n\
         }}\n\
         print(g());"
    );
    assert_eq!(
        run_deep(source),
        "306:1,7,3,1,function,:n1,n1,r1,n2,r2,thrown\n"
    );
}

/// A generator and an async function suspending at depths in the spill
/// scope resume into the slots.
#[test]
fn suspensions_past_the_window_resume_into_slots() {
    let deep = prefix(300);
    let source = format!(
        "function* gen() {{ var r = [{deep} yield 1, yield* [2, 3]]; return r.slice(-2).join(); }}\n\
         var it = gen();\n\
         print(it.next().value, it.next('a').value, it.next().value, JSON.stringify(it.next('b')));\n\
         var thrown = gen(); thrown.next();\n\
         try {{ thrown.throw(new Error('in')); }} catch (e) {{ print(e.message); }}\n\
         var returned = gen(); returned.next(); print(JSON.stringify(returned.return(9)));\n\
         async function f() {{ var r = [{deep} await 1, await Promise.resolve(2)]; \
           for await (var v of [3, 4]) r.push(v); return r.slice(-4).join(); }}\n\
         f().then(v => print('async', v));"
    );
    assert_eq!(
        run_deep(source),
        "1 2 3 {\"value\":\"a,\",\"done\":true}\nin\n{\"value\":9,\"done\":true}\nasync 1,2,3,4\n"
    );
}

/// Statements nested deeper than the window: every `for-of` record, catch
/// marker and `finally` return address past it is a slot.
#[test]
fn statements_nested_past_the_window_close_and_unwind() {
    let mut source = String::from(
        "var log = [];\n\
         function it(k) { var i = 0; return { [Symbol.iterator]() { return this; }, \
           next() { return { value: i++, done: i > 1 }; }, \
           return() { log.push(k); return {}; } }; }\n",
    );
    let loops: String = (0..100)
        .map(|index| format!("for (var v{index} of it({index})) "))
        .collect();
    source.push_str(&format!(
        "function a() {{ try {{ {loops}{{ throw new Error('x'); }} }} \
         catch (e) {{ log.push(e.message); }} return log.length + ':' + log[0] + ',' + log[99] + ',' + log[100]; }}\n\
         print(a());\n"
    ));
    let tries = "try { ".repeat(300);
    let catches: String = (0..300)
        .map(|index| format!(" }} catch (e{index}) {{ log.push({index}); throw e{index} + 1; }}"))
        .collect();
    source.push_str(&format!(
        "function b() {{ log = []; try {{ {tries}throw 1;{catches} }} \
         catch (z) {{ return z + ':' + log.length; }} }}\nprint(b());\n"
    ));
    let finallies: String = (0..300)
        .map(|index| format!(" }} finally {{ log.push({index}); }}"))
        .collect();
    source.push_str(&format!(
        "function c() {{ {tries}return 'r';{finallies} }}\n\
         log = []; print(c(), log.length, log[0], log[299]);\n"
    ));
    let enumerations: String = (0..260)
        .map(|index| format!("for (var k{index} in {{ a: 1 }}) "))
        .collect();
    source.push_str(&format!(
        "var count = 0; function d() {{ {enumerations}count++; return count; }}\nprint(d());\n"
    ));
    let generators: String = (0..90)
        .map(|index| format!("for (var y{index} of it({index})) "))
        .collect();
    source.push_str(&format!(
        "function* e() {{ {generators}yield 5; }}\n\
         log = []; var g = e(); print(g.next().value, JSON.stringify(g.return(9)), log.length, log[0], log[89]);\n"
    ));
    assert_eq!(
        run_deep(source),
        "101:99,0,x\n301:300\nr 300 0 299\n1\n5 {\"value\":9,\"done\":true} 90 89 0\n"
    );
}

#[test]
fn spilled_locals_and_temporaries_share_the_scope() {
    let locals: String = (0..300)
        .map(|index| format!("var l{index} = {index};"))
        .collect();
    let reads: String = (0..300)
        .map(|index| format!("l{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "function f() {{ {locals} var inner = () => l299 + l0; \
         var all = [{reads}]; return all.length + ':' + all[299] + ':' + Math.max({reads}) + ':' + inner(); }}\n\
         print(f());\n\
         function g() {{ 'use strict'; return h({reads2}); }} function h() {{ return arguments.length; }}\n\
         print(g());\n\
         function K() {{ this.n = arguments.length; }} print(new K({reads2}).n);",
        reads2 = digits(400)
    );
    assert_eq!(run_deep(source), "300:299:299:299\n400\n400\n");
}
