// mode: print
// case: instance and static shapes names source prototypes and extraction

class C {
    async /*i0*/ * /*i1*/ #instance /*i2*/ (left, right = 2) {
        yield await (left + right);
    }
    exposeInstance() { return this.#instance; }
    invokeInstance(left, right) { return this.#instance(left, right); }
    hasInstance(value) { return #instance in value; }

    static async /*s0*/ * /*s1*/ #staticMethod /*s2*/ (value) {
        yield await value;
    }
    static exposeStatic() { return this.#staticMethod; }
    static invokeStatic(value) { return this.#staticMethod(value); }
    static hasStatic(value) { return #staticMethod in value; }
}
var instance = new C();
var instanceMethod = instance.exposeInstance();
var staticMethod = C.exposeStatic();
var instanceIterator = instanceMethod.call({ ignored: true }, 40);
var staticIterator = staticMethod.call({ ignored: true }, 42);
var instancePrototypeDescriptor =
    Object.getOwnPropertyDescriptor(instanceMethod, "prototype");
var staticPrototypeDescriptor =
    Object.getOwnPropertyDescriptor(staticMethod, "prototype");
var constructError = "none";
var newTargetError = "none";
try {
    new instanceMethod();
} catch (error) {
    constructError = error.name;
}
try {
    Reflect.construct(function () {}, [], staticMethod);
} catch (error) {
    newTargetError = error.name;
}
print([
    typeof instanceMethod,
    instanceMethod.name,
    instanceMethod.length,
    staticMethod.name,
    staticMethod.length,
    Object.prototype.toString.call(instanceMethod),
    Object.getPrototypeOf(instanceMethod).constructor.name,
    Object.getPrototypeOf(instanceMethod) === Object.getPrototypeOf(async function* () {}),
    Object.prototype.hasOwnProperty.call(instanceMethod, "prototype"),
    Object.prototype.hasOwnProperty.call(staticMethod, "prototype"),
    Object.getPrototypeOf(instanceIterator) === instanceMethod.prototype,
    Object.getPrototypeOf(staticIterator) === staticMethod.prototype,
    Object.prototype.toString.call(instanceIterator),
    instancePrototypeDescriptor.writable,
    instancePrototypeDescriptor.enumerable,
    instancePrototypeDescriptor.configurable,
    staticPrototypeDescriptor.writable,
    staticPrototypeDescriptor.enumerable,
    staticPrototypeDescriptor.configurable,
    Object.getOwnPropertyNames(C.prototype).indexOf("#instance") < 0,
    Object.getOwnPropertyNames(C).indexOf("#staticMethod") < 0,
    instance.hasInstance(instance),
    instance.hasInstance({}),
    C.hasStatic(C),
    C.hasStatic(class D {}),
    Function.prototype.toString.call(instanceMethod),
    Function.prototype.toString.call(staticMethod),
    constructError,
    newTargetError
].join("|"));
instanceIterator.next().then(function (result) {
    print("instance=" + result.value + ":" + result.done);
});
staticIterator.next().then(function (result) {
    print("static=" + result.value + ":" + result.done);
});
// case: private brands and borrowed receivers cross await boundaries

var events = [];
class C {
    #value;
    constructor(value) {
        this.#value = value;
    }
    async *#read(delta = (events.push("instance-parameter"), 2)) {
        events.push("instance-body");
        await 0;
        yield this.#value + delta;
    }
    exposeInstance() { return this.#read; }
    invokeInstance() { return this.#read(); }
    readFrom(value) { return value.#read(); }
    hasInstance(value) { return #read in value; }

    static #staticValue = 40;
    static async *#staticRead(delta = (events.push("static-parameter"), 2)) {
        events.push("static-body");
        await 0;
        yield this.#staticValue + delta;
    }
    static exposeStatic() { return this.#staticRead; }
    static invokeStatic() { return this.#staticRead(); }
    static readFrom(value) { return value.#staticRead(); }
    static hasStatic(value) { return #staticRead in value; }
}
var instance = new C(40);
var other = new C(100);
var instanceMethod = instance.exposeInstance();
var staticMethod = C.exposeStatic();
var accessErrors = [];
try {
    C.prototype.exposeInstance.call({});
} catch (error) {
    accessErrors.push("instance=" + error.name);
}
try {
    C.exposeStatic.call(class D {});
} catch (error) {
    accessErrors.push("static=" + error.name);
}
try {
    instance.readFrom({});
} catch (error) {
    accessErrors.push("target-instance=" + error.name);
}
try {
    C.readFrom(class D {});
} catch (error) {
    accessErrors.push("target-static=" + error.name);
}
var ownIterator = instanceMethod.call(instance);
var otherIterator = instanceMethod.call(other);
var wrongInstanceIterator = instanceMethod.call({});
var ownStaticIterator = staticMethod.call(C);
var wrongStaticIterator = staticMethod.call(class D {});
print("sync=" + [
    accessErrors.join(","),
    instance.hasInstance(instance),
    instance.hasInstance({}),
    C.hasStatic(C),
    C.hasStatic(class D {}),
    events.join(",")
].join("|"));
ownIterator.next().then(function (result) {
    print("own=" + result.value + ":" + result.done);
});
otherIterator.next().then(function (result) {
    print("other=" + result.value + ":" + result.done);
});
wrongInstanceIterator.next().then(undefined, function (error) {
    print("wrong-instance=" + error.name);
});
ownStaticIterator.next().then(function (result) {
    print("own-static=" + result.value + ":" + result.done);
});
wrongStaticIterator.next().then(undefined, function (error) {
    print("wrong-static=" + error.name);
});
// case: parameters are synchronous and next requests settle in FIFO order

var events = [];
class C {
    async *#sequence(
        value = (events.push("parameter"), 40)
    ) {
        events.push("body");
        await 0;
        events.push("after-await");
        var sent = yield value + 2;
        events.push("after-yield");
        return sent + 1;
    }
    async *#fail(
        value = (
            events.push("throw-parameter"),
            function () { throw new RangeError("parameter"); }
        )()
    ) {
        events.push("miss-body");
        yield value;
    }
    sequence() { return this.#sequence(); }
    fail() { return this.#fail(); }
}
var value = new C();
var iterator = value.sequence();
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
var firstPromise = iterator.next();
events.push("after-first-next");
var secondPromise = iterator.next(41);
events.push("after-second-next");
print("next=" + [
    Object.getPrototypeOf(firstPromise) === Promise.prototype,
    Object.getPrototypeOf(secondPromise) === Promise.prototype,
    events.join(",")
].join("|"));
firstPromise.then(function (result) {
    print("first=" + result.value + ":" + result.done + "|" + events.join(","));
});
secondPromise.then(function (result) {
    print("second=" + result.value + ":" + result.done + "|" + events.join(","));
});
