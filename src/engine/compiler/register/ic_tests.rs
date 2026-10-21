//! The `SetField`, global and constructor caches and
//! polymorphic sites, on lowered programs. Debug builds count cache hits, so
//! there each test checks that the path it names was taken as well as what
//! the program printed and the entries it left; a release test run (no
//! counters) checks the latter two.

use oxide_core::bytecode::Opcode;
use oxide_core::heap::gc::HeapLimits;
use oxide_core::vm::ic::{IcEntry, IcKind, IcStats, MEGA_TAG, POLY_TAG};
use oxide_core::{Intrinsic, PropertyFlags, Runtime, Value};

use super::lower_script;
use crate::engine::compiler::compile_unlinked_script;

/// A runtime that runs scripts one after another, sharing globals.
struct Session {
    runtime: Runtime,
}

impl Session {
    fn new() -> Session {
        let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
        runtime.install_print().unwrap();
        runtime.capture_output();
        Session { runtime }
    }

    /// Runs `source`; returns what it printed, with an uncaught exception's
    /// text appended.
    fn run(&mut self, source: &str) -> String {
        let script = compile_unlinked_script(source).expect("compiles");
        let lowered =
            lower_script(&mut self.runtime, &script).unwrap_or_else(|error| panic!("{error}"));
        let handle = self.runtime.hold(Value::object(lowered.function));
        let result = self.runtime.run_script(lowered.function);
        self.runtime.release(handle);
        let mut output = self.runtime.take_output();
        if let Err(thrown) = result {
            let text = self.runtime.display(thrown.0).unwrap();
            output.push_str(&format!("uncaught: {text}\n"));
        }
        output
    }

    /// The counters, in a build that keeps them (debug, or `ic-stats`).
    fn stats(&self) -> Option<IcStats> {
        self.runtime.heap().ics().stats().cloned()
    }

    /// The store index of the `nth` instruction `op` of the unit named `unit`
    /// (the most recently published one with that name).
    fn site(&self, unit: &str, op: Opcode, nth: usize) -> usize {
        let code = self.runtime.code();
        let units: Vec<_> = code.units().collect();
        let (_, unit) = units
            .into_iter()
            .rev()
            .find(|(_, candidate)| &*candidate.debug.name == unit)
            .unwrap_or_else(|| panic!("no unit {unit}"));
        let insn = unit
            .code
            .iter()
            .filter(|insn| insn.op == op)
            .nth(nth)
            .unwrap_or_else(|| panic!("no {op:?} #{nth}"));
        (unit.ic_base + insn.site().unwrap()) as usize
    }

    fn entry(&self, site: usize) -> IcEntry {
        self.runtime.heap().ics().entry(site)
    }

    fn global(&mut self, name: &str) -> Value {
        let key = self.runtime.heap_mut().property_key(name).unwrap();
        let global = self.runtime.global_object();
        self.runtime.heap().get_property(global, key).unwrap()
    }
}

fn hits(before: &Option<IcStats>, after: &Option<IcStats>, kind: IcKind) -> Option<u64> {
    let (before, after) = (before.as_ref()?, after.as_ref()?);
    Some(after.mono_hits[kind as usize] - before.mono_hits[kind as usize])
}

fn poly_hits(before: &Option<IcStats>, after: &Option<IcStats>, kind: IcKind) -> Option<u64> {
    let (before, after) = (before.as_ref()?, after.as_ref()?);
    Some(after.poly_hits[kind as usize] - before.poly_hits[kind as usize])
}

fn misses(before: &Option<IcStats>, after: &Option<IcStats>, kind: IcKind) -> Option<u64> {
    let (before, after) = (before.as_ref()?, after.as_ref()?);
    let k = kind as usize;
    Some(after.fills[k] + after.unfilled[k] - before.fills[k] - before.unfilled[k])
}

/// Checks a count where the build keeps the counters.
#[track_caller]
fn counted(count: Option<u64>, holds: impl FnOnce(u64) -> bool) {
    if let Some(count) = count {
        assert!(holds(count), "count {count}");
    }
}

/// `this.a = …; this.b = …` in a constructor with reserved slots are
/// transition hits once filled; a write to an existing property is an
/// in-place hit.
#[test]
fn transitions_and_in_place_writes_hit() {
    let mut session = Session::new();
    let source = "function P(i) { this.a = i; this.b = i + 1; }
                  function work(n) { var p; for (var i = 0; i < n; i++) { p = new P(i); p.a = 7; }
                                     return p; }
                  var p = work(3);";
    assert_eq!(session.run(source), "");
    let a = session.site("P", Opcode::SetField, 0);
    let entry = session.entry(a);
    assert_ne!(entry.w0, entry.w1, "a transition entry");
    let before = session.stats();
    assert_eq!(session.run("p = work(100); print(p.a, p.b);"), "7 100\n");
    let after = session.stats();
    // Two transitions and one in-place write per iteration, all hits.
    counted(hits(&before, &after, IcKind::Set), |count| count == 300);
    counted(misses(&before, &after, IcKind::Set), |count| count == 0);
    let write = session.entry(session.site("work", Opcode::SetField, 0));
    assert_eq!(write.w0, write.w1, "an in-place entry");
}

/// A literal with one property reserves one slot, so a transition on it
/// misses at the full range, still completes the add, and keeps its entry.
#[test]
fn a_full_range_transition_misses_and_completes() {
    let mut session = Session::new();
    let before = session.stats();
    let output = session.run(
        "function f(o, v) { o.x = v; return o; }
         var total = 0;
         for (var i = 0; i < 10; i++) total += f({ y: 0 }, i).x;
         print(total);",
    );
    assert_eq!(output, "45\n");
    let after = session.stats();
    counted(misses(&before, &after, IcKind::Set), |count| count == 10);
    let entry = session.entry(session.site("f", Opcode::SetField, 0));
    assert!(entry.is_filled() && entry.w0 != entry.w1, "{entry:?}");
}

/// A setter added to `%Object.prototype%` after a transition entry was
/// filled must run: the entry's chain cell is cleared by the addition.
#[test]
fn a_setter_added_after_fill_runs() {
    let mut session = Session::new();
    let output = session.run(
        "var calls = 0;
         function setter(v) { calls = calls + v; }
         function f(o, v) { o.x = v; return o; }
         for (var i = 0; i < 5; i++) f({ y: 0, z: 0, w: 0 }, i);
         // Keeps both shapes alive, so a stress collection keeps the entry.
         var keep = f({ y: 0, z: 0, w: 0 }, 0);",
    );
    assert_eq!(output, "");
    let site = session.site("f", Opcode::SetField, 0);
    assert!(session.entry(site).is_filled());
    // Three properties reserve four slots: the add is a transition hit.
    let before = session.stats();
    assert_eq!(session.run("print(f({ y: 0, z: 0, w: 0 }, 9).x);"), "9\n");
    counted(hits(&before, &session.stats(), IcKind::Set), |count| {
        count == 1
    });
    let setter = session.global("setter");
    let runtime = &mut session.runtime;
    let x = runtime.heap_mut().property_key("x").unwrap();
    let object_prototype = runtime.intrinsic(Intrinsic::ObjectPrototype);
    runtime
        .heap_mut()
        .add_accessor(
            object_prototype,
            x,
            PropertyFlags::accessor(false, true),
            Value::UNDEFINED,
            setter,
        )
        .unwrap();
    let output = session.run("var o = f({ y: 0, z: 0, w: 0 }, 5); print(calls, o.x, 'x' in o);");
    assert_eq!(output, "5 undefined true\n");
}

/// A cached global read or write hits until the global object's dictionary
/// changes shape: `delete` bumps the generation.
#[test]
fn deleting_a_global_bumps_the_generation() {
    let mut session = Session::new();
    let output = session.run(
        "g = 1;
         function read() { return g; }
         function write(v) { g = v; }
         for (var i = 0; i < 5; i++) { write(i); read(); }
         print(read());",
    );
    assert_eq!(output, "4\n");
    let read = session.site("read", Opcode::GetGlobal, 0);
    let filled = session.entry(read);
    assert_eq!(filled.w0, session.runtime.heap().global_generation());
    let before = session.stats();
    assert_eq!(session.run("write(8); print(read());"), "8\n");
    let after = session.stats();
    counted(hits(&before, &after, IcKind::Global), |count| count >= 2);
    let generation = session.runtime.heap().global_generation();
    assert_eq!(
        session.run("print(delete globalThis.g); try { read(); } catch (e) { print(e.name); }"),
        "true\nReferenceError\n"
    );
    assert_ne!(session.runtime.heap().global_generation(), generation);
    assert_eq!(session.run("g = 'back'; print(read());"), "back\n");
    assert_eq!(
        session.entry(read).w0,
        session.runtime.heap().global_generation()
    );
}

/// `call0`'s shape: a function read from a global on every call hits.
#[test]
fn a_global_function_call_hits() {
    let mut session = Session::new();
    session.run(
        "function unit() { return 1; }
         function work(n) { var s = 0; for (var i = 0; i < n; i++) s = s + unit(); return s; }
         work(2);",
    );
    let before = session.stats();
    assert_eq!(session.run("print(work(50));"), "50\n");
    let after = session.stats();
    counted(hits(&before, &after, IcKind::Global), |count| count >= 50);
}

/// A constructor that grows its receiver past the four slots every receiver
/// gets learns the size: the second construction's receiver has room for
/// every property.
#[test]
fn a_chained_constructor_teaches_the_receiver_size() {
    let mut session = Session::new();
    let output = session.run(
        "function init(o) { o.c = 3; o.d = 4; o.e = 5; o.f = 6; o.g = 7; }
         function P() { this.a = 1; init(this); }
         function make() { return new P(); }
         var p = make();
         print(p.a, p.e);",
    );
    assert_eq!(output, "1 5\n");
    let site = session.site("make", Opcode::New, 0);
    let entry = session.entry(site);
    assert_eq!(
        entry.w2, 6,
        "the static count was 1, the receiver grew to 6"
    );
    let before = session.stats();
    assert_eq!(session.run("p = make(); print(p.d);"), "4\n");
    let after = session.stats();
    counted(hits(&before, &after, IcKind::Construct), |count| count == 1);
    // With the room reserved, init's stores are transition hits.
    counted(misses(&before, &after, IcKind::Set), |count| count == 0);
    // A replaced `prototype` is read live.
    assert_eq!(
        session.run("P.prototype = { tag: 't' }; print(make().tag);"),
        "t\n"
    );
}

/// A site that sees a second shape keeps both in a polymorphic row; a fifth
/// marks it megamorphic and it stops recording.
#[test]
fn polymorphic_rows_and_the_megamorphic_mark() {
    let mut session = Session::new();
    session.run(
        "function getX(o) { return o.x; }
         var a = { x: 1 }, b = { p: 0, x: 2 }, c = { p: 0, q: 0, x: 3 },
             d = { p: 0, q: 0, r: 0, x: 4 }, e = { p: 0, q: 0, r: 0, s: 0, x: 5 };
         getX(a); getX(b);",
    );
    let site = session.site("getX", Opcode::GetField, 0);
    assert_eq!(session.entry(site).w0, POLY_TAG);
    let before = session.stats();
    assert_eq!(session.run("print(getX(a) + getX(b));"), "3\n");
    let after = session.stats();
    counted(poly_hits(&before, &after, IcKind::Get), |count| count == 2);
    assert_eq!(session.run("print(getX(c) + getX(d));"), "7\n");
    assert_eq!(session.entry(site).w0, POLY_TAG);
    assert_eq!(session.run("print(getX(e), getX(a));"), "5 1\n");
    assert_eq!(session.entry(site).w0, MEGA_TAG);
}

/// A closure is a header and a function record until an own-property
/// operation needs its `length`, `name` or `prototype`: neither an arrow nor
/// a `function` takes value slots, and a `function`'s `prototype` object
/// exists only once read.
#[test]
fn untouched_closures_take_no_slots() {
    let mut session = Session::new();
    session.run(
        "var keep = []; var fill = (make) => { for (var i = 0; i < 1000; i++) keep[i] = make(); };",
    );
    let counts = |session: &mut Session| {
        session.runtime.run_gc();
        let report = session.runtime.memory_report();
        (report.objects.live, report.values.used_slots)
    };
    session.run("fill(() => 0);");
    let (objects, slots) = counts(&mut session);
    session.run("fill(() => () => 0);");
    assert_eq!(
        counts(&mut session),
        (objects + 1000, slots),
        "an arrow is pending"
    );
    session.run("fill(() => function () {});");
    assert_eq!(
        counts(&mut session),
        (objects + 1000, slots),
        "a function is pending"
    );
    // Named where it is created, a closure's unit carries the name.
    session.run("fill(() => { const named = (a) => a; return named; });");
    assert_eq!(
        counts(&mut session),
        (objects + 1000, slots),
        "a named arrow is pending"
    );
    // The literal's four slots, and none for its method.
    session.run("fill(() => ({ m() {} }));");
    assert_eq!(
        counts(&mut session),
        (objects + 2000, slots + 4 * 1000),
        "a method is pending"
    );
    assert_eq!(
        session.run("print(keep[0].m.name, keep[0].m.length);"),
        "m 0\n"
    );
    session.run("fill(() => function () {});");
    assert_eq!(session.run("keep.forEach((f) => f.prototype);"), "");
    let touched = counts(&mut session);
    assert_eq!(touched.0, objects + 2000, "one prototype object each");
    assert!(
        touched.1 >= slots + 5 * 1000,
        "the closures' and prototypes' slots"
    );
}

/// A pending closure answers every own-property operation as a
/// materialized one would.
#[test]
fn pending_closures_keep_their_own_properties() {
    let mut session = Session::new();
    let source = r#"
        function f(a, b) {}
        print(Object.getOwnPropertyNames(f).join(), f.hasOwnProperty('prototype'), 'name' in f);
        print(f.length, f.name, f.prototype === f.prototype, f.prototype.constructor === f);
        const g = function* () {};
        print(Object.getOwnPropertyNames(g.prototype).length,
              Object.getPrototypeOf(g.prototype) === Object.getPrototypeOf(function* () {}).prototype);
        const a = (x) => x;
        print(Object.getOwnPropertyNames(a).join(), a.length, a.name, a.hasOwnProperty('prototype'));
        function h() {} const p = { q: 1 }; h.prototype = p;
        print(h.prototype === p, new h().q, Object.getOwnPropertyNames(h).join());
        function d() {} delete d.length; print(d.length, Object.getOwnPropertyNames(d).join());
        function e() {} Object.defineProperty(e, 'name', { value: 'zz' });
        print(e.name, JSON.stringify(Object.getOwnPropertyDescriptor(e, 'prototype')));
        function b(x, y, z) {} const bound = b.bind(null, 1); print(bound.name, bound.length);
        function C() { this.v = 1; } print(new C().v, new C() instanceof C, {} instanceof function () {});
        function F() {} F.foo = 1; print(Object.getOwnPropertyNames(F).join());
        function G() {} Object.freeze(G); print(Object.isFrozen(G), typeof G.prototype);
        function H() {} Object.setPrototypeOf(H, null); print(Object.getOwnPropertyNames(H).join(), H.length);
        function I() {} const o = Object.create(I); print(o.length, o.name, typeof o.prototype);
        class Base { constructor() { return function () {}; } }
        class Sub extends Base { #x = 1; static has(o) { return #x in o; } }
        print(Sub.has(new Sub()));
        function P() {} P.prototype = 5; print(typeof new P(), P.prototype);
        function Q() {} Reflect.set(Q, 'prototype', 7, {}); print(typeof Q.prototype);
        async function M() {} print(M.hasOwnProperty('prototype'), M.name);
    "#;
    assert_eq!(
        session.run(source),
        "length,name,prototype true true\n\
         2 f true true\n\
         0 true\n\
         length,name 1 a false\n\
         true 1 length,name,prototype\n\
         0 name,prototype\n\
         zz {\"value\":{},\"writable\":true,\"enumerable\":false,\"configurable\":false}\n\
         bound b 2\n\
         1 true false\n\
         length,name,prototype,foo\n\
         true object\n\
         length,name,prototype 0\n\
         0 I object\n\
         true\n\
         object 5\n\
         object\n\
         false M\n"
    );
}

/// A read or a write of a pending property is never cached: a site that
/// meets many pending closures reads and writes each one's own value.
#[test]
fn a_pending_property_is_never_cached() {
    let mut session = Session::new();
    let source = "function get(f) { return f.length; }
                  function set(f, v) { f.prototype = v; }
                  var fs = [function (a) {}, function (a, b) {}, function (a, b, c) {}];
                  print(fs.map(get).join());
                  fs.forEach(set);
                  print(fs.map((f) => f.prototype).join(), fs.map(get).join());";
    assert_eq!(session.run(source), "1,2,3\n0,1,2 1,2,3\n");
}
