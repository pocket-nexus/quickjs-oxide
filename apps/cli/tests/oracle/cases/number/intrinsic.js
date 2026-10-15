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
    try { Reflect.construct(function() {}, [], fn); return true; }
    catch (_) { return false; }
}
var bitStorage = new ArrayBuffer(8);
var bitView = new DataView(bitStorage);
function bits(value) {
    if (Number.isNaN(value)) return "NaN";
    bitView.setFloat64(0, value, true);
    var text = bitView.getBigUint64(0, true).toString(16);
    return ("0000000000000000" + text).slice(-16);
}

var conversionLog = "";
var numericObject = {};
Object.defineProperty(numericObject, Symbol.toPrimitive, {
    value: function(hint) { conversionLog = conversionLog + hint + ","; return "42.5"; },
    configurable: true
});
print("call=" + [
    Number(), Number(undefined), Number(null), Number(false), Number(true),
    Number(0), 1 / Number(-0), Number(NaN), Number(""), Number(" 12.5 "),
    Number("0x10"), Number(0n), Number(9007199254740993n),
    Number(numericObject), conversionLog,
    observe(function() { return Number(Symbol("number")); })
].join("|"));

conversionLog = "";
var boxedZero = new Number();
var boxedNegativeZero = new Number(-0);
var boxedBigInt = new Number(9007199254740993n);
var boxedObject = new Number(numericObject);
print("new=" + [
    typeof boxedZero,
    Object.getPrototypeOf(boxedZero) === Number.prototype,
    Object.prototype.toString.call(boxedZero), boxedZero.valueOf(),
    Reflect.ownKeys(boxedZero).length,
    1 / boxedNegativeZero.valueOf(), boxedBigInt.valueOf(), boxedObject.valueOf(),
    conversionLog,
    observe(function() { return new Number(Symbol("number")); })
].join("|"));

var boxedSeven = new Number(7);
print("coercion=" + [
    Number(boxedSeven), +boxedSeven, boxedSeven + 1,
    boxedSeven == 7, boxedSeven === 7,
    boxedSeven.valueOf(), boxedSeven.toString(),
    Object.prototype.valueOf.call(boxedSeven) === boxedSeven
].join("|"));

var constructorKeys = Reflect.ownKeys(Number);
var prototypeKeys = Reflect.ownKeys(Number.prototype);
print("ctor-keys=" + constructorKeys.map(String).join(","));
print("proto-keys=" + prototypeKeys.map(String).join(","));
print("global-order=" + Reflect.ownKeys(globalThis).filter(function(key) {
    return key === "parseInt" || key === "parseFloat" ||
           key === "Infinity" || key === "NaN" || key === "undefined" ||
           key === "Number" || key === "Boolean";
}).map(String).join(","));
print("ctor-descriptors=" + [flags(globalThis, "Number")].concat(
    constructorKeys.map(function(key) { return flags(Number, key); })
).join("|"));
print("proto-descriptors=" + prototypeKeys.map(function(key) {
    return flags(Number.prototype, key);
}).join("|"));
print("graph=" + [
    typeof Number,
    Object.getPrototypeOf(Number) === Function.prototype,
    Number.prototype.constructor === Number,
    Object.getPrototypeOf(Number.prototype) === Object.prototype,
    Object.prototype.toString.call(Number.prototype),
    Number.prototype.valueOf(),
    Object.isExtensible(Number.prototype),
    Number.parseInt === parseInt, Number.parseFloat === parseFloat
].join("|"));
var numberCallables = [
    Number,
    Number.parseInt, Number.parseFloat,
    Number.isNaN, Number.isFinite, Number.isInteger, Number.isSafeInteger,
    Number.prototype.toExponential, Number.prototype.toFixed,
    Number.prototype.toPrecision, Number.prototype.toString,
    Number.prototype.toLocaleString, Number.prototype.valueOf
];
print("signatures=" + numberCallables.map(signature).join("|"));
print("callable-graph=" + numberCallables.map(function(fn) {
    return (Object.getPrototypeOf(fn) === Function.prototype) + ":" +
           flags(fn, "length") + ":" + flags(fn, "name") + ":" +
           isConstructor(fn);
}).join("|"));

var predicateBombHit = false;
var predicateBomb = {};
Object.defineProperty(predicateBomb, Symbol.toPrimitive, {
    value: function() { predicateBombHit = true; throw new Error("predicate coerced"); },
    configurable: true
});
print("predicates=" + [
    Number.isNaN(), Number.isNaN(undefined), Number.isNaN("NaN"),
    Number.isNaN(NaN), Number.isNaN(Infinity), Number.isNaN(0),
    Number.isFinite(), Number.isFinite(null), Number.isFinite("1"),
    Number.isFinite(0), Number.isFinite(Number.MAX_VALUE), Number.isFinite(Infinity),
    Number.isInteger(), Number.isInteger("1"), Number.isInteger(NaN),
    Number.isInteger(Infinity), Number.isInteger(-0), Number.isInteger(1),
    Number.isInteger(1.5), Number.isInteger(9007199254740992),
    Number.isSafeInteger(), Number.isSafeInteger("1"), Number.isSafeInteger(NaN),
    Number.isSafeInteger(Infinity), Number.isSafeInteger(-0),
    Number.isSafeInteger(Number.MAX_SAFE_INTEGER),
    Number.isSafeInteger(Number.MIN_SAFE_INTEGER),
    Number.isSafeInteger(9007199254740992),
    Number.isNaN(1n), Number.isFinite(Symbol("predicate")),
    Number.isInteger(predicateBomb), Number.isSafeInteger(predicateBomb), predicateBombHit
    , Number.isNaN(new Number(NaN)), Number.isFinite(new Number(0)),
    Number.isInteger(new Number(1)), Number.isSafeInteger(new Number(1))
].join("|"));

print("constants=" + [
    Number.MAX_VALUE, Number.MIN_VALUE, Number.NaN,
    Number.NEGATIVE_INFINITY, Number.POSITIVE_INFINITY, Number.EPSILON,
    Number.MAX_SAFE_INTEGER, Number.MIN_SAFE_INTEGER
].map(bits).join("|"));

print("methods=" + [
    Number.prototype.valueOf.call(12.5),
    (123.5).toString(), (-255).toString(16), (10).toString(2), (35).toString(36),
    (-0).toString(), (NaN).toString(), (Infinity).toString(),
    (123.5).toLocaleString(),
    (1.25).toFixed(1), (1.005).toFixed(2), (-0).toFixed(2), (1e21).toFixed(2),
    (123).toExponential(), (123).toExponential(0),
    (1.25).toExponential(1), (0).toExponential(2),
    (123.45).toPrecision(), (123.45).toPrecision(4),
    (0.0000012345).toPrecision(3), (1234567).toPrecision(3)
].join("|"));

print("rounding=" + [
    (25).toExponential(0), (-25).toExponential(0),
    (2.5).toPrecision(1), (-2.5).toPrecision(1),
    (1.125).toFixed(2), (-1.125).toFixed(2),
    (0.5).toFixed(0), (-0.5).toFixed(0), (-1e-10).toFixed(0),
    (1 - Math.pow(2, -53)).toString(12),
    (1.3).toString(7), (1.3).toString(35)
].join("|"));
print("valid-max=" + [
    (1).toFixed(100), (1).toExponential(100), (1).toPrecision(100)
].join("|"));

print("method-edges=" + [
    (10).toString(undefined), (10).toString(10),
    (1.5).toFixed(), (NaN).toFixed(2), (Infinity).toFixed(2),
    (-Infinity).toFixed(2),
    (NaN).toExponential(101), (Infinity).toExponential(101),
    (-Infinity).toExponential(-1),
    (NaN).toPrecision(0), (Infinity).toPrecision(0),
    (-Infinity).toPrecision(101),
    Number.prototype.toLocaleString.call(1.25, Symbol("ignored"))
].join("|"));

print("ranges=" + [
    observe(function() { return (1).toString(1); }),
    observe(function() { return (1).toString(37); }),
    observe(function() { return (1).toString(NaN); }),
    observe(function() { return (1).toFixed(-1); }),
    observe(function() { return (1).toFixed(101); }),
    observe(function() { return (1).toExponential(-1); }),
    observe(function() { return (1).toExponential(101); }),
    observe(function() { return (1).toPrecision(0); }),
    observe(function() { return (1).toPrecision(101); }),
    observe(function() { return (NaN).toFixed(101); }),
    observe(function() { return (Infinity).toFixed(-1); }),
    observe(function() { return (1).toString(Symbol("radix")); }),
    observe(function() { return (1).toFixed(1n); }),
    observe(function() { return (1).toExponential(1n); }),
    observe(function() { return (1).toPrecision(1n); })
].join("|"));

var numberSpoof = Object.create(Number.prototype);
print("brands=" + [
    observe(function() { return Number.prototype.valueOf.call("1"); }),
    observe(function() { return Number.prototype.toString.call("1"); }),
    observe(function() { return Number.prototype.toLocaleString.call("1"); }),
    observe(function() { return Number.prototype.toFixed.call("1"); }),
    observe(function() { return Number.prototype.toExponential.call("1"); }),
    observe(function() { return Number.prototype.toPrecision.call("1"); }),
    observe(function() { return Number.prototype.valueOf.call(numberSpoof); })
].join("|"));

var formatLog = "";
var formatArgument = {};
Object.defineProperty(formatArgument, Symbol.toPrimitive, {
    value: function(hint) { formatLog = formatLog + hint + ","; return 2; },
    configurable: true
});
var convertedFormats = [
    Number.prototype.toString.call(10, formatArgument),
    Number.prototype.toFixed.call(1.25, formatArgument),
    Number.prototype.toExponential.call(1.25, formatArgument),
    Number.prototype.toPrecision.call(1.25, formatArgument)
].join("|");
var convertedFormatLog = formatLog;
formatLog = "";
var badThisBeforeArgument = observe(function() {
    return Number.prototype.toFixed.call("1", formatArgument);
});
var badThisLog = formatLog;
formatLog = "";
var ignoredLocaleArgument = Number.prototype.toLocaleString.call(1.25, formatArgument);
print("conversion-order=" + [
    convertedFormats, convertedFormatLog,
    badThisBeforeArgument, badThisLog,
    ignoredLocaleArgument, formatLog
].join("|"));

var objectBoxA = Object.prototype.valueOf.call(3);
var objectBoxB = Object.prototype.valueOf.call(3);
print("object-links=" + [
    Object.prototype.toString.call(3),
    Object.getPrototypeOf(objectBoxA) === Number.prototype,
    Number.prototype.valueOf.call(objectBoxA),
    objectBoxA === objectBoxB,
    Object.prototype.toLocaleString.call(3)
].join("|"));

var directStrictThis;
var directSloppyThis;
Object.defineProperty(Number.prototype, "__strictMethod", {
    configurable: true,
    value: function() { "use strict"; directStrictThis = this; return this; }
});
Object.defineProperty(Number.prototype, "__sloppyMethod", {
    configurable: true,
    value: function() { directSloppyThis = this; return this.valueOf(); }
});
var directStrictResult = (7).__strictMethod();
var directSloppyResultA = (7).__sloppyMethod();
var directSloppyThisA = directSloppyThis;
var directSloppyResultB = (7).__sloppyMethod();
var directSloppyThisB = directSloppyThis;
print("method-receivers=" + [
    directStrictResult, directStrictThis === 7, typeof directStrictThis,
    directSloppyResultA, typeof directSloppyThisA,
    Object.getPrototypeOf(directSloppyThisA) === Number.prototype,
    directSloppyThisA.valueOf(), directSloppyResultB,
    directSloppyThisA === directSloppyThisB
].join("|"));

var strictGetThis;
var sloppyGetThis;
var strictSetThis;
var strictSetValue;
var sloppySetThis;
var sloppySetValue;
Object.defineProperty(Number.prototype, "__strictGet", {
    configurable: true,
    get: function() { "use strict"; strictGetThis = this; return this; }
});
Object.defineProperty(Number.prototype, "__sloppyGet", {
    configurable: true,
    get: function() { sloppyGetThis = this; return this.valueOf(); }
});
Object.defineProperty(Number.prototype, "__strictSet", {
    configurable: true,
    set: function(value) { "use strict"; strictSetThis = this; strictSetValue = value; }
});
Object.defineProperty(Number.prototype, "__sloppySet", {
    configurable: true,
    set: function(value) { sloppySetThis = this; sloppySetValue = value; }
});
var strictGetResult = (3).__strictGet;
var sloppyGetResult = (3).__sloppyGet;
var strictSetResult = (3).__strictSet = 7;
var sloppySetResult = (3).__sloppySet = 8;
print("accessors=" + [
    strictGetResult, strictGetThis === 3,
    sloppyGetResult, typeof sloppyGetThis,
    Object.getPrototypeOf(sloppyGetThis) === Number.prototype, sloppyGetThis.valueOf(),
    strictSetResult, strictSetThis === 3, strictSetValue,
    sloppySetResult, typeof sloppySetThis,
    Object.getPrototypeOf(sloppySetThis) === Number.prototype,
    sloppySetThis.valueOf(), sloppySetValue
].join("|"));

var getterHit = false;
var deleteHit = false;
Object.defineProperty(Number.prototype, "__rw", {
    value: 1, writable: true, configurable: true
});
Object.defineProperty(Number.prototype, "__ro", {
    value: 1, writable: false, configurable: true
});
Object.defineProperty(Number.prototype, "__getterOnly", {
    configurable: true,
    get: function() { getterHit = true; return 1; }
});
Object.defineProperty(Number.prototype, "__delete", {
    configurable: true,
    get: function() { deleteHit = true; return 1; }
});
var sloppyRw = (3).__rw = 2;
var sloppyRo = (3).__ro = 2;
var sloppyGetterOnly = (3).__getterOnly = 2;
var deleted = delete (3).__delete;
print("writes=" + [
    sloppyRw, (3).__rw,
    observe(function() { "use strict"; return (3).__rw = 3; }),
    sloppyRo, (3).__ro,
    observe(function() { "use strict"; return (3).__ro = 3; }),
    sloppyGetterOnly,
    observe(function() { "use strict"; return (3).__getterOnly = 3; }),
    getterHit, deleted, deleteHit,
    Object.prototype.hasOwnProperty.call(Number.prototype, "__delete"),
    (function() { "use strict"; return delete (3).__delete; })()
].join("|"));

print("tag-before=" + [
    Object.prototype.toString.call(3),
    Object.prototype.toString.call(boxedSeven),
    Object.prototype.toString.call(Number.prototype),
    Object.prototype.hasOwnProperty.call(Number.prototype, Symbol.toStringTag)
].join("|"));
var originalNumberToStringDescriptor =
    Object.getOwnPropertyDescriptor(Number.prototype, "toString");
var tagThis;
Object.defineProperty(Number.prototype, Symbol.toStringTag, {
    configurable: true,
    get: function() { "use strict"; tagThis = this; return "CustomNumber"; }
});
var localeGetterThis;
var localeCallThis;
function localeMethod() { "use strict"; localeCallThis = this; return this; }
Object.defineProperty(Number.prototype, "toString", {
    configurable: true,
    get: function() { "use strict"; localeGetterThis = this; return localeMethod; }
});
var customTag = Object.prototype.toString.call(3);
var customLocale = Object.prototype.toLocaleString.call(3);
print("custom-object-methods=" + [
    customTag, typeof tagThis, tagThis instanceof Number, tagThis.valueOf(),
    customLocale, localeGetterThis === 3, localeCallThis === 3
].join("|"));
var boxedCustomTag = Object.prototype.toString.call(boxedSeven);
var boxedTagThis = tagThis;
var prototypeCustomTag = Object.prototype.toString.call(Number.prototype);
var prototypeTagThis = tagThis;
print("tag-getter-detail=" + [
    boxedCustomTag, boxedTagThis === boxedSeven, boxedTagThis.valueOf(),
    prototypeCustomTag, prototypeTagThis === Number.prototype,
    prototypeTagThis.valueOf()
].join("|"));
Object.defineProperty(boxedSeven, Symbol.toStringTag, {
    value: "OwnNumber", configurable: true
});
tagThis = undefined;
print("tag-own=" + [
    Object.prototype.toString.call(boxedSeven), tagThis === undefined,
    boxedSeven.valueOf()
].join("|"));
Object.defineProperty(Number.prototype, Symbol.toStringTag, {
    value: 123, configurable: true
});
print("tag-nonstring=" + [
    Object.prototype.toString.call(3),
    Object.prototype.toString.call(boxedSeven)
].join("|"));

delete boxedSeven[Symbol.toStringTag];
delete Number.prototype[Symbol.toStringTag];
Object.defineProperty(Number.prototype, "toString", originalNumberToStringDescriptor);
[
    "__strictMethod", "__sloppyMethod",
    "__strictGet", "__sloppyGet", "__strictSet", "__sloppySet",
    "__rw", "__ro", "__getterOnly", "__delete"
].forEach(function(key) { delete Number.prototype[key]; });
print("tag-restored=" + [
    Object.prototype.toString.call(3),
    Object.prototype.toString.call(boxedSeven),
    Reflect.ownKeys(Number.prototype).map(String).join(",")
].join("|"));

var capturedParseInt = Number.parseInt;
var capturedParseFloat = Number.parseFloat;
var originalGlobalParseInt = parseInt;
var originalGlobalParseFloat = parseFloat;
globalThis.parseInt = function() { return 99; };
globalThis.parseFloat = function() { return 88; };
print("alias-capture=" + [
    capturedParseInt === originalGlobalParseInt,
    Number.parseInt === parseInt,
    Number.parseInt("10", 2), parseInt("10", 2),
    capturedParseFloat === originalGlobalParseFloat,
    Number.parseFloat === parseFloat,
    Number.parseFloat("1.5tail"), parseFloat("1.5tail")
].join("|"));
