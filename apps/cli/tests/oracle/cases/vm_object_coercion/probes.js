// mode: print
function show(value) {
    if (value === undefined) return "undefined";
    if (typeof value === "number" || typeof value === "boolean") return String(value);
    if (typeof value === "bigint") return String(value) + "n";
    if (typeof value === "string") return "string:" + value;
    return "unexpected";
}
function observe(thunk) {
    try {
        return show(thunk());
    } catch (error) {
        if (typeof error === "string") return "throw-string:" + error;
        return "throw:" + error.name + "|" + error.message;
    }
}
function bit(value) { return value ? 1 : 0; }
// case: numeric object coercion
let numericHint = "none";
const numericObject = {
    [Symbol.toPrimitive](hint) { numericHint = hint; return 6; }
};
const leftNumericSymbol = Symbol("left");
const rightNumericSymbol = Symbol("right");
console.log("unary=" + observe(() => +numericObject) + "," + observe(() => -numericObject));
console.log("binary=" + observe(() => numericObject - 2) + "," +
            observe(() => numericObject * 3) + "," +
            observe(() => numericObject / 2) + "," +
            observe(() => numericObject % 4));
let rightValueCalls = 0;
const rightValue = {
    [Symbol.toPrimitive]() { rightValueCalls++; return 1; }
};
console.log("symbol-left-right-value=" +
            observe(() => leftNumericSymbol - rightValue) + "," +
            observe(() => leftNumericSymbol * rightValue) + "," +
            observe(() => leftNumericSymbol / rightValue) + "," +
            observe(() => leftNumericSymbol % rightValue) +
            "|calls:" + rightValueCalls);
let rightThrowCalls = 0;
const rightThrow = {
    [Symbol.toPrimitive]() { rightThrowCalls++; throw "right"; }
};
console.log("symbol-left-right-throw=" +
            observe(() => leftNumericSymbol - rightThrow) + "," +
            observe(() => leftNumericSymbol * rightThrow) + "," +
            observe(() => leftNumericSymbol / rightThrow) + "," +
            observe(() => leftNumericSymbol % rightThrow) +
            "|calls:" + rightThrowCalls);
console.log("bigint-symbol-add=" + observe(() => 1n + rightNumericSymbol) + "," +
            observe(() => leftNumericSymbol + 1n));
console.log("bigint-symbol-sub=" + observe(() => 1n - rightNumericSymbol) + "," +
            observe(() => leftNumericSymbol - 1n));
console.log("relational=" + observe(() => numericObject < 7) + "," +
            observe(() => numericObject <= 6) + "," +
            observe(() => numericObject > 5) + "," +
            observe(() => numericObject >= 6));
console.log("numeric-hint=" + numericHint);
console.log("bigint-string-fraction=" + observe(() => 1n < "1.5") + "," +
            observe(() => "1.5" > 1n));
console.log("bigint-string-large=" +
            observe(() => 9007199254740992n < "9007199254740993") + "," +
            observe(() => "9007199254740993" > 9007199254740992n));
console.log("bigint-invalid-right=" + observe(() => 1n < "invalid") + "," +
            observe(() => 1n <= "invalid") + "," +
            observe(() => 1n > "invalid") + "," +
            observe(() => 1n >= "invalid"));
console.log("bigint-invalid-left=" + observe(() => "invalid" < 1n) + "," +
            observe(() => "invalid" <= 1n) + "," +
            observe(() => "invalid" > 1n) + "," +
            observe(() => "invalid" >= 1n));
// case: addition object coercion
let addHint = "none";
const stringObject = {
    [Symbol.toPrimitive](hint) { addHint = hint; return "x"; }
};
console.log("add-string=" + observe(() => stringObject + 2) + "|hint:" + addHint);
const bigintObject = { [Symbol.toPrimitive]() { return 7n; } };
console.log("add-mixed=" + observe(() => bigintObject + 1));
console.log("function-plus=" + observe(() => +(function () {})));
// case: bitwise object coercion
let bitwiseHints = "";
let bitwiseHintCalls = 0;
const bitwiseNumber = {
    [Symbol.toPrimitive](hint) {
        bitwiseHints = bitwiseHints + hint + ",";
        bitwiseHintCalls = bitwiseHintCalls + 1;
        return 6;
    }
};
console.log("bitwise-number=" + observe(() => ~bitwiseNumber) + "," +
            observe(() => bitwiseNumber & 3) + "," +
            observe(() => bitwiseNumber ^ 3) + "," +
            observe(() => bitwiseNumber | 3) +
            "|hints:" + bitwiseHints + "|calls:" + bitwiseHintCalls);

let bitwiseOrder = 0;
const bitwiseLeft = {
    [Symbol.toPrimitive]() { bitwiseOrder = bitwiseOrder * 10 + 1; return 5; }
};
const bitwiseRight = {
    [Symbol.toPrimitive]() { bitwiseOrder = bitwiseOrder * 10 + 2; return 3; }
};
let orderedAnd = observe(() => bitwiseLeft & bitwiseRight) + "@" + bitwiseOrder;
bitwiseOrder = 0;
let orderedXor = observe(() => bitwiseLeft ^ bitwiseRight) + "@" + bitwiseOrder;
bitwiseOrder = 0;
let orderedOr = observe(() => bitwiseLeft | bitwiseRight) + "@" + bitwiseOrder;
console.log("bitwise-order=" + orderedAnd + "," + orderedXor + "," + orderedOr);

let mixedOrder = 0;
const mixedBigIntLeft = {
    [Symbol.toPrimitive]() { mixedOrder = mixedOrder * 10 + 1; return 1n; }
};
const mixedNumberRight = {
    [Symbol.toPrimitive]() { mixedOrder = mixedOrder * 10 + 2; return 1; }
};
const mixedNumberLeft = {
    [Symbol.toPrimitive]() { mixedOrder = mixedOrder * 10 + 1; return 1; }
};
const mixedBigIntRight = {
    [Symbol.toPrimitive]() { mixedOrder = mixedOrder * 10 + 2; return 1n; }
};
let mixedBigIntNumberAnd = observe(() => mixedBigIntLeft & mixedNumberRight) + "@" + mixedOrder;
mixedOrder = 0;
let mixedBigIntNumberXor = observe(() => mixedBigIntLeft ^ mixedNumberRight) + "@" + mixedOrder;
mixedOrder = 0;
let mixedBigIntNumberOr = observe(() => mixedBigIntLeft | mixedNumberRight) + "@" + mixedOrder;
mixedOrder = 0;
let mixedNumberBigIntAnd = observe(() => mixedNumberLeft & mixedBigIntRight) + "@" + mixedOrder;
mixedOrder = 0;
let mixedNumberBigIntXor = observe(() => mixedNumberLeft ^ mixedBigIntRight) + "@" + mixedOrder;
mixedOrder = 0;
let mixedNumberBigIntOr = observe(() => mixedNumberLeft | mixedBigIntRight) + "@" + mixedOrder;
console.log("bitwise-mixed=" +
            mixedBigIntNumberAnd + "," + mixedBigIntNumberXor + "," + mixedBigIntNumberOr + "," +
            mixedNumberBigIntAnd + "," + mixedNumberBigIntXor + "," + mixedNumberBigIntOr);

const leftBitwiseSymbol = Symbol("left-bitwise");
let symbolRightCalls = 0;
const symbolRight = {
    [Symbol.toPrimitive]() { symbolRightCalls = symbolRightCalls + 1; return 1; }
};
console.log("bitwise-symbol-left=" +
            observe(() => leftBitwiseSymbol & symbolRight) + "," +
            observe(() => leftBitwiseSymbol ^ symbolRight) + "," +
            observe(() => leftBitwiseSymbol | symbolRight) +
            "|right-calls:" + symbolRightCalls);

const bitwiseSentinel = {};
let leftThrowCalls = 0;
let leftThrowRightCalls = 0;
const leftThrow = {
    [Symbol.toPrimitive]() { leftThrowCalls = leftThrowCalls + 1; throw bitwiseSentinel; }
};
const afterLeftThrow = {
    [Symbol.toPrimitive]() { leftThrowRightCalls = leftThrowRightCalls + 1; return 1; }
};
function bitwiseThrownSame(thunk) {
    try { thunk(); return 0; }
    catch (error) { return error === bitwiseSentinel ? 1 : 0; }
}
console.log("bitwise-left-throw=" +
            bitwiseThrownSame(() => ~leftThrow) + "," +
            bitwiseThrownSame(() => leftThrow & afterLeftThrow) + "," +
            bitwiseThrownSame(() => leftThrow ^ afterLeftThrow) + "," +
            bitwiseThrownSame(() => leftThrow | afterLeftThrow) +
            "|left-calls:" + leftThrowCalls + "|right-calls:" + leftThrowRightCalls);

let rightThrowLeftCalls = 0;
let rightThrowCalls = 0;
const beforeRightThrow = {
    [Symbol.toPrimitive]() { rightThrowLeftCalls = rightThrowLeftCalls + 1; return 1; }
};
const rightThrowBitwise = {
    [Symbol.toPrimitive]() { rightThrowCalls = rightThrowCalls + 1; throw bitwiseSentinel; }
};
console.log("bitwise-right-throw=" +
            bitwiseThrownSame(() => beforeRightThrow & rightThrowBitwise) + "," +
            bitwiseThrownSame(() => beforeRightThrow ^ rightThrowBitwise) + "," +
            bitwiseThrownSame(() => beforeRightThrow | rightThrowBitwise) +
            "|left-calls:" + rightThrowLeftCalls + "|right-calls:" + rightThrowCalls);
// case: shift object coercion
let shiftHints = "";
let shiftHintCalls = 0;
const shiftNumber = {
    [Symbol.toPrimitive](hint) {
        shiftHints = shiftHints + hint + ",";
        shiftHintCalls = shiftHintCalls + 1;
        return -1;
    }
};
console.log("shift-number=" + observe(() => shiftNumber << 1) + "," +
            observe(() => shiftNumber >> 1) + "," +
            observe(() => shiftNumber >>> 0) +
            "|hints:" + shiftHints + "|calls:" + shiftHintCalls);
console.log("shift-unsigned-result=" + observe(() => -1 >>> 0) + "," +
            observe(() => -2147483648 >>> 0) + "," +
            observe(() => -1 >>> 1));

let shiftOrder = "";
const shiftOrderedLeft = {
    [Symbol.toPrimitive]() { shiftOrder = shiftOrder + "l"; return 16; }
};
const shiftOrderedRight = {
    [Symbol.toPrimitive]() { shiftOrder = shiftOrder + "r"; return 1; }
};
function shiftEvalLeft() { shiftOrder = shiftOrder + "L"; return shiftOrderedLeft; }
function shiftEvalRight() { shiftOrder = shiftOrder + "R"; return shiftOrderedRight; }
let orderedShl = observe(() => shiftEvalLeft() << shiftEvalRight()) + "@" + shiftOrder;
shiftOrder = "";
let orderedSar = observe(() => shiftEvalLeft() >> shiftEvalRight()) + "@" + shiftOrder;
shiftOrder = "";
let orderedShr = observe(() => shiftEvalLeft() >>> shiftEvalRight()) + "@" + shiftOrder;
console.log("shift-order=" + orderedShl + "," + orderedSar + "," + orderedShr);

const shiftSentinel = {};
function shiftThrownSame(thunk) {
    try { thunk(); return 0; }
    catch (error) { return error === shiftSentinel ? 1 : 0; }
}
let shiftLeftThrowCalls = 0;
let shiftAfterLeftThrowCalls = 0;
const shiftLeftThrow = {
    [Symbol.toPrimitive]() {
        shiftLeftThrowCalls = shiftLeftThrowCalls + 1;
        throw shiftSentinel;
    }
};
const shiftAfterLeftThrow = {
    [Symbol.toPrimitive]() {
        shiftAfterLeftThrowCalls = shiftAfterLeftThrowCalls + 1;
        return 1;
    }
};
console.log("shift-left-throw=" +
            shiftThrownSame(() => shiftLeftThrow << shiftAfterLeftThrow) + "," +
            shiftThrownSame(() => shiftLeftThrow >> shiftAfterLeftThrow) + "," +
            shiftThrownSame(() => shiftLeftThrow >>> shiftAfterLeftThrow) +
            "|left-calls:" + shiftLeftThrowCalls +
            "|right-calls:" + shiftAfterLeftThrowCalls);

let shiftBeforeRightThrowCalls = 0;
let shiftRightThrowCalls = 0;
const shiftBeforeRightThrow = {
    [Symbol.toPrimitive]() {
        shiftBeforeRightThrowCalls = shiftBeforeRightThrowCalls + 1;
        return 1;
    }
};
const shiftRightThrow = {
    [Symbol.toPrimitive]() {
        shiftRightThrowCalls = shiftRightThrowCalls + 1;
        throw shiftSentinel;
    }
};
console.log("shift-right-throw=" +
            shiftThrownSame(() => shiftBeforeRightThrow << shiftRightThrow) + "," +
            shiftThrownSame(() => shiftBeforeRightThrow >> shiftRightThrow) + "," +
            shiftThrownSame(() => shiftBeforeRightThrow >>> shiftRightThrow) +
            "|left-calls:" + shiftBeforeRightThrowCalls +
            "|right-calls:" + shiftRightThrowCalls);

let shiftMixedOrder = 0;
const shiftBigIntLeft = {
    [Symbol.toPrimitive]() { shiftMixedOrder = shiftMixedOrder * 10 + 1; return 8n; }
};
const shiftNumberRight = {
    [Symbol.toPrimitive]() { shiftMixedOrder = shiftMixedOrder * 10 + 2; return 1; }
};
const shiftNumberLeft = {
    [Symbol.toPrimitive]() { shiftMixedOrder = shiftMixedOrder * 10 + 1; return 8; }
};
const shiftBigIntRight = {
    [Symbol.toPrimitive]() { shiftMixedOrder = shiftMixedOrder * 10 + 2; return 1n; }
};
let mixedBigIntNumberShl = observe(() => shiftBigIntLeft << shiftNumberRight) + "@" + shiftMixedOrder;
shiftMixedOrder = 0;
let mixedBigIntNumberSar = observe(() => shiftBigIntLeft >> shiftNumberRight) + "@" + shiftMixedOrder;
shiftMixedOrder = 0;
let mixedNumberBigIntShl = observe(() => shiftNumberLeft << shiftBigIntRight) + "@" + shiftMixedOrder;
shiftMixedOrder = 0;
let mixedNumberBigIntSar = observe(() => shiftNumberLeft >> shiftBigIntRight) + "@" + shiftMixedOrder;
console.log("shift-mixed=" + mixedBigIntNumberShl + "," + mixedBigIntNumberSar + "," +
            mixedNumberBigIntShl + "," + mixedNumberBigIntSar);

shiftMixedOrder = 0;
let unsignedBigIntNumber = observe(() => shiftBigIntLeft >>> shiftNumberRight) + "@" + shiftMixedOrder;
shiftMixedOrder = 0;
let unsignedNumberBigInt = observe(() => shiftNumberLeft >>> shiftBigIntRight) + "@" + shiftMixedOrder;
shiftMixedOrder = 0;
let unsignedBigIntBigInt = observe(() => shiftBigIntLeft >>> shiftBigIntRight) + "@" + shiftMixedOrder;
console.log("shift-unsigned-bigint=" + unsignedBigIntNumber + "," +
            unsignedNumberBigInt + "," + unsignedBigIntBigInt);

const shiftSymbol = Symbol("shift");
let shiftSymbolRightCalls = 0;
const shiftSymbolRight = {
    [Symbol.toPrimitive]() { shiftSymbolRightCalls = shiftSymbolRightCalls + 1; return 1n; }
};
console.log("shift-symbol-left=" + observe(() => shiftSymbol << shiftSymbolRight) + "," +
            observe(() => shiftSymbol >> shiftSymbolRight) + "," +
            observe(() => shiftSymbol >>> shiftSymbolRight) +
            "|right-calls:" + shiftSymbolRightCalls);
let shiftBeforeSymbolCalls = 0;
const shiftBeforeSymbol = {
    [Symbol.toPrimitive]() { shiftBeforeSymbolCalls = shiftBeforeSymbolCalls + 1; return 1n; }
};
console.log("shift-symbol-right=" + observe(() => shiftBeforeSymbol << shiftSymbol) + "," +
            observe(() => shiftBeforeSymbol >> shiftSymbol) + "," +
            observe(() => shiftBeforeSymbol >>> shiftSymbol) +
            "|left-calls:" + shiftBeforeSymbolCalls);
// case: exponentiation object coercion
let powerHints = "";
let powerHintCalls = 0;
const powerNumber = {
    [Symbol.toPrimitive](hint) {
        powerHints = powerHints + hint + ",";
        powerHintCalls = powerHintCalls + 1;
        return 2;
    }
};
console.log("power-number=" + observe(() => powerNumber ** 3) +
            "|hints:" + powerHints + "|calls:" + powerHintCalls);

let powerOrder = "";
const powerOrderedLeft = {
    [Symbol.toPrimitive]() { powerOrder = powerOrder + "l"; return 2; }
};
const powerOrderedRight = {
    [Symbol.toPrimitive]() { powerOrder = powerOrder + "r"; return 3; }
};
function powerEvalLeft() { powerOrder = powerOrder + "L"; return powerOrderedLeft; }
function powerEvalRight() { powerOrder = powerOrder + "R"; return powerOrderedRight; }
console.log("power-order=" + observe(() => powerEvalLeft() ** powerEvalRight()) +
            "@" + powerOrder);

powerOrder = "";
const powerA = {
    [Symbol.toPrimitive]() { powerOrder = powerOrder + "a"; return 2; }
};
const powerB = {
    [Symbol.toPrimitive]() { powerOrder = powerOrder + "b"; return 3; }
};
const powerC = {
    [Symbol.toPrimitive]() { powerOrder = powerOrder + "c"; return 2; }
};
function powerEvalA() { powerOrder = powerOrder + "A"; return powerA; }
function powerEvalB() { powerOrder = powerOrder + "B"; return powerB; }
function powerEvalC() { powerOrder = powerOrder + "C"; return powerC; }
console.log("power-right-associative=" +
            observe(() => powerEvalA() ** powerEvalB() ** powerEvalC()) +
            "@" + powerOrder);

const powerSentinel = {};
function powerThrownSame(thunk) {
    try { thunk(); return 0; }
    catch (error) { return error === powerSentinel ? 1 : 0; }
}
let powerLeftThrowCalls = 0;
let powerAfterLeftThrowCalls = 0;
const powerLeftThrow = {
    [Symbol.toPrimitive]() {
        powerLeftThrowCalls = powerLeftThrowCalls + 1;
        throw powerSentinel;
    }
};
const powerAfterLeftThrow = {
    [Symbol.toPrimitive]() {
        powerAfterLeftThrowCalls = powerAfterLeftThrowCalls + 1;
        return 3;
    }
};
console.log("power-left-throw=" +
            powerThrownSame(() => powerLeftThrow ** powerAfterLeftThrow) +
            "|left-calls:" + powerLeftThrowCalls +
            "|right-calls:" + powerAfterLeftThrowCalls);

let powerBeforeRightThrowCalls = 0;
let powerRightThrowCalls = 0;
const powerBeforeRightThrow = {
    [Symbol.toPrimitive]() {
        powerBeforeRightThrowCalls = powerBeforeRightThrowCalls + 1;
        return 2;
    }
};
const powerRightThrow = {
    [Symbol.toPrimitive]() {
        powerRightThrowCalls = powerRightThrowCalls + 1;
        throw powerSentinel;
    }
};
console.log("power-right-throw=" +
            powerThrownSame(() => powerBeforeRightThrow ** powerRightThrow) +
            "|left-calls:" + powerBeforeRightThrowCalls +
            "|right-calls:" + powerRightThrowCalls);

let powerMixedOrder = 0;
const powerBigIntLeft = {
    [Symbol.toPrimitive]() { powerMixedOrder = powerMixedOrder * 10 + 1; return 2n; }
};
const powerNumberRight = {
    [Symbol.toPrimitive]() { powerMixedOrder = powerMixedOrder * 10 + 2; return 3; }
};
const powerNumberLeft = {
    [Symbol.toPrimitive]() { powerMixedOrder = powerMixedOrder * 10 + 1; return 2; }
};
const powerBigIntRight = {
    [Symbol.toPrimitive]() { powerMixedOrder = powerMixedOrder * 10 + 2; return 3n; }
};
let powerBigIntNumber = observe(() => powerBigIntLeft ** powerNumberRight) +
                        "@" + powerMixedOrder;
powerMixedOrder = 0;
let powerNumberBigInt = observe(() => powerNumberLeft ** powerBigIntRight) +
                        "@" + powerMixedOrder;
console.log("power-mixed=" + powerBigIntNumber + "," + powerNumberBigInt);

powerMixedOrder = 0;
const powerMixedThrowRight = {
    [Symbol.toPrimitive]() {
        powerMixedOrder = powerMixedOrder * 10 + 2;
        throw powerSentinel;
    }
};
let powerMixedThrow = powerThrownSame(() => powerBigIntLeft ** powerMixedThrowRight) +
                      "@" + powerMixedOrder;
powerMixedOrder = 0;
const powerMixedSymbolRight = {
    [Symbol.toPrimitive]() {
        powerMixedOrder = powerMixedOrder * 10 + 2;
        return Symbol("mixed-power");
    }
};
let powerMixedSymbol = observe(() => powerBigIntLeft ** powerMixedSymbolRight) +
                       "@" + powerMixedOrder;
console.log("power-mixed-priority=" + powerMixedThrow + "," + powerMixedSymbol);

const powerSymbol = Symbol("power");
let powerAfterSymbolCalls = 0;
const powerAfterSymbol = {
    [Symbol.toPrimitive]() {
        powerAfterSymbolCalls = powerAfterSymbolCalls + 1;
        throw powerSentinel;
    }
};
console.log("power-symbol-before-throw=" +
            observe(() => powerSymbol ** powerAfterSymbol) +
            "|right-calls:" + powerAfterSymbolCalls);
let powerBeforeSymbolCalls = 0;
const powerBeforeSymbol = {
    [Symbol.toPrimitive]() {
        powerBeforeSymbolCalls = powerBeforeSymbolCalls + 1;
        throw powerSentinel;
    }
};
console.log("power-throw-before-symbol=" +
            powerThrownSame(() => powerBeforeSymbol ** powerSymbol) +
            "|left-calls:" + powerBeforeSymbolCalls);
let powerConvertedBeforeSymbolCalls = 0;
const powerConvertedBeforeSymbol = {
    [Symbol.toPrimitive]() {
        powerConvertedBeforeSymbolCalls = powerConvertedBeforeSymbolCalls + 1;
        return 2;
    }
};
console.log("power-symbol-right=" +
            observe(() => powerConvertedBeforeSymbol ** powerSymbol) +
            "|left-calls:" + powerConvertedBeforeSymbolCalls);

console.log("power-number-special=" +
            observe(() => (0 / 0) ** 0) + "," +
            observe(() => (0 / 0) ** 1) + "," +
            observe(() => 1 ** (1 / 0)) + "," +
            observe(() => (-1) ** (-1 / 0)) + "," +
            observe(() => (-2) ** 0.5) + "," +
            observe(() => 1 / ((-0) ** 3)) + "," +
            observe(() => 1 / ((-0) ** 2)) + "," +
            observe(() => (-0) ** -3) + "," +
            observe(() => (-0) ** -2) + "," +
            observe(() => (-1 / 0) ** 3) + "," +
            observe(() => 1 / ((-1 / 0) ** -3)) + "," +
            observe(() => 2 ** 1024) + "," +
            observe(() => 2 ** -1074) + "," +
            observe(() => 2 ** -1075));
// case: abstract equality object coercion
const symbolValue = Symbol("s");
function box(value) {
    return { [Symbol.toPrimitive]() { return value; } };
}
function equality(name, object, primitive) {
    console.log("eq-" + name + "=" +
        bit(object == primitive) + "," + bit(primitive == object) + "," +
        bit(object != primitive) + "," + bit(primitive != object));
}
equality("number", box(7), 7);
equality("string", box("x"), "x");
equality("bigint", box(7n), 7n);
equality("symbol", box(symbolValue), symbolValue);
equality("boolean", box(1), true);
// case: coercion order and errors
let order = 0;
const left = { [Symbol.toPrimitive]() { order = order * 10 + 1; return 1; } };
const right = { [Symbol.toPrimitive]() { order = order * 10 + 2; return 2; } };
console.log("order-add=" + observe(() => left + right) + "|order:" + order);
order = 0;
console.log("order-relational=" + observe(() => left < right) + "|order:" + order);

order = 0;
const ordinary = {
    valueOf() { order = order * 10 + 1; return ordinary; },
    toString() { order = order * 10 + 2; return "5"; },
};
console.log("ordinary-number=" + observe(() => +ordinary) + "|order:" + order);
order = 0;
console.log("ordinary-add=" + observe(() => ordinary + 1) + "|order:" + order);

const sentinel = {};
const getterThrow = {};
Object.defineProperty(getterThrow, Symbol.toPrimitive, {
    get() { throw sentinel; },
    configurable: true,
});
let getterSame = false;
try { +getterThrow; } catch (error) { getterSame = error === sentinel; }
console.log("getter-throw=" + (getterSame ? "same" : "changed"));

const methodThrow = { [Symbol.toPrimitive]() { throw sentinel; } };
let methodSame = false;
try { +methodThrow; } catch (error) { methodSame = error === sentinel; }
console.log("method-throw=" + (methodSame ? "same" : "changed"));

const objectReturn = { [Symbol.toPrimitive]() { return {}; } };
console.log("object-return=" + observe(() => +objectReturn));
const noncallable = { [Symbol.toPrimitive]: 1 };
console.log("noncallable=" + observe(() => +noncallable));

const stackObject = {
    [Symbol.toPrimitive]: function convert() { throw new Error("coerce"); }
};
function frameNames(stack) {
    return stack.trim().split("\n").slice(0, 3).map(line => {
        const match = /^\s*at ([^(]+?)(?: \(|$)/.exec(line);
        return match ? match[1].trim() : "?";
    }).join(",");
}
try {
    (function outer() { return +stackObject; })();
} catch (error) {
    console.log("stack=" + frameNames(error.stack));
}
