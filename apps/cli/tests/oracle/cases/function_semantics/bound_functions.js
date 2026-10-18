// A bound function's [[Prototype]] is its target's (10.4.1.3, BoundFunctionCreate).
// case: a bound function inherits its target's prototype
(function () {
    class Base {}
    class Derived extends Base {}
    var plain = function () {};
    Object.setPrototypeOf(plain, null);
    var bound = Function.prototype.bind.call(plain);
    return [
        Object.getPrototypeOf(Derived.bind(null)) === Base,
        Object.getPrototypeOf(bound) === null,
        Object.getPrototypeOf(function () {}.bind(null)) === Function.prototype
    ].join("|");
})()
// case: a proxy target's getPrototypeOf trap runs before the bound function exists
(function () {
    var log = [];
    var target = new Proxy(function () {}, {
        getPrototypeOf: function () { log.push("getPrototypeOf"); return Array.prototype; },
        getOwnPropertyDescriptor: function (t, key) {
            log.push("gOPD:" + String(key));
            return Reflect.getOwnPropertyDescriptor(t, key);
        }
    });
    var bound = Function.prototype.bind.call(target);
    return log.join(",") + "|" + (Object.getPrototypeOf(bound) === Array.prototype);
})()
// case: a bound function prints as a NativeFunction
(function () {
    function named() {}
    return named.bind(null).toString();
})()
