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

var bombHit = false;
var bomb = {};
Object.defineProperty(bomb, Symbol.toPrimitive, {
    value: function() { bombHit = true; throw "coerced"; },
    configurable: true
});

print("call=" + [
    Boolean(), Boolean(undefined), Boolean(null), Boolean(false), Boolean(true),
    Boolean(0), Boolean(-0), Boolean(NaN), Boolean(""), Boolean("0"),
    Boolean(0n), Boolean(1n), Boolean(Symbol("boolean")), Boolean(bomb), bombHit
].join("|"));

var boxedFalse = new Boolean(false);
var boxedTrue = new Boolean(true);
var boxedBomb = new Boolean(bomb);
print("new=" + [
    typeof boxedFalse,
    Object.getPrototypeOf(boxedFalse) === Boolean.prototype,
    Object.prototype.toString.call(boxedFalse),
    boxedFalse.valueOf(),
    Reflect.ownKeys(boxedFalse).length,
    boxedTrue.valueOf(), boxedBomb.valueOf(), bombHit
].join("|"));

print("coercion=" + [
    Boolean(boxedFalse), +boxedFalse, boxedFalse + 1,
    boxedFalse == false, boxedFalse === false,
    boxedFalse.valueOf(), boxedFalse.toString(),
    Object.prototype.valueOf.call(boxedFalse) === boxedFalse,
    +boxedTrue
].join("|"));

print("methods=" + [
    false.valueOf(), true.toString(),
    Boolean.prototype.valueOf(), Boolean.prototype.toString(),
    Object.prototype.toString.call(Boolean.prototype),
    Object.getPrototypeOf(Boolean.prototype) === Object.prototype
].join("|"));

print("ctor-keys=" + Reflect.ownKeys(Boolean).map(String).join(","));
print("proto-keys=" + Reflect.ownKeys(Boolean.prototype).map(String).join(","));
print("descriptors=" + [
    flags(globalThis, "Boolean"),
    flags(Boolean, "length"), flags(Boolean, "name"), flags(Boolean, "prototype"),
    flags(Boolean.prototype, "toString"), flags(Boolean.prototype, "valueOf"),
    flags(Boolean.prototype, "constructor")
].join("|"));
print("graph=" + [
    typeof Boolean,
    Object.getPrototypeOf(Boolean) === Function.prototype,
    Boolean.prototype.constructor === Boolean,
    Object.isExtensible(Boolean.prototype)
].join("|"));

print("brands=" + [
    observe(function() { return Boolean.prototype.valueOf.call(0); }),
    observe(function() { return Boolean.prototype.toString.call("false"); }),
    observe(function() { var detached = Boolean.prototype.valueOf; return detached(); })
].join("|"));

var objectBoxA = Object.prototype.valueOf.call(false);
var objectBoxB = Object.prototype.valueOf.call(false);
print("object-links=" + [
    Object.prototype.toString.call(false),
    Object.prototype.toString.call(true),
    Object.getPrototypeOf(objectBoxA) === Boolean.prototype,
    Boolean.prototype.valueOf.call(objectBoxA),
    objectBoxA === objectBoxB,
    Object.prototype.toLocaleString.call(false),
    Object.prototype.toLocaleString.call(true)
].join("|"));

var strictGetThis;
var sloppyGetThis;
var strictSetThis;
var strictSetValue;
var sloppySetThis;
var sloppySetValue;
Object.defineProperty(Boolean.prototype, "__strictGet", {
    configurable: true,
    get: function() { "use strict"; strictGetThis = this; return this; }
});
Object.defineProperty(Boolean.prototype, "__sloppyGet", {
    configurable: true,
    get: function() { sloppyGetThis = this; return this.valueOf(); }
});
Object.defineProperty(Boolean.prototype, "__strictSet", {
    configurable: true,
    set: function(value) { "use strict"; strictSetThis = this; strictSetValue = value; }
});
Object.defineProperty(Boolean.prototype, "__sloppySet", {
    configurable: true,
    set: function(value) { sloppySetThis = this; sloppySetValue = value; }
});
var strictGetResult = (false).__strictGet;
var sloppyGetResult = (false).__sloppyGet;
var strictSetResult = (false).__strictSet = 7;
var sloppySetResult = (false).__sloppySet = 8;
print("accessors=" + [
    strictGetResult, strictGetThis === false,
    sloppyGetResult, typeof sloppyGetThis,
    Object.getPrototypeOf(sloppyGetThis) === Boolean.prototype, sloppyGetThis.valueOf(),
    strictSetResult, strictSetThis === false, strictSetValue,
    sloppySetResult, typeof sloppySetThis,
    Object.getPrototypeOf(sloppySetThis) === Boolean.prototype,
    sloppySetThis.valueOf(), sloppySetValue
].join("|"));

var getterHit = false;
var deleteHit = false;
Object.defineProperty(Boolean.prototype, "__rw", {
    value: 1, writable: true, configurable: true
});
Object.defineProperty(Boolean.prototype, "__ro", {
    value: 1, writable: false, configurable: true
});
Object.defineProperty(Boolean.prototype, "__getterOnly", {
    configurable: true,
    get: function() { getterHit = true; return 1; }
});
Object.defineProperty(Boolean.prototype, "__delete", {
    configurable: true,
    get: function() { deleteHit = true; return 1; }
});
var sloppyRw = (false).__rw = 2;
var sloppyRo = (false).__ro = 2;
var sloppyGetterOnly = (false).__getterOnly = 2;
var deleted = delete (false).__delete;
print("writes=" + [
    sloppyRw, (false).__rw,
    observe(function() { "use strict"; return (false).__rw = 3; }),
    sloppyRo, (false).__ro,
    observe(function() { "use strict"; return (false).__ro = 3; }),
    sloppyGetterOnly,
    observe(function() { "use strict"; return (false).__getterOnly = 3; }),
    getterHit, deleted, deleteHit,
    Object.prototype.hasOwnProperty.call(Boolean.prototype, "__delete"),
    (function() { "use strict"; return delete (false).__delete; })()
].join("|"));

var tagThis;
Object.defineProperty(Boolean.prototype, Symbol.toStringTag, {
    configurable: true,
    get: function() { "use strict"; tagThis = this; return "CustomBoolean"; }
});
var localeGetterThis;
var localeCallThis;
function localeMethod() { "use strict"; localeCallThis = this; return this; }
Object.defineProperty(Boolean.prototype, "toString", {
    configurable: true,
    get: function() { "use strict"; localeGetterThis = this; return localeMethod; }
});
var customTag = Object.prototype.toString.call(false);
var customLocale = Object.prototype.toLocaleString.call(false);
print("custom-object-methods=" + [
    customTag, typeof tagThis, tagThis instanceof Boolean, tagThis.valueOf(),
    customLocale, localeGetterThis === false, localeCallThis === false
].join("|"));
