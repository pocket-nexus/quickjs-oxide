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
function isConstructor(value) {
    try {
        Reflect.construct(function () {}, [], value);
        return true;
    } catch (_) {
        return false;
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
    fileName: true,
    lineNumber: true,
    columnNumber: true,
};
var implementedKeys = Reflect.ownKeys(fp).filter(function (key) {
    return key === Symbol.hasInstance ||
           (typeof key === "string" && implementedNames[key] === true);
});
print("fp-keys=" + implementedKeys.map(String).join(","));

var debugNames = ["fileName", "lineNumber", "columnNumber"];
var getters = debugNames.map(function (name) {
    var descriptor = Object.getOwnPropertyDescriptor(fp, name);
    var getter = descriptor.get;
    var lengthDescriptor = Object.getOwnPropertyDescriptor(getter, "length");
    var nameDescriptor = Object.getOwnPropertyDescriptor(getter, "name");
    print("getter-meta=" + name + ":" + typeof getter + ":" + String(descriptor.set) +
          ":" + Number(descriptor.enumerable) + "," + Number(descriptor.configurable) +
          "|length:" + getter.length + ":" + Number(lengthDescriptor.writable) + "," +
          Number(lengthDescriptor.enumerable) + "," + Number(lengthDescriptor.configurable) +
          "|name:" + getter.name + ":" + Number(nameDescriptor.writable) + "," +
          Number(nameDescriptor.enumerable) + "," + Number(nameDescriptor.configurable) +
          "|keys:" + Reflect.ownKeys(getter).map(String).join(",") +
          "|proto:" + (Object.getPrototypeOf(getter) === fp) +
          "|constructor:" + isConstructor(getter) +
          "|source:" + hex(fp.toString.call(getter)));
    return getter;
});
print("getter-identities=" +
      (getters[0] !== getters[1] && getters[1] !== getters[2] && getters[0] !== getters[2]));

var primary = eval("\n  (function named(){})");
print("primary=" + String(getters[0].call(primary)) + "|" +
      String(getters[1].call(primary)) + "|" + String(getters[2].call(primary)));
print("primary-direct=" + String(primary.fileName) + "|" +
      String(primary.lineNumber) + "|" + String(primary.columnNumber));
print("primary-source=" + hex(fp.toString.call(primary)));
Object.defineProperty(primary, "name", { value: "renamed" });
print("renamed=" + String(primary.fileName) + "|" + String(primary.lineNumber) + "|" +
      String(primary.columnNumber) + "|" + hex(fp.toString.call(primary)));

var anonymous = eval("\n  (function(){})");
print("anonymous=" + String(anonymous.fileName) + "|" +
      String(anonymous.lineNumber) + "|" + String(anonymous.columnNumber) + "|" +
      hex(fp.toString.call(anonymous)));
print("command-line-file=" + String(hex.fileName));

var tabbed = eval("\t(function tabbed(){})");
var carriage = eval("/*x*/\r(function carriage(){})");
var crlf = eval("/*x*/\r\n(function crlf(){})");
var unicode = eval("/*😀*/\t(function unicode(){})");
var lineSeparator = eval("/*x*/\u2028(function lineSeparator(){})");
print("positions=" +
      "tab:" + tabbed.lineNumber + ":" + tabbed.columnNumber +
      "|cr:" + carriage.lineNumber + ":" + carriage.columnNumber +
      "|crlf:" + crlf.lineNumber + ":" + crlf.columnNumber +
      "|unicode:" + unicode.lineNumber + ":" + unicode.columnNumber +
      "|u2028:" + lineSeparator.lineNumber + ":" + lineSeparator.columnNumber);

var outer = eval("\n(function outer(){ return function inner(){}; })");
var inner = outer();
print("nested=" + outer.fileName + ":" + outer.lineNumber + ":" + outer.columnNumber +
      "|" + inner.fileName + ":" + inner.lineNumber + ":" + inner.columnNumber);

var bound = primary.bind(null);
var ordinary = {};
var receivers = [undefined, null, 1, ordinary, fp, bound, getters[0]];
print("invalid-receivers=" + getters.map(function (getter) {
    return receivers.map(function (receiver) {
        return String(getter.call(receiver, 1, 2));
    }).join(",");
}).join("|"));
