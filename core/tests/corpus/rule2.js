// Materialization rule 2: a write to a local reads the old value first.
function f(x) { return x + (x = 2); }
print(f(1), f(40));
function g(x) { x = x++ + x; return x; }
print(g(5));
var y = 1;
y = y++ + y;
print(y);
function h(a) {
    var b = a;
    var r = a + (a = 10) + b + a;
    return r;
}
print(h(3));
function swap(a, b) { var t = a; a = b; b = t; return a * 10 + b; }
print(swap(1, 2));
function chain(x) { var y = x; x = x + 1; y = y + x; return y; }
print(chain(7));
function post(i) { var a = i++; var b = i--; var c = --i; var d = ++i; return a + ',' + b + ',' + c + ',' + d + ',' + i; }
print(post(10));
