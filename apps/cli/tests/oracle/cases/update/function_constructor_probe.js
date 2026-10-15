// mode: print
function frames(stack, count) {
    return String(stack).split("\n").slice(0, count).join("|");
}
function text(value) {
    return value === undefined ? "undefined" : String(value);
}

try {
    Function("(1)++");
    print("invalid=missing");
} catch (error) {
    print("invalid=" + error.name + ":" + error.message + "|" +
          text(error.fileName) + ":" + text(error.lineNumber) + ":" +
          text(error.columnNumber) + "|" + frames(error.stack, 2));
}

globalThis.__qjo_update_symbol = Symbol();
var update = Function("\nreturn __qjo_update_symbol++");
try {
    update();
    print("runtime=missing");
} catch (error) {
    print("runtime=" + error.name + ":" + error.message + "|" +
          frames(error.stack, 1));
}

try {
    Function("\"use strict\"; eval++");
    print("strict-eval=missing");
} catch (error) {
    print("strict-eval=" + error.name + ":" + error.message + "|" +
          frames(error.stack, 2));
}

try {
    Function("\"use strict\"; arguments--");
    print("strict-arguments=missing");
} catch (error) {
    print("strict-arguments=" + error.name + ":" + error.message + "|" +
          frames(error.stack, 2));
}

print("strict-caller=" + (function () {
    "use strict";
    return Function("eval", "return eval++")(9);
})());
