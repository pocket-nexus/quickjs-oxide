// UTF-16 source boundary of the four Dynamic Function kinds. Sources stay
// ASCII; the lone surrogates are built at run time and cross the same
// StringBuffer -> indirect-eval boundary as QuickJS's js_function_constructor.
function unitHex(value) {
    value = String(value);
    var output = "";
    for (var index = 0; index < value.length; index++)
        output += ("0000" + value.charCodeAt(index).toString(16)).slice(-4);
    return output;
}
function diagnostic(error) {
    var frames = String(error.stack).split("\n").slice(0, 2).join("/");
    return [error.name, error.message, error.fileName,
            error.lineNumber, error.columnNumber, frames].join("|");
}
var high = String.fromCharCode(0xd801);
var low = String.fromCharCode(0xdc00);
var pair = high + low;
var privateUse = String.fromCharCode(0xe001);
function run(Ctor, body) {
    try {
        return body(Ctor);
    } catch (error) {
        if (error !== null && typeof error === "object")
            return "unexpected|" + diagnostic(error);
        return "unexpected|" + typeof error + "|" + String(error);
    }
}
// case: Function / lone high surrogate parameter diagnostic
run(Function, function (Ctor) {
    try {
        Ctor(high, "");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: Function / lone low surrogate body diagnostic
run(Function, function (Ctor) {
    try {
        Ctor("var " + low + "=1");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: Function / parameter and body comments preserve lone surrogates
run(Function, function (Ctor) {
    var functionValue = Ctor(
        "value/*" + high + "*/",
        "/*" + low + "*/ return value"
    );
    var rendered = Function.prototype.toString.call(functionValue);
    var highIndex = rendered.indexOf(high);
    var lowIndex = rendered.indexOf(low);
    return functionValue.name + "|" +
        unitHex(rendered.slice(highIndex, highIndex + 1)) + "|" +
        unitHex(rendered.slice(lowIndex, lowIndex + 1));
})
// case: Function / valid surrogate pair is an identifier in parameters and body
run(Function, function (Ctor) {
    var functionValue = Ctor(pair, "return " + pair);
    var rendered = Function.prototype.toString.call(functionValue);
    return functionValue.name + "|" + functionValue.length + "|" +
        (rendered.indexOf(pair) >= 0);
})
// case: Function / body ToString runs before a lone-surrogate parameter is parsed
run(Function, function (Ctor) {
    var log = "";
    var parameter = {
        toString: function () { log += "p"; return high; }
    };
    var body = {
        toString: function () { log += "b"; return ""; }
    };
    try {
        Ctor(parameter, body);
        return "no-throw";
    } catch (error) {
        return log + "|" + diagnostic(error);
    }
})
// case: GeneratorFunction / lone high surrogate parameter diagnostic
run((function*(){}).constructor, function (Ctor) {
    try {
        Ctor(high, "");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: GeneratorFunction / lone low surrogate body diagnostic
run((function*(){}).constructor, function (Ctor) {
    try {
        Ctor("var " + low + "=1");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: GeneratorFunction / parameter and body comments preserve lone surrogates
run((function*(){}).constructor, function (Ctor) {
    var functionValue = Ctor(
        "value/*" + high + "*/",
        "/*" + low + "*/ return value"
    );
    var rendered = Function.prototype.toString.call(functionValue);
    var highIndex = rendered.indexOf(high);
    var lowIndex = rendered.indexOf(low);
    return functionValue.name + "|" +
        unitHex(rendered.slice(highIndex, highIndex + 1)) + "|" +
        unitHex(rendered.slice(lowIndex, lowIndex + 1));
})
// case: GeneratorFunction / valid surrogate pair is an identifier in parameters and body
run((function*(){}).constructor, function (Ctor) {
    var functionValue = Ctor(pair, "return " + pair);
    var rendered = Function.prototype.toString.call(functionValue);
    return functionValue.name + "|" + functionValue.length + "|" +
        (rendered.indexOf(pair) >= 0);
})
// case: GeneratorFunction / body ToString runs before a lone-surrogate parameter is parsed
run((function*(){}).constructor, function (Ctor) {
    var log = "";
    var parameter = {
        toString: function () { log += "p"; return high; }
    };
    var body = {
        toString: function () { log += "b"; return ""; }
    };
    try {
        Ctor(parameter, body);
        return "no-throw";
    } catch (error) {
        return log + "|" + diagnostic(error);
    }
})
// case: AsyncFunction / lone high surrogate parameter diagnostic
run((async function(){}).constructor, function (Ctor) {
    try {
        Ctor(high, "");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: AsyncFunction / lone low surrogate body diagnostic
run((async function(){}).constructor, function (Ctor) {
    try {
        Ctor("var " + low + "=1");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: AsyncFunction / parameter and body comments preserve lone surrogates
run((async function(){}).constructor, function (Ctor) {
    var functionValue = Ctor(
        "value/*" + high + "*/",
        "/*" + low + "*/ return value"
    );
    var rendered = Function.prototype.toString.call(functionValue);
    var highIndex = rendered.indexOf(high);
    var lowIndex = rendered.indexOf(low);
    return functionValue.name + "|" +
        unitHex(rendered.slice(highIndex, highIndex + 1)) + "|" +
        unitHex(rendered.slice(lowIndex, lowIndex + 1));
})
// case: AsyncFunction / valid surrogate pair is an identifier in parameters and body
run((async function(){}).constructor, function (Ctor) {
    var functionValue = Ctor(pair, "return " + pair);
    var rendered = Function.prototype.toString.call(functionValue);
    return functionValue.name + "|" + functionValue.length + "|" +
        (rendered.indexOf(pair) >= 0);
})
// case: AsyncFunction / body ToString runs before a lone-surrogate parameter is parsed
run((async function(){}).constructor, function (Ctor) {
    var log = "";
    var parameter = {
        toString: function () { log += "p"; return high; }
    };
    var body = {
        toString: function () { log += "b"; return ""; }
    };
    try {
        Ctor(parameter, body);
        return "no-throw";
    } catch (error) {
        return log + "|" + diagnostic(error);
    }
})
// case: AsyncGeneratorFunction / lone high surrogate parameter diagnostic
run((async function*(){}).constructor, function (Ctor) {
    try {
        Ctor(high, "");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: AsyncGeneratorFunction / lone low surrogate body diagnostic
run((async function*(){}).constructor, function (Ctor) {
    try {
        Ctor("var " + low + "=1");
        return "no-throw";
    } catch (error) {
        return diagnostic(error);
    }
})
// case: AsyncGeneratorFunction / parameter and body comments preserve lone surrogates
run((async function*(){}).constructor, function (Ctor) {
    var functionValue = Ctor(
        "value/*" + high + "*/",
        "/*" + low + "*/ return value"
    );
    var rendered = Function.prototype.toString.call(functionValue);
    var highIndex = rendered.indexOf(high);
    var lowIndex = rendered.indexOf(low);
    return functionValue.name + "|" +
        unitHex(rendered.slice(highIndex, highIndex + 1)) + "|" +
        unitHex(rendered.slice(lowIndex, lowIndex + 1));
})
// case: AsyncGeneratorFunction / valid surrogate pair is an identifier in parameters and body
run((async function*(){}).constructor, function (Ctor) {
    var functionValue = Ctor(pair, "return " + pair);
    var rendered = Function.prototype.toString.call(functionValue);
    return functionValue.name + "|" + functionValue.length + "|" +
        (rendered.indexOf(pair) >= 0);
})
// case: AsyncGeneratorFunction / body ToString runs before a lone-surrogate parameter is parsed
run((async function*(){}).constructor, function (Ctor) {
    var log = "";
    var parameter = {
        toString: function () { log += "p"; return high; }
    };
    var body = {
        toString: function () { log += "b"; return ""; }
    };
    try {
        Ctor(parameter, body);
        return "no-throw";
    } catch (error) {
        return log + "|" + diagnostic(error);
    }
})
// case: Function / body string literal returns a lone high surrogate
run(Function, function (Ctor) {
    var functionValue = Ctor("return '" + high + "'");
    return unitHex(functionValue());
})
// case: Function / parameter default returns a lone low surrogate
run(Function, function (Ctor) {
    var functionValue = Ctor("value='" + low + "'", "return value");
    return unitHex(functionValue());
})
// case: Function / private-use text stays distinct from a lone-surrogate carrier
run(Function, function (Ctor) {
    var functionValue = Ctor("return '" + privateUse + high + "'");
    var rendered = Function.prototype.toString.call(functionValue);
    return unitHex(functionValue()) + "|" +
        (rendered.indexOf(privateUse + high) >= 0);
})
