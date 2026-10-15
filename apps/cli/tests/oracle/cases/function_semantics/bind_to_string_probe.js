// mode: print
function bit(value) { return value ? 1 : 0; }
function hex(value) {
    var out = "";
    for (var index = 0; index < value.length; index++) {
        var unit = value.charCodeAt(index).toString(16);
        out += ("0000" + unit).slice(-4);
    }
    return out;
}
function valueType(value) {
    return typeof value;
}
function descriptorText(key, descriptor) {
    if (Object.prototype.hasOwnProperty.call(descriptor, "value")) {
        return String(key) + ":data:" + valueType(descriptor.value) + ":" +
               bit(descriptor.writable) + "," + bit(descriptor.enumerable) + "," +
               bit(descriptor.configurable);
    }
    return String(key) + ":accessor:" + bit(descriptor.enumerable) + "," +
           bit(descriptor.configurable);
}
function show(value) {
    if (value === undefined) return "undefined";
    if (typeof value === "boolean" || typeof value === "number" ||
        typeof value === "bigint") return String(value);
    if (typeof value === "string") return "string:" + value;
    return "unexpected";
}
function observe(thunk) {
    try {
        return show(thunk());
    } catch (error) {
        if (typeof error === "string") return "throw-string:" + error;
        return "throw:" + error.name + "|" + error.message;
    }
}

var fp = Function.prototype;
var implementedNames = {
    length: true,
    name: true,
    caller: true,
    arguments: true,
    call: true,
    apply: true,
    bind: true,
    toString: true,
};
var implementedKeys = Reflect.ownKeys(fp).filter(function (key) {
    return key === Symbol.hasInstance ||
           (typeof key === "string" && implementedNames[key] === true);
});
print("fp-keys=" + implementedKeys.map(String).join(","));
print("fp-desc=" + implementedKeys.map(function (key) {
    return descriptorText(key, Object.getOwnPropertyDescriptor(fp, key));
}).join("|"));
var bindDescriptor = Object.getOwnPropertyDescriptor(fp, "bind");
var toStringDescriptor = Object.getOwnPropertyDescriptor(fp, "toString");
var bindLengthDescriptor = Object.getOwnPropertyDescriptor(bindDescriptor.value, "length");
var bindNameDescriptor = Object.getOwnPropertyDescriptor(bindDescriptor.value, "name");
var toStringLengthDescriptor = Object.getOwnPropertyDescriptor(toStringDescriptor.value, "length");
var toStringNameDescriptor = Object.getOwnPropertyDescriptor(toStringDescriptor.value, "name");
print("builtin-meta=" +
      "bind-length:" + bindLengthDescriptor.value + ":" + bit(bindLengthDescriptor.writable) +
      "," + bit(bindLengthDescriptor.enumerable) + "," + bit(bindLengthDescriptor.configurable) +
      "|bind-name:" + bindNameDescriptor.value + ":" + bit(bindNameDescriptor.writable) +
      "," + bit(bindNameDescriptor.enumerable) + "," + bit(bindNameDescriptor.configurable) +
      "|toString-length:" + toStringLengthDescriptor.value + ":" + bit(toStringLengthDescriptor.writable) +
      "," + bit(toStringLengthDescriptor.enumerable) + "," + bit(toStringLengthDescriptor.configurable) +
      "|toString-name:" + toStringNameDescriptor.value + ":" + bit(toStringNameDescriptor.writable) +
      "," + bit(toStringNameDescriptor.enumerable) + "," + bit(toStringNameDescriptor.configurable));

var anonymous = (0, function  ( a , b ) { return a + b; });
var named = (0, function named /*kept*/ (value) {
  return value;
});
var anonymousSource = fp.toString.call(anonymous);
var namedSource = fp.toString.call(named);
Object.defineProperty(anonymous, "name", { value: "changed anonymous" });
Object.defineProperty(named, "name", { value: "changed named" });
print("source-anonymous=" + hex(anonymousSource) + "|renamed:" +
      hex(fp.toString.call(anonymous)));
print("source-named=" + hex(namedSource) + "|renamed:" +
      hex(fp.toString.call(named)));

function nativeTarget(a) {}
var boundNativeTarget = nativeTarget.bind(null);
print("native-template=" +
      "prototype:" + hex(fp.toString.call(fp)) +
      "|call:" + hex(fp.toString.call(fp.call)) +
      "|apply:" + hex(fp.toString.call(fp.apply)) +
      "|bind:" + hex(fp.toString.call(fp.bind)) +
      "|toString:" + hex(fp.toString.call(fp.toString)) +
      "|bound:" + hex(fp.toString.call(boundNativeTarget)));
print("invalid-object=" + observe(function () { return fp.toString.call({}); }));
print("invalid-number=" + observe(function () { return fp.toString.call(1); }));

Object.defineProperty(fp.call, "name", { value: 17 });
print("native-number-name=" + hex(fp.toString.call(fp.call)));
Object.defineProperty(fp.apply, "name", { value: Symbol("native-name") });
print("native-symbol-name=" + observe(function () { return fp.toString.call(fp.apply); }));
var nativeNameSentinel = {};
Object.defineProperty(fp.bind, "name", {
    get: function () { throw nativeNameSentinel; },
});
var nativeNameIdentity = false;
try { fp.toString.call(fp.bind); } catch (error) { nativeNameIdentity = error === nativeNameSentinel; }
print("native-getter-throw=" + nativeNameIdentity);

function withLength(value, boundCount) {
    function lengthTarget(a, b, c, d, e, f) {}
    Object.defineProperty(lengthTarget, "length", { value: value });
    if (boundCount === 2) return fp.bind.call(lengthTarget, null, 1, 2).length;
    return fp.bind.call(lengthTarget, null, 1).length;
}
function withoutLength() {
    function lengthTarget() {}
    delete lengthTarget.length;
    return fp.bind.call(lengthTarget, null, 1).length;
}
print("bind-length=" +
      "int:" + withLength(5, 2) +
      "|fraction:" + withLength(5.9, 2) +
      "|infinity:" + withLength(Infinity, 1) +
      "|negative-infinity:" + withLength(-Infinity, 1) +
      "|nan:" + withLength(NaN, 1) +
      "|string:" + withLength("5", 1) +
      "|bigint:" + withLength(5n, 1) +
      "|symbol:" + withLength(Symbol("length"), 1) +
      "|absent:" + withoutLength());

function withName(value) {
    function nameTarget() {}
    Object.defineProperty(nameTarget, "name", { value: value });
    return fp.bind.call(nameTarget, null);
}
var namedBound = withName("renamed");
var numberNamedBound = withName(17);
var symbolNamedBound = withName(Symbol("name"));
var undefinedNamedBound = withName(undefined);
var boundNameDescriptor = Object.getOwnPropertyDescriptor(namedBound, "name");
print("bind-name=" +
      "string:" + namedBound.name +
      "|number:" + numberNamedBound.name +
      "|symbol:" + symbolNamedBound.name +
      "|undefined:" + undefinedNamedBound.name +
      "|desc:" + bit(boundNameDescriptor.writable) + "," +
      bit(boundNameDescriptor.enumerable) + "," + bit(boundNameDescriptor.configurable));

var getterOrder = 0;
function orderedTarget(a, b, c) {}
Object.defineProperty(orderedTarget, "length", {
    get: function () { getterOrder = getterOrder * 10 + 1; return 3; },
});
Object.defineProperty(orderedTarget, "name", {
    get: function () { getterOrder = getterOrder * 10 + 2; return "ordered"; },
});
var orderedBound = fp.bind.call(orderedTarget, null, 1);
print("bind-getter-order=" + getterOrder + "|length:" + orderedBound.length +
      "|name:" + orderedBound.name);

var lengthThrowSentinel = {};
var lengthThrowLateName = 0;
function lengthThrowTarget() {}
Object.defineProperty(lengthThrowTarget, "length", {
    get: function () { throw lengthThrowSentinel; },
});
Object.defineProperty(lengthThrowTarget, "name", {
    get: function () { lengthThrowLateName++; return "late"; },
});
var lengthThrowIdentity = false;
try { fp.bind.call(lengthThrowTarget, null); }
catch (error) { lengthThrowIdentity = error === lengthThrowSentinel; }
print("bind-length-throw=" + lengthThrowIdentity + "|late-name:" + lengthThrowLateName);

var nameThrowSentinel = {};
var nameThrowOrder = 0;
function nameThrowTarget() {}
Object.defineProperty(nameThrowTarget, "length", {
    get: function () { nameThrowOrder = nameThrowOrder * 10 + 1; return 2; },
});
Object.defineProperty(nameThrowTarget, "name", {
    get: function () { nameThrowOrder = nameThrowOrder * 10 + 2; throw nameThrowSentinel; },
});
var nameThrowIdentity = false;
try { fp.bind.call(nameThrowTarget, null); }
catch (error) { nameThrowIdentity = error === nameThrowSentinel; }
print("bind-name-throw=" + nameThrowIdentity + "|order:" + nameThrowOrder);

var inheritedLengthReads = 0;
var inheritedLengthPrototype = Object.create(fp);
Object.defineProperty(inheritedLengthPrototype, "length", {
    get: function () { inheritedLengthReads++; return 99; },
});
function inheritedLengthTarget() {}
delete inheritedLengthTarget.length;
Object.setPrototypeOf(inheritedLengthTarget, inheritedLengthPrototype);
var inheritedLengthBound = fp.bind.call(inheritedLengthTarget, null, 1);
print("bind-inherited-length=reads:" + inheritedLengthReads +
      "|length:" + inheritedLengthBound.length);

function callTarget(a, b, c) {
    "use strict";
    return a * 100 + b * 10 + c;
}
var callReceiver = {};
var simpleBound = fp.bind.call(callTarget, callReceiver, 1);
var firstBound = fp.bind.call(callTarget, callReceiver, 1);
var nestedBound = fp.bind.call(firstBound, {}, 2);
print("bound-call=simple:" + simpleBound(2, 3) +
      "|nested:" + nestedBound(3) +
      "|nested-name:" + nestedBound.name +
      "|nested-length:" + nestedBound.length);

function ConstructTarget() { return new.target; }
var ignoredBoundThis = {};
var ConstructBound = fp.bind.call(ConstructTarget, ignoredBoundThis);
var constructed = new ConstructBound();
function OtherTarget() {}
var explicitlyConstructed = Reflect.construct(ConstructBound, [], OtherTarget);
var constructArgMarker = {};
function ConstructArgTarget(value) { return value; }
var ConstructArgBound = fp.bind.call(ConstructArgTarget, null, constructArgMarker);
var constructedArg = new ConstructArgBound();
print("bound-construct=" +
      "arg:" + (constructedArg === constructArgMarker) +
      "|new-target-target:" + (constructed === ConstructTarget) +
      "|bound-own-prototype:" + Object.prototype.hasOwnProperty.call(ConstructBound, "prototype") +
      "|explicit-new-target:" + (explicitlyConstructed === OtherTarget));

var instanceMarker = {};
var instanceOther = {};
function InstanceTarget() {}
Object.defineProperty(InstanceTarget, Symbol.hasInstance, {
    value: function (candidate) { return candidate === instanceMarker; },
    configurable: true,
});
var InstanceBound = fp.bind.call(InstanceTarget, null);
var customTrue = instanceMarker instanceof InstanceBound;
var customFalse = instanceOther instanceof InstanceBound;
var instanceThrowSentinel = {};
Object.defineProperty(InstanceTarget, Symbol.hasInstance, {
    value: function () { throw instanceThrowSentinel; },
    configurable: true,
});
var instanceThrowIdentity = false;
try { instanceMarker instanceof InstanceBound; }
catch (error) { instanceThrowIdentity = error === instanceThrowSentinel; }
print("bound-has-instance=true:" + customTrue + "|false:" + customFalse +
      "|throw:" + instanceThrowIdentity);

function DeepInstanceTarget() {}
var DeepInstanceBound = DeepInstanceTarget;
for (var deepIndex = 0; deepIndex < 512; deepIndex++) {
    DeepInstanceBound = fp.bind.call(DeepInstanceBound, null);
}
print("deep-bound-has-instance=" +
      fp[Symbol.hasInstance].call(DeepInstanceBound, 1));

var targetFunctionPrototype = Object.create(fp);
function CrossRealmTarget() {}
Object.setPrototypeOf(CrossRealmTarget, targetFunctionPrototype);
var crossRealmBound = fp.bind.call(CrossRealmTarget, null);
print("cross-realm-prototype=" +
      (Object.getPrototypeOf(crossRealmBound) === fp));
