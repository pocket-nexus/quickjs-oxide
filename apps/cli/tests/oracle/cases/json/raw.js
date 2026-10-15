// CASES
// case: global-graph / Raw JSON methods expose the pinned JSON table and function metadata
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
                function method(name) {
                    var value = JSON[name];
                    var propertyDescriptor = Object.getOwnPropertyDescriptor(JSON, name);
                    var nameDescriptor = Object.getOwnPropertyDescriptor(value, "name");
                    var lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
                    return [
                        name,
                        propertyDescriptor.value === value,
                        bits(propertyDescriptor),
                        value.name,
                        value.length,
                        bits(nameDescriptor),
                        bits(lengthDescriptor),
                        Object.getOwnPropertyNames(value).join(","),
                        Object.getPrototypeOf(value) === Function.prototype,
                        typeof value,
                        isConstructor(value)
                    ].join(":");
                }
                return [
                    Reflect.ownKeys(JSON).map(String).join(","),
                    method("isRawJSON"),
                    method("rawJSON")
                ].join("|");
            })()
// case: returned-object / rawJSON returns a null prototype frozen branded ordinary object
            (function () {
                function bit(value) { return value ? 1 : 0; }
                function bits(descriptor) {
                    return "" + bit(descriptor.writable) +
                        bit(descriptor.enumerable) + bit(descriptor.configurable);
                }
                var value = JSON.rawJSON("1");
                var descriptor = Object.getOwnPropertyDescriptor(value, "rawJSON");
                return [
                    Object.getPrototypeOf(value) === null,
                    Object.prototype.toString.call(value),
                    Object.getOwnPropertyNames(value).join(","),
                    Object.getOwnPropertySymbols(value).length,
                    Object.keys(value).join(","),
                    descriptor.value,
                    bits(descriptor),
                    Object.isExtensible(value),
                    Object.isSealed(value),
                    Object.isFrozen(value),
                    JSON.isRawJSON(value),
                    value instanceof Object
                ].join("|");
            })()
// case: tostring-order / ToString uses string hint and propagates abrupt completion before validation
            (function () {
                var log = [];
                var exotic = {};
                exotic[Symbol.toPrimitive] = function (hint) {
                    log.push("p:" + hint);
                    return "1.20e+2";
                };
                var exoticText = JSON.rawJSON(exotic).rawJSON;

                var ordinary = {
                    toString: function () { log.push("t"); return {}; },
                    valueOf: function () { log.push("v"); return "false"; }
                };
                var ordinaryText = JSON.rawJSON(ordinary).rawJSON;

                var sentinel = {};
                var abrupt = {};
                abrupt[Symbol.toPrimitive] = function () {
                    log.push("a");
                    throw sentinel;
                };
                var abruptCaught = false;
                try { JSON.rawJSON(abrupt); }
                catch (error) { abruptCaught = error === sentinel; }

                var invalidError;
                try {
                    JSON.rawJSON({
                        toString: function () { log.push("i"); return "[]"; }
                    });
                } catch (error) {
                    invalidError = error.name + ":" + error.message;
                }

                var symbolError;
                try { JSON.rawJSON(Symbol("text")); }
                catch (error) { symbolError = error.name + ":" + error.message; }

                var missingError;
                try { JSON.rawJSON(); }
                catch (error) { missingError = error.name + ":" + error.message; }

                return [
                    exoticText,
                    ordinaryText,
                    abruptCaught,
                    invalidError,
                    symbolError,
                    missingError,
                    log.join(",")
                ].join("|");
            })()
// case: accepted-source / valid primitive texts retain exact spelling while non-string inputs canonicalize
            (function () {
                function units(value) {
                    var result = [];
                    for (var index = 0; index < value.length; index++) {
                        result.push(("0000" + value.charCodeAt(index).toString(16)).slice(-4));
                    }
                    return value.length + ":" + result.join(",");
                }
                var texts = [
                    "null",
                    "true",
                    "false",
                    "0",
                    "-0",
                    "1.2300",
                    "1e+9",
                    '"foo"',
                    '"\\u0061"',
                    '"a b"'
                ];
                var exact = true;
                var serialized = [];
                for (var index = 0; index < texts.length; index++) {
                    var raw = JSON.rawJSON(texts[index]);
                    exact = exact && raw.rawJSON === texts[index];
                    serialized.push(JSON.stringify(raw));
                }

                var literalLoneSurrogate = '"' + String.fromCharCode(0xd800) + '"';
                var lone = JSON.rawJSON(literalLoneSurrogate);
                return [
                    exact,
                    serialized.join(","),
                    JSON.rawJSON(-0).rawJSON,
                    JSON.rawJSON(1.1e1).rawJSON,
                    JSON.rawJSON(1.1e-1).rawJSON,
                    JSON.rawJSON(123456789012345678901234567890n).rawJSON,
                    units(lone.rawJSON),
                    units(JSON.stringify(lone))
                ].join("|");
            })()
// case: rejected-source / empty aggregate whitespace and malformed fragments use the fixed SyntaxError
            (function () {
                var invalidTexts = [
                    "",
                    " ",
                    "\t1",
                    "1\n",
                    "[]",
                    "{}",
                    "[1]",
                    '{"x":1}',
                    "01",
                    "-01",
                    "+1",
                    ".1",
                    "1.",
                    "1e",
                    "1e+",
                    "NaN",
                    "Infinity",
                    "undefined",
                    "True",
                    "FALSE",
                    "nul",
                    '"unterminated',
                    "'single'",
                    "/* comment */1",
                    String.fromCharCode(0xfeff) + "1",
                    "1" + String.fromCharCode(0x0b)
                ];
                var failures = [];
                function requireFixedSyntax(input, label) {
                    try {
                        JSON.rawJSON(input);
                        failures.push(label + ":accepted");
                    } catch (error) {
                        if (error.name !== "SyntaxError" ||
                            error.message !== "invalid rawJSON string") {
                            failures.push(label + ":" + error.name + ":" + error.message);
                        }
                    }
                }
                for (var index = 0; index < invalidTexts.length; index++) {
                    requireFixedSyntax(invalidTexts[index], "text" + index);
                }
                requireFixedSyntax(undefined, "undefined");
                requireFixedSyntax({}, "object");
                requireFixedSyntax([], "array");

                var symbolError;
                try { JSON.rawJSON(Symbol("raw")); }
                catch (error) { symbolError = error.name + ":" + error.message; }
                return failures.length + ":" + failures.join(",") + "|" + symbolError;
            })()
// case: unforgeable-brand / only direct RawJSON instances carry the trap-free internal brand
            (function () {
                var raw = JSON.rawJSON("7");
                var forged = Object.create(null);
                Object.defineProperty(forged, "rawJSON", {
                    value: "7",
                    writable: false,
                    enumerable: true,
                    configurable: false
                });
                Object.preventExtensions(forged);
                var inherited = Object.create(raw);
                var getterCalls = 0;
                var getterObject = {};
                Object.defineProperty(getterObject, "rawJSON", {
                    get: function () { getterCalls++; throw 71; },
                    enumerable: true
                });
                return [
                    JSON.isRawJSON(raw),
                    JSON.isRawJSON(forged),
                    JSON.isRawJSON(inherited),
                    JSON.isRawJSON({ rawJSON: "7" }),
                    JSON.isRawJSON(getterObject),
                    getterCalls,
                    JSON.isRawJSON(undefined),
                    JSON.isRawJSON(null),
                    JSON.isRawJSON(7),
                    JSON.isRawJSON("7"),
                    JSON.isRawJSON(Symbol("7")),
                    JSON.isRawJSON([]),
                    JSON.isRawJSON.call(123, raw),
                    JSON.isRawJSON()
                ].join("|");
            })()
// case: parse-isolation / rawJSON invokes the internal parser without observing a patched JSON.parse
            (function () {
                var rawJSON = JSON.rawJSON;
                var calls = 0;
                var sentinel = {};
                Object.defineProperty(JSON, "parse", {
                    configurable: true,
                    get: function () { calls++; throw sentinel; }
                });
                var valid = rawJSON("42");
                var invalidError;
                try { rawJSON("[]"); }
                catch (error) { invalidError = error.name + ":" + error.message; }
                return [
                    valid.rawJSON,
                    JSON.isRawJSON(valid),
                    calls,
                    invalidError
                ].join("|");
            })()
// case: stringify-splicing / toJSON and replacer precede raw splicing at every structural position
            (function () {
                var raw42 = JSON.rawJSON("42");
                var topLog = [];
                var top = JSON.stringify(raw42, function (key, value) {
                    topLog.push(key + ":" + JSON.isRawJSON(value));
                    return value;
                });
                var replacedAway = JSON.stringify(raw42, function (key, value) {
                    return key === "" ? 7 : value;
                });

                var createLog = [];
                var created = JSON.stringify({ x: 1, y: 2 }, function (key, value) {
                    createLog.push(key + ":" + typeof value);
                    if (key === "x") return JSON.rawJSON("2.50");
                    if (key === "y") return undefined;
                    return value;
                });

                var order = [];
                var source = {
                    toJSON: function (key) {
                        order.push("toJSON:" + key);
                        return JSON.rawJSON("3");
                    }
                };
                var fromToJSON = JSON.stringify(source, function (key, value) {
                    order.push("replacer:" + key + ":" + JSON.isRawJSON(value));
                    return value;
                });

                var bigint = JSON.stringify(
                    { tooBig: 9007199254740993n },
                    function (key, value) {
                        return typeof value === "bigint" ? JSON.rawJSON(value) : value;
                    }
                );
                var pretty = JSON.stringify({
                    a: [raw42],
                    b: JSON.rawJSON('"x"')
                }, null, 2).split("\n").join("\\n");

                return [
                    top,
                    topLog.join(","),
                    replacedAway,
                    created,
                    createLog.join(","),
                    fromToJSON,
                    order.join(","),
                    bigint,
                    pretty
                ].join("|");
            })()
// ORACLE_ONLY_CASES
// case: proxy-brand / proxy wrappers are never branded and isRawJSON invokes no proxy trap
        (function () {
            var raw = JSON.rawJSON("1");
            var trapCalls = 0;
            var trapped = new Proxy(raw, {
                get: function () { trapCalls++; throw 71; },
                getPrototypeOf: function () { trapCalls++; throw 72; },
                ownKeys: function () { trapCalls++; throw 73; }
            });
            var trappedBrand = JSON.isRawJSON(trapped);
            var revocable = Proxy.revocable(raw, {});
            revocable.revoke();
            var revokedBrand = JSON.isRawJSON(revocable.proxy);
            return [
                trappedBrand,
                trapCalls,
                revokedBrand,
                JSON.stringify(new Proxy(raw, {}))
            ].join("|");
        })()
