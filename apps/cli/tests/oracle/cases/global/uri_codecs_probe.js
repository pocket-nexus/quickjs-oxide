// mode: print
function renderString(value) {
    var result = "";
    for (var index = 0; index < value.length; index++) {
        var unit = value.charCodeAt(index);
        if (unit >= 32 && unit <= 126 && unit !== 92 && unit !== 124) {
            result += value.charAt(index);
        } else {
            result += "\\u" + ("000" + unit.toString(16)).slice(-4);
        }
    }
    return "s:" + result;
}
function observe(thunk) {
    try {
        return renderString(String(thunk()));
    } catch (error) {
        if (error !== null && typeof error === "object")
            return "throw:" + error.name + ":" + error.message;
        return "throw:" + String(error);
    }
}
function flags(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}
function isConstructor(value) {
    try {
        Reflect.construct(function () {}, [], value);
        return true;
    } catch (_) {
        return false;
    }
}
function hex2(value) {
    return ("0" + value.toString(16)).slice(-2);
}
function unchangedAscii(fn) {
    var result = [];
    for (var code = 0; code < 128; code++) {
        var input = String.fromCharCode(code);
        if (fn(input) === input)
            result.push(hex2(code));
    }
    return result.join(",");
}
function preservedEscapes(fn) {
    var result = [];
    for (var code = 0; code < 128; code++) {
        var input = "%" + hex2(code).toUpperCase();
        if (fn(input) === input)
            result.push(hex2(code));
    }
    return result.join(",");
}

var names = [
    "decodeURI", "decodeURIComponent", "encodeURI",
    "encodeURIComponent", "escape", "unescape"
];
var implementedGlobalNames = [
    "parseInt", "parseFloat", "isNaN", "isFinite",
    "decodeURI", "decodeURIComponent", "encodeURI", "encodeURIComponent",
    "escape", "unescape", "Infinity", "NaN", "undefined", "Number", "Boolean"
];
print("global-order=" + Reflect.ownKeys(globalThis).filter(function (key) {
    return implementedGlobalNames.indexOf(key) >= 0;
}).map(String).join(","));
print("graph=" + names.map(function (name) {
    var fn = globalThis[name];
    return name + ":" + Reflect.ownKeys(fn).map(String).join(",") + ":" +
           flags(globalThis, name) + ":" + fn.length + ":" + fn.name + ":" +
           flags(fn, "length") + ":" + flags(fn, "name") + ":" +
           (Object.getPrototypeOf(fn) === Function.prototype) + ":" +
           isConstructor(fn);
}).join("|"));

var primitiveValues = [
    null, false, true, 0, -0, NaN, Infinity, -Infinity,
    9007199254740993n, "A B/%"
];
names.forEach(function (name) {
    var fn = globalThis[name];
    var results = [
        observe(function () { return fn(); }),
        observe(function () { return fn(undefined); })
    ];
    primitiveValues.forEach(function (value) {
        results.push(observe(function () { return fn(value); }));
    });
    results.push(observe(function () { return fn(Symbol("uri")); }));
    print(name + "-primitives=" + results.join("|"));
});

var conversionLog = "";
var extraHit = false;
var exotic = {};
Object.defineProperty(exotic, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) {
        conversionLog += hint + ",";
        return "A B/\u00e9";
    }
});
var extra = {};
Object.defineProperty(extra, Symbol.toPrimitive, {
    configurable: true,
    value: function () { extraHit = true; throw 99; }
});
var fallback = {
    toString: function () { conversionLog += "toString,"; return "A B"; },
    valueOf: function () { conversionLog += "valueOf,"; return 1; }
};
var invalid = {};
Object.defineProperty(invalid, Symbol.toPrimitive, {
    configurable: true,
    value: function () { return {}; }
});
var arbitraryThrow = {};
Object.defineProperty(arbitraryThrow, Symbol.toPrimitive, {
    configurable: true,
    value: function () { throw 71; }
});
var objectResults = names.map(function (name) {
    var fn = globalThis[name];
    conversionLog = "";
    var exoticResult = observe(function () {
        return fn.call(Symbol("this"), exotic, extra);
    });
    var exoticLog = conversionLog;
    conversionLog = "";
    var fallbackResult = observe(function () { return fn(fallback); });
    var fallbackLog = conversionLog;
    return name + ":" + exoticResult + ":" + exoticLog + ":" +
           fallbackResult + ":" + fallbackLog + ":" +
           observe(function () { return fn(invalid); }) + ":" +
           observe(function () { return fn(arbitraryThrow); });
});
print("objects=" + objectResults.join("|") + ":" + extraHit);

var priorityLog = "";
var loneSurrogateResult = {};
Object.defineProperty(loneSurrogateResult, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) {
        priorityLog += "encode:" + hint + ",";
        return "\ud800";
    }
});
var malformedResult = {};
Object.defineProperty(malformedResult, Symbol.toPrimitive, {
    configurable: true,
    value: function (hint) {
        priorityLog += "decode:" + hint + ",";
        return "%";
    }
});
var thrownBeforeCodec = {};
Object.defineProperty(thrownBeforeCodec, Symbol.toPrimitive, {
    configurable: true,
    value: function () {
        priorityLog += "throw,";
        throw 73;
    }
});
print("error-priority=" + [
    observe(function () { return encodeURI(loneSurrogateResult); }),
    observe(function () { return decodeURIComponent(malformedResult); }),
    observe(function () { return encodeURIComponent(thrownBeforeCodec); }),
    priorityLog
].join("|"));

print("encodeURI-ascii=" + unchangedAscii(encodeURI));
print("encodeURIComponent-ascii=" + unchangedAscii(encodeURIComponent));
print("escape-ascii=" + unchangedAscii(escape));
print("decodeURI-preserved=" + preservedEscapes(decodeURI));
print("decodeURIComponent-preserved=" + preservedEscapes(decodeURIComponent));

var unicodeInputs = [
    "\u00e9", "\u4e2d", "\ud83d\ude00", "A \u00e9/\u4e2d?\ud83d\ude00#",
    "\u0000\u007f\u0080\u07ff\u0800\uffff\ud7ff\ue000\udbff\udfff"
];
["encodeURI", "encodeURIComponent"].forEach(function (name) {
    var fn = globalThis[name];
    print(name + "-unicode=" + unicodeInputs.map(function (input) {
        return observe(function () { return fn(input); });
    }).join("|"));
});
var surrogateInputs = [
    "\ud800", "\udfff", "A\ud800B", "\ud800\ud800", "\udc00\ud800"
];
["encodeURI", "encodeURIComponent"].forEach(function (name) {
    var fn = globalThis[name];
    print(name + "-surrogates=" + surrogateInputs.map(function (input) {
        return observe(function () { return fn(input); });
    }).join("|"));
});

var validEncodedInputs = [
    "%00%7F%C2%80%DF%BF%E0%A0%80%EF%BF%BF%F0%90%80%80%F4%8F%BF%BF",
    "%C3%A9%E4%B8%AD%F0%9F%98%80",
    "%23%24%26%2B%2C%2F%3A%3B%3D%3F%40",
    "%2f%3f%23%41",
    "plain-\u00e9-\ud83d\ude00"
];
["decodeURI", "decodeURIComponent"].forEach(function (name) {
    var fn = globalThis[name];
    print(name + "-valid=" + validEncodedInputs.map(function (input) {
        return observe(function () { return fn(input); });
    }).join("|"));
});
var malformedEncodedInputs = [
    "%", "%0", "%GG", "%C2", "%C2%20", "%80", "%C0%80", "%E0%80%80",
    "%ED%A0%80", "%F0%80%80%80", "%F4%90%80%80", "%F5%80%80%80",
    "%E2%28%A1", "%F0%9F%98", "%F0%9F%98%GG", "%41%"
];
["decodeURI", "decodeURIComponent"].forEach(function (name) {
    var fn = globalThis[name];
    print(name + "-malformed=" + malformedEncodedInputs.map(function (input) {
        return observe(function () { return fn(input); });
    }).join("|"));
});

var legacyInputs = [
    "", "AZaz09@*_+-./", " !#$%&\"'(),:;<=>?[\\]^`{|}~",
    "\u00e9", "\u0100", "\u4e2d", "\ud83d\ude00", "\ud800", "\udfff",
    "A\u0000\u00ff\u0100\uffff"
];
print("escape-cases=" + legacyInputs.map(function (input) {
    return observe(function () { return escape(input); });
}).join("|"));
var unescapeInputs = [
    "", "%41", "%4a%4A", "%u0041", "%u00e9", "%u4E2d",
    "%uD83D%uDE00", "%uD800", "%uDFFF", "%C3%A9", "%00%7f%FF",
    "%", "%0", "%GG", "%u", "%u0", "%u000", "%uGGGG", "%U0041",
    "%%41", "%2520", "%u{41}", "%u0041tail", "%41%u0042"
];
print("unescape-cases=" + unescapeInputs.map(function (input) {
    return observe(function () { return unescape(input); });
}).join("|"));
print("legacy-roundtrip=" + legacyInputs.map(function (input) {
    return observe(function () { return unescape(escape(input)); });
}).join("|"));
