// mode: print
function bits(descriptor) {
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}

var tag = Symbol.toStringTag;
var objectToString = Object.prototype.toString;
var initial = Object.getOwnPropertyDescriptor(globalThis, tag);
print("initial=" + [
    globalThis[tag], bits(initial),
    Object.prototype.hasOwnProperty.call(globalThis, tag),
    globalThis.propertyIsEnumerable(tag),
    objectToString.call(globalThis)
].join("|"));

print("readonly=" + [
    Reflect.set(globalThis, tag, "assigned-global"),
    globalThis[tag], objectToString.call(globalThis)
].join("|"));

Object.defineProperty(globalThis, tag, { value: "direct-global" });
var direct = Object.getOwnPropertyDescriptor(globalThis, tag);
print("direct-redefine=" + [
    globalThis[tag], bits(direct), globalThis.propertyIsEnumerable(tag),
    objectToString.call(globalThis)
].join("|"));
Object.defineProperty(globalThis, tag, { value: "global" });

var marker = "__quickjs_oxide_global_tag_last_string__";
Object.defineProperty(globalThis, marker, {
    value: 1, writable: true, enumerable: true, configurable: true
});
var extra = Symbol("extra-global-key");
Object.defineProperty(globalThis, extra, {
    value: 1, writable: true, enumerable: true, configurable: true
});
var keys = Reflect.ownKeys(globalThis);
var common = [
    "parseInt", "parseFloat", "isNaN", "isFinite",
    "decodeURI", "decodeURIComponent", "encodeURI", "encodeURIComponent",
    "escape", "unescape", "Infinity", "NaN", "undefined", "Number", "Boolean"
];
var selected = keys.filter(function(key) {
    return key === tag || key === extra ||
           (typeof key === "string" && common.indexOf(key) >= 0);
}).map(function(key) {
    if (key === tag) return "symbol:toStringTag";
    if (key === extra) return "symbol:extra";
    return "string:" + key;
});
var symbols = Object.getOwnPropertySymbols(globalThis);
var firstSymbol = keys.findIndex(function(key) { return typeof key === "symbol"; });
print("keys=" + [
    selected.join(","),
    keys.slice(0, firstSymbol).every(function(key) { return typeof key === "string"; }),
    keys.slice(firstSymbol).every(function(key) { return typeof key === "symbol"; }),
    symbols.length, symbols[0] === tag, symbols[1] === extra
].join("|"));
delete globalThis[extra];
delete globalThis[marker];

var deleted = delete globalThis[tag];
print("delete=" + [
    deleted,
    Object.getOwnPropertyDescriptor(globalThis, tag) === undefined,
    typeof globalThis[tag],
    Object.getOwnPropertySymbols(globalThis).indexOf(tag) < 0,
    objectToString.call(globalThis)
].join("|"));

Object.defineProperty(globalThis, tag, {
    value: "custom-global", writable: true, enumerable: true, configurable: true
});
print("redefine=" + [
    globalThis[tag],
    bits(Object.getOwnPropertyDescriptor(globalThis, tag)),
    globalThis.propertyIsEnumerable(tag),
    Object.getOwnPropertySymbols(globalThis).indexOf(tag) >= 0,
    objectToString.call(globalThis)
].join("|"));

var nonStrings = [undefined, null, false, 0, {}, Symbol("not-a-tag")];
print("non-string=" + nonStrings.map(function(value) {
    globalThis[tag] = value;
    return objectToString.call(globalThis);
}).join("|"));
globalThis[tag] = "restored-global";
print("restored=" + objectToString.call(globalThis));
