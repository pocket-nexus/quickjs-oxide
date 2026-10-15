// mode: print
// SEMANTIC_CASES
// case: instance and static shape names source descriptors and prototypes

var events = [];
var key = {
    toString: function () {
        events.push("key");
        return "computed";
    }
};
var described = Symbol("sym");
var anonymous = Symbol();
class C {
    async /*i0*/ * /*i1*/ fixed /*i2*/ (left, right = 2) {
        yield left + right;
    }
    async *["str-key"]() {}
    async *[1]() {}
    async *[described]() {}
    async *[anonymous]() {}
    async *[key]() { yield 42; }
    static async /*s0*/ * /*s1*/ fixedStatic /*s2*/ (value) {
        yield value;
    }
    static async *["static-key"]() {}
    static async *[2]() {}
    static async *[described]() {}
}
var method = C.prototype.fixed;
var staticMethod = C.fixedStatic;
var iterator = method(40);
var descriptor = Object.getOwnPropertyDescriptor(C.prototype, "fixed");
var staticDescriptor = Object.getOwnPropertyDescriptor(C, "fixedStatic");
var prototypeDescriptor = Object.getOwnPropertyDescriptor(method, "prototype");
var functionPrototype = Object.getPrototypeOf(method);
var instancePrototype = Object.getPrototypeOf(iterator);
var asyncGeneratorPrototype = Object.getPrototypeOf(instancePrototype);
var constructError = "none";
var newTargetError = "none";
try {
    new method();
} catch (error) {
    constructError = error.name;
}
try {
    Reflect.construct(function () {}, [], staticMethod);
} catch (error) {
    newTargetError = error.name;
}
print([
    events.join(","),
    C.prototype.fixed.name,
    C.prototype["str-key"].name,
    C.prototype[1].name,
    C.prototype[described].name,
    C.prototype[anonymous].name,
    C.prototype.computed.name,
    C.fixedStatic.name,
    C["static-key"].name,
    C[2].name,
    C[described].name,
    typeof method,
    method.length,
    staticMethod.length,
    Object.prototype.toString.call(method),
    functionPrototype.constructor.name,
    functionPrototype === Object.getPrototypeOf(async function* () {}),
    Object.prototype.hasOwnProperty.call(method, "prototype"),
    Object.prototype.hasOwnProperty.call(staticMethod, "prototype"),
    instancePrototype === method.prototype,
    Object.getPrototypeOf(method.prototype) === asyncGeneratorPrototype,
    Object.prototype.toString.call(iterator),
    descriptor.writable,
    descriptor.enumerable,
    descriptor.configurable,
    staticDescriptor.writable,
    staticDescriptor.enumerable,
    staticDescriptor.configurable,
    prototypeDescriptor.writable,
    prototypeDescriptor.enumerable,
    prototypeDescriptor.configurable,
    Object.keys(C.prototype).length,
    Object.keys(C).length,
    Function.prototype.toString.call(method),
    Function.prototype.toString.call(staticMethod),
    constructError,
    newTargetError
].join("|"));
// case: computed constructor publishes while computed static prototype throws

var constructorCalls = 0;
var events = [];
class C {
    constructor() {
        constructorCalls++;
    }
    async *["constructor"]() {
        yield 40;
    }
    async *prototype() {
        yield 2;
    }
    static async *constructor() {
        yield 1;
    }
}
var instance = new C();
var constructorDescriptor =
    Object.getOwnPropertyDescriptor(C.prototype, "constructor");
var staticPrototypeError = "none";
var Broken;
try {
    Broken = class {
        static async *[(events.push("prototype-key"), "prototype")]() {}
    };
    events.push("defined");
} catch (error) {
    staticPrototypeError = error.name;
}
print([
    constructorCalls,
    instance instanceof C,
    C.name,
    instance.constructor.name,
    instance.prototype.name,
    C.constructor.name,
    constructorDescriptor.writable,
    constructorDescriptor.enumerable,
    constructorDescriptor.configurable,
    staticPrototypeError,
    events.join(","),
    String(Broken)
].join("|"));
instance.constructor().next().then(function (result) {
    print("computed-constructor=" + result.value + ":" + result.done);
});
instance.prototype().next().then(function (result) {
    print("instance-prototype=" + result.value + ":" + result.done);
});
C.constructor().next().then(function (result) {
    print("static-constructor=" + result.value + ":" + result.done);
});
// case: parameters are synchronous while body await and yield are delayed

var events = [];
class C {
    async *ok(
        first,
        second = (events.push("parameter"), arguments[0] + 2)
    ) {
        events.push("body");
        await 0;
        events.push("after-await");
        var sent = yield [
            arguments.length,
            arguments[0],
            second,
            new.target === undefined
        ].join(",");
        return sent + 1;
    }
    async *fail(
        value = (
            events.push("throw-parameter"),
            function () { throw new RangeError("parameter"); }
        )()
    ) {
        events.push("miss-body");
        yield value;
    }
}
var value = new C();
var iterator = value.ok(40);
events.push("after-call");
var synchronous = "none";
try {
    value.fail();
    events.push("miss-call");
} catch (error) {
    synchronous = error.name + ":" + error.message;
    events.push("caught");
}
print("call=" + synchronous + "|" + events.join(","));
var first = iterator.next();
events.push("after-next");
print("next=" + events.join(","));
first.then(function (result) {
    print("first=" + result.value + ":" + result.done + "|" + events.join(","));
});
iterator.next(41).then(function (result) {
    print("second=" + result.value + ":" + result.done);
});
// case: base and derived super retain home objects with borrowed receivers

class Standalone {
    constructor(seed) {
        this.seed = seed;
    }
    async *read(fromParameter = super.toString()) {
        await 0;
        var sent = yield [
            fromParameter,
            super.hasOwnProperty("seed"),
            this.seed
        ].join(",");
        return sent + this.seed;
    }
    static async *read(
        fromParameter = super.call === Function.prototype.call
    ) {
        await 0;
        yield [fromParameter, this.name].join(",");
    }
}
class Base {
    get answer() { return this.seed + 2; }
    add(delta) { return this.seed + delta; }
    static get answer() { return this.seed + 2; }
    static add(delta) { return this.seed + delta; }
}
class Derived extends Base {
    constructor(seed) {
        super();
        this.seed = seed;
    }
    async *read(fromParameter = super.answer) {
        var before = super.answer;
        await 0;
        var sent = yield [
            fromParameter,
            before,
            super.answer,
            super.add(2),
            this.seed
        ].join(",");
        return super.add(sent);
    }
    static async *read(fromParameter = super.answer) {
        var before = super.answer;
        await 0;
        var sent = yield [
            fromParameter,
            before,
            super.answer,
            super.add(2),
            this.seed
        ].join(",");
        return super.add(sent);
    }
}
Derived.seed = 40;

function consume(label, iterator, sent) {
    return iterator.next().then(function (first) {
        print(label + "-first=" + first.value + ":" + first.done);
        return iterator.next(sent);
    }).then(function (second) {
        print(label + "-second=" + second.value + ":" + second.done);
    });
}

print("sync=ready");
consume("base-instance", new Standalone(40).read(), 2)
.then(function () {
    return consume(
        "base-borrowed",
        Standalone.prototype.read.call({ seed: 100 }),
        2
    );
})
.then(function () {
    return consume(
        "base-static",
        Standalone.read.call({ name: "borrowed-static" }),
        0
    );
})
.then(function () {
    return consume("derived-instance", new Derived(40).read(), 2);
})
.then(function () {
    return consume(
        "derived-borrowed",
        Derived.prototype.read.call({ seed: 100 }),
        2
    );
})
.then(function () {
    return consume("derived-static", Derived.read(), 2);
})
.then(function () {
    return consume(
        "derived-static-borrowed",
        Derived.read.call({ seed: 100 }),
        2
    );
});
// VALID_CONTEXTUAL_CASES
// case: instance and static async star prefixes

class C {
    async *instance() { yield await 20; }
    static async *method() { yield await 22; }
}
new C().instance().next().then(function (left) {
    C.method().next().then(function (right) {
        print(left.value + right.value);
    });
});
// case: comment separators computed names and escaped property names
var key = "left";
class C {
async /* */ *[key]() { yield await 20; }
static async /* */ *r\u0069ght() { yield await 22; }
}
new C().left().next().then(function (left) {
C.right().next().then(function (right) {
print(left.value + right.value);
});
});
// case: line terminators split async fields from generator methods

class C {
    async
    *instance() { yield 20; }
    static async
    *method() { yield 22; }
}
var value = new C();
print([
    Object.prototype.hasOwnProperty.call(value, "async"),
    String(value.async),
    Object.prototype.hasOwnProperty.call(C, "async"),
    String(C.async),
    Object.prototype.toString.call(C.prototype.instance),
    Object.prototype.toString.call(C.prototype.method),
    value.instance().next().value + value.method().next().value
].join("|"));
// case: async immediately before parens remains an ordinary method name

class C {
    async() { return 20; }
    static async() { return 22; }
}
print(new C().async() + C.async() + "|" + [
    Object.prototype.toString.call(C.prototype.async),
    Object.prototype.toString.call(C.async)
].join("|"));
// INVALID_CONTEXTUAL_CASES
// case: an instance async-generator constructor is forbidden
class C { async *constructor() {} }
// case: a static async-generator prototype method is forbidden
class C { static async *prototype() {} }
// case: async star cannot prefix an accessor declaration
class C { async *get value() {} }
// case: escaped async cannot act as the contextual prefix
class C { as\u0079nc *method() {} }
// case: strict class methods reject duplicate parameters
class C { async *method(value, value) {} }
// case: await is forbidden as an async-generator parameter binding
class C { async *method(await) {} }
// case: yield is forbidden as an async-generator parameter binding
class C { async *method(yield) {} }
// case: await expressions are forbidden in async-generator parameters
class C { async *method(value = await 1) {} }
// case: yield expressions are forbidden in async-generator parameters
class C { async *method(value = yield 1) {} }
// case: a non-simple parameter list forbids a strict body directive
class C { async *method(value = 1) { 'use strict'; } }
// case: super calls are forbidden in async-generator class methods
class C extends Object { async *method() { super(); } }
