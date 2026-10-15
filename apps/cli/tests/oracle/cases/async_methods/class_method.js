// mode: print
// SEMANTIC_CASES
// case: instance and static shape source descriptors and settlement

var events = [];
var answer = "pending";
class C {
    async /*a*/ add /*b*/ (left, right) /*c*/ {
        events.push("body");
        return await (left + right);
    }
    static async /*d*/ ["sum"] /*e*/ (left, right) {
        return await (left + right);
    }
}
var method = C.prototype.add;
var staticMethod = C.sum;
var promise = new C().add(19, 23);
events.push("after");
promise.then(function (value) {
    answer = value;
    events.push("then");
    print("settled=" + answer + "|" + events.join(","));
});
var constructError = "none";
var newTargetError = "none";
try {
    new method(1, 2);
} catch (error) {
    constructError = error.name;
}
try {
    Reflect.construct(function () {}, [], staticMethod);
} catch (error) {
    newTargetError = error.name;
}
var descriptor = Object.getOwnPropertyDescriptor(C.prototype, "add");
var staticDescriptor = Object.getOwnPropertyDescriptor(C, "sum");
print("shape=" + [
    typeof method,
    method.name,
    method.length,
    Object.prototype.hasOwnProperty.call(method, "prototype"),
    method.prototype === undefined,
    Object.prototype.toString.call(method),
    Object.getPrototypeOf(method).constructor.name,
    Object.getPrototypeOf(method) === Object.getPrototypeOf(async function () {}),
    Object.getPrototypeOf(promise) === Promise.prototype,
    descriptor.enumerable,
    descriptor.configurable,
    descriptor.writable,
    staticMethod.name,
    staticMethod.length,
    staticDescriptor.enumerable,
    staticDescriptor.configurable,
    staticDescriptor.writable,
    Function.prototype.toString.call(method),
    Function.prototype.toString.call(staticMethod),
    constructError,
    newTargetError,
    events.join(","),
    answer
].join("|"));
// case: parameter abrupt completion becomes a rejected promise

var events = [];
var outcome = "pending";
var synchronous = "none";
class C {
    async fail(
        value = (
            events.push("parameter"),
            function () { throw new RangeError("parameter"); }
        )()
    ) {
        events.push("body");
        return value;
    }
}
var promise;
try {
    promise = new C().fail();
    events.push("after-call");
} catch (error) {
    synchronous = error.name + ":" + error.message;
}
promise.then(undefined, function (error) {
    outcome = error.name + ":" + error.message;
    events.push("rejected");
    print("settled=" + outcome + "|" + events.join(","));
});
print("sync=" + [
    synchronous,
    Object.getPrototypeOf(promise) === Promise.prototype,
    outcome,
    events.join(",")
].join("|"));
// case: instance and static home objects survive parameters await and borrowing

class Base {
    get answer() { return this.seed + 2; }
    add(delta) { return this.seed + delta; }
    static get answer() { return this.seed + 2; }
    static add(delta) { return this.seed + delta; }
}
class C extends Base {
    constructor(seed) {
        super();
        this.seed = seed;
    }
    async read(fromParameter = super.answer) {
        var before = super.answer;
        await 0;
        return [
            fromParameter,
            before,
            super.answer,
            super.add(2),
            this.seed
        ].join(",");
    }
    static async read(fromParameter = super.answer) {
        var before = super.answer;
        await 0;
        return [
            fromParameter,
            before,
            super.answer,
            super.add(2),
            this.seed
        ].join(",");
    }
}
C.seed = 40;
var instance = new C(40);
instance.read().then(function (value) {
    print("instance=" + value);
});
C.read().then(function (value) {
    print("static=" + value);
});
C.prototype.read.call({ seed: 100 }).then(function (value) {
    print("borrowed-instance=" + value);
});
C.read.call({ seed: 100 }).then(function (value) {
    print("borrowed-static=" + value);
});
print("sync=instance|static|borrowed-instance|borrowed-static");
// CONTEXTUAL_VALID_CASES
// case: instance and static identifiers start async methods

class C {
    async instance() { return await 20; }
    static async method() { return await 22; }
}
new C().instance().then(function (left) {
    C.method().then(function (right) {
        print(left + right);
    });
});
// case: QuickJS ignores Unicode separators inside prefix block comments
class C { async /* */ instance() { return await 20; } static async /* */ method() { return await 22; } }
new C().instance().then(function (left) { C.method().then(function (right) { print(left + right); }); });
// case: computed and escaped property names retain async execution

var key = "left";
class C {
    async [key]() { return await 20; }
    static async r\u0069ght() { return await 22; }
}
new C().left().then(function (left) {
    C.right().then(function (right) {
        print(left + right);
    });
});
// case: async immediately before parens remains a synchronous method name

class C {
    async() { return 20; }
    static async() { return 22; }
}
print(new C().async() + C.async() + "|" + [
    Object.prototype.toString.call(C.prototype.async),
    Object.prototype.toString.call(C.async)
].join("|"));
// case: line terminators split async fields from following synchronous methods

class C {
    async
    instance() { return 20; }
    static async
    method() { return 22; }
}
var value = new C();
print(String(value.async) + "|" + String(C.async) + "|" + value.instance() + C.prototype.method());
// case: get await and static constructor are ordinary async method names

class C {
    async get() { return await 10; }
    async await() { return await 11; }
    static async constructor() { return await 21; }
}
var value = new C();
value.get().then(function (first) {
    value.await().then(function (second) {
        C.constructor().then(function (third) {
            print(first + second + third);
        });
    });
});
// CONTEXTUAL_INVALID_CASES
// case: an instance async constructor is not the class constructor
class C { async constructor() {} }
// case: a static async prototype method is forbidden
class C { static async prototype() {} }
// case: async cannot prefix an accessor declaration
class C { async get value() {} }
// case: await is forbidden as an async method parameter binding
class C { async method(await) {} }
// case: await expressions are forbidden in async method parameters
class C { async method(value = await 1) {} }
// case: escaped async cannot act as the contextual prefix
class C { as\u0079nc method() {} }
// case: strict class methods reject duplicate parameters
class C { async method(value, value) {} }
// case: QuickJS does not fall back to an async-named field before semicolon
class C { async; }
// case: super calls are forbidden in async class methods
class C extends Object { async method() { super(); } }
