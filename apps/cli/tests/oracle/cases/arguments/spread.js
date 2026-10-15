// CASES
// case: lowering / empty and interleaved spreads preserve fixed tails and a trailing comma
            (function () {
                function list() {
                    return Array.prototype.join.call(arguments, ",");
                }
                return list(...[], 1, ...[2, 3], 4, ...[5],);
            })()
// case: method / method receiver and callee then argument evaluation order survive spread lowering
            (function () {
                var log = [];
                function mark(label, value) { log.push(label); return value; }
                var object = { base: 10 };
                Object.defineProperty(object, "method", {
                    get: function () {
                        log.push("get");
                        return function () {
                            "use strict";
                            return (this === object) + "|" + this.base + "|" +
                                Array.prototype.join.call(arguments, ",") + "|" +
                                log.join(",");
                        };
                    }
                });
                var receiver = mark("base", object);
                return receiver.method(mark("a", 1), ...mark("spread", [2, 3]), mark("z", 4));
            })()
// case: QuickJS append / argument spread performs QuickJS's two observable iterator method Gets
            (function () {
                var gets = 0;
                var calls = 0;
                var iterable = {};
                Object.defineProperty(iterable, Symbol.iterator, {
                    get: function () {
                        gets++;
                        return function () {
                            calls++;
                            var done = false;
                            return {
                                next: function () {
                                    if (done) return { done: true };
                                    done = true;
                                    return { value: 7, done: false };
                                }
                            };
                        };
                    }
                });
                function identity(value) { return value; }
                return identity(...iterable) + "|" + gets + "|" + calls;
            })()
// case: QuickJS append / fast Array copies its source after the second iterator Get selects another iterator
            (function () {
                var source = [1, 2];
                var gets = 0;
                Object.defineProperty(source, Symbol.iterator, {
                    configurable: true,
                    get: function () {
                        gets++;
                        if (gets === 1) return Array.prototype.values;
                        return function () { return [99].values(); };
                    }
                });
                function list() {
                    return Array.prototype.join.call(arguments, ",");
                }
                return list(...source) + "|" + gets;
            })()
// case: call / a bare spread call supplies undefined to strict this
(function () { "use strict"; return this === undefined; })(...[])
// case: iterator / the iterator next method is fetched once and cached across all spread steps
            (function () {
                var gets = 0;
                var calls = 0;
                var iterable = {};
                iterable[Symbol.iterator] = function () {
                    var iterator = { index: 0 };
                    Object.defineProperty(iterator, "next", {
                        configurable: true,
                        get: function () {
                            gets++;
                            return function () {
                                calls++;
                                this.index++;
                                if (this.index === 1) {
                                    Object.defineProperty(this, "next", {
                                        configurable: true,
                                        value: function () { throw "replacement next"; }
                                    });
                                }
                                if (this.index <= 2) return { value: this.index, done: false };
                                return { done: true };
                            };
                        }
                    });
                    return iterator;
                };
                function list() {
                    return Array.prototype.join.call(arguments, ",");
                }
                return list(...iterable) + "|" + gets + "|" + calls;
            })()
// case: iterator / a next throw closes the iterator while a close throw cannot replace it
            (function () {
                var log = "";
                var iterable = {};
                iterable[Symbol.iterator] = function () {
                    return {
                        next: function () { log += "n"; throw "next throw"; },
                        return: function () { log += "r"; throw "close throw"; }
                    };
                };
                try {
                    (function () {})(...iterable);
                } catch (error) {
                    return error + "|" + log;
                }
                return "no throw";
            })()
// case: construct / construct spread preserves arguments receiver prototype and new.target
            (function () {
                function Constructor(a, b) {
                    this.argumentsSeen = a + ":" + b;
                    this.newTargetSeen = new.target === Constructor;
                }
                var value = new Constructor(...[20], 22);
                return value.argumentsSeen + "|" + value.newTargetSeen + "|" +
                    (value instanceof Constructor);
            })()
// case: eval / direct eval accepts an empty leading spread and ignores later arguments
            (function () {
                function run() {
                    let value = 1;
                    var result = eval(...[], "value = 42; 'direct'", ...["ignored"]);
                    return result + "|" + value;
                }
                return run();
            })()
// case: eval / a shadowed eval receives every spread argument with undefined strict this
            (function () {
                function run(eval) { return eval(...[1, 2], 3); }
                return run(function () {
                    "use strict";
                    return (this === undefined) + "|" +
                        Array.prototype.join.call(arguments, ",");
                });
            })()
// case: eval / an indirect eval spread remains global rather than capturing the caller lexical
            (function () {
                globalThis.argumentSpreadIndirect = 1;
                let argumentSpreadIndirect = 2;
                (0, eval)(...["globalThis.argumentSpreadIndirect = 42"]);
                return argumentSpreadIndirect + "|" + globalThis.argumentSpreadIndirect;
            })()
// case: values / sparse Arrays materialize undefined and String spread advances by Unicode code point
            (function () {
                function inspect() {
                    return arguments.length + "|" + typeof arguments[0] + "|" +
                        arguments[1] + "|" + arguments[2] + "|" +
                        arguments[3].length + "|" + arguments[3].charCodeAt(0) + "|" +
                        arguments[4].length + "|" + arguments[4].charCodeAt(0);
                }
                return inspect(...[, 2], ..."A\uD83D\uDCA9\uD800");
            })()
// case: error order / a non-callable target is rejected only after its spread iterator completes
            (function () {
                var log = "";
                var iterable = {};
                iterable[Symbol.iterator] = function () {
                    var index = 0;
                    return {
                        next: function () {
                            log += "n";
                            index++;
                            return index === 1 ? { value: 1, done: false } : { done: true };
                        }
                    };
                };
                try { ({ })(...iterable); } catch (error) { return error.name + "|" + log; }
                return "no throw";
            })()
// case: error order / an iterator acquisition throw wins before non-callability is tested
            (function () {
                var log = "";
                var iterable = {};
                Object.defineProperty(iterable, Symbol.iterator, {
                    get: function () { log += "g"; throw "spread throw"; }
                });
                try { ({ })(...iterable); } catch (error) { return error + "|" + log; }
                return "no throw";
            })()
// case: error order / a callable non-constructor is rejected after its spread iterator completes
            (function () {
                var log = "";
                var iterable = {};
                iterable[Symbol.iterator] = function () {
                    var index = 0;
                    return {
                        next: function () {
                            log += "n";
                            index++;
                            return index === 1 ? { value: 1, done: false } : { done: true };
                        }
                    };
                };
                var arrow = () => 0;
                try { new arrow(...iterable); } catch (error) { return error.name + "|" + log; }
                return "no throw";
            })()
// BOUNDARY_CASES
// case: argument limit / the QuickJS runtime accepts exactly 65534 spread arguments
            (function () {
                return Boolean(...Array(65534).fill(0));
            })()
// case: argument limit / the QuickJS runtime rejects 65535 spread arguments
            (function () {
                try { return Boolean(...Array(65535).fill(0)); }
                catch (error) { return error.name; }
            })()
// case: error order / the 65535 argument RangeError precedes constructability validation
            (function () {
                var arrow = () => 0;
                try { return new arrow(...Array(65535).fill(0)); }
                catch (error) { return error.name; }
            })()
