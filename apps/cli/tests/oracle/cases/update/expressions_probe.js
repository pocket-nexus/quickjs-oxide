// mode: print
function show(value) {
    if (value === undefined) return "undefined:undefined";
    if (value === null) return "object:null";
    return typeof value + ":" + String(value);
}
function observe(thunk) {
    try { return show(thunk()); }
    catch (error) { return "throw:" + error.name + ":" + error.message; }
}
function observeSource(source) {
    try { return show((0, eval)(source)); }
    catch (error) { return "throw:" + error.name + ":" + error.message; }
}

print("identifier-string=" + show((function(){
    var value = "41";
    var old = value++;
    return typeof old + ":" + old + "|" + typeof value + ":" + value;
})()));
print("identifier-bigint=" + show((function(){
    var value = 41n;
    var post = value--;
    var prefix = --value;
    return post * 10000n + prefix * 100n + value;
})()));

let log = "";
let coercible = {};
Object.defineProperty(coercible, Symbol.toPrimitive, {
    value(hint) { log += "v(" + hint + ")"; return 7; }
});
log = "";
const objectOld = coercible++;
print("identifier-object-post=" + [show(objectOld), show(coercible), log].join("|"));

let bigintCoercible = {};
Object.defineProperty(bigintCoercible, Symbol.toPrimitive, {
    value(hint) { log += "B(" + hint + ")"; return 7n; }
});
log = "";
const bigintObjectOld = bigintCoercible++;
print("identifier-object-bigint-post=" +
      [show(bigintObjectOld), show(bigintCoercible), log].join("|"));

let symbolValue = Symbol("update");
print("identifier-symbol=" + observe(() => symbolValue++) + "|" + typeof symbolValue);

let seenValue;
const computedOld = {};
Object.defineProperty(computedOld, Symbol.toPrimitive, {
    value(hint) { log += "v(" + hint + ")"; return 7; }
});
const target = {};
Object.defineProperty(target, "member", {
    get() { log += "g"; return computedOld; },
    set(value) { log += "s"; seenValue = value; },
    configurable: true
});
const key = {};
Object.defineProperty(key, Symbol.toPrimitive, {
    value(hint) { log += "k(" + hint + ")"; return "member"; }
});
function baseExpr() { log += "b"; return target; }
function keyExpr() { log += "q"; return key; }

log = ""; seenValue = undefined;
const computedPost = baseExpr()[keyExpr()]++;
print("computed-post=" + [show(computedPost), log, show(seenValue)].join("|"));
log = ""; seenValue = undefined;
const computedPrefix = ++baseExpr()[keyExpr()];
print("computed-prefix=" + [show(computedPrefix), log, show(seenValue)].join("|"));

const readonlyOld = {};
Object.defineProperty(readonlyOld, Symbol.toPrimitive, {
    value(hint) { log += "r(" + hint + ")"; return 4; }
});
const readonlyTarget = {};
Object.defineProperty(readonlyTarget, "value", {
    value: readonlyOld, writable: false, configurable: true
});
log = "";
const readonlySloppy = readonlyTarget.value++;
print("readonly-member-sloppy=" + [show(readonlySloppy),
      show(readonlyTarget.value === readonlyOld), log].join("|"));
log = "";
print("readonly-member-strict=" + observe(() =>
      (function(){ "use strict"; return ++readonlyTarget.value; })()) + "|" + log);

Object.defineProperty(globalThis, "readonlyIdentifier", {
    value: "4", writable: false, configurable: true
});
const readonlyIdentifierOld = readonlyIdentifier++;
print("readonly-identifier-sloppy=" +
      [show(readonlyIdentifierOld), show(readonlyIdentifier)].join("|"));
print("readonly-identifier-strict=" + observe(() =>
      (function(){ "use strict"; return ++readonlyIdentifier; })()));

print("missing-post=" + observe(() => updateMissing++));
log = "";
print("null-computed=" + observe(() => null[keyExpr()]++) + "|" + log);

print("asi-lf=" + show((function(){
    var x = 1, y = 4;
    var result = x
    ++y;
    return result * 100 + x * 10 + y;
})()));
print("power-updates=" + show((function(){
    var x = 2;
    return (++x ** 2) * 100 + (x++ ** 2) * 10 + x;
})()));
print("power-unary-error=" +
      observeSource("(function(){ var x = 2; -x++ ** 2; })()"));
print("invalid-update=" +
      observeSource("(function(){ var x = 2; ++(x ** 2); })()"));
print("strict-arguments=" +
      observeSource("\"use strict\"; arguments++"));
