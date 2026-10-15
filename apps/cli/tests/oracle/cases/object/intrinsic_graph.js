// mode: print
function bits(descriptor) {
    if ("value" in descriptor)
        return "D:" + Number(descriptor.writable) + Number(descriptor.enumerable) + Number(descriptor.configurable);
    return "A:" + Number(descriptor.enumerable) + Number(descriptor.configurable);
}
function isConstructor(value) {
    try { Reflect.construct(function(){}, [], value); return true; }
    catch (_) { return false; }
}
function callableMeta(value) {
    return value.name + ":" + value.length + ":" +
           (Object.getPrototypeOf(value) === Function.prototype) + ":" +
           (typeof value === "function") + ":" + isConstructor(value);
}
function methodMeta(owner, key) {
    var descriptor = Object.getOwnPropertyDescriptor(owner, key);
    return String(key) + ":" + callableMeta(descriptor.value) + ":" + bits(descriptor);
}
var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "Object");
var prototypeDescriptor = Object.getOwnPropertyDescriptor(Object, "prototype");
var constructorDescriptor = Object.getOwnPropertyDescriptor(Object.prototype, "constructor");
print("graph=" + [
    globalDescriptor.value === Object,
    prototypeDescriptor.value === Object.prototype,
    constructorDescriptor.value === Object,
    Object.getPrototypeOf(Object) === Function.prototype,
    Object.getPrototypeOf(Object.prototype) === null,
    bits(globalDescriptor),bits(prototypeDescriptor),bits(constructorDescriptor)
].join("|"));
var selected = ["length","name","prototype","create","getPrototypeOf","setPrototypeOf",
                "defineProperty","defineProperties","getOwnPropertyNames","getOwnPropertySymbols"];
print("ctor-prefix=" + Reflect.ownKeys(Object).filter(function(key) {
    return selected.indexOf(key) >= 0;
}).map(String).join(","));
print("proto-keys=" + Reflect.ownKeys(Object.prototype).map(String).join(","));
print("static-meta=" + ["create","getPrototypeOf","setPrototypeOf","defineProperty",
    "defineProperties","getOwnPropertyNames","getOwnPropertySymbols"].map(function(key) {
        return methodMeta(Object,key);
    }).join("|"));
print("proto-meta=" + ["toString","toLocaleString","valueOf","hasOwnProperty","isPrototypeOf",
    "propertyIsEnumerable","__defineGetter__","__defineSetter__","__lookupGetter__",
    "__lookupSetter__"].map(function(key) { return methodMeta(Object.prototype,key); }).join("|"));
var protoDescriptor = Object.getOwnPropertyDescriptor(Object.prototype,"__proto__");
print("proto-accessor=__proto__:" + bits(protoDescriptor) + ":" +
      callableMeta(protoDescriptor.get) + ":" + callableMeta(protoDescriptor.set));
