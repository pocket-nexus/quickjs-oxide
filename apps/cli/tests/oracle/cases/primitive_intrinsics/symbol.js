// mode: print
function flags(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}
function accessorFlags(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0") + ":" +
           (descriptor.get ? descriptor.get.name + "/" + descriptor.get.length : "-") + ":" +
           (descriptor.set ? descriptor.set.name + "/" + descriptor.set.length : "-");
}
function keyName(key) { return typeof key === "symbol" ? String(key) : key; }
function signature(fn) {
    return fn.name + ":" + fn.length + ":" + Reflect.ownKeys(fn).map(keyName).join(",");
}
function isConstructor(fn) {
    try { Reflect.construct(function () {}, [], fn); return true; }
    catch (_) { return false; }
}
function units(value) {
    if (value === undefined) return "undefined";
    var result = [];
    for (var index = 0; index < value.length; index++)
        result.push(value.charCodeAt(index).toString(16).padStart(4, "0"));
    return result.join(",");
}
function symbolRecord(symbol) {
    return units(symbol.description) + "/" + units(String(symbol));
}
function render(value) {
    if (typeof value === "symbol") return "symbol:" + symbolRecord(value);
    if (value !== null && typeof value === "object")
        return Object.prototype.toString.call(value);
    return String(value);
}
function observe(thunk) {
    try { return render(thunk()); }
    catch (error) {
        if (error !== null && typeof error === "object")
            return "throw:" + error.name + ":" + error.message;
        return "throw:" + String(error);
    }
}
function observeSymbol(thunk) {
    try { return "symbol:" + symbolRecord(thunk()); }
    catch (error) {
        if (error !== null && typeof error === "object")
            return "throw:" + error.name + ":" + error.message;
        return "throw:" + String(error);
    }
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
print("keys=" + Reflect.ownKeys(Symbol).map(keyName).join(",") + "|" +
      Reflect.ownKeys(Symbol.prototype).map(keyName).join(","));
var wellKnownNames = [
    "toPrimitive", "iterator", "match", "matchAll", "replace", "search", "split",
    "toStringTag", "isConcatSpreadable", "hasInstance", "species", "unscopables",
    "asyncIterator"
];
print("descriptors=" + [
    flags(globalThis, "Symbol"), flags(Symbol, "length"), flags(Symbol, "name"),
    flags(Symbol, "for"), flags(Symbol, "keyFor"),
    wellKnownNames.map(function (name) { return flags(Symbol, name); }).join(","),
    flags(Symbol, "prototype"), flags(Symbol.prototype, "toString"),
    flags(Symbol.prototype, "valueOf"), accessorFlags(Symbol.prototype, "description"),
    flags(Symbol.prototype, "constructor"), flags(Symbol.prototype, Symbol.toPrimitive),
    flags(Symbol.prototype, Symbol.toStringTag)
].join("|"));
print("graph=" + [
    typeof Symbol,
    Object.getPrototypeOf(Symbol) === Function.prototype,
    Symbol.prototype.constructor === Symbol,
    Object.getPrototypeOf(Symbol.prototype) === Object.prototype,
    Object.prototype.toString.call(Symbol.prototype),
    Object.isExtensible(Symbol.prototype),
    isConstructor(Symbol), isConstructor(Symbol.for), isConstructor(Symbol.keyFor)
].join("|"));
var descriptionGetter = Object.getOwnPropertyDescriptor(Symbol.prototype, "description").get;
print("signatures=" + [
    Symbol, Symbol.for, Symbol.keyFor, Symbol.prototype.toString,
    Symbol.prototype.valueOf, Symbol.prototype[Symbol.toPrimitive], descriptionGetter
].map(signature).join("|"));

var callArguments = [
    [], [undefined], [""], [null], [false], [0], [-0], [1.5], [1n],
    ["a\0b"], ["\ud800"], ["\udc00"], ["😀"]
];
var callResults = callArguments.map(function (arguments_) {
    return symbolRecord(Symbol.apply(undefined, arguments_));
});
callResults.push(Symbol("same") !== Symbol("same"));
callResults.push(Symbol() !== Symbol(""));
print("calls=" + callResults.join("|"));

var constructHit = false;
var constructBomb = {};
Object.defineProperty(constructBomb, Symbol.toPrimitive, {
    configurable: true,
    value: function () { constructHit = true; throw new Error("converted"); }
});
function OtherTarget() {}
print("construct=" + [
    observe(function () { return new Symbol(constructBomb); }), constructHit,
    observe(function () { return Reflect.construct(Symbol, [constructBomb], OtherTarget); }),
    constructHit
].join("|"));

var conversionLog = "";
var exotic = {};
Object.defineProperty(exotic, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { conversionLog += "exotic:" + hint + ","; return "E"; }
});
var fallback = {
    toString: function () { conversionLog += "toString,"; return {}; },
    valueOf: function () { conversionLog += "valueOf,"; return "V"; }
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
    observeSymbol(function () { return Symbol(exotic); }),
    observeSymbol(function () { return Symbol(fallback); }),
    observeSymbol(function () { return Symbol(invalidPrimitive); }),
    observeSymbol(function () { return Symbol(throwingPrimitive); }),
    observeSymbol(function () { return Symbol(Symbol("source")); }),
    conversionLog
].join("|"));

function registryRecord(value) {
    var symbol = Symbol.for(value);
    return symbolRecord(symbol) + "/" + units(Symbol.keyFor(symbol));
}
print("registry=" + [
    Symbol.for() === Symbol.for(undefined), registryRecord(undefined),
    registryRecord(null), registryRecord(false), registryRecord(-0), registryRecord(1n),
    registryRecord(""), registryRecord("a\0b"), registryRecord("\ud800"),
    registryRecord("😀")
].join("|"));
var registryLog = "";
var registryExotic = {};
Object.defineProperty(registryExotic, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { registryLog += "exotic:" + hint + ","; return "K"; }
});
var registryFallback = {
    toString: function () { registryLog += "toString,"; return {}; },
    valueOf: function () { registryLog += "valueOf,"; return "L"; }
};
print("registry-conversion=" + [
    Symbol.for(registryExotic) === Symbol.for("K"),
    Symbol.for(registryFallback) === Symbol.for("L"),
    observe(function () { return Symbol.for(Symbol("x")); }),
    observe(function () { return Symbol.keyFor(Symbol("fresh")); }),
    observe(function () { return Symbol.keyFor(Symbol.iterator); }),
    observe(function () { return Symbol.keyFor(Object(Symbol.for("wrapped"))); }),
    observe(function () { return Symbol.keyFor("x"); }),
    observe(function () { return Symbol.keyFor(); }), registryLog
].join("|"));

var seenWellKnown = [];
print("well-known=" + wellKnownNames.map(function (name) {
    var value = Symbol[name];
    var unique = seenWellKnown.indexOf(value) < 0;
    seenWellKnown.push(value);
    return name + ":" + units(value.description) + ":" +
           (Symbol.keyFor(value) === undefined) + ":" +
           (value === Symbol[name]) + ":" + unique;
}).join("|"));

var primitive = Symbol("x");
var noDescription = Symbol();
var wrapper = Object(primitive);
var spoof = Object.create(Symbol.prototype);
var toPrimitive = Symbol.prototype[Symbol.toPrimitive];
var hostileHint = {};
Object.defineProperty(hostileHint, "x", { get: function () { throw 91; } });
print("proto=" + [
    Symbol.prototype.toString.call(primitive), Symbol.prototype.toString.call(wrapper),
    Symbol.prototype.toString.call(noDescription),
    Symbol.prototype.valueOf.call(primitive) === primitive,
    Symbol.prototype.valueOf.call(wrapper) === primitive,
    toPrimitive.call(primitive, "default") === primitive,
    toPrimitive.call(primitive, "string") === primitive,
    toPrimitive.call(primitive, "number") === primitive,
    toPrimitive.call(primitive, hostileHint) === primitive,
    units(descriptionGetter.call(primitive)), units(descriptionGetter.call(noDescription)),
    observe(function () { return Symbol.prototype.valueOf.call(Symbol.prototype); }),
    observe(function () { return Symbol.prototype.toString.call(spoof); }),
    observe(function () { return descriptionGetter.call(null); }),
    Symbol.prototype.toString.call(primitive, hostileHint)
].join("|"));

var wrapper2 = Object.prototype.valueOf.call(primitive);
print("objects=" + [
    typeof wrapper, Object.getPrototypeOf(wrapper) === Symbol.prototype,
    Reflect.ownKeys(wrapper).length, wrapper.valueOf() === primitive,
    wrapper === wrapper2,
    Object.getPrototypeOf(wrapper2) === Symbol.prototype,
    Object.prototype.toString.call(primitive), Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(Symbol.prototype),
    Object.prototype.toLocaleString.call(primitive)
].join("|"));
delete Symbol.prototype[Symbol.toStringTag];
print("tag-delete=" + [
    Object.prototype.toString.call(primitive), Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(Symbol.prototype)
].join("|"));
Object.defineProperty(Symbol.prototype, Symbol.toStringTag, {
    value: "CustomSymbol", writable: false, enumerable: false, configurable: true
});
print("tag-custom=" + [
    Object.prototype.toString.call(primitive), Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(Symbol.prototype)
].join("|"));
Object.defineProperty(Symbol.prototype, Symbol.toStringTag, { value: 1 });
print("tag-non-string=" + [
    Object.prototype.toString.call(primitive), Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(Symbol.prototype)
].join("|"));

var strictGetThis, sloppyGetThis, strictSetThis, sloppySetThis;
Object.defineProperty(Symbol.prototype, "__strictGet", {
    configurable: true,
    get: function () { "use strict"; strictGetThis = this; return this; }
});
Object.defineProperty(Symbol.prototype, "__sloppyGet", {
    configurable: true,
    get: function () { sloppyGetThis = this; return this.valueOf(); }
});
Object.defineProperty(Symbol.prototype, "__strictSet", {
    configurable: true,
    set: function () { "use strict"; strictSetThis = this; }
});
Object.defineProperty(Symbol.prototype, "__sloppySet", {
    configurable: true,
    set: function () { sloppySetThis = this; }
});
var strictGetResult = Symbol("get").__strictGet;
var sloppyGetResult = Symbol("get").__sloppyGet;
var strictSetResult = Symbol("strict-set").__strictSet = 7;
var sloppySetResult = Symbol("sloppy-set").__sloppySet = 8;
print("accessors=" + [
    typeof strictGetResult, units(strictGetResult.description),
    typeof strictGetThis, strictGetThis === strictGetResult,
    typeof sloppyGetResult, units(sloppyGetResult.description), typeof sloppyGetThis,
    Object.getPrototypeOf(sloppyGetThis) === Symbol.prototype,
    units(sloppyGetThis.valueOf().description),
    strictSetResult, typeof strictSetThis, units(strictSetThis.description),
    sloppySetResult, typeof sloppySetThis,
    Object.getPrototypeOf(sloppySetThis) === Symbol.prototype,
    units(sloppySetThis.valueOf().description)
].join("|"));

Object.defineProperty(Symbol.prototype, "__rw", {
    value: 1, writable: true, configurable: true
});
Object.defineProperty(Symbol.prototype, "__ro", {
    value: 1, writable: false, configurable: true
});
var deleteHit = false;
Object.defineProperty(Symbol.prototype, "__delete", {
    configurable: true,
    get: function () { deleteHit = true; return 1; }
});
print("writes=" + [
    (Symbol("x").__rw = 2), Symbol("x").__rw,
    observe(function () { "use strict"; return Symbol("x").__rw = 3; }),
    (Symbol("x").__ro = 2), Symbol("x").__ro,
    observe(function () { "use strict"; return Symbol("x").__ro = 3; }),
    delete Symbol("x").__delete, deleteHit,
    Object.prototype.hasOwnProperty.call(Symbol.prototype, "__delete"),
    (function () { "use strict"; return delete Symbol("x").__delete; })()
].join("|"));
