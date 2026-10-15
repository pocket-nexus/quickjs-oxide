// mode: print
// flags:
// flags: --strip-source
// flags: -s
function hex(value) {
    var out = "";
    for (var i = 0; i < value.length; i++)
        out += value.charCodeAt(i).toString(16).padStart(4, "0");
    return out;
}
function bits(descriptor) {
    return Number(descriptor.writable) + "," +
           Number(descriptor.enumerable) + "," +
           Number(descriptor.configurable);
}
function text(value) {
    return value === undefined ? "undefined" : String(value);
}
function isConstructor(value) {
    try {
        Reflect.construct(function () {}, [], value);
        return true;
    } catch (_) {
        return false;
    }
}
function firstTwoFrames(stack) {
    return String(stack).split("\n").slice(0, 2).join("|");
}
var fp = Function.prototype;
var objectPrototype = Object.prototype;
var lengthDescriptor = Object.getOwnPropertyDescriptor(Function, "length");
var nameDescriptor = Object.getOwnPropertyDescriptor(Function, "name");
var prototypeDescriptor = Object.getOwnPropertyDescriptor(Function, "prototype");
var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "Function");
var constructorDescriptor = Object.getOwnPropertyDescriptor(fp, "constructor");
print("ctor-keys=" + Reflect.ownKeys(Function).map(String).join(","));
print("ctor-meta=" + Function.length + "|" + Function.name + "|" +
      bits(lengthDescriptor) + "|" + bits(nameDescriptor) + "|" +
      (prototypeDescriptor.value === fp) + "|" + bits(prototypeDescriptor));
print("global=" + (globalDescriptor.value === Function) + "|" + bits(globalDescriptor));
var implementedGlobals = ["Error", "EvalError", "RangeError", "ReferenceError",
                          "SyntaxError", "TypeError", "URIError", "InternalError", "Function"];
print("global-order=" + Reflect.ownKeys(globalThis).filter(function (key) {
    return implementedGlobals.indexOf(key) >= 0;
}).join(","));
print("fp-keys=" + Reflect.ownKeys(fp).map(String).join(","));
print("fp-constructor=" + (constructorDescriptor.value === Function) + "|" +
      bits(constructorDescriptor));
print("graph=" + [
    Object.getPrototypeOf(Function) === fp,
    Object.getPrototypeOf(fp) === objectPrototype,
    isConstructor(Function),
    Function instanceof Function,
    fp instanceof Function
].join(","));
print("native-source=" + hex(fp.toString.call(Function)));

var empty = Function();
print("empty=" + empty.name + "|" + empty.length + "|" + text(empty()) + "|" +
      hex(fp.toString.call(empty)) + "|" + text(empty.fileName) + "|" +
      text(empty.lineNumber) + "|" + text(empty.columnNumber) + "|" +
      (Object.getPrototypeOf(empty) === fp));

var add = Function("a", "b", "return a + b");
print("multi=" + add.name + "|" + add.length + "|" + add(20, 22) + "|" +
      hex(fp.toString.call(add)) + "|" + text(add.fileName) + "|" +
      text(add.lineNumber) + "|" + text(add.columnNumber));
var power = Function("a", "b", "return a ** b");
print("power=" + power(2, 10));

var viaNew = new Function("return 9");
print("function-new=" + viaNew() + "|" + (Object.getPrototypeOf(viaNew) === fp));
var maker = Function("return 1");
var instance = new maker();
print("dynamic-new=" + typeof instance + "|" +
      (Object.getPrototypeOf(instance) === maker.prototype));

var NewTarget = Function("");
var customPrototype = {};
NewTarget.prototype = customPrototype;
var custom = Reflect.construct(Function, ["return 5"], NewTarget);
var customResult = (Object.getPrototypeOf(custom) === customPrototype) + "|" + custom();
NewTarget.prototype = 1;
var fallback = Reflect.construct(Function, ["return 6"], NewTarget);
print("new-target=" + customResult + "|" +
      (Object.getPrototypeOf(fallback) === fp) + "|" + fallback());

var duplicate = Function("a", "a", "return a");
print("sloppy-duplicate=" + duplicate.length + "|" + duplicate(1, 2));

var conversionLog = "";
var conversionCustomPrototype = {};
var conversionParameter = {
    toString: function () { conversionLog += "p"; return "a"; }
};
var conversionBody = {
    toString: function () { conversionLog += "b"; return "return a"; }
};
var ConversionNewTarget = Function.bind(null);
Object.defineProperty(ConversionNewTarget, "prototype", {
    get: function () { conversionLog += "x"; return conversionCustomPrototype; },
    configurable: true
});
var converted = Reflect.construct(
    Function, [conversionParameter, conversionBody], ConversionNewTarget);
print("conversion-success=" + conversionLog + "|" + converted(7));
conversionLog = "";
conversionParameter.toString = function () { conversionLog += "p"; return "a-"; };
try {
    Reflect.construct(Function, [conversionParameter, conversionBody], ConversionNewTarget);
    print("conversion-parse=missing");
} catch (error) {
    print("conversion-parse=" + conversionLog + "|" + error.name);
}
conversionLog = "";
conversionParameter.toString = function () { conversionLog += "t"; throw "stop"; };
try {
    Reflect.construct(Function, [conversionParameter, conversionBody], ConversionNewTarget);
    print("conversion-throw=missing");
} catch (error) {
    print("conversion-throw=" + conversionLog + "|" + error);
}

try {
    Function("a", "a", "\"use strict\"; return a");
    print("strict-error=missing");
} catch (error) {
    print("strict-error=" + error.name + ":" + error.message + "|" + text(error.fileName) + ":" +
          text(error.lineNumber) + ":" + text(error.columnNumber) + "|" +
          firstTwoFrames(error.stack));
}
try {
    Function("a-", "return 1");
    print("parameter-error=missing");
} catch (error) {
    print("parameter-error=" + error.name + ":" + error.message + "|" + text(error.fileName) + ":" +
          text(error.lineNumber) + ":" + text(error.columnNumber) + "|" +
          firstTwoFrames(error.stack));
}
try {
    Function("a", "return )");
    print("body-error=missing");
} catch (error) {
    print("body-error=" + error.name + ":" + error.message + "|" + text(error.fileName) + ":" +
          text(error.lineNumber) + ":" + text(error.columnNumber) + "|" +
          firstTwoFrames(error.stack));
}
try {
    Function("null", "return 1");
    print("formal-error=missing");
} catch (error) {
    print("formal-error=" + error.name + ":" + error.message + "|" + text(error.fileName) + ":" +
          text(error.lineNumber) + ":" + text(error.columnNumber) + "|" +
          firstTwoFrames(error.stack));
}
try {
    Function("({[if (0) 0;]})");
    print("computed-if-error=missing");
} catch (error) {
    print("computed-if-error=" + error.name + ":" + error.message + "|" +
          text(error.fileName) + ":" + text(error.lineNumber) + ":" +
          text(error.columnNumber) + "|" + firstTwoFrames(error.stack));
}
try {
    Function("[for (x of [1]) x]");
    print("array-for-error=missing");
} catch (error) {
    print("array-for-error=" + error.name + ":" + error.message + "|" +
          text(error.fileName) + ":" + text(error.lineNumber) + ":" +
          text(error.columnNumber) + "|" + firstTwoFrames(error.stack));
}
