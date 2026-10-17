// mode: completion
// case: base class observation
(function () {
    var out = [];
    function add(name, value) { out.push(name + "=" + value); }

    var defaults = 0;
    class C {
        constructor(value = (defaults++, 41)) { this.value = value; }
        get next() { return this.value + 1; }
        set next(value) { this.value = value - 1; }
        read() { return this.value; }
        static answer() { return 42; }
    }
    var callError;
    try { C(); }
    catch (error) { callError = error.name + ":" + error.message; }
    var instance = new C();
    instance.next = 43;
    add("constructor", [callError, defaults, instance.read(), instance.next,
        C.answer(), C.name, C.length, instance instanceof C].join("|"));

    var lateDefaults = 0;
    class LateDefault {
        constructor(first, second = (lateDefaults++, 42), third) {
            this.value = second;
        }
    }
    class RestConstructor {
        constructor(...values) { this.count = values.length; }
    }
    var lateCallError;
    try { LateDefault(1); }
    catch (error) { lateCallError = error.name; }
    var lateDefaultsAfterCall = lateDefaults;
    var lateInstance = new LateDefault(1);
    add("parameters", [lateCallError, lateDefaultsAfterCall, lateDefaults,
        LateDefault.length, lateInstance.value, RestConstructor.length,
        new RestConstructor(1, 2, 3).count].join("|"));

    class StringConstructor {
        "constructor"() { this.value = 42; }
    }
    class ComputedConstructor {
        ["constructor"]() { return 43; }
    }
    add("constructor-names", [new StringConstructor().value,
        new ComputedConstructor() instanceof ComputedConstructor,
        ComputedConstructor.prototype.constructor()].join("|"));

    var ctorPrototype = Object.getOwnPropertyDescriptor(C, "prototype");
    var prototypeCtor = Object.getOwnPropertyDescriptor(C.prototype, "constructor");
    var method = Object.getOwnPropertyDescriptor(C.prototype, "read");
    var accessor = Object.getOwnPropertyDescriptor(C.prototype, "next");
    add("descriptors", [ctorPrototype.writable, ctorPrototype.enumerable,
        ctorPrototype.configurable, prototypeCtor.writable,
        prototypeCtor.enumerable, prototypeCtor.configurable,
        method.writable, method.enumerable, method.configurable,
        accessor.enumerable, accessor.configurable].join("|"));
    add("method-names", [C.prototype.read.name, C.answer.name,
        accessor.get.name, accessor.set.name].join("|"));

    var order = "";
    function key(name) { order += name; return name; }
    class Computed {
        [key("i")]() { return 20; }
        static [key("s")]() { return 22; }
        get [key("g")]() { return 42; }
    }
    add("computed", [order, new Computed().i() + Computed.s(),
        Object.getOwnPropertyDescriptor(Computed.prototype, "g").get.name].join("|"));

    Object.prototype.baseClassValue = 20;
    Function.prototype.baseStaticValue = 22;
    class Home {
        read() { return super.baseClassValue + 22; }
        static read() { return super.baseStaticValue + 20; }
    }
    class ConstructorHome {
        constructor() { this.answer = super.baseClassValue + 22; }
    }
    add("super", [new Home().read(), Home.read(),
        new ConstructorHome().answer].join("|"));
    delete Object.prototype.baseClassValue;
    delete Function.prototype.baseStaticValue;

    var Anonymous = class {};
    var Named = class Inner {};
    add("names", [Anonymous.name, Named.name, typeof Inner].join("|"));
    add("special-names", (function () {
        class eval {}
        var NamedArguments = class arguments {};
        return [eval.name, NamedArguments.name].join("|");
    })());

    var Captured = class InnerCapture { self() { return InnerCapture; } };
    var capturedInstance = new Captured();
    var originalCaptured = Captured;
    Captured = null;
    var methodStrictError;
    class StrictMethod { run() { classMethodLeak = 1; } }
    try { new StrictMethod().run(); }
    catch (error) { methodStrictError = error.name; }
    var methodConstructError;
    try { new C.prototype.read(); }
    catch (error) { methodConstructError = error.name; }
    var computedPrototypeError;
    try { (class { static ["prototype"]() {} }); }
    catch (error) { computedPrototypeError = error.name; }
    add("methods", [capturedInstance.self() === originalCaptured,
        methodStrictError, typeof classMethodLeak, methodConstructError,
        computedPrototypeError].join("|"));
    add("source", [String(C).indexOf("class C {") === 0,
        String(C.prototype.read).indexOf("read()") === 0].join("|"));

    var declarationTdz;
    try { Before; }
    catch (error) { declarationTdz = error.name; }
    class Before {}
    var computedTdz;
    try { class During { [During]() {} } }
    catch (error) { computedTdz = error.name; }
    var computedAssignment;
    try { (class { [classBaseLeak = 1]() {} }); }
    catch (error) { computedAssignment = error.name; }
    var strictOuterError;
    try {
        (function () {
            "use strict";
            return class { [strictOuterClassLeak = 1]() {} };
        })();
    }
    catch (error) { strictOuterError = error.name; }
    var evalClassTdz;
    try { class EvalClass { [eval("EvalClass")]() {} } }
    catch (error) { evalClassTdz = error.name; }
    var recovered;
    try { (class { [(function () { throw 1; })()]() {} }); }
    catch (_) { recoveredAfterClass = 42; recovered = recoveredAfterClass; }
    add("scope", [declarationTdz, computedTdz, computedAssignment,
        typeof classBaseLeak, strictOuterError, typeof strictOuterClassLeak,
        evalClassTdz, recovered].join("|"));
    delete classBaseLeak;
    delete recoveredAfterClass;

    class PrimitiveReturn { constructor() { return 1; } }
    class ObjectReturn { constructor() { return { answer: 42 }; } }
    add("returns", [(new PrimitiveReturn()) instanceof PrimitiveReturn,
        new ObjectReturn().answer].join("|"));

    return out.join("\n");
})()
// case: a computed key's eval runs sloppy code
(function () {
    class SloppyEvalKey {
        [eval("with ({ value: 'eval' }) value")]() { return 42; }
    }
    return new SloppyEvalKey().eval();
})()
