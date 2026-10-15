// mode: completion-name
// case: lookup / dot and computed reads start at the HomeObject prototype
            (function () {
                var grand = { inherited: 20 };
                var proto = Object.create(grand);
                proto.direct = 22;
                var object = {
                    __proto__: proto,
                    direct: 100,
                    inherited: 100,
                    read() { return super.direct + super["inherited"]; }
                };
                return object.read();
            })()
// case: receiver / super getters calls and setters use the current call receiver
            (function () {
                var proto = {
                    method(addend) { return this.base + addend; },
                    get current() { return this.base + 1; },
                    set current(value) { this.saved = value + 1; }
                };
                var home = {
                    __proto__: proto,
                    method() { return super.method(2); },
                    get read() { return super.current; },
                    set write(value) { super.current = value; }
                };
                var receiver = { base: 40 };
                var descriptor = Object.getOwnPropertyDescriptor(home, "read");
                var setter = Object.getOwnPropertyDescriptor(home, "write").set;
                var methodResult = home.method.call(receiver);
                var getterResult = descriptor.get.call(receiver);
                setter.call(receiver, 41);
                return [
                    methodResult,
                    getterResult,
                    receiver.saved,
                    Object.hasOwn(home, "saved")
                ].join("|");
            })()
// case: receiver / QuickJS super-call getter rewrite reads with the frozen base before calling with this
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
                    call() { return super.callable(); }
                };
                var receiver = { base: 40 };
                return [home.call.call(receiver), getterReceiver === proto].join("|");
            })()
// case: home-object / an extracted method retains its HomeObject and observes its current prototype
            (function () {
                var first = { value: 1 };
                var second = { value: 42 };
                var home = {
                    __proto__: first,
                    method() { return super.value; }
                };
                var method = home.method;
                Object.setPrototypeOf(home, second);
                return method.call({ value: 100 });
            })()
// case: assignment / simple assignment returns the RHS and sends the current receiver to super Set
            (function () {
                var proto = {
                    set value(input) { this.saved = input + 1; }
                };
                var home = {
                    __proto__: proto,
                    write(input) { return super.value = input; }
                };
                var receiver = {};
                var result = home.write.call(receiver, 41);
                return [result, receiver.saved, Object.hasOwn(home, "saved")].join("|");
            })()
// case: assignment / compound assignment performs receiver-sensitive Get then Set once each
            (function () {
                var log = "";
                var proto = {
                    get value() { log += "g"; return this.count; },
                    set value(input) { log += "s"; this.count = input; }
                };
                var home = {
                    __proto__: proto,
                    count: 40,
                    bump() { return super.value += 2; }
                };
                var result = home.bump();
                return [result, home.count, log].join("|");
            })()
// case: assignment / logical assignments short circuit RHS and super Set independently
            (function () {
                var log = "";
                var rhsCalls = 0;
                var proto = {
                    get value() { log += "g"; return this.slot; },
                    set value(input) { log += "s"; this.slot = input; }
                };
                var home = {
                    __proto__: proto,
                    and(rhs) { return super.value &&= rhs(); },
                    or(rhs) { return super.value ||= rhs(); },
                    nil(rhs) { return super.value ??= rhs(); }
                };
                function rhs() { rhsCalls += 1; return 42; }
                var a = { slot: 0 };
                var b = { slot: 0 };
                var c = { slot: null };
                var andResult = home.and.call(a, rhs);
                var orResult = home.or.call(b, rhs);
                var nilResult = home.nil.call(c, rhs);
                return [
                    andResult,
                    orResult,
                    nilResult,
                    a.slot,
                    b.slot,
                    c.slot,
                    rhsCalls,
                    log
                ].join("|");
            })()
// case: assignment / JS_PROP_THROW_STRICT distinguishes rejected sloppy and strict super writes
            (function () {
                var home = {
                    sloppy() {
                        Object.freeze(this);
                        super.value = 1;
                        return 42;
                    },
                    strict() {
                        "use strict";
                        Object.freeze(this);
                        super.value = 1;
                    }
                };
                var sloppyReceiver = {};
                var strictReceiver = {};
                var strictError;
                var result = home.sloppy.call(sloppyReceiver);
                try { home.strict.call(strictReceiver); }
                catch (error) { strictError = error.name; }
                return [
                    result,
                    strictError,
                    Object.hasOwn(sloppyReceiver, "value"),
                    Object.hasOwn(strictReceiver, "value")
                ].join("|");
            })()
// case: update / postfix and prefix updates preserve their distinct result values
            (function () {
                var log = "";
                var proto = {
                    get value() { log += "g"; return this.count; },
                    set value(input) { log += "s"; this.count = input; }
                };
                var home = {
                    __proto__: proto,
                    count: 40,
                    update() {
                        var post = super.value++;
                        var pre = ++super.value;
                        return [post, pre, this.count, log].join("|");
                    }
                };
                return home.update();
            })()
// case: null-base / null HomeObject prototypes throw after assignment RHS evaluation
            (function () {
                var rhsCalls = 0;
                var home = {
                    __proto__: null,
                    read() { return super.value; },
                    write() { return super.value = (rhsCalls += 1, 42); }
                };
                var readError;
                var writeError;
                try { home.read(); } catch (error) { readError = error.name; }
                try { home.write(); } catch (error) { writeError = error.name; }
                return [
                    readError,
                    writeError,
                    rhsCalls,
                    Object.hasOwn(home, "value")
                ].join("|");
            })()
// case: null-base / computed super read coerces its key before diagnosing a null base
            (function () {
                var key = { toString() { throw "key coercion"; } };
                var home = { __proto__: null, read() { return super[key]; } };
                try { home.read(); } catch (error) { return error; }
            })()
// case: null-base / null super assignment rejects the base before deferred key coercion
            (function () {
                var log = [];
                var key = { toString() { log.push("key"); return "value"; } };
                var home = {
                    __proto__: null,
                    write() { return super[key] = (log.push("rhs"), 42); }
                };
                var errorName;
                try { home.write(); } catch (error) { errorName = error.name; }
                return [errorName, log.join(",")].join("|");
            })()
// case: computed-order / SuperBase is fixed before deferred computed-key conversion
            (function () {
                var log = [];
                var first = {
                    get value() { log.push("get:first"); return this.stored; },
                    set value(input) { log.push("set:first"); this.stored = input; }
                };
                var second = {
                    get value() { log.push("get:second"); return -1; },
                    set value(input) { log.push("set:second"); this.stored = -1; }
                };
                var home = {
                    __proto__: first,
                    stored: 40,
                    read() { return super[key]; },
                    compound() { return super[key] += 1; },
                    prefix() { return ++super[key]; }
                };
                var key = {
                    toString() {
                        log.push("key");
                        Object.setPrototypeOf(home, second);
                        return "value";
                    }
                };
                Object.setPrototypeOf(home, first);
                var read = home.read();
                Object.setPrototypeOf(home, first);
                var compound = home.compound();
                Object.setPrototypeOf(home, first);
                var prefix = home.prefix();
                return [read, compound, prefix, home.stored, log.join(",")].join("|");
            })()
// case: computed-order / computed method and accessor keys run in source order and retain HomeObject
            (function () {
                var log = [];
                function key(name) { log.push("key:" + name); return name; }
                var proto = {
                    method() { return this.base + 2; },
                    get value() { return this.base + 1; },
                    set value(input) { this.saved = input + 1; }
                };
                var home = {
                    __proto__: (log.push("proto"), proto),
                    [key("method")]() {
                        log.push("method-body");
                        return super.method();
                    },
                    get [key("read")]() {
                        log.push("get-body");
                        return super.value;
                    },
                    set [key("write")](input) {
                        log.push("set-body");
                        super.value = input;
                    }
                };
                var receiver = { base: 40 };
                var methodResult = home.method.call(receiver);
                var getter = Object.getOwnPropertyDescriptor(home, "read").get;
                var setter = Object.getOwnPropertyDescriptor(home, "write").set;
                var getterResult = getter.call(receiver);
                setter.call(receiver, 41);
                return [
                    methodResult,
                    getterResult,
                    receiver.saved,
                    log.join(",")
                ].join("|");
            })()
// case: reference / delete super property is a runtime ReferenceError
            (function () {
                var home = { remove() { return delete super.value; } };
                try { home.remove(); } catch (error) { return error.name; }
            })()
// case: reference / for-of assignment rotates the yielded value behind the super Reference
            (function () {
                var proto = { set value(input) { this.total += input; } };
                var home = {
                    __proto__: proto,
                    total: 0,
                    run() {
                        for (super.value of [20, 22]) {}
                        return this.total;
                    }
                };
                return home.run();
            })()
// case: grammar / direct super calls are forbidden in ObjectLiteral methods
({ method() { super(); } })
