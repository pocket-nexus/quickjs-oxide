// mode: print
function bits(descriptor) {
    return "D:" + Number(descriptor.writable) +
           Number(descriptor.enumerable) + Number(descriptor.configurable);
}
function isConstructor(value) {
    try { Reflect.construct(function(){}, [], value); return true; }
    catch (_) { return false; }
}
function methodMeta(key) {
    var descriptor = Object.getOwnPropertyDescriptor(Math, key);
    var value = descriptor.value;
    return key + ":" + value.name + ":" + value.length + ":" +
           (Object.getPrototypeOf(value) === Function.prototype) + ":" +
           (typeof value === "function") + ":" + isConstructor(value) + ":" +
           bits(descriptor) + ":" + bits(Object.getOwnPropertyDescriptor(value, "name")) +
           ":" + bits(Object.getOwnPropertyDescriptor(value, "length"));
}
var methods = [
    "min","max","abs","floor","ceil","round","sqrt",
    "acos","asin","atan","atan2","cos","exp","log","pow","sin","tan",
    "trunc","sign","cosh","sinh","tanh","acosh","asinh","atanh",
    "expm1","log1p","log2","log10","cbrt","hypot","random",
    "f16round","fround","imul","clz32","sumPrecise"
];
var constants = ["E","LN10","LN2","LOG2E","LOG10E","PI","SQRT1_2","SQRT2"];
var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "Math");
print("graph=" + [
    globalDescriptor.value === Math,
    Object.getPrototypeOf(Math) === Object.prototype,
    Object.prototype.toString.call(Math),
    Object.isExtensible(Math),
    bits(globalDescriptor)
].join("|"));
print("keys=" + Reflect.ownKeys(Math).map(String).join(","));
print("methods=" + methods.map(methodMeta).join("|"));
print("constants=" + constants.map(function(key) {
    var descriptor = Object.getOwnPropertyDescriptor(Math, key);
    return key + ":" + String(descriptor.value) + ":" + bits(descriptor);
}).join("|"));
var tag = Object.getOwnPropertyDescriptor(Math, Symbol.toStringTag);
print("tag=" + String(tag.value) + ":" + bits(tag));
