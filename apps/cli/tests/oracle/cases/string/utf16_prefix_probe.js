// mode: print
function hex(value) {
    value = String(value);
    var out = "";
    for (var i = 0; i < value.length; i++)
        out += ("0000" + value.charCodeAt(i).toString(16)).slice(-4);
    return out;
}
function render(value) {
    if (value === undefined) return "u";
    if (typeof value === "number" && Number.isNaN(value)) return "nan";
    if (typeof value === "string") return "s:" + hex(value);
    return String(value);
}
function flags(object, key) {
    var d = Object.getOwnPropertyDescriptor(object, key);
    return (d.writable ? "1" : "0") + (d.enumerable ? "1" : "0") +
           (d.configurable ? "1" : "0");
}
function signature(fn) {
    return hex(fn.name) + ":" + fn.length + ":" +
           Reflect.ownKeys(fn).map(hex).join(",") + ":" + constructable(fn);
}
function constructable(fn) {
    try { Reflect.construct(function () {}, [], fn); return true; }
    catch (_) { return false; }
}
function observe(thunk) {
    try { return "ok:" + render(thunk()); }
    catch (error) { return "throw:" + hex(error.name) + ":" + hex(error.message); }
}

var names = ["length", "at", "charCodeAt", "charAt", "concat", "codePointAt",
             "isWellFormed", "toWellFormed", "toString", "valueOf"];
var methods = names.slice(1, 8);
var filtered = Reflect.ownKeys(String.prototype).filter(function (key) {
    return names.indexOf(key) >= 0;
});
print("surface=" + filtered.map(hex).join(",") + "|" +
      names.slice(0, 8).map(function (name) { return flags(String.prototype, name); }).join("|"));
print("metadata=" + methods.map(function (name) {
    return signature(String.prototype[name]);
}).join("|"));

var source = "A\ud83d\ude00\ud800Z";
var indices = [undefined, NaN, -0, 0, 1, 2, 3, 4, 5, -1, -2, -5, -6,
               1.9, -1.9, Infinity, -Infinity, 2147483648, -2147483649];
print("at=" + indices.map(function (index) { return render(String.prototype.at.call(source, index)); }).join("|"));
print("charAt=" + indices.map(function (index) { return render(String.prototype.charAt.call(source, index)); }).join("|"));
print("charCodeAt=" + indices.map(function (index) { return render(String.prototype.charCodeAt.call(source, index)); }).join("|"));
print("codePointAt=" + indices.map(function (index) { return render(String.prototype.codePointAt.call(source, index)); }).join("|"));

var conversionLog = "";
var receiver = {};
Object.defineProperty(receiver, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { conversionLog += "r:" + hint + ","; return source; }
});
var indexObject = {};
Object.defineProperty(indexObject, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) { conversionLog += "i:" + hint + ","; return 1.9; }
});
var ordered = String.prototype.at.call(receiver, indexObject);
print("order=" + hex(conversionLog) + "|" + render(ordered));

var genericWrapper = Object.prototype.valueOf.call("xy");
print("generic=" + [render(String.prototype.at.call("x")),
      render(String.prototype.at.call(123, 1)),
      render(String.prototype.charAt.call(true, 0)),
      render(String.prototype.codePointAt.call(1n, 0)),
      render(String.prototype.at.call(genericWrapper, -1))].join("|"));

var concatLog = "";
var fallback = {};
var concatReceiver = {
    toString: function () { concatLog += "rt,"; return fallback; },
    valueOf: function () { concatLog += "rv,"; return "R"; }
};
var argA = { toString: function () { concatLog += "at,"; return "A"; } };
var argB = {
    toString: function () { concatLog += "bt,"; return fallback; },
    valueOf: function () { concatLog += "bv,"; return "B"; }
};
var argC = { toString: function () { concatLog += "c,"; throw new RangeError("C"); } };
var argD = { toString: function () { concatLog += "d,"; return "D"; } };
var noArgs = String.prototype.concat.call("R");
var concatenated = String.prototype.concat.call(concatReceiver, argA, argB);
var successLog = concatLog;
concatLog = "";
var thrown = observe(function () {
    return String.prototype.concat.call(concatReceiver, argA, argC, argD);
});
print("concat=" + [render(noArgs), render(concatenated), hex(successLog),
      thrown, hex(concatLog)].join("|"));

var wellInputs = ["", "abc", "\ud83d\ude00", "\ud800", "\udc00",
                  "\udc00\ud800", "A\ud800B\udc00C", "\ud800\udc00\ud800\udc00"];
var well = wellInputs.map(function (value) {
    return String.prototype.isWellFormed.call(value) + ":" +
           hex(String.prototype.toWellFormed.call(value));
}).join("|");
var wellLog = "";
var wellReceiver = { toString: function () { wellLog += "r,"; return "\ud800X"; } };
var ignored = { toString: function () { wellLog += "arg,"; throw 1; } };
var wellBoolean = String.prototype.isWellFormed.call(wellReceiver, ignored);
var wellString = String.prototype.toWellFormed.call(wellReceiver, ignored);
print("well=" + well + "|ignored:" + wellBoolean + ":" + hex(wellString) + ":" + hex(wellLog));

print("errors=" + [
    observe(function () { return String.prototype.at.call(null, 0); }),
    observe(function () { return String.prototype.at.call(undefined, 0); }),
    observe(function () { return String.prototype.at.call(Symbol("receiver"), 0); }),
    observe(function () { return String.prototype.at.call("x", 1n); }),
    observe(function () { return String.prototype.at.call("x", Symbol("index")); }),
    observe(function () { return String.prototype.concat.call("x", Symbol("argument")); })
].join("|"));
