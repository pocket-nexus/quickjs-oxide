// case: lookahead / a single identifier parameter needs no parentheses
(value => value + 1)(41)
// case: lookahead / empty and multiple parenthesized parameters are recognized
            (function () {
                return (() => 42)() + "|" +
                    ((left, right) => left + right)(20, 22);
            })()
// case: lookahead / a parenthesized expression is not stolen by arrow lookahead
(1 + 2) * 14
// case: lookahead / a line terminator after an identifier parameter rejects the arrow
            (function () {
                try { eval("value\n=> value"); return "accepted"; }
                catch (error) { return error.name; }
            })()
// case: lookahead / a line terminator after a closing parameter paren rejects the arrow
            (function () {
                try { eval("(value)\n=> value"); return "accepted"; }
                catch (error) { return error.name; }
            })()
// case: lookahead / a line terminator inside a block comment rejects the arrow
            (function () {
                try {
                    eval("value /* line\n break */ => value");
                    return "accepted";
                } catch (error) {
                    return error.name;
                }
            })()
// case: lookahead / a same-line block comment before the arrow is permitted
(value /* no LineTerminator */ => value)(42)
// case: lookahead / an always-reserved word cannot be an arrow parameter
            (function () {
                try { eval("enum => 1"); return "accepted"; }
                catch (error) { return error.name; }
            })()
// case: lookahead / a statement keyword cannot be an arrow parameter
            (function () {
                try { eval("switch => 1"); return "accepted"; }
                catch (error) { return error.name; }
            })()
// case: lookahead / a strict reserved word cannot be an arrow parameter
            (function () {
                "use strict";
                try { eval("package => 1"); return "accepted"; }
                catch (error) { return error.name; }
            })()
// case: body / an expression body consumes a conditional AssignmentExpression
(value => value ? 0 : 42)(false)
// case: body / a block body has its own statements and explicit return
            (() => {
                var answer = 40;
                answer += 2;
                return answer;
            })()
// case: body / a parenthesized object literal remains an expression body
(() => ({ answer: 42 }))().answer
// case: body / an empty block body returns undefined
typeof (() => {})()
// case: strictness / duplicate simple parameters are rejected in sloppy code
            (function () {
                try { eval("(value, value) => value"); return "accepted"; }
                catch (error) { return error.name; }
            })()
// case: strictness / duplicate simple parameters are rejected before a strict body
            (function () {
                try {
                    eval("(value, value) => { 'use strict'; return value; }");
                    return "accepted";
                } catch (error) {
                    return error.name;
                }
            })()
// case: strictness / a block directive makes undeclared assignment strict
            (function () {
                delete globalThis.__qjo_arrow_strict_write;
                return (() => {
                    "use strict";
                    try {
                        __qjo_arrow_strict_write = 1;
                        return "accepted";
                    } catch (error) {
                        return error.name;
                    }
                })();
            })()
// case: strictness / an arrow inherits undefined this from a strict outer call
            (function () {
                "use strict";
                return (() => this)() === undefined;
            })()
// case: metadata / length counts simple parameters and has the standard descriptor flags
            (function () {
                var zero = () => 0;
                var one = value => value;
                var two = (left, right) => left + right;
                var descriptor = Object.getOwnPropertyDescriptor(two, "length");
                return [
                    zero.length,
                    one.length,
                    two.length,
                    descriptor.writable,
                    descriptor.enumerable,
                    descriptor.configurable
                ].join("|");
            })()
// case: metadata / anonymous arrows gain names from assignment and property contexts
            (function () {
                var assigned = value => value;
                var object = { property: value => value };
                var computed = { ["computed"]: value => value };
                var target;
                target = value => value;
                return [
                    (value => value).name,
                    assigned.name,
                    object.property.name,
                    computed.computed.name,
                    target.name
                ].join("|");
            })()
// case: metadata / toString preserves exact expression-body arrow source
            (function () {
                var arrow = ( /*a*/ value /*b*/ ) => /*c*/ value + 1;
                return Function.prototype.toString.call(arrow);
            })()
// case: metadata / toString preserves exact block-body arrow source
            (function () {
                var arrow = value => { /*body*/ return value; };
                return Function.prototype.toString.call(arrow);
            })()
// case: lexical-bindings / a Program arrow captures the script global this
(() => this)() === globalThis
// case: lexical-bindings / call and bind receivers do not replace lexical this
            (function () {
                var arrow = () => this.tag;
                return arrow.call({ tag: "call" }) + "|" +
                    arrow.bind({ tag: "bind" })();
            }).call({ tag: "outer" })
// case: lexical-bindings / an arrow resolves arguments from the nearest ordinary function
            (function (first, second) {
                return (() => arguments[0] + arguments[1])();
            })(20, 22)
// case: lexical-bindings / an arrow observes the outer call and construct new target
            (function () {
                function Target() {
                    this.matches = (() => new.target === Target)();
                    return this.matches;
                }
                var receiver = {};
                var call = Target.call(receiver);
                var constructed = new Target();
                return call + "|" + receiver.matches + "|" +
                    constructed.matches;
            })()
// case: lexical-bindings / multiple arrow layers relay locals this and arguments
            (function (left, right) {
                var nested = () => () =>
                    left + right + this.offset + arguments.length;
                return nested().call({ offset: 1000 });
            }).call({ offset: 10 }, 10, 20)
// case: with / an escaped arrow retains with lookup and honors unscopables
            (function () {
                var x = 40;
                var arrow;
                var environment = { x: 500, y: 2 };
                environment[Symbol.unscopables] = { x: true };
                with (environment) {
                    arrow = () => x + y;
                }
                return arrow.call({ x: 0, y: 0 });
            })()
// case: direct-eval / direct eval in an arrow sees its parameter and outer lexical bindings
            (function (value) {
                return (add =>
                    eval("value + add + arguments[0] + this.offset"))(20);
            }).call({ offset: 2 }, 10)
// case: direct-eval / strict direct eval preserves typeof this as an authenticated pseudo read
            (function () {
                "use strict";
                return eval("typeof this") + "|" +
                    (eval("this") === undefined);
            })()
// case: direct-eval / direct eval in an arrow observes the outer new target
            (function () {
                function Target() {
                    this.matches = (() => eval("new.target === Target"))();
                    return this.matches;
                }
                var receiver = {};
                var call = Target.call(receiver);
                var constructed = new Target();
                return call + "|" + receiver.matches + "|" +
                    constructed.matches;
            })()
// case: direct-eval / an eval-created arrow and nested eval retain with and unscopables
            (function () {
                var x = 500;
                var environment = { x: 1000 };
                environment[Symbol.unscopables] = { x: true };
                var factory;
                with (environment) {
                    factory = eval('value => eval("x + value")');
                }
                return factory(10);
            })()
// case: direct-eval / an arrow returned by eval retains an eval var binding
            (function () {
                var arrow = eval("var captured = 40; value => captured + value");
                return arrow(2);
            })()
// case: function-object / arrows and bound arrows have no prototype and are not constructors
            (function () {
                var arrow = () => 42;
                var constructError = "none";
                var boundConstructError = "none";
                var reflectError = "none";
                try { new arrow(); }
                catch (error) { constructError = error.name; }
                var bound = arrow.bind({ ignored: true });
                try { new bound(); }
                catch (error) { boundConstructError = error.name; }
                try { Reflect.construct(function () {}, [], arrow); }
                catch (error) { reflectError = error.name; }
                return [
                    Object.prototype.hasOwnProperty.call(arrow, "prototype"),
                    arrow.prototype === undefined,
                    Object.getPrototypeOf(arrow) === Function.prototype,
                    constructError,
                    boundConstructError,
                    reflectError
                ].join("|");
            })()
