// mode: print
function flags(descriptor) {
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}
function units(value) {
    var result = [];
    for (var index = 0; index < value.length; index++)
        result.push(value.charCodeAt(index).toString(16).padStart(4, "0"));
    return result.join(",");
}
function keyName(key) { return typeof key === "symbol" ? String(key) : key; }
function box(value) { return (function () { return this; }).call(value); }
function dataProperty(object, key, value) {
    return Reflect.defineProperty(object, key, {
        value: value, writable: true, enumerable: true, configurable: true
    });
}

var payload = "A😀\ud800";
var wrapper = box(payload);
var prototype = Object.getPrototypeOf(wrapper);
var prototypeLength = Object.getOwnPropertyDescriptor(prototype, "length");
var wrapperLength = Object.getOwnPropertyDescriptor(wrapper, "length");
print("prototype-length=" + [
    prototypeLength.value, flags(prototypeLength),
    Object.prototype.toString.call(prototype), Object.isExtensible(prototype)
].join("|"));
print("wrapper=" + [
    Reflect.ownKeys(wrapper).map(keyName).join(","), wrapperLength.value,
    flags(wrapperLength), Object.isExtensible(wrapper)
].join("|"));

var indexRows = [];
for (var index = 0; index < 4; index++) {
    var descriptor = Object.getOwnPropertyDescriptor(wrapper, String(index));
    indexRows.push(index + ":" + units(descriptor.value) + ":" +
                   flags(descriptor) + ":" +
                   Object.prototype.hasOwnProperty.call(wrapper, String(index)));
}
indexRows.push("4:" +
               (Object.getOwnPropertyDescriptor(wrapper, "4") === undefined) + ":" +
               Object.prototype.hasOwnProperty.call(wrapper, "4"));
indexRows.push("length:" + wrapperLength.value + ":" + flags(wrapperLength) + ":" +
               Object.prototype.hasOwnProperty.call(wrapper, "length"));
print("get-has=" + indexRows.join("|"));

var tail = Symbol("tail");
dataProperty(wrapper, "foo", 1);
dataProperty(wrapper, "01", 2);
dataProperty(wrapper, "8", 3);
dataProperty(wrapper, tail, 4);
print("merged-keys=" + Reflect.ownKeys(wrapper).map(keyName).join(","));
print("merged-properties=" + ["8", "foo", "01", tail].map(function (key) {
    var descriptor = Object.getOwnPropertyDescriptor(wrapper, key);
    return keyName(key) + ":" + descriptor.value + ":" + flags(descriptor) + ":" +
           Object.prototype.hasOwnProperty.call(wrapper, key);
}).join("|"));

print("compatible=" + [
    Reflect.defineProperty(wrapper, "0", {}),
    Reflect.defineProperty(wrapper, "0", { value: "A" }),
    Reflect.defineProperty(wrapper, "0", {
        value: "A", writable: false, enumerable: true, configurable: false
    }),
    Reflect.defineProperty(wrapper, "1", { value: "\ud83d" })
].join("|"));
print("incompatible=" + [
    Reflect.defineProperty(wrapper, "0", { value: "X" }),
    Reflect.defineProperty(wrapper, "0", { writable: true }),
    Reflect.defineProperty(wrapper, "0", { enumerable: false }),
    Reflect.defineProperty(wrapper, "0", { configurable: true }),
    Reflect.defineProperty(wrapper, "0", { get: undefined })
].join("|"));

print("delete=" + [
    Reflect.deleteProperty(wrapper, "0"), Reflect.deleteProperty(wrapper, "length"),
    Reflect.deleteProperty(wrapper, "8"), Reflect.deleteProperty(wrapper, "missing"),
    Reflect.deleteProperty(wrapper, "foo"),
    Object.prototype.hasOwnProperty.call(wrapper, "0"),
    Object.prototype.hasOwnProperty.call(wrapper, "8"),
    Reflect.ownKeys(wrapper).map(keyName).join(",")
].join("|"));

var fixed = box(payload);
dataProperty(fixed, "foo", 1);
Object.preventExtensions(fixed);
print("prevent=" + [
    Object.isExtensible(fixed),
    Reflect.defineProperty(fixed, "0", {}),
    Reflect.defineProperty(fixed, "0", { value: "A" }),
    Reflect.defineProperty(fixed, "0", { value: "X" }),
    Reflect.defineProperty(fixed, "8", {
        value: 8, writable: true, enumerable: true, configurable: true
    }),
    Reflect.defineProperty(fixed, "foo", { value: 2 }), fixed.foo,
    Reflect.deleteProperty(fixed, "0"), Reflect.deleteProperty(fixed, "length"),
    Reflect.deleteProperty(fixed, "foo"),
    Reflect.ownKeys(fixed).map(keyName).join(",")
].join("|"));
print("tags=" + [
    Object.prototype.toString.call(wrapper),
    Object.prototype.toString.call(prototype)
].join("|"));
