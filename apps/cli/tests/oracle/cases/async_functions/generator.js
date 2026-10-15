// mode: print
// case: intrinsic graph callable and iterator descriptors

async function* sample(left, right = 0) {
    yield left + right;
}
var iterator = sample(19, 23);
var functionPrototype = Object.getPrototypeOf(sample);
var instancePrototype = Object.getPrototypeOf(iterator);
var asyncGeneratorPrototype = Object.getPrototypeOf(instancePrototype);
var asyncIteratorPrototype = Object.getPrototypeOf(asyncGeneratorPrototype);
var callablePrototypeDescriptor =
    Object.getOwnPropertyDescriptor(sample, "prototype");
var nextDescriptor =
    Object.getOwnPropertyDescriptor(asyncGeneratorPrototype, "next");
var returnDescriptor =
    Object.getOwnPropertyDescriptor(asyncGeneratorPrototype, "return");
var throwDescriptor =
    Object.getOwnPropertyDescriptor(asyncGeneratorPrototype, "throw");
print([
    typeof sample,
    sample.name,
    sample.length,
    Object.prototype.toString.call(sample),
    functionPrototype.constructor.name,
    functionPrototype.constructor.length,
    "AsyncGeneratorFunction" in globalThis,
    functionPrototype === Object.getPrototypeOf(async function* () {}),
    functionPrototype.prototype === asyncGeneratorPrototype,
    instancePrototype === sample.prototype,
    asyncIteratorPrototype[Symbol.asyncIterator].call(iterator) === iterator,
    Object.getPrototypeOf(asyncIteratorPrototype) === Object.prototype,
    Object.prototype.toString.call(iterator),
    callablePrototypeDescriptor.writable,
    callablePrototypeDescriptor.enumerable,
    callablePrototypeDescriptor.configurable,
    nextDescriptor.value.name,
    nextDescriptor.value.length,
    nextDescriptor.writable,
    nextDescriptor.enumerable,
    nextDescriptor.configurable,
    returnDescriptor.value.name,
    returnDescriptor.value.length,
    returnDescriptor.writable,
    returnDescriptor.enumerable,
    returnDescriptor.configurable,
    throwDescriptor.value.name,
    throwDescriptor.value.length,
    throwDescriptor.writable,
    throwDescriptor.enumerable,
    throwDescriptor.configurable,
    Object.getOwnPropertyDescriptor(
        asyncGeneratorPrototype,
        Symbol.toStringTag
    ).value,
    Object.getOwnPropertyDescriptor(
        asyncGeneratorPrototype,
        Symbol.toStringTag
    ).writable,
    Object.getOwnPropertyDescriptor(
        asyncGeneratorPrototype,
        Symbol.toStringTag
    ).enumerable,
    Object.getOwnPropertyDescriptor(
        asyncGeneratorPrototype,
        Symbol.toStringTag
    ).configurable
].join("|"));
// case: parameters execute and throw synchronously before body start

var events = [];
async function* ok(
    value = (events.push("parameter"), 40)
) {
    events.push("body");
    yield value + 2;
}
async function* fail(
    value = (
        events.push("throw-parameter"),
        function () { throw new RangeError("parameter"); }
    )()
) {
    events.push("miss");
    yield value;
}
var iterator;
var synchronous = "none";
try {
    iterator = ok();
    events.push("after-ok-call");
} catch (error) {
    synchronous = "ok:" + error.name;
}
try {
    fail();
    events.push("miss-call");
} catch (error) {
    synchronous = error.name + ":" + error.message;
    events.push("caught");
}
print("call=" + [
    synchronous,
    events.join(","),
    Object.prototype.toString.call(iterator)
].join("|"));
var request = iterator.next();
events.push("after-next");
print("next=" + [
    Object.getPrototypeOf(request) === Promise.prototype,
    events.join(",")
].join("|"));
request.then(function (result) {
    events.push("settled");
    print("settled=" + [
        result.value,
        result.done,
        events.join(",")
    ].join("|"));
});
// case: protocol methods always return promises including wrong receivers

var sample = (async function* () {})();
var intrinsic = Object.getPrototypeOf(Object.getPrototypeOf(sample));
var sync = [];
function invoke(label, callback) {
    try {
        var promise = callback();
        sync.push(
            label + ":" +
            (Object.getPrototypeOf(promise) === Promise.prototype)
        );
        return promise;
    } catch (error) {
        sync.push(label + ":throw:" + error.name);
        return Promise.reject(error);
    }
}
var requests = [
    invoke("next", function () {
        return (async function* () {})().next(1);
    }),
    invoke("return", function () {
        return (async function* () {})().return(20);
    }),
    invoke("throw", function () {
        return (async function* () {})().throw(22);
    }),
    invoke("wrong-next", function () {
        return intrinsic.next.call({});
    }),
    invoke("wrong-return", function () {
        return intrinsic.return.call({});
    }),
    invoke("wrong-throw", function () {
        return intrinsic.throw.call({});
    })
];
print("sync=" + sync.join("|"));
function result(promise) {
    return promise.then(
        function (record) {
            return "ok:" + String(record.value) + ":" + record.done;
        },
        function (error) {
            return "reject:" +
                (error && error.name ? error.name : String(error));
        }
    );
}
Promise.all(requests.map(result)).then(function (values) {
    print("settled=" + values.join("|"));
});
// case: first next input is ignored and yield and return await assimilate

var events = [];
var thenable = {};
Object.defineProperty(thenable, "then", {
    get: function () {
        events.push("get");
        return function (resolve) {
            events.push("call");
            resolve(20);
        };
    }
});
async function* flow() {
    events.push("body");
    var resumed = yield await thenable;
    events.push("resume:" + resumed);
    return await Promise.resolve(resumed + 2);
}
var iterator = flow();
var first = iterator.next(999);
events.push("after-first");
print("sync=" + events.join("|"));
first.then(function (result) {
    events.push("first:" + result.value + ":" + result.done);
    print("first=" + events.join("|"));
    var second = iterator.next(40);
    events.push("after-second");
    second.then(function (result) {
        events.push("second:" + result.value + ":" + result.done);
        print("second=" + events.join("|"));
    });
});
// case: queued requests resume await in FIFO order and drain after completion

var events = [];
var release;
var gate = new Promise(function (resolve) {
    release = resolve;
});
async function* queued() {
    events.push("start");
    var value = await gate;
    events.push("after-await:" + value);
    var resumed = yield value;
    events.push("after-yield:" + resumed);
    return Promise.resolve(value + 2);
}
var iterator = queued();
var first = iterator.next();
var second = iterator.next(100);
var third = iterator.next(200);
events.push("queued");
print("sync=" + [
    first,
    second,
    third
].every(function (promise) {
    return Object.getPrototypeOf(promise) === Promise.prototype;
}) + "|" + events.join("|"));
function show(result) {
    return String(result.value) + ":" + result.done;
}
Promise.all([first, second, third]).then(function (results) {
    print("drain=" + results.map(show).join("|") + "|" + events.join(","));
});
release(40);
// case: return runs and awaits finally before settling

var events = [];
async function* guarded() {
    try {
        events.push("try");
        yield 1;
        events.push("miss");
    } finally {
        events.push("finally-start");
        await 0;
        events.push("finally-end");
    }
}
var iterator = guarded();
var first = iterator.next();
events.push("sync");
print("sync=" + events.join("|"));
first.then(function (result) {
    events.push("yield:" + result.value + ":" + result.done);
    var closing = iterator.return(Promise.resolve(42));
    events.push("after-return");
    closing.then(function (result) {
        events.push("return:" + result.value + ":" + result.done);
        print("settled=" + events.join("|"));
    });
});
// case: poisoned Promise constructor rejects await in the same driver entry

var events = [];
var promise = Promise.resolve(1);
Object.defineProperty(promise, "constructor", {
    get: function () {
        events.push("get");
        throw "boom";
    }
});
async function* flow() {
    try {
        await promise;
    } catch (error) {
        events.push("catch:" + error);
    }
    events.push("after");
    yield 42;
}
var request = flow().next();
events.push("after-next");
print(events.join("|"));
request.then(function (result) {
    events.push("settled:" + result.value + ":" + result.done);
    print(events.join("|"));
});
// case: poisoned completed return remains an asynchronous rejection

var events = [];
var promise = Promise.resolve(1);
Object.defineProperty(promise, "constructor", {
    get: function () {
        events.push("get");
        throw "boom";
    }
});
var iterator = (async function* () {})();
var request;
try {
    request = iterator.return(promise);
    events.push(
        "return:" +
        (Object.getPrototypeOf(request) === Promise.prototype)
    );
} catch (error) {
    events.push("throw:" + error);
}
events.push("after");
print(events.join("|"));
request.then(
    function () {
        events.push("ok");
    },
    function (error) {
        events.push("reject:" + error);
        print(events.join("|"));
    }
);
// case: iterator-result then getter reentry follows the active QuickJS driver

var iterator;
var events = [];
Object.defineProperty(Object.prototype, "then", {
    configurable: true,
    get: function () {
        events.push("get");
        delete Object.prototype.then;
        iterator.next().then(function (result) {
            events.push("nested:" + result.value + ":" + result.done);
        });
        return undefined;
    }
});
async function* reentrant() {
    yield 1;
    yield 2;
}
iterator = reentrant();
iterator.next().then(function (result) {
    events.push("outer:" + result.value + ":" + result.done);
});
Promise.resolve()
    .then(function () {})
    .then(function () {})
    .then(function () {
        print(events.join("|"));
    });
