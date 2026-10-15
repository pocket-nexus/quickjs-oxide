// mode: print
function units(s) {
    var out = [];
    for (var i = 0; i < s.length; i++)
        out.push(("0000" + s.charCodeAt(i).toString(16)).slice(-4));
    return out.join(",");
}
var symbolA = Symbol("a"), symbolB = Symbol("b");
function token(key) {
    if (key === symbolA) return "@a";
    if (key === symbolB) return "@b";
    return key;
}
var ordered = {};
ordered.beta = 1;
ordered["4294967295"] = 2;
ordered["2147483648"] = 3;
ordered["01"] = 4;
ordered["4294967294"] = 5;
ordered["0"] = 6;
ordered["-0"] = 7;
ordered[symbolA] = 8;
ordered["2"] = 9;
ordered[symbolB] = 10;
print("keys=" + Reflect.ownKeys(ordered).map(token).join("|"));

var moved = { a: 1, b: 2, c: 3 };
delete moved.a;
moved.a = 4;
print("delete=" + Reflect.ownKeys(moved).join("|"));

var defaults = {};
Object.defineProperty(defaults, "x", { value: 7 });
var d = Object.getOwnPropertyDescriptor(defaults, "x");
print("defaults=" + d.value + "," + Number(d.writable) + "," +
      Number(d.enumerable) + "," + Number(d.configurable));

var frozen = {};
Object.defineProperty(frozen, "nan", { value: NaN, enumerable: true });
Object.defineProperty(frozen, "zero", { value: 0, enumerable: true });
print("frozen=" + Reflect.defineProperty(frozen, "nan", { value: NaN }) + "," +
      Reflect.defineProperty(frozen, "zero", { value: -0 }));

var parent = {};
Object.defineProperty(parent, "w", { value: 1, writable: true, configurable: true });
Object.defineProperty(parent, "r", { value: 1, writable: false, configurable: true });
var child = Object.create(parent);
print("inherited=" + Reflect.set(child, "w", 2) + "," +
      Reflect.set(child, "r", 2) + "," + child.w + "," + child.r);

var receiverTarget = {};
Object.defineProperty(receiverTarget, "x", {
    value: 1, writable: true, configurable: true
});
var receiver = {};
print("receiver=" + Reflect.set(receiverTarget, "x", 2, receiver) + "," +
      receiverTarget.x + "," + receiver.x);

var fixed = Object.create(null);
Object.preventExtensions(fixed);
var first = {}, second = {};
print("proto=" + Reflect.setPrototypeOf(fixed, null) + "," +
      Reflect.setPrototypeOf(fixed, {}) + "," +
      Reflect.setPrototypeOf(first, second) + "," +
      Reflect.setPrototypeOf(second, first));

var surrogate = {};
surrogate["\uD800"] = 1;
surrogate["\uFFFD"] = 2;
surrogate["\uD801"] = 3;
print("surrogates=" + Reflect.ownKeys(surrogate).map(units).join("|"));

var registry = Symbol.for("Symbol.iterator");
print("symbols=" + String(Symbol.keyFor(Symbol.iterator)) + "," +
      Symbol.keyFor(registry) + "," + (Symbol.iterator === registry));
