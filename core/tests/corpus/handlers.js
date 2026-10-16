// Handler depths: live operands below a try, throws across frames.
function thrower(v) { throw v; }
function f(a) {
    var r = [a, (function () {
        try { try { throw 1; } catch (e) { throw e + 1; } } catch (e2) { return e2; }
    })()];
    return r;
}
print(f(7));
function g(a) {
    return [a, (() => { try { return thrower(5); } catch (e) { return e * 2; } })(), a + 1];
}
print(g(1));
function deep(n) { if (n === 0) throw new RangeError('bottom'); return deep(n - 1) + 1; }
try { deep(50); } catch (e) { print(e.name, e.message, e instanceof RangeError, e instanceof Error); }
try { thrower({ code: 42 }); } catch (e) { print(e.code); }
try { thrower(undefined); } catch (e) { print(e); }
function rethrow() { try { thrower('a'); } catch (e) { throw e + 'b'; } }
try { rethrow(); } catch (e) { print(e); }
var count = 0;
for (var i = 0; i < 5; i++) { try { if (i % 2) throw i; count += 10; } catch (e) { count += e; } }
print(count);
function mid() { var x = 1 + (function () { try { return 2; } finally { } })(); return x; }
print(mid());
try { null.x; } catch (e) { print(e instanceof TypeError); }
try { undefinedName; } catch (e) { print(e instanceof ReferenceError, e.message); }
try { (void 0)(); } catch (e) { print(e instanceof TypeError); }
