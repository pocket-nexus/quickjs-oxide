// mode: print
function value(value) {
    if (typeof value === "bigint") return String(value) + "n";
    if (typeof value === "number" && Object.is(value, -0)) return "-0";
    return String(value);
}

function completion(thunk) {
    try { return "return:" + value(thunk()); }
    catch (error) { return "throw:" + error.name; }
}

function diagnostic(thunk) {
    try { return "return:" + value(thunk()); }
    catch (error) { return "throw:" + error.name + ":" + error.message; }
}

function bits(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return Number(descriptor.writable) + "" + Number(descriptor.enumerable) +
        Number(descriptor.configurable);
}

function isConstructor(fn) {
    try { Reflect.construct(function () {}, [], fn); return true; }
    catch (_) { return false; }
}

function emit(label, observation) {
    print(label + "=" + observation);
}

emit("namespace", [
    Reflect.ownKeys(Atomics).map(String).join(","),
    bits(globalThis, "Atomics"),
    Object.getPrototypeOf(Atomics) === Object.prototype,
    Object.prototype.toString.call(Atomics)
].join("|"));

emit("metadata", [
    "add:3", "and:3", "or:3", "sub:3", "xor:3", "exchange:3",
    "compareExchange:4", "load:2", "store:3", "isLockFree:1",
    "pause:0", "wait:4", "notify:3"
].map(function (entry) {
    var parts = entry.split(":"), name = parts[0], length = parts[1];
    var fn = Atomics[name];
    return name + ":" + fn.name + ":" + fn.length + ":" +
        (fn.length === Number(length)) + ":" + bits(Atomics, name) + ":" +
        isConstructor(fn) + ":" + Reflect.ownKeys(fn).map(String).join(",");
}).join(";"));

emit("tag", (function () {
    var descriptor = Object.getOwnPropertyDescriptor(
        Atomics, Symbol.toStringTag);
    return value(descriptor.value) + ":" + Number(descriptor.writable) +
        Number(descriptor.enumerable) + Number(descriptor.configurable);
})());

emit("number-rmw", (function () {
    var view = new Int32Array(new ArrayBuffer(8));
    view[1] = 6;
    var observations = [];
    function step(name, operand) {
        observations.push(name + ":" + Atomics[name](view, 1, operand) +
            ">" + view[1]);
    }
    step("add", 5);
    step("and", 6);
    step("or", 8);
    step("xor", 3);
    step("sub", 4);
    step("exchange", -7);
    observations.push("compare-hit:" + Atomics.compareExchange(view, 1, -7, 19) +
        ">" + view[1]);
    observations.push("compare-miss:" + Atomics.compareExchange(view, 1, -7, 23) +
        ">" + view[1]);
    observations.push("load:" + Atomics.load(view, 1));
    return observations.join("|");
})());

emit("bigint-rmw", (function () {
    var view = new BigInt64Array(new ArrayBuffer(8));
    view[0] = 6n;
    var observations = [];
    function step(name, operand) {
        observations.push(name + ":" + value(Atomics[name](view, 0, operand)) +
            ">" + value(view[0]));
    }
    step("add", 5n);
    step("and", 6n);
    step("or", 8n);
    step("xor", 3n);
    step("sub", 4n);
    step("exchange", -7n);
    observations.push("compare-hit:" +
        value(Atomics.compareExchange(view, 0, -7n, 19n)) +
        ">" + value(view[0]));
    observations.push("compare-miss:" +
        value(Atomics.compareExchange(view, 0, -7n, 23n)) +
        ">" + value(view[0]));
    observations.push("load:" + value(Atomics.load(view, 0)));
    return observations.join("|");
})());

emit("unsigned-rmw", (function () {
    var view = new BigUint64Array(new ArrayBuffer(8));
    view[0] = (1n << 64n) - 1n;
    var old = Atomics.add(view, 0, 2n);
    return value(old) + ">" + value(view[0]) + "|" +
        value(Atomics.exchange(view, 0, -1n)) + ">" + value(view[0]);
})());

emit("uint32-return", (function () {
    var view = new Uint32Array(new ArrayBuffer(4));
    view[0] = 0xffffffff;
    var old = Atomics.add(view, 0, 1);
    return typeof old + ":" + old + ">" + typeof Atomics.load(view, 0) +
        ":" + Atomics.load(view, 0);
})());

emit("store-return", (function () {
    var small = new Int8Array(new ArrayBuffer(1));
    var wide = Atomics.store(small, 0, 257.9);
    var storedWide = small[0];
    var medium = new Int16Array(new ArrayBuffer(2));
    var mediumWide = Atomics.store(medium, 0, 65535.9);
    var storedMediumWide = medium[0];
    var nan = Atomics.store(small, 0, NaN);
    var storedNan = small[0];
    var positiveInfinity = Atomics.store(small, 0, Infinity);
    var storedPositiveInfinity = small[0];
    var negativeInfinity = Atomics.store(small, 0, -Infinity);
    var storedNegativeInfinity = small[0];
    var negativeZero = Atomics.store(small, 0, -0);
    var big = new BigInt64Array(new ArrayBuffer(8));
    var input = (1n << 65n) + 3n;
    var bigReturn = Atomics.store(big, 0, input);
    return value(wide) + ">" + storedWide + "|" + value(mediumWide) +
        ">" + storedMediumWide + "|" + value(nan) + ">" + storedNan +
        "|" + value(positiveInfinity) + ">" + storedPositiveInfinity +
        "|" + value(negativeInfinity) + ">" + storedNegativeInfinity +
        "|" + value(negativeZero) + ">" + small[0] + "|" +
        value(bigReturn) + ">" + value(big[0]);
})());

emit("invalid-views", (function () {
    var detachedBuffer = new ArrayBuffer(4);
    var detached = new Int32Array(detachedBuffer);
    detachedBuffer.transfer();
    var candidates = [
        new Float32Array(1), new Float64Array(1), new Uint8ClampedArray(1),
        new DataView(new ArrayBuffer(4)), [], {}, detached
    ];
    return candidates.map(function (candidate) {
        return completion(function () { return Atomics.load(candidate, 0); });
    }).join(",");
})());

emit("invalid-order", (function () {
    var log = [];
    var index = { valueOf: function () { log.push("index"); return 0; } };
    var operand = { valueOf: function () { log.push("value"); return 1; } };
    var numberBigInt = completion(function () {
        return Atomics.add(new Int32Array(1), 0, 1n);
    });
    var bigintNumber = completion(function () {
        return Atomics.add(new BigInt64Array(1), 0, 1);
    });
    var invalid = completion(function () {
        return Atomics.add(new Float32Array(1), index, operand);
    });
    return invalid + ":" + log.join(">") + "|" + numberBigInt + "|" +
        bigintNumber;
})());

emit("detach-index", (function () {
    function run(name) {
        var buffer = new ArrayBuffer(4), view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.transfer(); return 0;
        } };
        var operand = { valueOf: function () { log.push("value"); return 1; } };
        var result = diagnostic(function () {
            if (name === "load") return Atomics.load(view, index);
            return Atomics[name](view, index, operand);
        });
        return name + ":" + result + ":" + log.join(">");
    }
    return run("load") + "|" + run("add") + "|" + run("store");
})());

emit("detach-value", (function () {
    function run(name) {
        var buffer = new ArrayBuffer(4), view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () { log.push("index"); return 0; } };
        var operand = { valueOf: function () {
            log.push("value"); buffer.transfer(); return 1;
        } };
        var result = diagnostic(function () {
            return Atomics[name](view, index, operand);
        });
        return name + ":" + result + ":" + log.join(">");
    }
    return run("add") + "|" + run("store");
})());

emit("detach-compare", (function () {
    function run(detachAt) {
        var buffer = new ArrayBuffer(4), view = new Int32Array(buffer), log = [];
        var expected = { valueOf: function () {
            log.push("expected");
            if (detachAt === "expected") buffer.transfer();
            return 0;
        } };
        var replacement = { valueOf: function () {
            log.push("replacement");
            if (detachAt === "replacement") buffer.transfer();
            return 1;
        } };
        var result = diagnostic(function () {
            return Atomics.compareExchange(view, 0, expected, replacement);
        });
        return detachAt + ":" + result + ":" + log.join(">");
    }
    function expectedThrows() {
        var view = new Int32Array(1), log = [];
        var expected = { valueOf: function () {
            log.push("expected"); throw new RangeError("sentinel");
        } };
        var replacement = { valueOf: function () {
            log.push("replacement"); return 1;
        } };
        var result = completion(function () {
            return Atomics.compareExchange(view, 0, expected, replacement);
        });
        return "expected-throws:" + result + ":" + log.join(">");
    }
    return run("expected") + "|" + run("replacement") + "|" +
        expectedThrows();
})());

emit("rab-revalidate", (function () {
    function grow() {
        var buffer = new ArrayBuffer(0, { maxByteLength: 4 });
        var view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.resize(4); return 0;
        } };
        return diagnostic(function () { return Atomics.load(view, index); }) +
            ":" + log.join(">") + ":" + view.length;
    }
    function shrink() {
        var buffer = new ArrayBuffer(4, { maxByteLength: 4 });
        var view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.resize(0); return 0;
        } };
        return diagnostic(function () { return Atomics.load(view, index); }) +
            ":" + log.join(">") + ":" + view.length;
    }
    function fixedOutOfBounds() {
        var buffer = new ArrayBuffer(8, { maxByteLength: 8 });
        var view = new Int32Array(buffer, 4, 1), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.resize(4); return 0;
        } };
        return diagnostic(function () { return Atomics.load(view, index); }) +
            ":" + log.join(">") + ":" + view.length;
    }
    function shrinkDuringValue() {
        var buffer = new ArrayBuffer(4, { maxByteLength: 4 });
        var view = new Int32Array(buffer), log = [];
        var operand = { valueOf: function () {
            log.push("value"); buffer.resize(0); return 1;
        } };
        return diagnostic(function () { return Atomics.add(view, 0, operand); }) +
            ":" + log.join(">") + ":" + view.length;
    }
    function fixedShrinkDuringValue() {
        var buffer = new ArrayBuffer(8, { maxByteLength: 8 });
        var view = new Int32Array(buffer, 4, 1), log = [];
        var operand = { valueOf: function () {
            log.push("value"); buffer.resize(4); return 1;
        } };
        return diagnostic(function () { return Atomics.add(view, 0, operand); }) +
            ":" + log.join(">") + ":" + view.length;
    }
    return grow() + "|" + shrink() + "|" + fixedOutOfBounds() + "|" +
        shrinkDuringValue() + "|" + fixedShrinkDuringValue();
})());

emit("notify", (function () {
    function ordinary(View) {
        var view = new View(new ArrayBuffer(View.BYTES_PER_ELEMENT)), log = [];
        var index = { valueOf: function () { log.push("index"); return 0; } };
        var count = { valueOf: function () { log.push("count"); return 2; } };
        return completion(function () { return Atomics.notify(view, index, count); }) +
            ":" + log.join(">");
    }
    var invalidLog = [];
    var invalid = completion(function () {
        return Atomics.notify(
            new Uint32Array(1),
            { valueOf: function () { invalidLog.push("index"); return 0; } },
            { valueOf: function () { invalidLog.push("count"); return 1; } }
        );
    });
    return ordinary(Int32Array) + "|" + ordinary(BigInt64Array) + "|" +
        invalid + ":" + invalidLog.join(">");
})());

emit("notify-old-length", (function () {
    function grow() {
        var buffer = new ArrayBuffer(0, { maxByteLength: 4 });
        var view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.resize(4); return 0;
        } };
        var count = { valueOf: function () { log.push("count"); return 1; } };
        return completion(function () { return Atomics.notify(view, index, count); }) +
            ":" + log.join(">");
    }
    function shrink() {
        var buffer = new ArrayBuffer(4, { maxByteLength: 4 });
        var view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.resize(0); return 0;
        } };
        var count = { valueOf: function () { log.push("count"); return 1; } };
        return completion(function () { return Atomics.notify(view, index, count); }) +
            ":" + log.join(">");
    }
    function detach() {
        var buffer = new ArrayBuffer(4), view = new Int32Array(buffer), log = [];
        var index = { valueOf: function () {
            log.push("index"); buffer.transfer(); return 0;
        } };
        var count = { valueOf: function () { log.push("count"); return 1; } };
        return completion(function () { return Atomics.notify(view, index, count); }) +
            ":" + log.join(">");
    }
    return grow() + "|" + shrink() + "|" + detach();
})());

emit("wait", (function () {
    function ordinary(View, expected) {
        var view = new View(new ArrayBuffer(View.BYTES_PER_ELEMENT)), log = [];
        var index = { valueOf: function () { log.push("index"); return 0; } };
        var valueArg = { valueOf: function () { log.push("value"); return expected; } };
        var timeout = { valueOf: function () { log.push("timeout"); return 0; } };
        return diagnostic(function () {
            return Atomics.wait(view, index, valueArg, timeout);
        }) + ":" + log.join(">");
    }
    return ordinary(Int32Array, 0) + "|" + ordinary(BigInt64Array, 0n) + "|" +
        diagnostic(function () { return Atomics.wait(new Uint32Array(1), 0, 0, 0); });
})());

emit("is-lock-free", (function () {
    var inputs = [undefined, null, false, true, -1, 0, 1, 2, 3, 4, 8, 16,
        1.9, Infinity, -Infinity, NaN, "4"];
    var observations = inputs.map(function (input) {
        return value(input) + ":" + completion(function () {
            return Atomics.isLockFree(input);
        });
    });
    var log = [];
    var objectResult = Atomics.isLockFree({ valueOf: function () {
        log.push("value"); return 8;
    } });
    observations.push("object:" + objectResult + ":" + log.join(">"));
    observations.push("bigint:" + completion(function () {
        return Atomics.isLockFree(1n);
    }));
    observations.push("symbol:" + completion(function () {
        return Atomics.isLockFree(Symbol());
    }));
    observations.push("wrap-positive:" + completion(function () {
        return Atomics.isLockFree(4294967297);
    }));
    observations.push("wrap-negative:" + completion(function () {
        return Atomics.isLockFree(-4294967295);
    }));
    observations.push("huge-positive:" + completion(function () {
        return Atomics.isLockFree(Number.MAX_VALUE);
    }));
    observations.push("huge-negative:" + completion(function () {
        return Atomics.isLockFree(-Number.MAX_VALUE);
    }));
    return observations.join("|");
})());

emit("pause", (function () {
    var accepted = [undefined, 0, -0, 42, -42, 42.0, Number.MAX_SAFE_INTEGER];
    var rejected = [0.5, NaN, Infinity, -Infinity, "1", null, true, 1n,
        new Number(1)];
    return completion(function () { return Atomics.pause(); }) + "|" +
        accepted.map(function (input) {
            return value(input) + ":" + completion(function () {
                return Atomics.pause(input);
            });
        }).join(",") + "|" + rejected.map(function (input) {
            return value(input) + ":" + completion(function () {
                return Atomics.pause(input);
            });
        }).join(",");
})());
