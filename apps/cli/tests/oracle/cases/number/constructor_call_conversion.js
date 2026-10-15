// mode: print
function numberText(value) {
    if (Number.isNaN(value)) return "NaN";
    if (Object.is(value, -0)) return "-0";
    return String(value);
}
function observe(thunk) {
    try { return numberText(thunk()); }
    catch (error) {
        if (error !== null && typeof error === "object")
            return "throw:" + error.name + ":" + error.message;
        return "throw:" + String(error);
    }
}

print("call=" + [
    Number(), Number(undefined), Number(null), Number(false), Number(true),
    Number(17), Number(3.5), Number(-0), Number(" 0x10 "), Number("1.25")
].map(numberText).join("|"));

print("bigint=" + [
    Number(9007199254740993n),
    Number(9007199254740995n),
    Number(18446744073709553664n),
    Number(18446744073709553665n),
    Number(1n << 1024n),
    Number(-(1n << 1024n))
].map(numberText).join("|"));

var exoticLog = "";
var exotic = {
    [Symbol.toPrimitive]: function(hint) {
        exoticLog += "exotic:" + hint + "|";
        return 41;
    }
};
var fallbackLog = "";
var fallback = {
    valueOf: function() { fallbackLog += "valueOf|"; return {}; },
    toString: function() { fallbackLog += "toString|"; return "42"; }
};
var throwing = {
    [Symbol.toPrimitive]: function() { throw 77; }
};
print("object=" + [
    numberText(Number(exotic)), exoticLog,
    numberText(Number(fallback)), fallbackLog,
    observe(function() { return Number(throwing); })
].join("|"));
print("symbol=" + observe(function() { return Number(Symbol("number")); }));
