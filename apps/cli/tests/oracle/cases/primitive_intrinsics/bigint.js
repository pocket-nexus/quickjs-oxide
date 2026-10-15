// mode: print
function flags(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}
function observe(thunk) {
    try { return String(thunk()); }
    catch (error) { return "throw:" + error.name + ":" + error.message; }
}
function signature(fn) {
    return fn.name + ":" + fn.length + ":" + Reflect.ownKeys(fn).map(String).join(",");
}
function isConstructor(fn) {
    try { Reflect.construct(function () {}, [], fn); return true; }
    catch (_) { return false; }
}

var implementedGlobals = [
    "parseInt", "parseFloat", "isNaN", "isFinite",
    "decodeURI", "decodeURIComponent", "encodeURI", "encodeURIComponent",
    "escape", "unescape", "Infinity", "NaN", "undefined", "Number",
    "Boolean", "String", "Math", "Reflect", "Symbol", "globalThis", "BigInt"
];
print("global-order=" + Reflect.ownKeys(globalThis).filter(function (key) {
    return typeof key === "string" && implementedGlobals.indexOf(key) >= 0;
}).join(","));
print("keys=" + Reflect.ownKeys(BigInt).map(String).join(",") + "|" +
      Reflect.ownKeys(BigInt.prototype).map(String).join(","));
print("descriptors=" + [
    flags(globalThis, "BigInt"), flags(BigInt, "length"), flags(BigInt, "name"),
    flags(BigInt, "asUintN"), flags(BigInt, "asIntN"), flags(BigInt, "prototype"),
    flags(BigInt.prototype, "toString"), flags(BigInt.prototype, "valueOf"),
    flags(BigInt.prototype, "constructor"),
    flags(BigInt.prototype, Symbol.toStringTag)
].join("|"));
print("graph=" + [
    typeof BigInt,
    Object.getPrototypeOf(BigInt) === Function.prototype,
    BigInt.prototype.constructor === BigInt,
    Object.getPrototypeOf(BigInt.prototype) === Object.prototype,
    Object.prototype.toString.call(BigInt.prototype),
    Object.isExtensible(BigInt.prototype),
    isConstructor(BigInt), isConstructor(BigInt.asUintN), isConstructor(BigInt.asIntN)
].join("|"));
print("signatures=" + [BigInt, BigInt.asUintN, BigInt.asIntN,
    BigInt.prototype.toString, BigInt.prototype.valueOf].map(signature).join("|"));

var callCases = [
    function () { return BigInt(); },
    function () { return BigInt(undefined); },
    function () { return BigInt(null); },
    function () { return BigInt(false); },
    function () { return BigInt(true); },
    function () { return BigInt(0); },
    function () { return BigInt(-0); },
    function () { return BigInt(42); },
    function () { return BigInt(9007199254740992); },
    function () { return BigInt(1.5); },
    function () { return BigInt(NaN); },
    function () { return BigInt(Infinity); },
    function () { return BigInt(""); },
    function () { return BigInt("  -42  "); },
    function () { return BigInt("0xff"); },
    function () { return BigInt("+0x1"); },
    function () { return BigInt("1n"); },
    function () { return BigInt(123n); },
    function () { return BigInt(Symbol("x")); }
];
print("calls=" + callCases.map(observe).join("|"));

var constructHit = false;
var constructBomb = {};
Object.defineProperty(constructBomb, Symbol.toPrimitive, {
    configurable: true,
    value: function () { constructHit = true; throw new Error("converted"); }
});
function OtherTarget() {}
print("construct=" + [
    observe(function () { return new BigInt(constructBomb); }), constructHit,
    observe(function () { return Reflect.construct(BigInt, [constructBomb], OtherTarget); }),
    constructHit
].join("|"));

var conversionLog = "";
var exotic = {};
Object.defineProperty(exotic, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { conversionLog += "exotic:" + hint + ","; return "42"; }
});
var fallback = {
    valueOf: function () { conversionLog += "valueOf,"; return {}; },
    toString: function () { conversionLog += "toString,"; return "43"; }
};
var invalidPrimitive = {};
Object.defineProperty(invalidPrimitive, Symbol.toPrimitive, {
    configurable: true,
    value: function () { conversionLog += "invalid,"; return {}; }
});
var throwingPrimitive = {};
Object.defineProperty(throwingPrimitive, Symbol.toPrimitive, {
    configurable: true,
    value: function () { conversionLog += "throw,"; throw new RangeError("sentinel"); }
});
print("conversion=" + [
    observe(function () { return BigInt(exotic); }),
    observe(function () { return BigInt(fallback); }),
    observe(function () { return BigInt(invalidPrimitive); }),
    observe(function () { return BigInt(throwingPrimitive); }),
    conversionLog
].join("|"));

print("as-values=" + [
    BigInt.asUintN(0, -1n), BigInt.asIntN(0, -1n),
    BigInt.asUintN(8, 257n), BigInt.asIntN(8, 255n),
    BigInt.asIntN(8, -129n),
    BigInt.asUintN(63, -1n), BigInt.asUintN(64, -1n),
    BigInt.asUintN(64, 9223372036854775808n),
    BigInt.asUintN(128, -18446744073709551616n),
    BigInt.asUintN(65, -18446744073709551616n),
    BigInt.asIntN(65, 18446744073709551616n),
    BigInt.asUintN(1.9, 3n), BigInt.asIntN(NaN, 9n)
].join("|"));

var asLog = "";
var goodBits = {};
Object.defineProperty(goodBits, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { asLog += "bits:" + hint + ","; return 8; }
});
var goodValue = {};
Object.defineProperty(goodValue, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { asLog += "value:" + hint + ","; return "-1"; }
});
var untouchedValue = {};
Object.defineProperty(untouchedValue, Symbol.toPrimitive, {
    configurable: true,
    value: function () { asLog += "untouched,"; return 1n; }
});
var zeroValue = {};
Object.defineProperty(zeroValue, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { asLog += "zero-value:" + hint + ","; return "5"; }
});
print("as-order=" + [
    observe(function () { return BigInt.asUintN(goodBits, goodValue); }),
    observe(function () { return BigInt.asUintN(-1, untouchedValue); }),
    observe(function () { return BigInt.asUintN(0, zeroValue); }),
    observe(function () { return BigInt.asUintN(1n, untouchedValue); }),
    asLog
].join("|"));
var asErrorCases = [
    function () { return BigInt.asUintN(-1, 1n); },
    function () { return BigInt.asUintN(Infinity, 1n); },
    function () { return BigInt.asUintN(9007199254740992, 1n); },
    function () { return BigInt.asUintN(8, undefined); },
    function () { return BigInt.asUintN(8, null); },
    function () { return BigInt.asUintN(8, 1); },
    function () { return BigInt.asUintN(8, Symbol("x")); },
    function () { return BigInt.asUintN(8, "bad"); }
];
print("as-errors=" + asErrorCases.map(observe).join("|"));

var boundary = 1n << 1048575n;
var overflowText = "0b1" + "0".repeat(1048576);
print("limits=" + [
    observe(function () { return BigInt(overflowText); }),
    observe(function () { return BigInt.asUintN(1048576, boundary) === (-1n << 1048575n); }),
    observe(function () { return BigInt.asUintN(1048577, boundary); }),
    observe(function () { return BigInt.asUintN(1048639, boundary); }),
    observe(function () { return BigInt.asUintN(1048640, boundary) === boundary; }),
    observe(function () { return boundary.toString(2).length; }),
    observe(function () { return boundary.toString(16).length; }),
    observe(function () { return boundary.toString(10); }),
    observe(function () { return (-1n << 1048575n).toString(2); })
].join("|"));

var radixLog = "";
var radixObject = {};
Object.defineProperty(radixObject, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { radixLog += hint + ","; return 16; }
});
var brandBomb = {};
Object.defineProperty(brandBomb, Symbol.toPrimitive, {
    configurable: true,
    value: function () { radixLog += "brand-bomb,"; return 10; }
});
var protoCases = [
    function () { return (255n).toString(); },
    function () { return (-255n).toString(16); },
    function () { return (35n).toString(36); },
    function () { return BigInt.prototype.toString.call(255n, radixObject); },
    function () { return BigInt.prototype.valueOf.call(-7n); },
    function () { return BigInt.prototype.valueOf(); },
    function () { return BigInt.prototype.toString.call({}, brandBomb); },
    function () { return (1n).toString(1); },
    function () { return (1n).toString(37); },
    function () { return (1n).toString(1n); }
];
print("proto=" + protoCases.map(observe).join("|") + "|" + radixLog);

var wrapper = Object(123n);
var wrapper2 = Object.prototype.valueOf.call(123n);
print("objects=" + [
    typeof wrapper,
    Object.getPrototypeOf(wrapper) === BigInt.prototype,
    Reflect.ownKeys(wrapper).length,
    BigInt.prototype.valueOf.call(wrapper),
    wrapper === wrapper2,
    Object.prototype.toString.call(123n),
    Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(BigInt.prototype),
    Object.prototype.toLocaleString.call(123n)
].join("|"));
delete BigInt.prototype[Symbol.toStringTag];
print("tag-delete=" + [
    Object.prototype.toString.call(123n),
    Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(BigInt.prototype)
].join("|"));
Object.defineProperty(BigInt.prototype, Symbol.toStringTag, {
    value: "CustomBigInt", writable: false, enumerable: false, configurable: true
});
print("tag-custom=" + [
    Object.prototype.toString.call(123n),
    Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(BigInt.prototype)
].join("|"));

var strictGetThis, sloppyGetThis, strictSetThis, sloppySetThis;
Object.defineProperty(BigInt.prototype, "__strictGet", {
    configurable: true,
    get: function () { "use strict"; strictGetThis = this; return this; }
});
Object.defineProperty(BigInt.prototype, "__sloppyGet", {
    configurable: true,
    get: function () { sloppyGetThis = this; return this.valueOf(); }
});
Object.defineProperty(BigInt.prototype, "__strictSet", {
    configurable: true,
    set: function () { "use strict"; strictSetThis = this; }
});
Object.defineProperty(BigInt.prototype, "__sloppySet", {
    configurable: true,
    set: function () { sloppySetThis = this; }
});
var strictGetResult = (1n).__strictGet;
var sloppyGetResult = (1n).__sloppyGet;
var strictSetResult = (1n).__strictSet = 7;
var sloppySetResult = (1n).__sloppySet = 8;
print("accessors=" + [
    strictGetResult, typeof strictGetThis, strictGetThis === 1n,
    sloppyGetResult, typeof sloppyGetThis,
    Object.getPrototypeOf(sloppyGetThis) === BigInt.prototype,
    sloppyGetThis.valueOf(),
    strictSetResult, typeof strictSetThis, strictSetThis === 1n,
    sloppySetResult, typeof sloppySetThis,
    Object.getPrototypeOf(sloppySetThis) === BigInt.prototype,
    sloppySetThis.valueOf()
].join("|"));

Object.defineProperty(BigInt.prototype, "__rw", {
    value: 1, writable: true, configurable: true
});
Object.defineProperty(BigInt.prototype, "__ro", {
    value: 1, writable: false, configurable: true
});
var deleteHit = false;
Object.defineProperty(BigInt.prototype, "__delete", {
    configurable: true,
    get: function () { deleteHit = true; return 1; }
});
print("writes=" + [
    ((1n).__rw = 2), (1n).__rw,
    observe(function () { "use strict"; return (1n).__rw = 3; }),
    ((1n).__ro = 2), (1n).__ro,
    observe(function () { "use strict"; return (1n).__ro = 3; }),
    delete (1n).__delete, deleteHit,
    Object.prototype.hasOwnProperty.call(BigInt.prototype, "__delete"),
    (function () { "use strict"; return delete (1n).__delete; })()
].join("|"));
