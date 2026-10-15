// case: global-graph / JSON and JSON.parse expose the pinned lazy-global shape and metadata
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
                var parseDescriptor = Object.getOwnPropertyDescriptor(JSON, "parse");
                var nameDescriptor = Object.getOwnPropertyDescriptor(JSON.parse, "name");
                var lengthDescriptor = Object.getOwnPropertyDescriptor(JSON.parse, "length");
                var tagDescriptor = Object.getOwnPropertyDescriptor(JSON, Symbol.toStringTag);
                return [
                    globalDescriptor.value === JSON,
                    bits(globalDescriptor),
                    Object.getPrototypeOf(JSON) === Object.prototype,
                    Object.isExtensible(JSON),
                    Object.prototype.toString.call(JSON),
                    Reflect.ownKeys(JSON).map(String).join(","),
                    parseDescriptor.value === JSON.parse,
                    bits(parseDescriptor),
                    JSON.parse.name,
                    JSON.parse.length,
                    bits(nameDescriptor),
                    bits(lengthDescriptor),
                    Object.getOwnPropertyNames(JSON.parse).join(","),
                    Object.getPrototypeOf(JSON.parse) === Function.prototype,
                    typeof JSON.parse,
                    isConstructor(JSON.parse),
                    tagDescriptor.value,
                    bits(tagDescriptor)
                ].join("|");
            })()
// case: strict-grammar / only JSON whitespace tokens strings and number grammar are accepted
            (function () {
                function status(text) {
                    try {
                        JSON.parse(text);
                        return "ok";
                    } catch (error) {
                        return error.name;
                    }
                }
                var invalid = [
                    "",
                    " ",
                    "[1,]",
                    '{"a":1,}',
                    "01",
                    "-01",
                    "+1",
                    ".1",
                    "1.",
                    "1e",
                    "1e+",
                    "0x10",
                    "NaN",
                    "Infinity",
                    "undefined",
                    "'x'",
                    "{a:1}",
                    "/* comment */1",
                    "[1 2]",
                    '{"a" 1}',
                    '{"a":}',
                    "true false",
                    String.fromCharCode(0xfeff) + "1",
                    String.fromCharCode(0x0b) + "1",
                    String.fromCharCode(0x0c) + "1",
                    '"' + String.fromCharCode(1) + '"',
                    '"\\u005"',
                    '"\\x41"'
                ];
                var results = [];
                for (var index = 0; index < invalid.length; index++) {
                    results.push(status(invalid[index]));
                }
                return [
                    JSON.parse("\t\r\n 42 ") === 42,
                    JSON.parse('"\\f"').charCodeAt(0),
                    JSON.parse('"' + String.fromCharCode(0x2028) + '"').charCodeAt(0),
                    results.join(",")
                ].join("|");
            })()
// case: primitives-utf16 / primitive coercions escapes surrogate pairs and lone surrogates preserve UTF-16 units
            (function () {
                function units(value) {
                    var result = [];
                    for (var index = 0; index < value.length; index++) {
                        result.push(("0000" + value.charCodeAt(index).toString(16)).slice(-4));
                    }
                    return value.length + ":" + result.join(",");
                }
                var missing;
                try { JSON.parse(); } catch (error) { missing = error.name; }
                var symbolError;
                try { JSON.parse(Symbol("text")); } catch (error) { symbolError = error.name; }
                return [
                    JSON.parse(null) === null,
                    JSON.parse(false) === false,
                    JSON.parse(true) === true,
                    Object.is(JSON.parse(0), 0),
                    JSON.parse(3.14),
                    JSON.parse('""').length,
                    units(JSON.parse('"A\\nB\\tC"')),
                    units(JSON.parse('"\\u0000\\/\\\\\\\""')),
                    units(JSON.parse('"\\ud83d\\ude00"')),
                    units(JSON.parse('"\\ud800"')),
                    units(JSON.parse('"\\udc00"')),
                    units(JSON.parse('"é"')),
                    units(Object.keys(JSON.parse('{"\\ud800":1}'))[0]),
                    missing,
                    symbolError
                ].join("|");
            })()
// case: objects-and-keys / duplicate keys overwrite in place and __proto__ is an ordinary own data property
            (function () {
                var value = JSON.parse(
                    '{"2":"two","01":"leading","4294967295":"not-index",' +
                    '"1":"one","4294967294":"max-index","x":1,"x":2,' +
                    '"__proto__":{"polluted":true},"__proto__":7,"a":3}'
                );
                var descriptor = Object.getOwnPropertyDescriptor(value, "__proto__");
                var array = JSON.parse('[1,2]');
                var lengthDescriptor = Object.getOwnPropertyDescriptor(array, "length");
                return [
                    Object.keys(value).join(","),
                    value.x,
                    value.__proto__,
                    Object.getPrototypeOf(value) === Object.prototype,
                    Object.prototype.hasOwnProperty.call(value, "__proto__"),
                    descriptor.value,
                    descriptor.writable,
                    descriptor.enumerable,
                    descriptor.configurable,
                    typeof Object.prototype.polluted,
                    Object.keys(array).join(","),
                    array.length,
                    lengthDescriptor.writable,
                    lengthDescriptor.enumerable,
                    lengthDescriptor.configurable
                ].join("|");
            })()
// case: number-edges / JSON number lexemes round through the pinned binary64 conversion
            (function () {
                function observation(text) {
                    var value = JSON.parse(text);
                    return text + "=" + String(value) + ":" +
                        Object.is(value, -0) + ":" + Number.isFinite(value);
                }
                return [
                    observation("-0"),
                    observation("0"),
                    observation("9007199254740991"),
                    observation("9007199254740992"),
                    observation("9007199254740993"),
                    observation("1.7976931348623157e308"),
                    observation("1.7976931348623159e308"),
                    observation("1e400"),
                    observation("-1e400"),
                    observation("5e-324"),
                    observation("4e-324"),
                    observation("1e-324"),
                    observation("2.2250738585072014e-308"),
                    observation("2.225073858507201e-308")
                ].join("|");
            })()
// case: tostring-order / text ToString completes before parsing and reviver classification preserves abrupt values
            (function () {
                var log = [];
                var text = {};
                text[Symbol.toPrimitive] = function (hint) {
                    log.push("primitive:" + hint + ":" + (this === text));
                    return '{"answer":42}';
                };
                var first = JSON.parse(text, function (key, value) {
                    log.push("reviver:" + key);
                    return value;
                });

                var fallback = {
                    toString: function () { log.push("toString"); return {}; },
                    valueOf: function () { log.push("valueOf"); return "false"; }
                };
                var second = JSON.parse(fallback);

                var sentinel = { marker: "sentinel" };
                var poison = {};
                Object.defineProperty(poison, Symbol.toPrimitive, {
                    get: function () { log.push("poison-get"); throw sentinel; }
                });
                var poisonResult;
                try {
                    JSON.parse(poison, function () { log.push("poison-reviver"); });
                } catch (error) {
                    poisonResult = error === sentinel;
                }

                var invalidCalls = 0;
                var invalidName;
                try {
                    JSON.parse("{", function () { invalidCalls++; });
                } catch (error) {
                    invalidName = error.name;
                }

                var fakeReviver = {};
                Object.defineProperty(fakeReviver, "call", {
                    get: function () { log.push("fake-call-get"); throw sentinel; }
                });
                var ignored = JSON.parse("1", fakeReviver);

                var symbolName;
                try { JSON.parse(Symbol()); } catch (error) { symbolName = error.name; }
                return [
                    first.answer,
                    second,
                    poisonResult,
                    invalidName,
                    invalidCalls,
                    ignored,
                    symbolName,
                    JSON.parse(new String("true")),
                    JSON.parse(1n),
                    log.join(",")
                ].join("|");
            })()
// case: reviver-postorder / reviver walks integer keys first and nested children before their parents with exact receivers
            (function () {
                var calls = [];
                var contexts = [];
                var wrapperOkay = false;
                var value = JSON.parse(
                    '{"p1":0,"p2":{"a":[1,{"b":2}]},"p1":3,"2":4,"1":5}',
                    function (key, current, context) {
                        var owner;
                        if (key === "") owner = "wrapper";
                        else if (Array.isArray(this)) owner = "array";
                        else if (Object.prototype.hasOwnProperty.call(this, "b")) owner = "nested";
                        else if (Object.prototype.hasOwnProperty.call(this, "a")) owner = "p2";
                        else owner = "root";
                        var source = Object.prototype.hasOwnProperty.call(context, "source")
                            ? context.source
                            : "-";
                        calls.push(key + ":" + owner + ":" + typeof current + ":" + source +
                            ":" + arguments.length + ":" +
                            (Object.getPrototypeOf(context) === Object.prototype));
                        contexts.push(context);
                        if (key === "") {
                            wrapperOkay = this[""] === current &&
                                Object.getPrototypeOf(this) === Object.prototype;
                        }
                        return current;
                    }
                );
                var uniqueContexts = true;
                for (var left = 0; left < contexts.length; left++) {
                    for (var right = left + 1; right < contexts.length; right++) {
                        if (contexts[left] === contexts[right]) uniqueContexts = false;
                    }
                }
                return [
                    calls.join(","),
                    wrapperOkay,
                    uniqueContexts,
                    value.p1,
                    value.p2.a[1].b
                ].join("|");
            })()
// case: reviver-mutation / deletion redefinition forward mutation and captured array length follow QuickJS internalization
            (function () {
                var calls = [];
                var value = JSON.parse(
                    '{"a":1,"b":2,"arr":[3,4],"tail":5}',
                    function (key, current, context) {
                        var source = Object.prototype.hasOwnProperty.call(context, "source")
                            ? context.source
                            : "-";
                        calls.push(key + ":" + String(current) + ":" + source);
                        if (key === "a") {
                            delete this.b;
                            this.added = 9;
                            return undefined;
                        }
                        if (key === "b") return 20;
                        if (key === "0" && Array.isArray(this)) {
                            delete this[1];
                            this[2] = 6;
                            return undefined;
                        }
                        if (key === "1" && Array.isArray(this)) return 40;
                        return current;
                    }
                );
                return [
                    calls.join(","),
                    Object.keys(value).join(","),
                    Object.prototype.hasOwnProperty.call(value, "a"),
                    value.b,
                    value.added,
                    value.arr.length,
                    Object.keys(value.arr).join(","),
                    0 in value.arr,
                    value.arr[1],
                    value.arr[2]
                ].join("|");
            })()
// case: reviver-prototype-and-descriptors / reviver Get sees prototypes while failed create and delete on non-configurable properties stay silent
            (function () {
                Object.prototype.__qjoJsonInherited = 30;
                Array.prototype[1] = 40;
                try {
                    var objectValue = JSON.parse(
                        '{"a":1,"__qjoJsonInherited":2}',
                        function (key, current, context) {
                            if (key === "a") delete this.__qjoJsonInherited;
                            return current;
                        }
                    );
                    var arrayValue = JSON.parse('[1,2]', function (key, current) {
                        if (key === "0") delete this[1];
                        return current;
                    });
                    var failedCreate = JSON.parse('{"a":1,"b":2}', function (key, current) {
                        if (key === "a") {
                            Object.defineProperty(this, "b", { configurable: false });
                        }
                        if (key === "b") return 22;
                        return current;
                    });
                    var failedDelete = JSON.parse('{"a":1,"b":2}', function (key, current) {
                        if (key === "a") {
                            Object.defineProperty(this, "b", { configurable: false });
                        }
                        if (key === "b") return undefined;
                        return current;
                    });
                    return [
                        objectValue.__qjoJsonInherited,
                        Object.prototype.hasOwnProperty.call(objectValue, "__qjoJsonInherited"),
                        arrayValue[1],
                        Object.prototype.hasOwnProperty.call(arrayValue, "1"),
                        failedCreate.b,
                        Object.getOwnPropertyDescriptor(failedCreate, "b").configurable,
                        failedDelete.b,
                        Object.prototype.hasOwnProperty.call(failedDelete, "b")
                    ].join("|");
                } finally {
                    delete Object.prototype.__qjoJsonInherited;
                    delete Array.prototype[1];
                }
            })()
// case: reviver-abrupt / reviver and forward getter abrupt completions preserve identity and stop later callbacks
            (function () {
                var first = { kind: "first" };
                var firstCalls = [];
                var firstCaught;
                try {
                    JSON.parse('{"a":1,"b":2}', function (key, current) {
                        firstCalls.push(key);
                        if (key === "a") throw first;
                        return current;
                    });
                } catch (error) {
                    firstCaught = error === first;
                }

                var second = { kind: "second" };
                var secondCalls = [];
                var secondCaught;
                try {
                    JSON.parse('{"a":1,"b":2}', function (key, current) {
                        secondCalls.push(key);
                        if (key === "a") {
                            Object.defineProperty(this, "b", {
                                configurable: true,
                                enumerable: true,
                                get: function () { throw second; }
                            });
                        }
                        return current;
                    });
                } catch (error) {
                    secondCaught = error === second;
                }
                return [
                    firstCaught,
                    firstCalls.join(","),
                    secondCaught,
                    secondCalls.join(",")
                ].join("|");
            })()
// case: context-source / the third reviver argument preserves exact primitive lexemes and empty object contexts
            (function () {
                var calls = [];
                var allDescriptors = true;
                var value = JSON.parse(
                    ' {"s":"\\u0041","n":1.2300e+2,"t":true,"f":false,"z":null,' +
                    '"a":[-0,5e-324],"o":{"x":42}} ',
                    function (key, current, context) {
                        var descriptor = Object.getOwnPropertyDescriptor(context, "source");
                        var source = descriptor === undefined ? "-" : descriptor.value;
                        if (descriptor !== undefined) {
                            allDescriptors = allDescriptors && descriptor.writable &&
                                descriptor.enumerable && descriptor.configurable;
                        }
                        calls.push(key + ":" + source + ":" +
                            Object.keys(context).join(",") + ":" +
                            (Object.getPrototypeOf(context) === Object.prototype));
                        return current;
                    }
                );
                var rootSource;
                JSON.parse("  42 \n", function (key, current, context) {
                    if (key === "") rootSource = context.source;
                    return current;
                });
                return [
                    calls.join("|"),
                    allDescriptors,
                    rootSource,
                    value.s,
                    value.n,
                    Object.is(value.a[0], -0),
                    value.a[1]
                ].join("~");
            })()
// case: context-source-duplicate-hash / the ninth object parse record switches duplicate source lookup from first to latest
            (function () {
                function xObservation(text) {
                    var seen = "unvisited";
                    var value = JSON.parse(text, function (key, current, context) {
                        if (key === "x") {
                            seen = Object.prototype.hasOwnProperty.call(context, "source")
                                ? context.source
                                : "-";
                        }
                        return current;
                    });
                    return seen + ":" + value.x + ":" + Object.keys(value).join(",");
                }
                return [
                    xObservation('{"x":2,"a":0,"b":0,"c":0,"d":0,"e":0,"f":0,"x":2.0}'),
                    xObservation('{"x":2,"a":0,"b":0,"c":0,"d":0,"e":0,"f":0,"g":0,"x":2.0}'),
                    xObservation('{"x":1,"a":0,"b":0,"c":0,"d":0,"e":0,"f":0,"x":2}'),
                    xObservation('{"x":1,"a":0,"b":0,"c":0,"d":0,"e":0,"f":0,"g":0,"x":2}')
                ].join("|");
            })()
