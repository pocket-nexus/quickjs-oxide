function P(x) { this.x = x; this.y = x * 2; }
var p = new P(3);
print(p.x, p.y, p instanceof P);
function R() { this.a = 1; return { b: 2 }; }
var r = new R();
print(r.a, r.b, r instanceof R);
function Q() { this.a = 1; return 5; }
var q = new Q();
print(q.a, q instanceof Q);
function E() {}
var e = new E();
print(typeof e, e instanceof E, e instanceof P);
function Chain(n) { this.n = n; this.next = n > 0 ? new Chain(n - 1) : null; }
var c = new Chain(3);
print(c.n, c.next.n, c.next.next.next.n, c.next.next.next.next);
var noArgs = new P;
print(noArgs.x, noArgs.y);
try { new (() => 1)(); } catch (err) { print(err instanceof TypeError); }
try { new 5; } catch (err) { print(err instanceof TypeError); }
function Counter() { this.count = 0; }
Counter.prototype.add = function (k) { this.count = this.count + k; return this; };
print(new Counter().add(2).add(3).count);
var made = [];
for (var i = 0; i < 50; i++) made[i] = new P(i);
print(made[49].y, made.length);
var err = new Error('boom');
print(err.message, err instanceof Error, err.name, err);
var te = TypeError('called');
print(te instanceof TypeError, te.message, te);
