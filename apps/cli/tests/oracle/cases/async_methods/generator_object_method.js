// mode: print
// SEMANTIC_CASES
// case: shape source descriptors prototype graph and nonconstructibility

var object = {
    async /*a*/ * /*b*/ method /*c*/ (left, right = 2) {
        yield left + right;
    }
};
var method = object.method;
var iterator = method(40);
var descriptor = Object.getOwnPropertyDescriptor(object, "method");
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
    Reflect.construct(function () {}, [], method);
} catch (error) {
    newTargetError = error.name;
}
print([
    typeof method,
    method.name,
    method.length,
    Object.prototype.toString.call(method),
    functionPrototype.constructor.name,
    functionPrototype === Object.getPrototypeOf(async function* () {}),
    Object.prototype.hasOwnProperty.call(method, "prototype"),
    instancePrototype === method.prototype,
    Object.getPrototypeOf(method.prototype) === asyncGeneratorPrototype,
    Object.prototype.toString.call(iterator),
    descriptor.writable,
    descriptor.enumerable,
    descriptor.configurable,
    prototypeDescriptor.writable,
    prototypeDescriptor.enumerable,
    prototypeDescriptor.configurable,
    Function.prototype.toString.call(method),
    constructError,
    newTargetError
].join("|"));
// case: computed and symbol names preserve ordering and proto spelling

var events = [];
var key = {
    toString: function () {
        events.push("key");
        return "computed";
    }
};
var described = Symbol("sym");
var anonymous = Symbol();
var object = {
    before: (events.push("before"), 0),
    async *fixed() {},
    async *["str-key"]() {},
    async *[1]() {},
    async *[described]() {},
    async *[anonymous]() {},
    async *[key]() { yield 42; },
    async *__proto__() {
        yield Object.getPrototypeOf(this) === Object.prototype;
    },
    after: (events.push("after"), 0)
};
print([
    events.join(","),
    object.fixed.name,
    object["str-key"].name,
    object[1].name,
    object[described].name,
    object[anonymous].name,
    object.computed.name,
    object.__proto__.name,
    Object.getPrototypeOf(object) === Object.prototype,
    Object.keys(object).join(","),
    Reflect.ownKeys(object).length
].join("|"));
object.computed().next().then(function (result) {
    print(result.value + ":" + result.done);
});
object.__proto__().next().then(function (result) {
    print(result.value + ":" + result.done);
});
// case: parameters and home object survive await yield and borrowing

var events = [];
var base = {
    get x() {
        return this.seed + 2;
    },
    add(value) {
        return this.seed + value;
    }
};
var object = {
    __proto__: base,
    seed: 40,
    async *method(
        left = (events.push("parameter"), super.x),
        right = 2
    ) {
        events.push("body");
        var before = super.x;
        await 0;
        var sent = yield left + right;
        return [
            before,
            super.x,
            super.add(2),
            this.seed,
            sent
        ].join(",");
    }
};
var method = object.method;
var iterator = method.call({ seed: 100 });
events.push("after-call");
print("call=" + events.join(","));
var first = iterator.next();
events.push("after-next");
print("next=" + events.join(","));
first.then(function (result) {
    print("first=" + result.value + ":" + result.done);
});
iterator.next(7).then(function (result) {
    print("second=" + result.value + ":" + result.done);
});
// case: parameter abrupt completion is synchronous and body stays suspended

var events = [];
var object = {
    async *ok(value = (events.push("parameter"), 42)) {
        events.push("body");
        yield value;
    },
    async *fail(
        value = (
            events.push("throw-parameter"),
            function () { throw new RangeError("parameter"); }
        )()
    ) {
        events.push("miss-body");
        yield value;
    }
};
var iterator = object.ok();
events.push("after-ok");
var synchronous = "none";
try {
    object.fail();
    events.push("miss-call");
} catch (error) {
    synchronous = error.name + ":" + error.message;
    events.push("caught");
}
print("call=" + synchronous + "|" + events.join(","));
iterator.next().then(function (result) {
    events.push("settled");
    print("next=" + result.value + ":" + result.done + "|" + events.join(","));
});
// VALID_CONTEXTUAL_CASES
// case: comment separators and computed keyword names
var object = { async /* */ * ['await']() { yield 42; } };
object.await().next().then(function (result) { print(result.value); });
// case: async before parens stays an ordinary method name

var object = { async() { return 42; } };
print(object.async() + "|" + Object.prototype.toString.call(object.async));
// INVALID_CONTEXTUAL_CASES
// case: a line terminator prevents async from prefixing the star
var object = { async
*method() {} };
// case: a multiline-comment terminator prevents the async prefix
var object = { async /*
*/ *method() {} };
// case: a direct U+2028 terminator prevents the async prefix
var object = { async *method() {} };
// case: escaped async cannot act as the contextual prefix
var object = { as\u0079nc *method() {} };
// case: duplicate parameters are forbidden
var object = { async *method(value, value) {} };
// case: await is forbidden as a parameter binding
var object = { async *method(await) {} };
// case: yield is forbidden as a parameter binding
var object = { async *method(yield) {} };
// case: await expressions are forbidden in parameters
var object = { async *method(value = await 1) {} };
// case: yield expressions are forbidden in parameters
var object = { async *method(value = yield 1) {} };
// case: a non-simple parameter list forbids a strict body directive
var object = { async *method(value = 1) { 'use strict'; } };
// case: super calls are forbidden in object methods
var object = { async *method() { super(); } };
