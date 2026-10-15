// mode: print
// SEMANTIC_CASES
// case: shape source await and promise settlement

var events = [];
var answer = "pending";
var object = { async /*a*/ add /*b*/ (left, right) /*c*/ { events.push("body"); return await (left + right); } };
var method = object.add;
var promise = method(19, 23);
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
    Reflect.construct(function () {}, [], method);
} catch (error) {
    newTargetError = error.name;
}
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
    Function.prototype.toString.call(method),
    constructError,
    newTargetError,
    events.join(","),
    answer
].join("|"));
// case: parameter abrupt completion becomes a rejected promise

var events = [];
var outcome = "pending";
var synchronous = "none";
var object = {
    async fail(
        value = (
            events.push("parameter"),
            function () { throw new RangeError("parameter"); }
        )()
    ) {
        events.push("body");
        return value;
    }
};
var promise;
try {
    promise = object.fail();
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
// case: home object super properties survive parameters and await

var base = {
    get answer() {
        return this.seed + 2;
    },
    add(delta) {
        return this.seed + delta;
    }
};
var object = {
    __proto__: base,
    seed: 40,
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
};
var own = "pending";
var borrowed = "pending";
object.read().then(function (value) {
    own = value;
    print("own=" + own);
});
object.read.call({ seed: 100 }).then(function (value) {
    borrowed = value;
    print("borrowed=" + borrowed);
});
print("sync=" + own + "|" + borrowed);
// CONTEXTUAL_VALID_CASES
// case: async before an identifier starts an async method

var object = { async method() { return await 42; } };
object.method().then(function (value) {
    print(value + "|" + Object.prototype.toString.call(object.method));
});
// case: a comment without a line terminator preserves the async prefix

var object = { async /* no LineTerminator */ method() { return await 42; } };
object.method().then(print);
// case: QuickJS ignores U+2028 inside an async-prefix block comment
var object = { async /* */ method() { return await 42; } };
object.method().then(print);
// case: QuickJS ignores U+2029 inside an async-prefix block comment
var object = { async /* */ method() { return await 42; } };
object.method().then(print);
// case: async accepts a computed property name

var key = "method";
var object = { async [key]() { return await 42; } };
object.method().then(print);
// case: the property name after async may contain an escape

var object = { async m\u0065thod() { return await 42; } };
object.method().then(print);
// case: async immediately before parens is an ordinary method name

var object = { async() { return 42; } };
print(object.async() + "|" + Object.prototype.toString.call(object.async));
// case: a line terminator before parens still leaves an ordinary async-named method

var object = { async
() { return 42; } };
print(object.async() + "|" + Object.prototype.toString.call(object.async));
// case: escaped async remains an ordinary method name before parens

var object = { as\u0079nc() { return 42; } };
print(object.async() + "|" + Object.prototype.toString.call(object.async));
// case: a comma separates an async shorthand from the following method

var async = 42;
var object = { async,
    method() { return this.async; }
};
print(object.method());
// case: get and await are valid async method property names

var object = {
    async get() { return await 20; },
    async await() { return await 22; }
};
object.get().then(function (left) {
    object.await().then(function (right) {
        print(left + right);
    });
});
// CONTEXTUAL_INVALID_CASES
// case: a line terminator prevents async from prefixing a following identifier
var object = { async
method() {} };
// case: a multiline comment line terminator prevents the async prefix
var object = { async /*
*/ method() {} };
// case: a direct U+2028 line terminator prevents the async prefix
var object = { async method() {} };
// case: a direct U+2029 line terminator prevents the async prefix
var object = { async method() {} };
// case: escaped async cannot act as the contextual prefix
var object = { as\u0079nc method() {} };
// case: async cannot prefix an accessor declaration
var object = { async get value() {} };
// case: await is forbidden as an async method parameter binding
var object = { async method(await) {} };
// case: await expressions are forbidden in async method parameters
var object = { async method(value = await 1) {} };
// case: a line terminator prevents async from prefixing a generator star
var object = { async
*method() {} };
