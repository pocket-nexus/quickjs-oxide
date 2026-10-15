// mode: print
// case: private async shape source reflection and settlement

var events = [];
class C {
    async /*a*/ #sum /*b*/ (left, right = 0) {
        events.push("body");
        return await (left + right);
    }
    expose() { return this.#sum; }
    invoke(left, right) { return this.#sum(left, right); }
    static async /*c*/ #sumStatic /*d*/ (left, right) {
        return await (left + right);
    }
    static expose() { return this.#sumStatic; }
    static invoke(left, right) { return this.#sumStatic(left, right); }
}
var instanceMethod = new C().expose();
var staticMethod = C.expose();
var promise = new C().invoke(19, 23);
var staticPromise = C.invoke(20, 22);
events.push("after");
var constructError = "none";
try {
    new instanceMethod();
} catch (error) {
    constructError = error.name;
}
print("shape=" + [
    typeof instanceMethod,
    instanceMethod.name,
    instanceMethod.length,
    Object.prototype.hasOwnProperty.call(instanceMethod, "prototype"),
    instanceMethod.prototype === undefined,
    Object.prototype.toString.call(instanceMethod),
    Object.getPrototypeOf(instanceMethod).constructor.name,
    Object.getPrototypeOf(instanceMethod) === Object.getPrototypeOf(async function () {}),
    Object.getPrototypeOf(promise) === Promise.prototype,
    staticMethod.name,
    staticMethod.length,
    Object.getOwnPropertyNames(C.prototype).indexOf("#sum") < 0,
    Object.getOwnPropertyNames(C).indexOf("#sumStatic") < 0,
    Function.prototype.toString.call(instanceMethod),
    Function.prototype.toString.call(staticMethod),
    constructError,
    events.join(",")
].join("|"));
promise.then(function (value) {
    print("instance=" + value);
});
staticPromise.then(function (value) {
    print("static=" + value);
});
// case: private brands and home objects survive await and extraction

class Base {
    get answer() { return this.seed + 2; }
    static get answer() { return this.seed + 2; }
}
class C extends Base {
    constructor(seed) {
        super();
        this.seed = seed;
    }
    async #instance(fromParameter = super.answer) {
        var before = super.answer;
        await 0;
        return [fromParameter, before, super.answer, this.seed].join(",");
    }
    exposeInstance() { return this.#instance; }
    invokeInstance() { return this.#instance(); }
    hasInstance(value) { return #instance in value; }
    static async #static(fromParameter = super.answer) {
        var before = super.answer;
        await 0;
        return [fromParameter, before, super.answer, this.seed].join(",");
    }
    static exposeStatic() { return this.#static; }
    static invokeStatic() { return this.#static(); }
    static hasStatic(value) { return #static in value; }
}
C.seed = 40;
var instance = new C(40);
var instanceMethod = instance.exposeInstance();
var staticMethod = C.exposeStatic();
var instanceBrand = "none";
var staticBrand = "none";
try {
    C.prototype.invokeInstance.call({ seed: 100 });
} catch (error) {
    instanceBrand = error.name + ":" + error.message;
}
try {
    C.invokeStatic.call({ seed: 100 });
} catch (error) {
    staticBrand = error.name + ":" + error.message;
}
print("sync=" + [
    instance.hasInstance(instance),
    instance.hasInstance({}),
    C.hasStatic(C),
    C.hasStatic(class D {}),
    instanceBrand,
    staticBrand
].join("|"));
instance.invokeInstance().then(function (value) {
    print("instance=" + value);
});
C.invokeStatic().then(function (value) {
    print("static=" + value);
});
instanceMethod.call({ seed: 100 }).then(function (value) {
    print("borrowed-instance=" + value);
});
staticMethod.call({ seed: 100 }).then(function (value) {
    print("borrowed-static=" + value);
});
// case: private access timing and async rejection boundaries

var events = [];
var synchronous = [];
class C {
    #value = 42;
    async #parameter(
        value = (
            events.push("parameter"),
            function () { throw new RangeError("parameter"); }
        )()
    ) {
        return value;
    }
    async #body() {
        events.push("body");
        await 0;
        return this.#value;
    }
    callParameter() { return this.#parameter(); }
    callBodyOn(value) { return value.#body(events.push("argument")); }
    exposeBody() { return this.#body; }
    assignBody() { this.#body = 1; }
}
var instance = new C();
var parameterPromise;
try {
    parameterPromise = instance.callParameter();
    events.push("after-parameter");
} catch (error) {
    synchronous.push("parameter=" + error.name);
}
try {
    instance.callBodyOn({});
} catch (error) {
    synchronous.push("access=" + error.name + ":" + error.message);
}
var bodyPromise;
try {
    bodyPromise = instance.exposeBody().call({});
    events.push("after-body");
} catch (error) {
    synchronous.push("body=" + error.name);
}
try {
    instance.assignBody();
} catch (error) {
    synchronous.push("assign=" + error.name + ":" + error.message);
}
print("sync=" + synchronous.join(",") + "|" + events.join(","));
parameterPromise.then(undefined, function (error) {
    print("parameter=" + error.name + ":" + error.message + "|" + events.join(","));
});
bodyPromise.then(undefined, function (error) {
    print("body=" + error.name + ":" + error.message + "|" + events.join(","));
});
