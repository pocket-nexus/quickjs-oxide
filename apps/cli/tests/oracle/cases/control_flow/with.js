// case: strict code rejects with during direct-eval parsing
            (function () {
                try {
                    eval("\"use strict\"; with ({}) {}");
                    return "none";
                } catch (error) {
                    return error.name;
                }
            })()
// case: single-statement contextual let honors ASI after with
            (function () {
                if (false) { with ({}) let
                    value = 1; }
                if (false) { with ({}) let
                    {} }
                return "parsed";
            })()
// case: with applies ToObject and rejects nullish values
            (function () {
                var observations = [];
                with ("abc") { observations.push(length); }
                try { with (null) {} }
                catch (error) { observations.push(error.name); }
                try { with (undefined) {} }
                catch (error) { observations.push(error.name); }
                return observations.join("|");
            })()
// case: unscopables skips the inner binding and preserves nested fallback order
            (function () {
                var outer = { x: 10, y: 11 };
                var inner = { x: 20, y: 21 };
                inner[Symbol.unscopables] = { x: true };
                with (outer) {
                    with (inner) { return x + "|" + y; }
                }
            })()
// case: Get rechecks HasProperty after an unscopables getter deletes the selection
            (function () {
                var outer = { x: 10 };
                var inner = { x: 20 };
                Object.defineProperty(inner, Symbol.unscopables, {
                    get: function () { delete inner.x; return {}; },
                    configurable: true
                });
                with (outer) {
                    with (inner) { return x; }
                }
            })()
// case: simple compound logical prefix and postfix writes target the with object
            (function () {
                var object = { x: 1, a: 2, b: 0, c: 4, d: 5 };
                var old;
                with (object) {
                    x = 3;
                    a += 4;
                    b ||= 7;
                    c &&= 8;
                    old = d++;
                    ++x;
                }
                return [object.x, object.a, object.b, object.c, object.d, old].join("|");
            })()
// case: simple assignment keeps its selected base when the RHS deletes the property
            (function () {
                var object = { x: 1 };
                with (object) { x = (delete object.x, 9); }
                return object.x + "|" + ("x" in object);
            })()
// case: compound assignment keeps its selected base when the RHS deletes the property
            (function () {
                var object = { x: 1 };
                with (object) { x += (delete object.x, 4); }
                return object.x + "|" + ("x" in object);
            })()
// case: logical assignment keeps its selected base when the RHS deletes the property
            (function () {
                var object = { x: 1 };
                with (object) { x &&= (delete object.x, 7); }
                return object.x + "|" + ("x" in object);
            })()
// case: postfix update rechecks HasProperty after coercion deletes the property
            (function () {
                var object = {};
                var value = {
                    valueOf: function () { delete object.x; return 4; }
                };
                object.x = value;
                var old;
                with (object) { old = x++; }
                return old + "|" + object.x + "|" + ("x" in object);
            })()
// case: a strict child reports a missing selected property after its RHS deletes it
            (function () {
                var object = { x: 1 };
                var child;
                with (object) {
                    child = function () {
                        "use strict";
                        x = (delete object.x, 3);
                    };
                }
                var errorName = "none";
                try { child(); }
                catch (error) { errorName = error.name; }
                return errorName + "|" + ("x" in object) + "|" + typeof x;
            })()
// case: TypedArray prototype numeric-index rejection composes with repeated with HasProperty
            (function () {
                function environment() {
                    var object = Object.create(new Int32Array(1));
                    Object.defineProperty(object, "NaN", {
                        configurable: true,
                        value: 100
                    });
                    return object;
                }

                var sloppy = environment();
                var sloppyError = "none";
                var sloppyResult;
                try {
                    with (sloppy) {
                        sloppyResult = NaN = (delete sloppy.NaN, 0);
                    }
                } catch (error) {
                    sloppyError = error.name;
                }

                var strict = environment();
                var strictChild;
                with (strict) {
                    strictChild = function () {
                        "use strict";
                        NaN = (delete strict.NaN, 0);
                    };
                }
                var strictError = "none";
                try { strictChild(); }
                catch (error) { strictError = error.name; }

                return [
                    sloppyError,
                    sloppyResult,
                    Object.getOwnPropertyDescriptor(sloppy, "NaN") === undefined,
                    "NaN" in sloppy,
                    strictError,
                    Object.getOwnPropertyDescriptor(strict, "NaN") === undefined,
                    "NaN" in strict
                ].join("|");
            })()
// case: failed selected writes are silent in sloppy code and throw in strict code
            (function () {
                var object = {};
                Object.defineProperty(object, "x", {
                    value: 1,
                    writable: false,
                    configurable: true
                });
                var sloppyResult, sloppyError = "none";
                try { with (object) { sloppyResult = x = 2; } }
                catch (error) { sloppyError = error.name; }
                var strictChild;
                with (object) {
                    strictChild = function () { "use strict"; return x = 3; };
                }
                var strictError = "none";
                try { strictChild(); }
                catch (error) { strictError = error.name; }
                return sloppyError + "|" + sloppyResult + "|" + object.x +
                    "|" + strictError;
            })()
// case: with methods receive the object while lexical fallback receives undefined
            (function () {
                var object = {
                    tag: "with",
                    method: function () { "use strict"; return this.tag; }
                };
                function fallback() { "use strict"; return this === undefined; }
                var selected, missed;
                with (object) {
                    selected = method();
                    missed = fallback();
                }
                return selected + "|" + missed;
            })()
// case: an eval var object selected under syntactic with becomes the call receiver
            (function () {
                eval(
                    "var marker = 17; " +
                    "var readMarker = function () { " +
                    "  \"use strict\"; return this.marker; " +
                    "}"
                );
                with ({}) { return readMarker(); }
            })()
// case: direct eval imports with lookup without importing reference-call receiver rules
            (function () {
                var object = {
                    method: function () { "use strict"; return this === undefined; }
                };
                with (object) { return eval("method()"); }
            })()
// case: direct eval assignment resolves its imported with environment after the RHS
            (function () {
                var object = { x: 1 };
                var x = 7;
                with (object) { eval("x = (delete object.x, 2)"); }
                return object.x + "|" + ("x" in object) + "|" + x;
            })()
// case: an eval-created closure imports with lookup but still calls with undefined receiver
            (function () {
                var object = {
                    method: function () { "use strict"; return this === undefined; }
                };
                with (object) {
                    var closure = eval("(function () { return method(); })");
                    return closure();
                }
            })()
// case: delete distinguishes selected fallback and missing bindings
            (function () {
                var local = 3;
                var object = { x: 1 };
                var selected, fallback, missing;
                with (object) {
                    selected = delete x;
                    fallback = delete local;
                    missing = delete absent;
                }
                return selected + "|" + fallback + "|" + missing + "|" +
                    ("x" in object) + "|" + local;
            })()
// case: for-in and for-of assignment targets resolve through with on every iteration
            (function () {
                var object = { x: "" };
                var output = "";
                with (object) {
                    for (x in { a: 1, b: 2 }) output += x;
                    for (x of "cd") output += x;
                }
                return output + "|" + object.x;
            })()
// case: readonly fallback throws before evaluating the assignment RHS
            (function () {
                const locked = 1;
                var sideEffect = 0;
                var errorName = "none";
                try { with ({}) { locked = (sideEffect = 1, 2); } }
                catch (error) { errorName = error.name; }
                return errorName + "|" + sideEffect + "|" + locked;
            })()
// case: readonly fallback rejects compound logical and update references eagerly
            (function () {
                const locked = 1;
                var sideEffect = 0;
                var compound = "none", logical = "none", update = "none";
                try { with ({}) { locked += (sideEffect = 1); } }
                catch (error) { compound = error.name; }
                var afterCompound = sideEffect;
                sideEffect = 0;
                try { with ({}) { locked &&= (sideEffect = 2); } }
                catch (error) { logical = error.name; }
                var afterLogical = sideEffect;
                try { with ({}) { locked++; } }
                catch (error) { update = error.name; }
                return compound + "|" + afterCompound + "|" +
                    logical + "|" + afterLogical + "|" + update;
            })()
// case: a missing strict global reference stays missing across RHS creation
            (function () {
                delete globalThis.__qjo_with_missing_strict;
                with ({}) {
                    return (function () {
                        "use strict";
                        try {
                            __qjo_with_missing_strict =
                                (globalThis.__qjo_with_missing_strict = 1, 2);
                            return "none";
                        } catch (error) {
                            return error.name + "|" +
                                globalThis.__qjo_with_missing_strict;
                        }
                    })();
                }
            })()
// case: loop closures retain the with object from each closed scope
            (function () {
                var closures = [];
                var objects = [{ x: 1 }, { x: 2 }];
                for (var index = 0; index < 2; index++) {
                    with (objects[index]) {
                        closures.push(function () { return x; });
                    }
                }
                return closures[0]() + "|" + closures[1]();
            })()
// case: a var initializer inside with assigns the object and not the function var
            (function () {
                var object = { x: 1 };
                var x;
                with (object) { var x = 2; }
                return object.x + "|" + x;
            })()
// case: a var initializer selects its with reference before evaluating the RHS
            (function () {
                var object = { x: 1 };
                var x;
                with (object) { var x = (delete object.x, 2); }
                return object.x + "|" + ("x" in object) + "|" + x;
            })()
// case: eval variable objects use prototype-aware HasProperty under with
            (function () {
                eval("var leak = function () { \"use strict\"; return this; }");
                with ({}) {
                    Object.setPrototypeOf(leak(), { inherited: 42 });
                    return inherited;
                }
            })()
