// mode: print
function hex(value) {
    value = String(value);
    var out = "";
    for (var i = 0; i < value.length; i++)
        out += ("0000" + value.charCodeAt(i).toString(16)).slice(-4);
    return out;
}
function flags(object, key) {
    var d = Object.getOwnPropertyDescriptor(object, key);
    return (d.writable ? "1" : "0") + (d.enumerable ? "1" : "0") +
           (d.configurable ? "1" : "0");
}
function signature(fn) {
    return hex(fn.name) + ":" + fn.length + ":" +
           Reflect.ownKeys(fn).map(hex).join(",");
}
function constructable(fn) {
    try { Reflect.construct(function () {}, [], fn); return true; }
    catch (_) { return false; }
}
function observe(thunk) {
    try { return "ok:" + hex(thunk()); }
    catch (error) { return "throw:" + hex(error.name) + ":" + hex(error.message); }
}

var source = "A\ud800B";
var ts = String.prototype.toString;
var vo = String.prototype.valueOf;
var filtered = Reflect.ownKeys(String.prototype).filter(function (key) {
    return key === "length" || key === "toString" || key === "valueOf";
});
print("surface=" + filtered.map(hex).join(",") + "|" +
      [flags(String.prototype, "length"), flags(String.prototype, "toString"),
       flags(String.prototype, "valueOf")].join("|"));
print("metadata=" + signature(ts) + "|" + signature(vo) + "|" +
      (ts === vo) + "|" + constructable(ts) + "|" + constructable(vo));

var wrapper = Object.prototype.valueOf.call(source);
var spoof = Object.create(String.prototype);
var changed = Object.prototype.valueOf.call(source);
Object.setPrototypeOf(changed, Object.prototype);
print("brand=" + [hex(ts.call(source)), hex(vo.call(wrapper)),
      hex(ts.call(String.prototype)), observe(function () { return ts.call(spoof); }),
      hex(vo.call(changed)), hex(Object.prototype.toString.call(changed))].join("|"));

var ordinary = Object.prototype.valueOf.call(source);
ordinary.toString = function () { return "override"; };
ordinary.valueOf = function () { return 7; };
print("override=" + [hex(ts.call(ordinary)), hex(vo.call(ordinary)),
      hex(decodeURI(ordinary)), ordinary + 1, +ordinary].join("|"));

var strictGetThis;
Object.defineProperty(String.prototype, "__strictGet", {
    configurable: true,
    get: function () { "use strict"; strictGetThis = this; return this; }
});
var getResult = source.__strictGet;
print("get=" + [hex(getResult), strictGetThis === source].join("|"));

var strictSetThis, strictSetValue;
Object.defineProperty(String.prototype, "__strictSet", {
    configurable: true,
    set: function (value) { "use strict"; strictSetThis = this; strictSetValue = value; }
});
var setResult = source.__strictSet = 7;
print("set=" + [setResult, strictSetThis === source, strictSetValue,
      observe(function () { "use strict"; return source.__missing = 1; }),
      observe(function () { "use strict"; return source.length = 1; })].join("|"));

var boxA = Object.prototype.valueOf.call(source);
var boxB = Object.prototype.valueOf.call(source);
print("object=" + [hex(Object.prototype.toString.call(source)),
      hex(Object.prototype.toLocaleString.call(source)), boxA !== boxB,
      Object.getPrototypeOf(boxA) === String.prototype, hex(vo.call(boxA)),
      Reflect.ownKeys(boxA).map(hex).join(","), flags(boxA, "length")].join("|"));

Object.defineProperty(String.prototype, Symbol.toStringTag, {
    configurable: true, value: "CustomString"
});
var customTag = Object.prototype.toString.call(source);
Object.defineProperty(String.prototype, Symbol.toStringTag, { value: 1 });
var nonStringTag = Object.prototype.toString.call(source);
delete String.prototype[Symbol.toStringTag];
var deletedTag = Object.prototype.toString.call(source);
print("tags=" + [hex(customTag), hex(nonStringTag), hex(deletedTag)].join("|"));

var tagThis;
Object.defineProperty(String.prototype, Symbol.toStringTag, {
    configurable: true,
    get: function () { "use strict"; tagThis = this; throw new RangeError("tag"); }
});
var tagThrow = observe(function () { return Object.prototype.toString.call(source); });
delete String.prototype[Symbol.toStringTag];
print("tag-throw=" + [tagThrow, hex(vo.call(tagThis)),
      Object.getPrototypeOf(tagThis) === String.prototype].join("|"));
