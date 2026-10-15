// mode: completion-name
// case: escaped-live / an escaped arrow keeps lexical this while reading the HomeObject's live prototype
            (function () {
                var getterThis;
                var first = { get value() { getterThis = this; return -1; } };
                var second = { get value() { getterThis = this; return this.base + 2; } };
                var home = {
                    __proto__: first,
                    make() { return () => super.value; }
                };
                var receiver = { base: 40 };
                var arrow = home.make.call(receiver);
                Object.setPrototypeOf(home, second);
                return [arrow(), getterThis === receiver].join("|");
            })()
// case: nested / nested arrows inherit lexical this and the enclosing method's HomeObject
            (function () {
                var getterThis;
                var proto = {
                    get value() { getterThis = this; return this.base + 2; }
                };
                var home = {
                    __proto__: proto,
                    make() { return () => () => super.value; }
                };
                var receiver = { base: 40 };
                return [home.make.call(receiver)()(), getterThis === receiver].join("|");
            })()
// case: accessor / an escaped arrow from an accessor inherits its lexical this and HomeObject
            (function () {
                var getterThis;
                var proto = {
                    get value() { getterThis = this; return this.base + 2; }
                };
                var home = {
                    __proto__: proto,
                    get make() { return () => super.value; }
                };
                var receiver = { base: 40 };
                var getter = Object.getOwnPropertyDescriptor(home, "make").get;
                var arrow = getter.call(receiver);
                return [arrow(), getterThis === receiver].join("|");
            })()
// case: mixed-entry / arguments, new.target, and a function hoist coexist with lexical arrow super
            (function () {
                var proto = {
                    get value() { return this.base + 2; }
                };
                var home = {
                    __proto__: proto,
                    make(marker) {
                        function hoisted() { return 1; }
                        return () => [
                            super.value,
                            this.base,
                            arguments[0],
                            new.target === undefined,
                            hoisted()
                        ].join("|");
                    }
                };
                var receiver = { base: 40 };
                return home.make.call(receiver, "arg")();
            })()
// case: call-receiver / an arrow super call reads its getter from the base and calls with lexical this
            (function () {
                var getterReceiver;
                var proto = {
                    get callable() {
                        getterReceiver = this;
                        return function () { return this.base + 2; };
                    }
                };
                var home = {
                    __proto__: proto,
                    make() { return () => super.callable(); }
                };
                var receiver = { base: 40 };
                var result = home.make.call(receiver)();
                return [result, getterReceiver === proto].join("|");
            })()
// case: write / computed super assignment through an arrow writes to lexical this
            (function () {
                var proto = {
                    set value(input) { this.saved = input + 1; }
                };
                var home = {
                    __proto__: proto,
                    make() { return (key, input) => super[key] = input; }
                };
                var receiver = {};
                var result = home.make.call(receiver)("value", 41);
                return [
                    result,
                    receiver.saved,
                    Object.hasOwn(home, "saved"),
                    Object.hasOwn(proto, "saved")
                ].join("|");
            })()
// case: update / postfix and prefix super updates in an arrow preserve values and lexical receiver
            (function () {
                var log = "";
                var proto = {
                    get value() { log += "g"; return this.count; },
                    set value(input) { log += "s"; this.count = input; }
                };
                var home = {
                    __proto__: proto,
                    make() {
                        return () => {
                            var post = super.value++;
                            var pre = ++super.value;
                            return [post, pre, this.count, log].join("|");
                        };
                    }
                };
                var receiver = { count: 40 };
                return home.make.call(receiver)();
            })()
// case: strictness / an arrow super write uses inherited or local strictness independently
            (function () {
                var home = {
                    sloppy() {
                        return () => {
                            Object.freeze(this);
                            super.value = 1;
                            return 42;
                        };
                    },
                    outerStrict() {
                        "use strict";
                        return () => {
                            Object.freeze(this);
                            super.value = 1;
                        };
                    },
                    localStrict() {
                        return () => {
                            "use strict";
                            Object.freeze(this);
                            super.value = 1;
                        };
                    }
                };
                var sloppyReceiver = {};
                var outerReceiver = {};
                var localReceiver = {};
                var outerError;
                var localError;
                var result = home.sloppy.call(sloppyReceiver)();
                try { home.outerStrict.call(outerReceiver)(); }
                catch (error) { outerError = error.name; }
                try { home.localStrict.call(localReceiver)(); }
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
// case: delete / delete super in an arrow throws before computed-key coercion
            (function () {
                var log = [];
                var key = {
                    toString() { log.push("coerce"); throw "key coercion"; }
                };
                var home = {
                    make() {
                        return () => delete super[(log.push("expr"), key)];
                    }
                };
                try { home.make()(); }
                catch (error) { return [error.name, log.join(",")].join("|"); }
            })()
// case: grammar / a normal function cuts off an enclosing method's super binding
({ method() { return function () { return super.value; }; } })
// case: grammar / a global arrow has no super binding to inherit
() => super.value
