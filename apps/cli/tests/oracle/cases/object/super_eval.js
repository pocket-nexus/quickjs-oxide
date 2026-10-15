// mode: completion-name
// case: method-read / dot and computed direct eval use the live HomeObject prototype and method receiver
            (function () {
                var getterThis;
                var first = { get value() { getterThis = this; return -1; } };
                var second = {
                    get value() { getterThis = this; return this.base + 2; }
                };
                var home = {
                    __proto__: first,
                    read(key) {
                        var dot = eval("super.value");
                        var computed = eval("super[key]");
                        return [dot, computed, getterThis === this].join("|");
                    }
                };
                var receiver = { base: 40 };
                Object.setPrototypeOf(home, second);
                return home.read.call(receiver, "value");
            })()
// case: strictness / caller and eval-source strictness govern rejected direct-eval super writes
            (function () {
                var home = {
                    sloppy() {
                        Object.freeze(this);
                        return eval("super.value = 1");
                    },
                    outerStrict() {
                        "use strict";
                        Object.freeze(this);
                        return eval("super.value = 1");
                    },
                    localStrict() {
                        Object.freeze(this);
                        return eval("'use strict'; super.value = 1");
                    }
                };
                var sloppyReceiver = {};
                var outerReceiver = {};
                var localReceiver = {};
                var outerError;
                var localError;
                var result = home.sloppy.call(sloppyReceiver);
                try { home.outerStrict.call(outerReceiver); }
                catch (error) { outerError = error.name; }
                try { home.localStrict.call(localReceiver); }
                catch (error) { localError = error.name; }
                return [
                    result,
                    outerError,
                    localError,
                    Object.hasOwn(sloppyReceiver, "value"),
                    Object.hasOwn(outerReceiver, "value"),
                    Object.hasOwn(localReceiver, "value")
                ].join("|");
            })()
// case: call-receiver / direct-eval super call reads from the base and invokes with the method receiver
            (function () {
                var getterThis;
                var callThis;
                var proto = {
                    get callable() {
                        getterThis = this;
                        return function () {
                            callThis = this;
                            return this.base + 2;
                        };
                    }
                };
                var home = {
                    __proto__: proto,
                    invoke() { return eval("super.callable()"); }
                };
                var receiver = { base: 40 };
                var result = home.invoke.call(receiver);
                return [
                    result,
                    getterThis === proto,
                    callThis === receiver
                ].join("|");
            })()
// case: write / computed direct-eval super assignment writes through the current receiver
            (function () {
                var proto = {
                    set value(input) { this.saved = input + 1; }
                };
                var home = {
                    __proto__: proto,
                    write(key, input) { return eval("super[key] = input"); }
                };
                var receiver = {};
                var result = home.write.call(receiver, "value", 41);
                return [
                    result,
                    receiver.saved,
                    Object.hasOwn(home, "saved"),
                    Object.hasOwn(proto, "saved")
                ].join("|");
            })()
// case: update / direct-eval postfix and prefix super updates preserve values receiver and ordering
            (function () {
                var log = "";
                var proto = {
                    get value() { log += "g"; return this.count; },
                    set value(input) { log += "s"; this.count = input; }
                };
                var home = {
                    __proto__: proto,
                    update() {
                        return eval(
                            "var post = super.value++;" +
                            "var pre = ++super.value;" +
                            "[post, pre, this.count, log].join('|')"
                        );
                    }
                };
                return home.update.call({ count: 40 });
            })()
// case: delete / direct-eval delete super throws before computed-key coercion
            (function () {
                var log = [];
                var key = {
                    toString() { log.push("coerce"); throw "key coercion"; }
                };
                var home = {
                    remove() {
                        return eval(
                            "delete super[(log.push('expr'), key)]"
                        );
                    }
                };
                try { home.remove(); }
                catch (error) { return [error.name, log.join(",")].join("|"); }
            })()
// case: accessors / getter and setter direct eval inherit their own HomeObject this and arguments
            (function () {
                var proto = {
                    get value() { return this.base + 2; },
                    set value(input) { this.saved = input + 2; }
                };
                var home = {
                    __proto__: proto,
                    get answer() { return eval("super.value"); },
                    set answer(input) {
                        eval("super.value = arguments[0]");
                    }
                };
                var descriptor = Object.getOwnPropertyDescriptor(home, "answer");
                var receiver = { base: 40 };
                var value = descriptor.get.call(receiver);
                descriptor.set.call(receiver, 40);
                return [value, receiver.saved].join("|");
            })()
// case: authored-arrow / an escaped authored arrow carries method direct-eval super across a live prototype change
            (function () {
                var getterThis;
                var first = { value: -1 };
                var second = {
                    get value() { getterThis = this; return this.base + 2; }
                };
                var home = {
                    __proto__: first,
                    make() { return () => eval("super.value"); }
                };
                var receiver = { base: 40 };
                var arrow = home.make.call(receiver);
                Object.setPrototypeOf(home, second);
                return [arrow(), getterThis === receiver].join("|");
            })()
// case: eval-created-arrow / an escaped arrow created by eval retains imported this and HomeObject cells
            (function () {
                var getterThis;
                var first = { value: -1 };
                var second = {
                    get value() { getterThis = this; return this.base + 2; }
                };
                var home = {
                    __proto__: first,
                    make() { return eval("() => super.value"); }
                };
                var receiver = { base: 40 };
                var arrow = home.make.call(receiver);
                Object.setPrototypeOf(home, second);
                return [arrow(), getterThis === receiver].join("|");
            })()
// case: nested-eval / method and arrow nested direct eval recursively preserve super capability
            (function () {
                var proto = {
                    get value() { return this.base + 2; }
                };
                var home = {
                    __proto__: proto,
                    method() { return eval("eval('super.value')"); },
                    arrow() {
                        return (() => eval("eval('super.value')"))();
                    }
                };
                var receiver = { base: 40 };
                return [
                    home.method.call(receiver),
                    home.arrow.call(receiver)
                ].join("|");
            })()
// case: first-slot-wins / eval-visible pseudo relays are reused by authored super in the same escaped arrow
            (function () {
                var getterThis = [];
                var proto = {
                    get value() {
                        getterThis.push(this);
                        return this.base + 2;
                    }
                };
                var home = {
                    __proto__: proto,
                    make() {
                        return () => [
                            super.value,
                            eval("super.value"),
                            super.value
                        ].join("|");
                    }
                };
                var receiver = { base: 40 };
                var arrow = home.make.call(receiver);
                return [
                    arrow(),
                    getterThis.length,
                    getterThis[0] === receiver,
                    getterThis[1] === receiver,
                    getterThis[2] === receiver
                ].join("|");
            })()
// case: ordinary-cutoff / ordinary functions cut off authored and eval-created method super capability
            (function () {
                var proto = { value: 42 };
                var home = {
                    __proto__: proto,
                    authored() {
                        super.value;
                        function inner() { return eval("super.value"); }
                        return inner();
                    },
                    arrow() {
                        super.value;
                        function inner() {
                            return (() => eval("super.value"))();
                        }
                        return inner();
                    },
                    created() {
                        super.value;
                        return eval("(function () { return super.value; })");
                    }
                };
                var authoredError;
                var arrowError;
                var createdError;
                try { home.authored(); }
                catch (error) { authoredError = error.name; }
                try { home.arrow(); }
                catch (error) { arrowError = error.name; }
                try { home.created(); }
                catch (error) { createdError = error.name; }
                return [authoredError, arrowError, createdError].join("|");
            })()
// case: global-cutoff / global direct eval and a global arrow have no HomeObject capability
            var directError;
            var arrowError;
            try { eval("super.value"); }
            catch (error) { directError = error.name; }
            try { (() => eval("super.value"))(); }
            catch (error) { arrowError = error.name; }
            [directError, arrowError].join("|");
// case: indirect-cutoff / indirect eval inside a method does not inherit its HomeObject capability
            (function () {
                var home = {
                    method() { return (0, eval)("super.value"); }
                };
                try { home.method(); }
                catch (error) { return error.name; }
            })()
// case: alias-cutoff / an eval alias called inside a method remains indirect and has no HomeObject
            (function () {
                var alias = eval;
                var home = {
                    method() { return alias("super.value"); }
                };
                try { home.method(); }
                catch (error) { return error.name; }
            })()
// case: super-call / object-method and global direct eval reject super call before arguments execute
            var log = [];
            var methodError;
            var arrowError;
            var nestedError;
            var globalError;
            var nestedSource = "super(log.push('nested'))";
            var home = {
                method() {
                    try { eval("super(log.push('method'))"); }
                    catch (error) { methodError = error.name; }
                    try { (() => eval("super(log.push('arrow'))"))(); }
                    catch (error) { arrowError = error.name; }
                    try { eval("eval(nestedSource)"); }
                    catch (error) { nestedError = error.name; }
                }
            };
            home.method();
            try { eval("super(log.push('global'))"); }
            catch (error) { globalError = error.name; }
            [methodError, arrowError, nestedError, globalError, log.join(",")].join("|");
