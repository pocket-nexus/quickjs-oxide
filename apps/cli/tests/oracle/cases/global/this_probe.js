// mode: print
function bits(descriptor) {
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}

var root = globalThis;
var reader = (function() { return globalThis; });
var strictWriter = (function(value) { "use strict"; globalThis = value; });
var implemented = [
    "Error", "EvalError", "RangeError", "ReferenceError", "SyntaxError",
    "TypeError", "URIError", "InternalError", "AggregateError", "Array", "Object", "Function", "Iterator", "parseInt",
    "parseFloat", "isNaN", "isFinite", "decodeURI", "decodeURIComponent",
    "encodeURI", "encodeURIComponent", "escape", "unescape", "Infinity",
    "NaN", "undefined", "eval", "Number", "Boolean", "String", "Math", "Reflect", "Symbol", "globalThis", "BigInt", "Date", "RegExp", "JSON", "Proxy", "Map", "Set", "WeakMap", "WeakSet", "ArrayBuffer", "SharedArrayBuffer", "Uint8ClampedArray", "Int8Array", "Uint8Array", "Int16Array", "Uint16Array", "Int32Array", "Uint32Array", "BigInt64Array", "BigUint64Array", "Float16Array", "Float32Array", "Float64Array", "DataView", "Atomics", "Promise", "WeakRef", "FinalizationRegistry"
];
var keys = Reflect.ownKeys(root);
var firstSymbol = keys.findIndex(function(key) { return typeof key === "symbol"; });

print("initial=" + [
    globalThis === root,
    bits(Object.getOwnPropertyDescriptor(root, "globalThis")),
    root.globalThis === root,
    globalThis === this
].join("|"));
print("keys=" + [
    keys.filter(function(key) {
        return typeof key === "string" && implemented.indexOf(key) >= 0;
    }).join(","),
    keys.slice(0, firstSymbol).every(function(key) { return typeof key === "string"; }) &&
        keys.slice(firstSymbol).every(function(key) { return typeof key === "symbol"; }),
    keys[keys.length - 1] === Symbol.toStringTag
].join("|"));

strictWriter(17);
print("assignment=" + [
    root.globalThis,
    bits(Object.getOwnPropertyDescriptor(root, "globalThis"))
].join("|"));
root.globalThis = root;

var deleted = delete root.globalThis;
print("delete=" + [
    deleted,
    Object.getOwnPropertyDescriptor(root, "globalThis") === undefined,
    typeof globalThis
].join("|"));

var strictMissing;
try {
    strictWriter(19);
    strictMissing = "no-throw";
} catch (error) {
    strictMissing = error.name + ":" + error.message + ":" +
                    (error instanceof ReferenceError);
}
print("strict-missing=" + [
    strictMissing,
    Object.getOwnPropertyDescriptor(root, "globalThis") === undefined
].join("|"));

globalThis = root;
print("sloppy=" + [
    globalThis === root,
    bits(Object.getOwnPropertyDescriptor(root, "globalThis"))
].join("|"));

Object.defineProperty(root, "globalThis", {
    value: root, writable: true, enumerable: false, configurable: true
});
print("direct=" + [
    globalThis === root,
    bits(Object.getOwnPropertyDescriptor(root, "globalThis"))
].join("|"));

Object.defineProperty(root, "globalThis", {
    get: function() { "use strict"; return this; },
    set: function(value) {
        "use strict";
        this.globalThisSetterValue = value;
        this.globalThisSetterReceiver = this;
    },
    enumerable: false,
    configurable: true
});
var accessor = Object.getOwnPropertyDescriptor(root, "globalThis");
strictWriter(47);
print("accessor=" + [
    reader() === root,
    typeof accessor.get,
    typeof accessor.set,
    root.globalThisSetterValue,
    root.globalThisSetterReceiver === root,
    accessor.enumerable,
    accessor.configurable
].join("|"));

delete root.globalThis;
var missing;
try {
    reader();
    missing = "no-throw";
} catch (error) {
    missing = error.name + ":" + error.message + ":" +
              (error instanceof ReferenceError);
}
print("captured-delete=" + [
    Object.getOwnPropertyDescriptor(root, "globalThis") === undefined,
    typeof globalThis,
    missing
].join("|"));

Object.defineProperty(root, "globalThis", {
    value: root, writable: true, enumerable: false, configurable: true
});
print("reconnect=" + [
    reader() === root,
    bits(Object.getOwnPropertyDescriptor(root, "globalThis"))
].join("|"));
