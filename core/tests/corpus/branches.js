// Every compare-jump form, the `typeof` and nullish tests, and `>>>`.
function forms(a, b) {
    var out = '';
    var i = a;
    do { out += 'a'; i = i + 1; } while (i < b);
    i = a; do { out += 'b'; i = i + 1; } while (i <= b);
    i = b; do { out += 'c'; i = i - 1; } while (i > a);
    i = b; do { out += 'd'; i = i - 1; } while (i >= a);
    if (a <= b) out += 'e';
    if (a >= b) out += 'f';
    i = a; do { out += 'g'; i = i + 1; } while (i == b);
    if (a == b) out += 'h';
    if (a != b) out += 'i';
    i = a; do { out += 'j'; i = i + 1; } while (i === b);
    if (a !== b) out += 'k';
    var n = 0;
    do { n++; } while (n <= 5);
    do { n--; } while (n > 3);
    do { n++; } while (n >= 10);
    if (n >= 3) out += 'l';
    do { n++; } while (n == 5);
    if (n != 6) out += 'm';
    return out + n;
}
print(forms(1, 3), forms(2, 2), forms(NaN, 1), forms(0.5, 2.5));
function nullish(x) {
    var r = '';
    if (x === undefined) r += 'U';
    if (x === null) r += 'N';
    if (typeof x === 'undefined') r += 'T';
    if (typeof x === 'function') r += 'F';
    if (x == null) r += 'n';
    r += x?.v ?? '-';
    return r;
}
print(nullish(undefined), nullish(null), nullish(print), nullish({ v: 3 }), nullish(0), 'abc'?.length);
function unsignedShift(a, b) { return a >>> b; }
print(unsignedShift(-1, 0), unsignedShift(-8, 1), unsignedShift(8, 33), unsignedShift(1.9, 0));
function bare(x) { if (x) return; return 'fell'; }
print(bare(1), bare(0));
var isU = (v) => v === undefined, isN = (v) => v === null;
print(isU(void 0), isN(null), isU(null), typeof isU === 'function', typeof missing === 'undefined');
