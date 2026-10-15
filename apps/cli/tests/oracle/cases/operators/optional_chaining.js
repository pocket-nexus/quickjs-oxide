// mode: completion-name
// case: basic member / fixed and computed members return values while nullish bases skip the key
            (function () {
                function text(value) {
                    return value === void 0 ? "undefined" : String(value);
                }
                var conversions = 0;
                var key = {
                    toString: function () {
                        conversions++;
                        return "answer";
                    }
                };
                var object = { answer: 42 };
                return [
                    object?.answer,
                    object?.[key],
                    text(null?.answer),
                    text(undefined?.[key]),
                    conversions
                ].join("|");
            })()
// case: basic call / bare member computed and chained optional calls select the intended boundary
            (function () {
                function text(value) {
                    return value === void 0 ? "undefined" : String(value);
                }
                function plain(value) {
                    return (this === globalThis) + ":" + value;
                }
                function factory() {
                    return { answer: 42 };
                }
                var object = {
                    base: 40,
                    method: function (value) {
                        return this.base + value;
                    }
                };
                var nil = null;
                return [
                    plain?.(42),
                    object?.method(2),
                    object.method?.(2),
                    object?.["method"]?.(2),
                    factory?.()?.answer,
                    text(nil?.method(2)),
                    text(nil?.(1)),
                    text(nil?.()?.answer)
                ].join("|");
            })()
// case: basic call / an optional call still rejects a present non-callable value
({ value: 1 }).value?.()
// case: basic call / an optional base does not make a present method call optional
({ method: 1 })?.method()
// case: short circuit / nullish member and call boundaries skip keys arguments and the remaining chain
            (function () {
                var log = "";
                function base(value) {
                    log += "b";
                    return value;
                }
                function key() {
                    log += "k";
                    return "method";
                }
                function argument() {
                    log += "a";
                    return 2;
                }
                var first = base(null)?.[key()](argument());
                var second = base({ method: null })?.[key()]?.(argument());
                var third = base({
                    method: function (value) {
                        log += "c";
                        return value + 40;
                    }
                })?.[key()]?.(argument());
                return [String(first), String(second), third, log].join("|");
            })()
// case: short circuit / the base is evaluated once and only traversed getters execute
            (function () {
                var baseCalls = 0;
                var gets = 0;
                function base() {
                    baseCalls++;
                    return {
                        get child() {
                            gets++;
                            return { answer: 42 };
                        }
                    };
                }
                var answer = base()?.child.answer;
                var skipped = (function () {
                    baseCalls++;
                    return null;
                })()?.child.answer;
                return [answer, String(skipped), baseCalls, gets].join("|");
            })()
// case: chain extent / one optional boundary covers its tail while grouping and a nonoptional null tail throw
            (function () {
                var first = null?.child.answer.value;
                var second = ({ child: null })?.child?.answer.value;
                var third = ({ child: { answer: { value: 42 } } })?.child.answer?.value;
                var groupedError;
                var tailError;
                try {
                    (null?.child).answer;
                } catch (error) {
                    groupedError = error.name;
                }
                try {
                    ({ child: null })?.child.answer;
                } catch (error) {
                    tailError = error.name;
                }
                return [
                    String(first),
                    String(second),
                    third,
                    groupedError,
                    tailError
                ].join("|");
            })()
// case: receiver / dot computed optional-base and grouped calls retain the member receiver
            (function () {
                var object = {
                    base: 40,
                    method: function (value) {
                        return (this === object) + ":" + (this.base + value);
                    }
                };
                var conversions = 0;
                var key = {
                    toString: function () {
                        conversions++;
                        return "method";
                    }
                };
                return [
                    object.method?.(2),
                    object?.method(2),
                    object?.[key]?.(2),
                    (object?.method)(2),
                    conversions
                ].join("|");
            })()
// case: receiver / nullish grouped fixed spread and multi-edge calls evaluate arguments then throw
            (function () {
                var log = "";
                var errors = [];
                try {
                    (null?.method)((log += "f", 1));
                } catch (error) {
                    errors.push(error.name);
                }
                try {
                    (null?.method)(...(log += "s", [1]));
                } catch (error) {
                    errors.push(error.name);
                }
                try {
                    (({ child: null })?.child?.method)((log += "m", 1));
                } catch (error) {
                    errors.push(error.name);
                }
                return errors.join(",") + "|" + log;
            })()
// case: QuickJS with quirk / an optional call of a with-resolved identifier hits QuickJS stack verification
            (function () {
                var scope = {
                    base: 40,
                    method: function (value) {
                        return (this === scope) + ":" + (this.base + value);
                    }
                };
                var result;
                with (scope) {
                    result = method?.(2);
                }
                return result;
            })()
// case: receiver / QuickJS reads optional super getters with the prototype and calls with the live receiver
            (function () {
                var getterReceivers = [];
                var receiver;
                var proto = {
                    get method() {
                        getterReceivers.push(this === proto ? "proto" : "other");
                        return function (value) {
                            return (this.base + value) + ":" + (this === receiver);
                        };
                    }
                };
                var home = {
                    __proto__: proto,
                    dot(value) {
                        return super.method?.(value);
                    },
                    computed(value) {
                        return super["method"]?.(value);
                    },
                    missing(side) {
                        return super.absent?.(side());
                    }
                };
                receiver = { base: 40 };
                var sideCalls = 0;
                var missing = home.missing.call(receiver, function () {
                    sideCalls++;
                    return 1;
                });
                return [
                    home.dot.call(receiver, 2),
                    home.computed.call(receiver, 2),
                    String(missing),
                    sideCalls,
                    getterReceivers.join(",")
                ].join("|");
            })()
// case: eval / eval optional call is indirect and cannot see the caller lexical environment
            (function () {
                var __qjo_optional_local = 41;
                var direct = eval("__qjo_optional_local + 1");
                var indirect = eval?.("typeof __qjo_optional_local");
                return [direct, indirect, __qjo_optional_local].join("|");
            })()
// case: eval / a shadowed eval remains an ordinary optional call and skips a nullish argument
            (function () {
                var calls = 0;
                var eval = function (source) {
                    "use strict";
                    calls++;
                    return (this === void 0) + ":" + source;
                };
                var first = eval?.("shadow");
                eval = null;
                var second = eval?.((calls++, "skipped"));
                return [first, String(second), calls].join("|");
            })()
// case: delete / delete short circuits nullish keys and preserves sloppy and strict property rules
            (function () {
                var log = "";
                var object = { answer: 42 };
                var fixed = {};
                Object.defineProperty(fixed, "answer", {
                    value: 42,
                    configurable: false
                });
                var skipped = delete null?.[log += "key"];
                var removed = delete object?.answer;
                var rejected = delete fixed?.answer;
                var strictDelete = Function(
                    "value",
                    "\"use strict\"; return delete value?.answer;"
                );
                var strictError;
                try {
                    strictDelete(fixed);
                } catch (error) {
                    strictError = error.name;
                }
                return [
                    skipped,
                    removed,
                    Object.hasOwn(object, "answer"),
                    rejected,
                    strictDelete(null),
                    strictError,
                    log
                ].join("|");
            })()
// case: grouped boundary / grouping ends the optional chain so its result can be constructed
            (function () {
                function Constructor(value) {
                    this.answer = value;
                }
                var holder = { Constructor: Constructor };
                return new (holder?.Constructor)(42).answer;
            })()
// case: grouped boundary / grouping ends the optional chain while retaining a member tag receiver
            (function () {
                var holder = {
                    tag: function (strings, value) {
                        return [
                            this === holder,
                            strings[0],
                            value,
                            strings[1]
                        ].join(":");
                    }
                };
                return (holder?.tag)`a${42}b`;
            })()
// case: special base / new.target can be the base of an optional member chain
            (function () {
                function Probe() {
                    if (new.target === void 0) {
                        return String(new.target?.name);
                    }
                    this.seen = new.target?.name;
                }
                return [Probe(), new Probe().seen].join("|");
            })()
// case: async / a nullish optional call skips an await argument while a callable path suspends
            (function () {
                var log = "";
                async function probe(callback) {
                    var value = callback?.(await (log += "a", 40));
                    log += "r";
                    return value;
                }
                var skipped = probe(null);
                var afterSkipped = log;
                log = "";
                var called = probe(function (value) {
                    log += "c";
                    return value + 2;
                });
                var afterCalled = log;
                return [
                    afterSkipped,
                    skipped instanceof Promise,
                    afterCalled,
                    called instanceof Promise
                ].join("|");
            })()
// case: generator / a nullish optional call skips a yield argument while a callable path resumes it
            (function () {
                function* probe(callback) {
                    return callback?.(yield 40);
                }
                var skipped = probe(null).next();
                var iterator = probe(function (value) {
                    return value + 2;
                });
                var first = iterator.next();
                var second = iterator.next(40);
                return [
                    skipped.done,
                    String(skipped.value),
                    first.done,
                    first.value,
                    second.done,
                    second.value
                ].join("|");
            })()
// case: private element / optional private fields and methods short circuit before brand checks on nullish bases
            (function () {
                class Box {
                    #value = 40;
                    #add(value) {
                        return this.#value + value;
                    }
                    static read(value) {
                        return value?.#value;
                    }
                    static call(value) {
                        return value?.#add?.(2);
                    }
                }
                var box = new Box();
                var brandError;
                try {
                    Box.read({});
                } catch (error) {
                    brandError = error.name;
                }
                return [
                    Box.read(box),
                    String(Box.read(null)),
                    String(Box.read(void 0)),
                    Box.call(box),
                    String(Box.call(null)),
                    brandError
                ].join("|");
            })()
// case: private element / a private field after an earlier optional link remains inside the same chain
            (function () {
                class Box {
                    #value = 42;
                    read(value) {
                        return value?.holder.#value;
                    }
                }
                var box = new Box();
                var nullHolderError;
                var wrongBrandError;
                try {
                    box.read({ holder: null });
                } catch (error) {
                    nullHolderError = error.name;
                }
                try {
                    box.read({ holder: {} });
                } catch (error) {
                    wrongBrandError = error.name;
                }
                return [
                    box.read({ holder: box }),
                    String(box.read(null)),
                    nullHolderError,
                    wrongBrandError
                ].join("|");
            })()
// case: QuickJS private quirk / grouping an optional private method loses the receiver before fixed or optional call
            (function () {
                class Box {
                    #receiver() {
                        return this === void 0 ? "undefined" : "instance";
                    }
                    test() {
                        return [
                            this?.#receiver(),
                            (this?.#receiver)(),
                            (this?.#receiver)?.()
                        ].join("|");
                    }
                }
                return new Box().test();
            })()
// case: QuickJS private quirk / delete of an optional private access returns true without deleting the private field
            (function () {
                class Box {
                    #value = 42;
                    test(value) {
                        var result = delete value?.#value;
                        var after;
                        try {
                            after = value?.#value;
                        } catch (error) {
                            after = error.name;
                        }
                        return result + ":" + after;
                    }
                }
                var box = new Box();
                var wrongBrand;
                try {
                    box.test({});
                } catch (error) {
                    wrongBrand = error.name;
                }
                return [
                    box.test(box),
                    box.test(null),
                    wrongBrand
                ].join("|");
            })()
// case: early error assignment / an optional member cannot be a simple assignment target
var object = {}; object?.answer = 42;
// case: early error assignment / an optional member cannot be a compound assignment target
var object = { answer: 1 }; object?.answer += 1;
// case: early error assignment / an optional member cannot be a logical assignment target
var object = { answer: 0 }; object?.answer ||= 1;
// case: early error destructuring / an optional member cannot be an array destructuring assignment target
            var object = {};
            var values = [42];
            [object?.answer] = values;
// case: early error iteration / an optional member cannot be a for-in assignment target
            var object = {};
            var source = { answer: 42 };
            for (object?.answer in source) {}
// case: early error iteration / an optional member cannot be a for-of assignment target
            var object = {};
            var source = [42];
            for (object?.answer of source) {}
// case: early error update / an optional member cannot be a postfix update target
var object = {}; object?.answer++;
// case: early error update / an optional member cannot be a prefix update target
var object = {}; ++object?.answer;
// case: early error new / new cannot directly construct an optional member chain
            var holder = { Constructor: function () {} };
            new holder?.Constructor();
// case: early error new / new cannot directly construct an optional call chain
            var Constructor = function () {};
            new Constructor?.();
// case: early error template / a tagged template cannot directly follow an optional member chain
            var holder = { tag: function () {} };
            holder?.tag`value`;
// case: early error template / an optional tag call cannot use tagged-template syntax
            var tag = function () {};
            tag?.`value`;
// case: early error super / super itself cannot be the base of an optional member chain
            class Base {}
            class Derived extends Base {
                method() {
                    return super?.value;
                }
            }
// case: early error private / delete of an ordinary private access remains a syntax error
            var optional = null?.value;
            class Box {
                #value;
                remove(value) {
                    return delete value.#value;
                }
            }
// case: early error private / an optional private name must still be declared by an enclosing class
var value = {}; value?.#missing;
