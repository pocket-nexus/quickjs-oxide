// mode: completion-name
// case: raw-cooked / cooked escapes and physical CRLF normalization remain distinct from raw text
(function () {
                function units(value) {
                    if (value === void 0) return "undefined";
                    var result = "";
                    for (var index = 0; index < value.length; index++) {
                        if (index) result += ",";
                        result += ("0000" + value.charCodeAt(index).toString(16)).slice(-4);
                    }
                    return value.length + ":" + result;
                }
                function tag(strings, value) {
                    return [
                        units(strings[0]),
                        units(strings[1]),
                        units(strings.raw[0]),
                        units(strings.raw[1]),
                        value
                    ].join("|");
                }
                return tag`a\n\x42\u0043
${42}z\t`;
            })()
// case: invalid-escape / a tagged malformed escape produces undefined cooked entries while preserving raw source
            (function () {
                function units(value) {
                    if (value === void 0) return "undefined";
                    var result = "";
                    for (var index = 0; index < value.length; index++) {
                        if (index) result += ",";
                        result += ("0000" + value.charCodeAt(index).toString(16)).slice(-4);
                    }
                    return value.length + ":" + result;
                }
                function tag(strings, value) {
                    return [
                        units(strings[0]),
                        units(strings[1]),
                        units(strings.raw[0]),
                        units(strings.raw[1]),
                        value
                    ].join("|");
                }
                return tag`\xG0${42}\u{110000}`;
            })()
// case: descriptors / template and raw arrays have QuickJS frozen descriptors and realm Array prototypes
            (function () {
                function bits(descriptor) {
                    return [
                        typeof descriptor.value,
                        descriptor.writable,
                        descriptor.enumerable,
                        descriptor.configurable
                    ].join(":");
                }
                function tag(strings) {
                    return [
                        Object.isFrozen(strings),
                        Object.isFrozen(strings.raw),
                        Object.getPrototypeOf(strings) === Array.prototype,
                        Object.getPrototypeOf(strings.raw) === Array.prototype,
                        bits(Object.getOwnPropertyDescriptor(strings, "0")),
                        bits(Object.getOwnPropertyDescriptor(strings, "length")),
                        bits(Object.getOwnPropertyDescriptor(strings, "raw")),
                        bits(Object.getOwnPropertyDescriptor(strings.raw, "0")),
                        bits(Object.getOwnPropertyDescriptor(strings.raw, "length"))
                    ].join("|");
                }
                return tag`a${1}b`;
            })()
// case: same-site-identity / repeated evaluation of one site reuses its template object and raw array
            (function () {
                var firstStrings;
                var firstRaw;
                var observations = [];
                function tag(strings, value) {
                    observations.push(firstStrings === void 0
                        ? "new"
                        : String(firstStrings === strings) + ":" + String(firstRaw === strings.raw));
                    if (firstStrings === void 0) {
                        firstStrings = strings;
                        firstRaw = strings.raw;
                    }
                    return value;
                }
                function site(value) { return tag`a${value}b`; }
                return [site(1), site(2), observations.join(",")].join("|");
            })()
// case: different-site-identity / textually equal but distinct sites receive distinct template objects and raw arrays
            (function () {
                function tag(strings) { return strings; }
                var first = tag`same`;
                var second = tag`same`;
                return [
                    first !== second,
                    first.raw !== second.raw,
                    first[0] === second[0],
                    first.raw[0] === second.raw[0]
                ].join("|");
            })()
// case: member-this / dot and computed member tags receive their base object as this
            (function () {
                var receiver = {
                    base: 40,
                    tag: function (strings, value) {
                        return [
                            this === receiver,
                            this.base + value,
                            strings[0],
                            strings[1]
                        ].join(":");
                    }
                };
                return [
                    receiver.tag`a${2}b`,
                    receiver["tag"]`c${2}d`
                ].join("|");
            })()
// case: evaluation-order / member resolution precedes substitutions and tag invocation follows every substitution
            (function () {
                var log = "";
                var receiver = {
                    get tag() {
                        log += "g";
                        return function (strings, first, second) {
                            log += "t";
                            return [
                                this === receiver,
                                first + second,
                                strings[0],
                                strings[1],
                                strings[2]
                            ].join(":");
                        };
                    }
                };
                function base() { log += "b"; return receiver; }
                function one() { log += "1"; return 10; }
                function two() { log += "2"; return 20; }
                var result = base().tag`a${one()}b${two()}c`;
                return log + "|" + result;
            })()
// case: abrupt-order / getter and substitution failures stop later work while non-callable rejection follows arguments
            (function () {
                var observations = [];
                var log = "";
                var getterThrow = {
                    get tag() { log += "g"; throw "getter"; }
                };
                try { getterThrow.tag`${(log += "x")}`; }
                catch (error) { observations.push(log + ":" + error); }

                log = "";
                var substitutionThrow = {
                    get tag() {
                        log += "g";
                        return function () { log += "t"; };
                    }
                };
                function one() { log += "1"; throw "sub"; }
                function two() { log += "2"; return 2; }
                try { substitutionThrow.tag`${one()}${two()}`; }
                catch (error) { observations.push(log + ":" + error); }

                log = "";
                var nonCallable = {
                    get tag() { log += "g"; return 0; }
                };
                try {
                    nonCallable.tag`${(log += "1", 1)}${(log += "2", 2)}`;
                } catch (error) {
                    observations.push(log + ":" + error.name);
                }
                return observations.join("|");
            })()
// case: new-precedence / new constructs the result of the tagged call with the following argument list
            (function () {
                var log = "";
                var constructor;
                function tag(strings) {
                    log += "t" + strings[0];
                    constructor = function (value) {
                        log += "c";
                        this.value = value;
                    };
                    return constructor;
                }
                var instance = new tag`x`(42);
                return [
                    log,
                    instance.value,
                    instance instanceof constructor
                ].join("|");
            })()
// case: chained-tags / a tagged result can immediately tag the following template without a receiver
            (function () {
                var log = "";
                function first(strings) {
                    log += "f:" + strings[0];
                    return function (nextStrings) {
                        "use strict";
                        log += ",s:" + nextStrings[0] + ":" + (this === void 0);
                        return 42;
                    };
                }
                var value = first`a``b`;
                return value + "|" + log;
            })()
// case: with-receiver / an identifier tag resolved through with receives the with object as this
            (function () {
                var result;
                var receiver = {
                    base: 40,
                    tag: function (strings, value) {
                        return [
                            this === receiver,
                            this.base + value,
                            strings[0],
                            strings[1]
                        ].join(":");
                    }
                };
                with (receiver) {
                    result = tag`a${2}b`;
                }
                return result;
            })()
// case: tagged-eval / eval used as a tag is an ordinary call and returns its non-string template argument
            (function () {
                var marker = 0;
                var result = eval`marker = 42`;
                return [
                    typeof result,
                    Object.getPrototypeOf(result) === Array.prototype,
                    Object.isFrozen(result),
                    result[0],
                    marker
                ].join("|");
            })()
// case: closure-site-identity / closures instantiated from one child bytecode share its tagged-template site
            (function () {
                function tag(strings) { return strings; }
                function factory() {
                    return function () { return tag`same`; };
                }
                var firstClosure = factory();
                var secondClosure = factory();
                var first = firstClosure();
                var second = secondClosure();
                return [
                    first === second,
                    first.raw === second.raw,
                    firstClosure !== secondClosure
                ].join("|");
            })()
// case: dynamic-compilation-sites / authored eval and separately constructed Function bytecode own distinct sites
            (function () {
                function tag(strings) { return strings; }
                var authored = tag`same`;
                var evalFirst = eval("tag`same`");
                var evalSecond = eval("tag`same`");
                var firstFunction = Function("tag", "return tag`same`;");
                var secondFunction = Function("tag", "return tag`same`;");
                var functionFirst = firstFunction(tag);
                var functionFirstAgain = firstFunction(tag);
                var functionSecond = secondFunction(tag);
                return [
                    authored !== evalFirst,
                    evalFirst !== evalSecond,
                    authored !== functionFirst,
                    functionFirst === functionFirstAgain,
                    functionFirst !== functionSecond
                ].join("|");
            })()
// case: super-member-this / super member tags and direct eval preserve the method receiver
            (function () {
                var prototype = {
                    tag: function (strings, value) {
                        return [
                            this.value + value,
                            strings[0],
                            strings[1]
                        ].join(":");
                    }
                };
                var object = {
                    value: 40,
                    __proto__: prototype,
                    run() {
                        return [
                            super.tag`a${2}b`,
                            eval("super.tag`c${2}d`")
                        ].join("|");
                    }
                };
                return object.run();
            })()
// case: newline-continuation / a line terminator before the template does not trigger automatic semicolon insertion
            (function () {
                var calls = 0;
                function tag(strings) {
                    calls += 1;
                    return strings[0];
                }
                var value = tag
                `line`;
                return calls + "|" + value;
            })()
