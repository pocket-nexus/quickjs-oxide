function classify(v) {
    switch (v) {
        case 1: return 'one';
        case 'two': return 'two';
        case 3:
        case 4: return 'three-or-four';
        default: return 'other';
    }
}
print(classify(1), classify('two'), classify(4), classify('1'), classify(9));
function fall(n) { var out = ''; switch (n) { case 0: out += 'a'; case 1: out += 'b'; break; case 2: out += 'c'; default: out += 'd'; } return out; }
print(fall(0), fall(1), fall(2), fall(5));
var out = '';
outer: for (var i = 0; i < 3; i++) {
    for (var j = 0; j < 3; j++) {
        if (j === 1) continue outer;
        if (i === 2) break outer;
        out += i + '' + j + ' ';
    }
}
print(out);
var n = 0;
do { n++; } while (n < 5);
print(n);
var w = 10, steps = 0;
while (w > 1) { w = w % 2 === 0 ? w / 2 : 3 * w + 1; steps++; }
print(steps);
block: { if (true) break block; print('not printed'); }
print(1 && 2, 0 && 2, 0 || 'b', 'a' || 'b', null ?? 'd', 0 ?? 'd', undefined ?? null);
var calls = 0;
function side() { calls++; return true; }
var r1 = false && side(), r2 = true || side(), r3 = null ?? side();
print(r1, r2, r3, calls);
var o = null;
print(o?.x, o?.x.y, ({ x: { y: 1 } })?.x?.y);
var count = 0;
for (var k = 0; k < 10; k++) { if (k % 3 === 0) continue; count += k; }
print(count);
var s = 0;
for (let a = 0, b = 10; a < b; a++, b--) s += a * b;
print(s);
var x = 5;
x += 3; x -= 1; x *= 2; x /= 7; x %= 3; x **= 3;
var y = 6; y <<= 2; y >>= 1; y >>>= 1; y &= 7; y |= 8; y ^= 3;
print(x, y);
var z = null; z ??= 'set'; var t = 1; t ||= 2; var u = 1; u &&= 3;
print(z, t, u);
