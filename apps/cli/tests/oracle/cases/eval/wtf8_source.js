// Lone UTF-16 surrogates built at run time cross direct/indirect eval (the
// JS_ToCStringLen2 WTF-8 boundary). Sources stay ASCII; results are UTF-16 hex.
function unitHex(value) {
    value = String(value);
    var output = "";
    for (var index = 0; index < value.length; index++)
        output += ("0000" + value.charCodeAt(index).toString(16)).slice(-4);
    return output;
}
var high = String.fromCharCode(0xd800);
var low = String.fromCharCode(0xdfff);
var privateUse = String.fromCharCode(0xe000);
var quote = String.fromCharCode(0x27);
var slash = String.fromCharCode(0x2f);
var backslash = String.fromCharCode(0x5c);
function run(body) {
    try {
        return body();
    } catch (error) {
        if (error !== null && typeof error === "object")
            return "throw|object|" + unitHex(error.name) + "|" + unitHex(error.message);
        return "throw|" + typeof error + "|" + unitHex(error);
    }
}
// case: string literal / sloppy direct eval preserves a raw lone high surrogate
run(function () {
    return unitHex(eval(quote + high + quote));
})
// case: string literal / strict direct eval preserves a raw lone low surrogate
run(function () {
    "use strict"; return unitHex(eval(quote + low + quote));
})
// case: string literal / indirect eval preserves both kinds of lone surrogate independently
run(function () {
    var first = (0, eval)(quote + high + quote);
    var second = (0, eval)(quote + low + quote);
    return unitHex(first) + "|" + unitHex(second);
})
// case: template literal / direct eval preserves a lone high surrogate in template text
run(function () {
    return unitHex(eval("`A" + high + "B`"));
})
// case: template literal / indirect eval preserves a lone low surrogate in template text
run(function () {
    return unitHex((0, eval)("`C" + low + "D`"));
})
// case: carrier collision / a real private-use code point stays distinct from the surrogate carrier
run(function () {
    return unitHex(eval(quote + privateUse + high + quote));
})
// case: regexp literal / direct eval preserves and matches a raw lone high surrogate atom
run(function () {
    var pattern = eval(slash + high + slash);
    return unitHex(pattern.source) + "|" + pattern.test(high) + "|" + pattern.test(privateUse);
})
// case: regexp literal / indirect eval preserves a raw lone low surrogate in a Unicode class
run(function () {
    var pattern = (0, eval)(slash + "[" + low + "]" + slash + "u");
    return unitHex(pattern.source) + "|" + pattern.test(low) + "|" + pattern.test(high);
})
// case: regexp literal / a backslash followed by a raw lone high surrogate remains an identity escape
run(function () {
    var pattern = eval(slash + backslash + high + slash);
    return unitHex(pattern.source) + "|" + pattern.test(high);
})
// case: comments / a direct-eval line comment accepts a lone high surrogate
run(function () {
    return String(eval("var marker = 42; //" + high + "\nmarker"));
})
// case: comments / an indirect-eval block comment accepts a lone low surrogate
run(function () {
    return String((0, eval)("var __qjoWtf8Marker = 42; /*" + low + "*/ __qjoWtf8Marker"));
})
// case: early error / a raw lone high surrogate in an identifier reports its second-line location
run(function () {
    try {
        eval("0;\nvar a" + high + " = 1");
        return "no-throw";
    } catch (error) {
        return [error.name, error.fileName, error.lineNumber, error.columnNumber].join("|");
    }
})
// case: early error / a raw lone low surrogate in an indirect-eval identifier is a SyntaxError
run(function () {
    try {
        (0, eval)("var a" + low + " = 1");
        return "no-throw";
    } catch (error) {
        return [error.name, error.fileName, error.lineNumber, error.columnNumber].join("|");
    }
})
// case: early error / a lone surrogate cannot become part of a RegExp group name
run(function () {
    try {
        eval(slash + "(?<a" + high + ">.)" + slash + "u");
        return "no-throw";
    } catch (error) {
        return error.name;
    }
})
// case: debug source / Function toString reconstructs the original private-use and surrogate units
run(function () {
    var source = "(function preserved(){return " + quote + privateUse + high + quote + "})";
    var functionValue = eval(source);
    var rendered = Function.prototype.toString.call(functionValue);
    var firstQuote = rendered.indexOf(quote);
    var lastQuote = rendered.lastIndexOf(quote);
    return String(rendered === source.slice(1, -1)) + "|" +
        unitHex(rendered.slice(firstQuote + 1, lastQuote));
})
