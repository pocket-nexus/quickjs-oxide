// case: pairing / a getter and setter with the same key merge into one accessor
            (function () {
                var object = {
                    get answer() { return this.stored + 1; },
                    set answer(value) { this.stored = value - 1; }
                };
                object.answer = 42;
                var descriptor = Object.getOwnPropertyDescriptor(object, "answer");
                return [
                    object.answer,
                    object.stored,
                    typeof descriptor.get,
                    typeof descriptor.set,
                    descriptor.enumerable,
                    descriptor.configurable
                ].join("|");
            })()
// case: pairing / repeated accessor halves replace only their matching half
            (function () {
                var object = {
                    set value(input) { this.first = input; },
                    get value() { return 1; },
                    get value() { return 42; },
                    set other(input) { this.second = input; },
                    get other() { return 42; },
                    set other(input) { this.third = input; }
                };
                var value = Object.getOwnPropertyDescriptor(object, "value");
                var other = Object.getOwnPropertyDescriptor(object, "other");
                value.set.call(object, 20);
                other.set.call(object, 22);
                return [
                    value.get.call(object),
                    object.first,
                    other.get.call(object),
                    object.second,
                    object.third
                ].join("|");
            })()
// case: pairing / data and method definitions replace accessor descriptor kinds in source order
            (function () {
                var a = { x: 1, get x() { return 42; } };
                var b = { get x() { return 1; }, x: 42 };
                var c = { x() { return 1; }, set x(value) { this.seen = value; } };
                var d = { get x() { return 1; }, x() { return 42; } };
                var ad = Object.getOwnPropertyDescriptor(a, "x");
                var bd = Object.getOwnPropertyDescriptor(b, "x");
                var cd = Object.getOwnPropertyDescriptor(c, "x");
                var dd = Object.getOwnPropertyDescriptor(d, "x");
                c.x = 42;
                return [
                    a.x,
                    typeof ad.get,
                    ad.set === undefined,
                    b.x,
                    bd.value,
                    cd.get === undefined,
                    typeof cd.set,
                    c.seen,
                    dd.value(),
                    dd.writable
                ].join("|");
            })()
// case: property-keys / fixed computed string numeric keyword and Symbol keys infer accessor names
            (function () {
                var described = Symbol("named");
                var missing = Symbol();
                var empty = Symbol("");
                var object = {
                    get fixed() {},
                    set "quoted"(value) {},
                    get 7() {},
                    set if(value) {},
                    get [described]() {},
                    set [described](value) {},
                    get [missing]() {},
                    set [empty](value) {}
                };
                return [
                    Object.getOwnPropertyDescriptor(object, "fixed").get.name,
                    Object.getOwnPropertyDescriptor(object, "quoted").set.name,
                    Object.getOwnPropertyDescriptor(object, "7").get.name,
                    Object.getOwnPropertyDescriptor(object, "if").set.name,
                    Object.getOwnPropertyDescriptor(object, described).get.name,
                    Object.getOwnPropertyDescriptor(object, described).set.name,
                    Object.getOwnPropertyDescriptor(object, missing).get.name,
                    Object.getOwnPropertyDescriptor(object, empty).set.name
                ].join("|");
            })()
// case: grammar / a line terminator is allowed after contextual get and set
            (function () {
                var object = {
                    get
                    answer() { return 42; },
                    set
                    answer(value) { this.saved = value; }
                };
                var descriptor = Object.getOwnPropertyDescriptor(object, "answer");
                descriptor.set.call(object, 41);
                return [
                    descriptor.get.call(object),
                    object.saved,
                    descriptor.get.name,
                    descriptor.set.name
                ].join("|");
            })()
// case: property-keys / computed accessor keys are converted exactly once per definition
            (function () {
                var log = "";
                var getterKey = {
                    [Symbol.toPrimitive]() { log += "g"; return "answer"; }
                };
                var setterKey = {
                    [Symbol.toPrimitive]() { log += "s"; return "answer"; }
                };
                var object = {
                    get [getterKey]() { return this.saved; },
                    set [setterKey](value) { this.saved = value; }
                };
                object.answer = 42;
                return log + "|" + object.answer;
            })()
// case: function-object / accessor function metadata and property descriptors match QuickJS
            (function () {
                var object = {
                    get answer() { return 42; },
                    set answer(value) {}
                };
                var property = Object.getOwnPropertyDescriptor(object, "answer");
                var getName = Object.getOwnPropertyDescriptor(property.get, "name");
                var setName = Object.getOwnPropertyDescriptor(property.set, "name");
                var getLength = Object.getOwnPropertyDescriptor(property.get, "length");
                var setLength = Object.getOwnPropertyDescriptor(property.set, "length");
                return [
                    property.get.name,
                    property.set.name,
                    property.get.length,
                    property.set.length,
                    "value" in property,
                    "writable" in property,
                    property.enumerable,
                    property.configurable,
                    getName.writable,
                    getName.enumerable,
                    getName.configurable,
                    setName.writable,
                    setName.enumerable,
                    setName.configurable,
                    getLength.writable,
                    getLength.enumerable,
                    getLength.configurable,
                    setLength.writable,
                    setLength.enumerable,
                    setLength.configurable
                ].join("|");
            })()
// case: call-environment / accessors own dynamic this arguments and new target
            (function () {
                var descriptor = Object.getOwnPropertyDescriptor({
                    get answer() {
                        return this.base + arguments.length +
                            (typeof new.target === "undefined" ? 1 : 0);
                    },
                    set answer(value) {
                        this.saved = value + arguments.length +
                            (typeof new.target === "undefined" ? 1 : 0);
                    }
                }, "answer");
                var receiver = { base: 41 };
                var getter = descriptor.get.call(receiver);
                descriptor.set.call(receiver, 40);
                return getter + "|" + receiver.saved;
            })()
// case: call-environment / accessors inherit strictness rather than becoming strict automatically
            (function () {
                var sloppy = Object.getOwnPropertyDescriptor({
                    get x() { return this; }
                }, "x").get;
                var strict = (function () {
                    "use strict";
                    return Object.getOwnPropertyDescriptor({
                        get x() { return this; }
                    }, "x").get;
                })();
                return (sloppy() === globalThis) + "|" +
                    (strict() === undefined);
            })()
// case: direct-eval / direct eval resolves accessor-owned this arguments and new target
            (function () {
                var descriptor = Object.getOwnPropertyDescriptor({
                    get answer() {
                        return eval(
                            "this.base + arguments.length + " +
                            "(typeof new.target === 'undefined' ? 1 : 0)"
                        );
                    },
                    set answer(value) {
                        this.saved = eval(
                            "arguments[0] + arguments.length + " +
                            "(typeof new.target === 'undefined' ? 1 : 0)"
                        );
                    }
                }, "answer");
                var receiver = { base: 41 };
                var getter = descriptor.get.call(receiver);
                descriptor.set.call(receiver, 40);
                return getter + "|" + receiver.saved;
            })()
// case: scope / an accessor property name does not create a private function binding
            (function () {
                var answer = 41;
                return ({ get answer() { return answer + 1; } }).answer;
            })()
// case: parameters / a simple setter accepts one parameter and a trailing comma
            (function () {
                var descriptor = Object.getOwnPropertyDescriptor({
                    set answer(value,) { this.saved = value; }
                }, "answer");
                var receiver = {};
                descriptor.set.call(receiver, 42);
                return receiver.saved + "|" + descriptor.set.length;
            })()
// case: parameters / getter and setter arity violations are syntax errors
            (function () {
                var sources = [
                    "({ get x(value) {} })",
                    "({ get x(,) {} })",
                    "({ set x() {} })",
                    "({ set x(left, right) {} })",
                    "({ set x(...values) {} })"
                ];
                var result = [];
                for (var index = 0; index < sources.length; index++) {
                    try { eval(sources[index]); result.push("accepted"); }
                    catch (error) { result.push(error.name); }
                }
                return result.join("|");
            })()
// case: function-object / getter and setter functions are callable non-constructors without prototypes
            (function () {
                var descriptor = Object.getOwnPropertyDescriptor({
                    get answer() { return 42; },
                    set answer(value) {}
                }, "answer");
                var getError = "none";
                var setError = "none";
                try { new descriptor.get(); } catch (error) { getError = error.name; }
                try { new descriptor.set(); } catch (error) { setError = error.name; }
                return [
                    descriptor.get(),
                    getError,
                    setError,
                    Object.hasOwn(descriptor.get, "prototype"),
                    Object.hasOwn(descriptor.set, "prototype")
                ].join("|");
            })()
// case: proto / __proto__ accessors are ordinary own properties and not ProtoSetter entries
            (function () {
                var object = {
                    __proto__: null,
                    get __proto__() { return 42; },
                    set __proto__(value) { this.saved = value; }
                };
                object.__proto__ = 41;
                var descriptor = Object.getOwnPropertyDescriptor(object, "__proto__");
                return [
                    Object.getPrototypeOf(object) === null,
                    Object.hasOwn(object, "__proto__"),
                    object.__proto__,
                    object.saved,
                    typeof descriptor.get,
                    typeof descriptor.set
                ].join("|");
            })()
// case: source / Function.prototype.toString preserves accessor source spans
            (function () {
                var descriptor = Object.getOwnPropertyDescriptor({
                    get answer(){ return 42; },
                    set answer(value){ this.saved = value; }
                }, "answer");
                return Function.prototype.toString.call(descriptor.get) + "|" +
                    Function.prototype.toString.call(descriptor.set);
            })()
