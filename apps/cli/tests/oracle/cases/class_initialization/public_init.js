// case: computed keys, element ordering, and field descriptors
(function () {
    var order = [];
    var counts = {};
    function key(name) {
        counts[name] = (counts[name] || 0) + 1;
        order.push("key:" + name);
        return name;
    }

    class Ordered {
        [key("i1")] = (order.push("instance:i1"), 11);
        static [key("s1")] = (order.push("static:s1"), 21);
        static { order.push("block:1"); }
        [key("i2")] = (order.push("instance:i2"), 12);
        static [key("s2")] = (order.push("static:s2"), 22);
        static { order.push("block:2"); }
        constructor() { order.push("body"); }
    }

    var afterDefinition = order.join(",");
    var first = new Ordered();
    var second = new Ordered();

    var setterCalls = 0;
    class DescriptorBase {}
    Object.defineProperty(DescriptorBase.prototype, "value", {
        configurable: true,
        set: function () { setterCalls++; }
    });
    Object.defineProperty(DescriptorBase, "staticValue", {
        configurable: true,
        set: function () { setterCalls++; }
    });
    class DescriptorDerived extends DescriptorBase {
        value = 41;
        static staticValue = 42;
    }
    var descriptorInstance = new DescriptorDerived();
    var instanceDescriptor = Object.getOwnPropertyDescriptor(descriptorInstance, "value");
    var staticDescriptor = Object.getOwnPropertyDescriptor(DescriptorDerived, "staticValue");

    return [
        afterDefinition,
        order.join(","),
        [counts.i1, counts.s1, counts.i2, counts.s2].join(","),
        [first.i1, first.i2, second.i1, second.i2, Ordered.s1, Ordered.s2].join(","),
        setterCalls,
        [instanceDescriptor.value, instanceDescriptor.writable,
            instanceDescriptor.enumerable, instanceDescriptor.configurable].join(","),
        [staticDescriptor.value, staticDescriptor.writable,
            staticDescriptor.enumerable, staticDescriptor.configurable].join(",")
    ].join("|");
})()
// case: base, derived, and early-installed instance initializer timing
(function () {
    var baseOrder = [];
    class BaseTiming {
        field = (baseOrder.push("field"), 42);
        constructor(value = (baseOrder.push("parameter"), 1)) {
            baseOrder.push("body");
            this.value = value;
        }
    }
    var base = new BaseTiming();

    var replacement = { kind: "replacement" };
    var derivedOrder = [];
    class ReplacementBase {
        constructor() {
            derivedOrder.push("super");
            return replacement;
        }
    }
    class ReplacementDerived extends ReplacementBase {
        field = (derivedOrder.push("field"), 42);
        constructor() {
            derivedOrder.push("before");
            super();
            derivedOrder.push("after");
        }
    }
    var derived = new ReplacementDerived();

    var installOrder = [];
    class InstalledBeforeStatic {
        field = (installOrder.push("instance"), 42);
        static made = (installOrder.push("static"), new this());
        static { installOrder.push("block:" + this.made.field); }
    }

    return [
        baseOrder.join(","),
        base.field,
        derivedOrder.join(","),
        derived === replacement,
        derived.field,
        installOrder.join(",")
    ].join("|");
})()
// case: anonymous class field names exist before nested static initialization
(function () {
    class Outer {
        fixed = class { static seen = this.name; };
        ["computed"] = class { static seen = this.name; };
        static sfixed = class { static seen = this.name; };
        static ["scomputed"] = class { static seen = this.name; };
    }
    var instance = new Outer();
    return [
        instance.fixed.name, instance.fixed.seen,
        instance.computed.name, instance.computed.seen,
        Outer.sfixed.name, Outer.sfixed.seen,
        Outer.scomputed.name, Outer.scomputed.seen
    ].join("|");
})()
// case: computed constructor and prototype field names remain grammar-valid
(function () {
    function errorName(source) {
        try { eval(source); return "none"; }
        catch (error) { return error.name; }
    }
    class ComputedNames {
        ["constructor"] = 11;
        ["prototype"] = 12;
        static ["constructor"] = 21;
    }
    var instance = new ComputedNames();
    return [
        instance.constructor,
        instance.prototype,
        ComputedNames.constructor,
        errorName("(class { static ['prototype'] = 22; })")
    ].join("|");
})()
// case: arguments context boundaries and QuickJS shorthand capture
(function () {
    function errorName(source) {
        try { eval(source); return "none"; }
        catch (error) { return error.name; }
    }

    var directEvalErrors = [
        errorName("new (class { field = eval('arguments'); })"),
        errorName("(class { static field = eval('arguments'); })"),
        errorName("(class { static { eval('arguments'); } })")
    ].join(",");

    class OrdinaryBoundary {
        field = (function () { return arguments.length; })(1, 2);
        static staticField = (function () { return arguments[0]; })(41);
        static {
            this.block = (function () { return arguments[0]; })(42);
        }
    }
    var ordinary = new OrdinaryBoundary();

    var shorthand = (function () {
        class CapturesOuterArguments {
            field = ({arguments});
            static staticField = ({arguments});
            static { this.block = ({arguments}); }
            evalField = eval("({arguments})");
        }
        var instance = new CapturesOuterArguments();
        return [
            instance.field.arguments[0],
            CapturesOuterArguments.staticField.arguments[0],
            CapturesOuterArguments.block.arguments[0],
            instance.evalField.arguments[0]
        ].join(",");
    })(43);

    var bindings = (function () {
        class ArgumentsBindings {
            field = (class arguments {});
            static staticField = (class arguments {});
            static { class arguments {} this.blockAccepted = true; }
        }
        var instance = new ArgumentsBindings();
        class EvalBinding {
            field = eval("(class arguments {})");
        }
        return [
            instance.field.name,
            ArgumentsBindings.staticField.name,
            ArgumentsBindings.blockAccepted,
            new EvalBinding().field.name
        ].join(",");
    })();

    return [
        directEvalErrors,
        [ordinary.field, OrdinaryBoundary.staticField, OrdinaryBoundary.block].join(","),
        shorthand,
        bindings
    ].join("|");
})()
// case: static-block await restrictions stop at arrow bodies and functions
(function () {
    function errorName(source) {
        try { eval(source); return "none"; }
        catch (error) { return error.name; }
    }
    var directEvalError = errorName("(class { static { eval('await'); } })");

    class AwaitBoundaries {
        static {
            this.concise = () => await;
            this.block = () => { let await = 40; return await + 2; };
            this.ordinary = function (await) { return await; };
            this.nestedArrow = function () { return ((await) => await)(42); };
        }
    }
    return [
        typeof AwaitBoundaries.concise,
        AwaitBoundaries.block(),
        AwaitBoundaries.ordinary(42),
        AwaitBoundaries.nestedArrow(),
        directEvalError
    ].join("|");
})()
// case: NamedEvaluation survives unresolved forward control-flow edges
(function () {
    var out = [];
    if (true) {
        let C = class { static seen = this.name; };
        out.push(C.name + ":" + C.seen);
    }
    try {
        let C = class { static seen = this.name; };
        out.push(C.name + ":" + C.seen);
    } catch (error) {
        out.push(error.name);
    }

    function direct(C = class { static seen = this.name; }) {
        return C.name + ":" + C.seen;
    }
    function array([C = class { static seen = this.name; }] = []) {
        return C.name + ":" + C.seen;
    }
    function object({C = class { static seen = this.name; }} = {}) {
        return C.name + ":" + C.seen;
    }
    out.push(direct(), array(), object());

    var C = "old";
    var caught;
    try {
        C = class { static { throw 42; } };
    } catch (error) {
        caught = error;
    }
    out.push(C + ":" + caught);
    return out.join("|");
})()
// case: authored loop backedges rebuild fresh class initialization state
(function () {
    var out = [];
    for (var index = 0; index < 2; index++) {
        class Fresh {
            field = (out.push("instance:" + index), index);
            static value = (out.push("static:" + index), index);
            static { out.push("block:" + this.value); }
        }
        out.push("value:" + new Fresh().field);
    }
    return out.join(",");
})()
