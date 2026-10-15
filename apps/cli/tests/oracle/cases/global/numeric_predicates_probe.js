// mode: print
function flags(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}
function isConstructor(value) {
    try {
        Reflect.construct(function () {}, [], value);
        return true;
    } catch (_) {
        return false;
    }
}
function observe(thunk) {
    try { return String(thunk()); }
    catch (error) {
        if (error !== null && typeof error === "object")
            return "throw:" + error.name + ":" + error.message;
        return "throw:" + String(error);
    }
}

var globalNames = [
    "parseInt", "parseFloat", "isNaN", "isFinite",
    "Infinity", "NaN", "undefined", "Number", "Boolean"
];
print("global-order=" + Reflect.ownKeys(globalThis).filter(function (key) {
    return globalNames.indexOf(key) >= 0;
}).join(","));

print("graph=" + [
    Reflect.ownKeys(isNaN).map(String).join(","),
    Reflect.ownKeys(isFinite).map(String).join(","),
    flags(globalThis, "isNaN"), flags(globalThis, "isFinite"),
    isNaN.length, isNaN.name, isFinite.length, isFinite.name,
    flags(isNaN, "length"), flags(isNaN, "name"),
    flags(isFinite, "length"), flags(isFinite, "name"),
    Object.getPrototypeOf(isNaN) === Function.prototype,
    Object.getPrototypeOf(isFinite) === Function.prototype,
    isConstructor(isNaN), isConstructor(isFinite),
    isNaN === Number.isNaN, isFinite === Number.isFinite
].join("|"));

var raw = [
    undefined, null, false, true, 0, -0, NaN, Infinity, -Infinity,
    Number.MIN_VALUE, Number.MAX_VALUE, "", " ", "0", "1.5", "Infinity", "x",
    "0x10", "0b10", "0o10", "-0x10", "-0b10", "+Infinity", "-Infinity", "12x"
];
print("raw-isNaN=" + raw.map(function (value) { return isNaN(value); }).join(","));
print("raw-isFinite=" + raw.map(function (value) { return isFinite(value); }).join(","));

var extraHit = false;
var extra = {
    [Symbol.toPrimitive]: function () { extraHit = true; throw 99; }
};
print("call-shape=" + [
    isNaN.call({ ignored: true }, "x", extra),
    isFinite.call(Symbol("this"), 1, extra),
    extraHit
].join("|"));

var coercionLog = "";
var nanExotic = {
    [Symbol.toPrimitive]: function (hint) {
        coercionLog += "nan:" + hint + "|";
        return "x";
    }
};
var finiteExotic = {
    [Symbol.toPrimitive]: function (hint) {
        coercionLog += "finite:" + hint + "|";
        return "1";
    }
};
var fallback = {
    valueOf: function () { coercionLog += "valueOf|"; return {}; },
    toString: function () { coercionLog += "toString|"; return "1"; }
};
var invalid = {
    [Symbol.toPrimitive]: function () { return {}; }
};
var arbitraryThrow = {
    [Symbol.toPrimitive]: function () { throw 71; }
};
print("objects=" + [
    isNaN(nanExotic), isFinite(finiteExotic), isFinite(fallback),
    coercionLog,
    observe(function () { return isNaN(invalid); }),
    observe(function () { return isFinite(arbitraryThrow); }),
    isNaN(new Number(NaN)), isFinite(new Number(0)),
    Number.isNaN(new Number(NaN)), Number.isFinite(new Number(0))
].join("|"));

print("type-errors=" + [
    observe(function () { return isNaN(1n); }),
    observe(function () { return isFinite(1n); }),
    observe(function () { return isNaN(Symbol("nan")); }),
    observe(function () { return isFinite(Symbol("finite")); }),
    Number.isNaN(1n), Number.isFinite(Symbol("static"))
].join("|"));

var originalGlobalNaN = isNaN;
var originalGlobalFinite = isFinite;
var originalStaticNaN = Number.isNaN;
var originalStaticFinite = Number.isFinite;
globalThis.isNaN = function () { return "replacement-global"; };
Number.isFinite = function () { return "replacement-static"; };
print("mutation=" + [
    globalThis.isNaN !== originalGlobalNaN,
    Number.isNaN === originalStaticNaN,
    Number.isNaN !== globalThis.isNaN,
    Number.isFinite !== originalStaticFinite,
    globalThis.isFinite === originalGlobalFinite,
    globalThis.isFinite !== Number.isFinite
].join("|"));
