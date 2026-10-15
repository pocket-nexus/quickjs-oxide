// case: property-keys / fixed computed string numeric and keyword keys define named methods
            (function () {
                var computedKey = "computed";
                var object = {
                    fixed() { return 1; },
                    [computedKey]() { return 2; },
                    "quoted"() { return 3; },
                    7() { return 4; },
                    if() { return 5; }
                };
                return [
                    object.fixed(),
                    object.computed(),
                    object.quoted(),
                    object[7](),
                    object.if(),
                    object.fixed.name,
                    object.computed.name,
                    object.quoted.name,
                    object[7].name,
                    object.if.name
                ].join("|");
            })()
// case: property-keys / computed Symbol keys use QuickJS method name formatting
            (function () {
                var described = Symbol("named");
                var missing = Symbol();
                var empty = Symbol("");
                var object = {
                    [described]() {},
                    [missing]() {},
                    [empty]() {}
                };
                return [
                    object[described].name,
                    object[missing].name,
                    object[empty].name
                ].join("|");
            })()
// case: property-keys / contextual get set and async keys remain ordinary methods before a paren
            (function () {
                var object = {
                    get() { return 20; },
                    set(value) { return value; },
                    async() { return 2; }
                };
                return [
                    object.get() + object.set(20) + object.async(),
                    object.get.name,
                    object.set.name,
                    object.async.name,
                    object.get.length,
                    object.set.length,
                    object.async.length
                ].join("|");
            })()
// case: call-environment / a method owns dynamic this rather than capturing its defining receiver
            (function () {
                var object = {
                    base: 100,
                    method(delta) { return this.base + delta; }
                };
                var method = object.method;
                return object.method(-58) + "|" + method.call({ base: 40 }, 2);
            })()
// case: call-environment / methods are not automatically strict but inherit outer strictness
            (function () {
                var sloppy = { method() { return this; } }.method;
                var strict = (function () {
                    "use strict";
                    return { method() { return this; } }.method;
                })();
                return (sloppy() === globalThis) + "|" +
                    (strict() === undefined);
            })()
// case: call-environment / a method owns arguments and binds it to the current call
            (function (outer) {
                var outerArguments = arguments;
                return ({
                    method(left, right) {
                        return (arguments !== outerArguments) + "|" +
                            arguments.length + "|" +
                            (left + right) + "|" +
                            (arguments[0] + arguments[1]);
                    }
                }).method(20, 22);
            })(99)
// case: call-environment / a directly called method has its own undefined new target
({ method() { return typeof new.target; } }).method()
// case: direct-eval / direct eval resolves the method this arguments and new target owners
            (function () {
                var method = ({
                    method(value) {
                        return eval(
                            "this.tag + arguments[0] + " +
                            "(typeof new.target === 'undefined')"
                        );
                    }
                }).method;
                return method.call({ tag: 19 }, 22);
            })()
// case: scope / the property key does not create a private function self binding
            (function () {
                var method = 40;
                return ({ method() { return method + 2; } }).method();
            })()
// case: parameters / duplicate simple method parameters are an early error in sloppy code
            (function () {
                try {
                    eval("({ method(value, value) { return value; } })");
                    return "accepted";
                } catch (error) {
                    return error.name;
                }
            })()
// case: parameters / a trailing comma is accepted and does not increase method length
            (function () {
                var method = ({ method(value,) { return value + 1; } }).method;
                return method(41) + "|" + method.length;
            })()
// case: function-object / method name length property and function descriptors match QuickJS
            (function () {
                var object = { method(left, right) {} };
                var method = object.method;
                var property = Object.getOwnPropertyDescriptor(object, "method");
                var name = Object.getOwnPropertyDescriptor(method, "name");
                var length = Object.getOwnPropertyDescriptor(method, "length");
                return [
                    method.name,
                    method.length,
                    property.value === method,
                    property.writable,
                    property.enumerable,
                    property.configurable,
                    name.value,
                    name.writable,
                    name.enumerable,
                    name.configurable,
                    length.value,
                    length.writable,
                    length.enumerable,
                    length.configurable,
                    Object.prototype.hasOwnProperty.call(method, "prototype"),
                    method.prototype === undefined,
                    Object.getPrototypeOf(method) === Function.prototype
                ].join("|");
            })()
// case: function-object / methods remain callable but are not constructors or new targets
            (function () {
                var method = { method() { return 42; } }.method;
                var constructError = "none";
                var newTargetError = "none";
                try { new method(); }
                catch (error) { constructError = error.name; }
                try { Reflect.construct(function () {}, [], method); }
                catch (error) { newTargetError = error.name; }
                return method() + "|" + constructError + "|" + newTargetError;
            })()
// case: proto / a __proto__ method is an own data property not a ProtoSetter
            (function () {
                var object = {
                    __proto__: null,
                    __proto__() { return 42; }
                };
                var descriptor = Object.getOwnPropertyDescriptor(object, "__proto__");
                return [
                    Object.getPrototypeOf(object) === null,
                    Object.hasOwn(object, "__proto__"),
                    object.__proto__(),
                    descriptor.writable,
                    descriptor.enumerable,
                    descriptor.configurable
                ].join("|");
            })()
