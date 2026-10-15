// case: global-graph / JSON.stringify exposes the pinned lazy-global shape and metadata
            (function () {
                function bit(value) { return value ? 1 : 0; }
                function bits(descriptor) {
                    return "" + bit(descriptor.writable) +
                        bit(descriptor.enumerable) + bit(descriptor.configurable);
                }
                function isConstructor(value) {
                    try {
                        Reflect.construct(function () {}, [], value);
                        return true;
                    } catch (_) {
                        return false;
                    }
                }
                var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "JSON");
                var stringifyDescriptor = Object.getOwnPropertyDescriptor(JSON, "stringify");
                var nameDescriptor = Object.getOwnPropertyDescriptor(JSON.stringify, "name");
                var lengthDescriptor = Object.getOwnPropertyDescriptor(JSON.stringify, "length");
                var tagDescriptor = Object.getOwnPropertyDescriptor(JSON, Symbol.toStringTag);
                return [
                    globalDescriptor.value === JSON,
                    bits(globalDescriptor),
                    Object.getPrototypeOf(JSON) === Object.prototype,
                    Object.isExtensible(JSON),
                    Object.prototype.toString.call(JSON),
                    Reflect.ownKeys(JSON).map(String).join(","),
                    stringifyDescriptor.value === JSON.stringify,
                    bits(stringifyDescriptor),
                    JSON.stringify.name,
                    JSON.stringify.length,
                    bits(nameDescriptor),
                    bits(lengthDescriptor),
                    Object.getOwnPropertyNames(JSON.stringify).join(","),
                    Object.getPrototypeOf(JSON.stringify) === Function.prototype,
                    typeof JSON.stringify,
                    isConstructor(JSON.stringify),
                    tagDescriptor.value,
                    bits(tagDescriptor)
                ].join("|");
            })()
// case: root-primitives / root values wrappers and unrepresentable values follow the JSON type table
            (function () {
                function observation(value) {
                    var result = JSON.stringify(value);
                    return typeof result + ":" + String(result);
                }
                var fn = function () {};
                return [
                    observation(undefined),
                    observation(null),
                    observation(true),
                    observation(false),
                    observation("A\nB"),
                    observation(-0),
                    observation(1.25),
                    observation(NaN),
                    observation(Infinity),
                    observation(-Infinity),
                    observation(fn),
                    observation(Symbol("x")),
                    observation(new Boolean(false)),
                    observation(new Number(3)),
                    observation(new String("xy")),
                    JSON.stringify({ u: undefined, f: fn, s: Symbol("x"), n: null }),
                    JSON.stringify([undefined, fn, Symbol("x"), null])
                ].join("|");
            })()
// case: arrays-objects-and-keys / arrays use length while objects use enumerable own string keys in pinned order
            (function () {
                var proto = { inherited: "ignored" };
                var object = Object.create(proto);
                object["10"] = "ten";
                object["2"] = "two";
                object.b = 1;
                object.a = 2;
                Object.defineProperty(object, "hidden", { value: 3, enumerable: false });
                Object.defineProperty(object, "__proto__", {
                    value: 4, writable: true, enumerable: true, configurable: true
                });
                object[Symbol("symbol-key")] = 5;

                var array = [];
                array[2] = "x";
                array.extra = "ignored";
                Object.defineProperty(array, "hidden", { value: 6, enumerable: false });
                return [
                    JSON.stringify(object),
                    JSON.stringify(array),
                    JSON.stringify([, 1, ,]),
                    JSON.stringify(Object.create(null)),
                    JSON.stringify({ value: Symbol("ignored"), keep: 1 })
                ].join("|");
            })()
// case: quoted-utf16 / string quoting escapes controls and lone surrogates while preserving scalar pairs
            (function () {
                function units(value) {
                    var result = [];
                    for (var index = 0; index < value.length; index++) {
                        result.push(("0000" + value.charCodeAt(index).toString(16)).slice(-4));
                    }
                    return value.length + ":" + result.join(",");
                }
                var controls = "\"\\/\b\f\n\r\t" + String.fromCharCode(0, 1, 31);
                var pair = String.fromCharCode(0xd83d, 0xde00);
                var high = String.fromCharCode(0xd800);
                var low = String.fromCharCode(0xdc00);
                var separators = String.fromCharCode(0x2028, 0x2029);
                var keyed = {};
                keyed[high] = low;
                return [
                    units(JSON.stringify(controls)),
                    units(JSON.stringify(pair)),
                    units(JSON.stringify(high)),
                    units(JSON.stringify(low)),
                    units(JSON.stringify(separators)),
                    units(JSON.stringify(keyed))
                ].join("|");
            })()
// case: number-formatting / finite binary64 values use QuickJS number formatting and non-finite values become null
            (function () {
                return [
                    JSON.stringify(-0),
                    JSON.stringify(0),
                    JSON.stringify(0.000001),
                    JSON.stringify(0.0000001),
                    JSON.stringify(100000000000000000000),
                    JSON.stringify(1000000000000000000000),
                    JSON.stringify(9007199254740991),
                    JSON.stringify(9007199254740993),
                    JSON.stringify(1.7976931348623157e308),
                    JSON.stringify(5e-324),
                    JSON.stringify(NaN),
                    JSON.stringify(Infinity),
                    JSON.stringify(-Infinity),
                    JSON.stringify([NaN, Infinity, -Infinity, -0])
                ].join("|");
            })()
// case: tojson-and-replacer / toJSON precedes the replacer and callbacks receive exact keys values and holders
            (function () {
                var log = [];
                var replacement = { y: 2 };
                var nested = { x: 1 };
                Object.defineProperty(nested, "toJSON", {
                    value: function (key) {
                        log.push("toJSON:nested:" + key + ":" + (this === nested));
                        return replacement;
                    }
                });
                var value = { a: nested, b: 3 };
                Object.defineProperty(value, "toJSON", {
                    value: function (key) {
                        log.push("toJSON:root:" + key + ":" + (this === value));
                        return this;
                    }
                });
                var rootHolder;
                function replacer(key, current) {
                    var owner = key === "" ? "wrapper" :
                        this === value ? "root" :
                        this === replacement ? "replacement" : "other";
                    log.push("replacer:" + key + ":" + owner + ":" + typeof current);
                    if (key === "") rootHolder = this;
                    if (key === "b") return undefined;
                    return current;
                }
                var result = JSON.stringify(value, replacer);
                return [
                    result,
                    log.join(","),
                    rootHolder[""] === value,
                    Object.getPrototypeOf(rootHolder) === Object.prototype,
                    Object.keys(rootHolder).join(",")
                ].join("|");
            })()
// case: replacer-array / replacer arrays snapshot length use Get coerce wrappers and deduplicate first occurrence
            (function () {
                var log = [];
                var boxed = new String("a");
                boxed.toString = function () { log.push("boxed"); return "a"; };
                var keys = ["b", "unused", 2, "b", , new Number(3), {}, Symbol("s"), boxed];
                Object.defineProperty(keys, "0", {
                    configurable: true,
                    enumerable: true,
                    get: function () {
                        log.push("get0");
                        keys[1] = "c";
                        keys.push("late");
                        return "b";
                    }
                });
                Object.defineProperty(keys, "2", {
                    configurable: true,
                    enumerable: true,
                    get: function () { log.push("get2"); return 2; }
                });
                Array.prototype[4] = "a";
                try {
                    var value = {
                        "2": "two", "3": "three", a: "A", b: "B", c: "C", late: "L"
                    };
                    return [
                        JSON.stringify(value, keys),
                        JSON.stringify([value], ["b"]),
                        log.join(","),
                        keys.length
                    ].join("|");
                } finally {
                    delete Array.prototype[4];
                }
            })()
// case: space-and-order / gap coercion follows replacer normalization and clamps or truncates to ten UTF-16 units
            (function () {
                function visible(value) {
                    var result = "";
                    for (var index = 0; index < value.length; index++) {
                        var unit = value.charCodeAt(index);
                        result += unit === 10 ? "\\n" : String.fromCharCode(unit);
                    }
                    return result;
                }
                function units(value) {
                    var result = [];
                    for (var index = 0; index < value.length; index++) {
                        result.push(("0000" + value.charCodeAt(index).toString(16)).slice(-4));
                    }
                    return value.length + ":" + result.join(",");
                }
                var log = [];
                var propertyList = ["a", "b"];
                Object.defineProperty(propertyList, "0", {
                    configurable: true,
                    get: function () { log.push("replacer-get"); return "a"; }
                });
                var space = new Number(2);
                space.valueOf = function () { log.push("space-valueOf"); return 4; };
                var value = { a: 1, b: { c: 2 } };
                var surrogateGap = "123456789" + String.fromCharCode(0xd800) + "ignored";
                return [
                    visible(JSON.stringify(value, propertyList, space)),
                    visible(JSON.stringify(value, null, 99)),
                    visible(JSON.stringify(value, null, "abcdefghijk")),
                    visible(JSON.stringify(value, null, -1)),
                    units(JSON.stringify({ a: 1 }, null, surrogateGap)),
                    log.join(",")
                ].join("|");
            })()
// case: cycles-and-shared-objects / only ancestor cycles throw while shared acyclic objects serialize at each path
            (function () {
                function thrown(value, replacer) {
                    try {
                        JSON.stringify(value, replacer);
                        return "none";
                    } catch (error) {
                        return error.name + ":" + error.message;
                    }
                }
                var shared = { x: 1 };
                var objectCycle = {};
                objectCycle.self = objectCycle;
                var arrayCycle = [];
                arrayCycle[0] = arrayCycle;
                var rescued = {};
                rescued.self = rescued;
                Object.defineProperty(rescued, "toJSON", {
                    value: function () { return 7; }
                });
                return [
                    JSON.stringify({ a: shared, b: shared }),
                    JSON.stringify([shared, shared]),
                    thrown(objectCycle),
                    thrown(arrayCycle),
                    JSON.stringify(rescued),
                    JSON.stringify(objectCycle, function (key, value) {
                        return key === "self" ? undefined : value;
                    })
                ].join("|");
            })()
// case: deep-iterative-traversal / acyclic traversal passes the historical 256-level cutoff within the portable QuickJS C-stack budget
            (function () {
                function serializedLength(depth) {
                    var value = 0;
                    for (var index = 0; index < depth; index++) value = [value];
                    return JSON.stringify(value).length;
                }
                return serializedLength(257) + "|" + serializedLength(512);
            })()
// case: bigint / BigInt throws after transformation but toJSON and replacers can rescue it
            (function () {
                function thrown(value) {
                    try {
                        JSON.stringify(value);
                        return "none";
                    } catch (error) {
                        return error.name + ":" + error.message;
                    }
                }
                var direct = thrown(1n);
                var member = thrown({ x: 1n });
                var element = thrown([1n]);
                var log = [];
                BigInt.prototype.toJSON = function (key) {
                    log.push(key + ":" + typeof this + ":" + this.valueOf());
                    return String(this.valueOf()) + "n";
                };
                try {
                    return [
                        direct,
                        member,
                        element,
                        JSON.stringify(2n),
                        JSON.stringify({ x: 3n }),
                        JSON.stringify([4n]),
                        JSON.stringify({ x: 5n }, function (key, value) {
                            return typeof value === "bigint" ? Number(value) : value;
                        }),
                        log.join(",")
                    ].join("|");
                } finally {
                    delete BigInt.prototype.toJSON;
                }
            })()
// case: mutation-snapshots / object keys and array length snapshot while later Gets observe deletion mutation and prototypes
            (function () {
                var object = {};
                Object.defineProperty(object, "a", {
                    configurable: true,
                    enumerable: true,
                    get: function () {
                        delete object.b;
                        object.c = 30;
                        Object.defineProperty(object, "c", { enumerable: false });
                        object.d = 4;
                        return 1;
                    }
                });
                object.b = 2;
                object.c = 3;
                var array = [0, 1, 2];
                Object.defineProperty(array, "0", {
                    configurable: true,
                    enumerable: true,
                    get: function () {
                        delete array[1];
                        array[2] = 20;
                        array.push(3);
                        return 0;
                    }
                });
                Array.prototype[1] = 11;
                try {
                    return [
                        JSON.stringify(object),
                        Object.keys(object).join(","),
                        JSON.stringify(array),
                        array.length,
                        Object.keys(array).join(",")
                    ].join("|");
                } finally {
                    delete Array.prototype[1];
                }
            })()
// case: abrupt-order / property-list and gap work precede traversal and abrupt values preserve identity
            (function () {
                var sentinel = { marker: "sentinel" };
                var log = [];
                var propertyList = ["a"];
                Object.defineProperty(propertyList, "0", {
                    configurable: true,
                    get: function () { log.push("property-list"); return "a"; }
                });
                var gap = new String(" ");
                gap.toString = function () { log.push("gap"); throw sentinel; };
                var value = { a: 1 };
                Object.defineProperty(value, "toJSON", {
                    get: function () { log.push("toJSON-get"); return undefined; }
                });
                var gapCaught = false;
                try { JSON.stringify(value, propertyList, gap); }
                catch (error) { gapCaught = error === sentinel; }

                var getterCalls = [];
                var getterValue = {};
                Object.defineProperty(getterValue, "a", {
                    enumerable: true,
                    get: function () { getterCalls.push("a"); throw sentinel; }
                });
                getterValue.b = 2;
                var getterCaught = false;
                try { JSON.stringify(getterValue); }
                catch (error) { getterCaught = error === sentinel; }

                var replacerCalls = [];
                var replacerCaught = false;
                try {
                    JSON.stringify({ a: 1, b: 2 }, function (key, current) {
                        replacerCalls.push(key);
                        if (key === "a") throw sentinel;
                        return current;
                    });
                } catch (error) {
                    replacerCaught = error === sentinel;
                }
                return [
                    gapCaught,
                    log.join(","),
                    getterCaught,
                    getterCalls.join(","),
                    replacerCaught,
                    replacerCalls.join(",")
                ].join("|");
            })()
