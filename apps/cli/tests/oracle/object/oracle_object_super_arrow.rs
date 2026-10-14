use crate::grouped_case_oracle::assert_cases_match_quickjs;
use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_completion_oracle::rust_case_name_observation as rust_observation;

// Pins lexical SuperProperty inheritance through arrows nested in synchronous
// ObjectLiteral methods against QuickJS 2026-06-04. Direct eval, classes,
// parameter initializers, async methods, and generators remain separate slices.
const CASES: &[Case] = &[
    Case {
        group: "escaped-live",
        description: "an escaped arrow keeps lexical this while reading the HomeObject's live prototype",
        source: r#"
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
        "#,
        expected: "return|string|42|true",
    },
    Case {
        group: "nested",
        description: "nested arrows inherit lexical this and the enclosing method's HomeObject",
        source: r#"
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
        "#,
        expected: "return|string|42|true",
    },
    Case {
        group: "accessor",
        description: "an escaped arrow from an accessor inherits its lexical this and HomeObject",
        source: r#"
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
        "#,
        expected: "return|string|42|true",
    },
    Case {
        group: "mixed-entry",
        description: "arguments, new.target, and a function hoist coexist with lexical arrow super",
        source: r#"
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
        "#,
        expected: "return|string|42|40|arg|true|1",
    },
    Case {
        group: "call-receiver",
        description: "an arrow super call reads its getter from the base and calls with lexical this",
        source: r#"
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
        "#,
        expected: "return|string|42|true",
    },
    Case {
        group: "write",
        description: "computed super assignment through an arrow writes to lexical this",
        source: r#"
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
        "#,
        expected: "return|string|41|42|false|false",
    },
    Case {
        group: "update",
        description: "postfix and prefix super updates in an arrow preserve values and lexical receiver",
        source: r#"
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
        "#,
        expected: "return|string|40|42|42|gsgs",
    },
    Case {
        group: "strictness",
        description: "an arrow super write uses inherited or local strictness independently",
        source: r#"
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
        "#,
        expected: "return|string|42|TypeError|TypeError|false|false|false",
    },
    Case {
        group: "delete",
        description: "delete super in an arrow throws before computed-key coercion",
        source: r#"
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
        "#,
        expected: "return|string|ReferenceError|expr",
    },
    Case {
        group: "grammar",
        description: "a normal function cuts off an enclosing method's super binding",
        source: "({ method() { return function () { return super.value; }; } })",
        expected: "throw|object|SyntaxError",
    },
    Case {
        group: "grammar",
        description: "a global arrow has no super binding to inherit",
        source: "() => super.value",
        expected: "throw|object|SyntaxError",
    },
];

#[test]
fn object_super_arrow_semantics_match_pinned_quickjs() {
    assert_cases_match_quickjs(
        "object-super-arrow",
        "object-super-arrow oracle",
        CASES,
        |oracle, case| {
            super::quickjs_object_super_oracle::observe_completion_name_only(
                oracle,
                case.source,
                case.group,
                case.description,
            )
        },
        rust_observation,
    );
}
