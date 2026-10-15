// mode: print
function show(value) {
    if (value === undefined) return "undefined:undefined";
    if (value === null) return "object:null";
    return typeof value + ":" + String(value);
}
function observe(thunk) {
    try { return show(thunk()); }
    catch (error) { return "throw:" + error.name + ":" + error.message; }
}

let log = "";
let seenThis = false;
let seenValue;
const proto = {};
const target = Object.create(proto);
Object.defineProperty(proto, "member", {
    set(value) { log += "s"; seenThis = this === target; seenValue = value; },
    configurable: true
});
const key = {
    [Symbol.toPrimitive](hint) { log += "k(" + hint + ")"; return "member"; }
};
Object.defineProperty(proto, "compound", {
    get() { log += "g"; return oldValue; },
    set(value) { log += "s"; seenThis = this === target; seenValue = value; },
    configurable: true
});
let oldValue = 2;
let identifierOld = 2;
let identifierGetThis = false;
let identifierSetThis = false;
Object.defineProperty(globalThis, "identifierAccessor", {
    get() { log += "G"; identifierGetThis = this === globalThis; return identifierOld; },
    set(value) { log += "S"; identifierSetThis = this === globalThis; seenValue = value; },
    configurable: true
});
Object.defineProperty(globalThis, "identifierReadonly", {
    value: 2, writable: false, configurable: true
});
Object.defineProperty(globalThis, "identifierThrowing", {
    get() { log += "G"; throw new Error("identifier getter"); },
    configurable: true
});
const compoundKey = {
    [Symbol.toPrimitive](hint) { log += "k(" + hint + ")"; return "compound"; }
};
function baseExpr() { log += "b"; return target; }
function keyExpr() { log += "q"; return key; }
function compoundKeyExpr() { log += "q"; return compoundKey; }
function rhsExpr() { log += "r"; return 9; }
function throwingRhs() { log += "r"; throw new Error("rhs"); }
const throwingKey = {};
Object.defineProperty(throwingKey, Symbol.toPrimitive, {
    value: function() { log += "k!"; throw new Error("key"); }
});
function throwingKeyExpr() { log += "q"; return throwingKey; }
const powerLeft = function(){};
const powerRight = function(){};
powerLeft.valueOf = function(){ log += "l"; return 2; };
powerRight.valueOf = function(){ log += "p"; return 3; };
function powerRhsExpr() { log += "R"; return powerRight; }
let powerPhase = "";
const powerPhaseLeft = function(){};
const powerPhaseRight = function(){};
function powerThrow(message) { throw new Error(message); }
powerPhaseLeft.valueOf = function(){
    log += "l";
    powerPhase === "lhs-convert" && powerThrow("lhs conversion");
    return 2;
};
powerPhaseRight.valueOf = function(){
    log += "p";
    powerPhase === "rhs-convert" && powerThrow("rhs conversion");
    return 3;
};
function phasePowerRhs() {
    log += "r";
    powerPhase === "rhs" && powerThrow("rhs");
    return powerPhaseRight;
}
Object.defineProperty(target, "powerFailure", {
    get() {
        log += "g";
        powerPhase === "getter" && powerThrow("getter");
        return powerPhaseLeft;
    },
    set(value) {
        log += "s";
        powerPhase === "setter" && powerThrow("setter");
        seenValue = value;
    },
    configurable: true
});
const powerOuter = {};
const powerInner = {};
let powerOuterSeen;
let powerInnerSeen;
Object.defineProperty(powerOuter, "value", {
    get() { log += "G"; return 2; },
    set(value) { log += "S"; powerOuterSeen = value; },
    configurable: true
});
Object.defineProperty(powerInner, "value", {
    get() { log += "g"; return 3; },
    set(value) { log += "s"; powerInnerSeen = value; },
    configurable: true
});
function rightPowerRhs() { log += "r"; return 2; }
function bigintPowerRhs() { log += "r"; return 1n; }
const powerSymbol = Symbol("power");
function symbolPowerRhs() { log += "r"; return powerSymbol; }

log = ""; seenThis = false; seenValue = undefined;
const setResult = baseExpr()[keyExpr()] = rhsExpr();
print("ordered-set=" + [show(setResult), log, show(seenThis), show(seenValue),
      show(Object.hasOwn(target, "member")), show(target.member)].join("|"));

log = "";
const deleteResult = delete baseExpr()[keyExpr()];
print("ordered-delete=" + [show(deleteResult), log,
      show(Object.hasOwn(target, "member")), show(target.member)].join("|"));

log = ""; seenThis = false; seenValue = undefined;
const compoundResult = baseExpr()[compoundKeyExpr()] += rhsExpr();
print("ordered-compound=" + [show(compoundResult), log, show(seenThis), show(seenValue),
      show(Object.hasOwn(target, "compound"))].join("|"));
oldValue = 14; log = ""; seenThis = false; seenValue = undefined;
const bitwiseCompoundResult = baseExpr()[compoundKeyExpr()] &= rhsExpr();
print("ordered-bitwise-compound=" + [show(bitwiseCompoundResult), log,
      show(seenThis), show(seenValue), show(Object.hasOwn(target, "compound"))].join("|"));
oldValue = 3; log = ""; seenThis = false; seenValue = undefined;
const shiftCompoundResult = baseExpr()[compoundKeyExpr()] <<= rhsExpr();
print("ordered-shift-compound=" + [show(shiftCompoundResult), log,
      show(seenThis), show(seenValue), show(Object.hasOwn(target, "compound"))].join("|"));
oldValue = powerLeft; log = ""; seenThis = false; seenValue = undefined;
const powerCompoundResult = baseExpr()[compoundKeyExpr()] **= powerRhsExpr();
print("ordered-power-compound=" + [show(powerCompoundResult), log,
      show(seenThis), show(seenValue), show(Object.hasOwn(target, "compound"))].join("|"));
function printPowerFailure(label, phase) {
    powerPhase = phase; log = ""; seenValue = undefined;
    print(label + "=" + observe(() => target.powerFailure **= phasePowerRhs()) +
          "|" + log + "|" + show(seenValue));
}
printPowerFailure("power-getter-failure", "getter");
printPowerFailure("power-rhs-failure", "rhs");
printPowerFailure("power-lhs-convert-failure", "lhs-convert");
printPowerFailure("power-rhs-convert-failure", "rhs-convert");
printPowerFailure("power-setter-failure", "setter");
log = ""; powerOuterSeen = undefined; powerInnerSeen = undefined;
const powerRightAssociative = powerOuter.value **= powerInner.value **= rightPowerRhs();
print("power-right-assoc=" + [show(powerRightAssociative), log,
      show(powerOuterSeen), show(powerInnerSeen)].join("|"));
let powerAnonymousName = "unset";
Function.prototype.valueOf = function(){ powerAnonymousName = this.name; return 3; };
target.powerAnonymous = 2;
const powerAnonymousResult = target.powerAnonymous **= function(){};
delete Function.prototype.valueOf;
print("power-anonymous-name=" + [show(powerAnonymousResult),
      show(powerAnonymousName)].join("|"));
log = "";
print("null-compound=" + observe(() => null[compoundKeyExpr()] += rhsExpr()) + "|" + log);
log = "";
print("null-bitwise-compound=" + observe(() => null[compoundKeyExpr()] &= rhsExpr()) + "|" + log);
log = "";
print("null-shift-compound=" + observe(() => null[compoundKeyExpr()] <<= rhsExpr()) + "|" + log);
log = "";
print("null-power-compound=" + observe(() => null[compoundKeyExpr()] **= rhsExpr()) + "|" + log);

const compoundSymbol = Symbol("compound-key");
const otherCompoundSymbol = Symbol("compound-key");
target[compoundSymbol] = 3;
const symbolKeyObject = {
    [Symbol.toPrimitive](hint) { log += "k(" + hint + ")"; return compoundSymbol; }
};
log = "";
const objectSymbolCompound = target[symbolKeyObject] += 4;
print("object-symbol-compound=" + [show(objectSymbolCompound), log,
      show(target[compoundSymbol]), show(target[otherCompoundSymbol])].join("|"));
const binaryNullishSymbol = compoundSymbol ?? otherCompoundSymbol;
print("binary-nullish-symbol=" + [show(binaryNullishSymbol === compoundSymbol),
      show(binaryNullishSymbol === otherCompoundSymbol)].join("|"));

target[compoundSymbol] = 0;
log = "";
const objectSymbolLogical = target[symbolKeyObject] ||= 8;
print("object-symbol-logical=" + [show(objectSymbolLogical), log,
      show(target[compoundSymbol]), show(target[otherCompoundSymbol])].join("|"));
log = "";
const objectSymbolLogicalShort = target[symbolKeyObject] ||= 10;
print("object-symbol-logical-short=" + [show(objectSymbolLogicalShort), log,
      show(target[compoundSymbol]), show(target[otherCompoundSymbol])].join("|"));

target.arithmetic = 20;
print("arithmetic=" + [show(target.arithmetic += 2), show(target.arithmetic -= 4),
      show(target.arithmetic *= 3), show(target.arithmetic /= 2),
      show(target.arithmetic %= 5)].join("|"));
target.bitwise = 14;
print("bitwise=" + [show(target.bitwise &= 11), show(target.bitwise ^= 3),
      show(target.bitwise |= 4)].join("|"));
target.shift = -8;
print("shift=" + [show(target.shift >>= 1), show(target.shift >>>= 1),
      show(target.shift <<= 1)].join("|"));
target.power = 2;
print("power=" + [show(target.power **= 3), show(target.power **= 2)].join("|"));

oldValue = 2; log = ""; seenThis = false; seenValue = undefined;
const logicalAndSet = baseExpr()[compoundKeyExpr()] &&= rhsExpr();
print("logical-and-set=" + [show(logicalAndSet), log, show(seenThis), show(seenValue)].join("|"));
oldValue = 0; log = ""; seenThis = false; seenValue = undefined;
const logicalAndSkip = baseExpr()[compoundKeyExpr()] &&= rhsExpr();
print("logical-and-skip=" + [show(logicalAndSkip), log, show(seenThis), show(seenValue)].join("|"));
oldValue = 0; log = ""; seenThis = false; seenValue = undefined;
const logicalOrSet = baseExpr()[compoundKeyExpr()] ||= rhsExpr();
print("logical-or-set=" + [show(logicalOrSet), log, show(seenThis), show(seenValue)].join("|"));
oldValue = 2; log = ""; seenThis = false; seenValue = undefined;
const logicalOrSkip = baseExpr()[compoundKeyExpr()] ||= rhsExpr();
print("logical-or-skip=" + [show(logicalOrSkip), log, show(seenThis), show(seenValue)].join("|"));
oldValue = null; log = ""; seenThis = false; seenValue = undefined;
const logicalNullishSet = baseExpr()[compoundKeyExpr()] ??= rhsExpr();
print("logical-nullish-set=" + [show(logicalNullishSet), log, show(seenThis), show(seenValue)].join("|"));
oldValue = false; log = ""; seenThis = false; seenValue = undefined;
const logicalNullishSkip = baseExpr()[compoundKeyExpr()] ??= rhsExpr();
print("logical-nullish-skip=" + [show(logicalNullishSkip), log, show(seenThis), show(seenValue)].join("|"));

const coercionBomb = {};
Object.defineProperty(coercionBomb, Symbol.toPrimitive, {
    value: function() { log += "p"; throw new Error("coerced"); }
});
log = "";
const binaryNullishObject = coercionBomb ?? null;
print("binary-nullish-object=" + [show(binaryNullishObject === coercionBomb), log].join("|"));
oldValue = coercionBomb; log = "";
const logicalObjectAnd = baseExpr()[compoundKeyExpr()] &&= rhsExpr();
print("logical-object-and=" + [show(logicalObjectAnd), log].join("|"));
oldValue = coercionBomb; log = "";
const logicalObjectOr = baseExpr()[compoundKeyExpr()] ||= rhsExpr();
print("logical-object-or=" + [show(logicalObjectOr === coercionBomb), log].join("|"));
oldValue = coercionBomb; log = "";
const logicalObjectNullish = baseExpr()[compoundKeyExpr()] ??= rhsExpr();
print("logical-object-nullish=" + [show(logicalObjectNullish === coercionBomb), log].join("|"));
oldValue = 1; log = "";
print("logical-rhs-throw=" + observe(() =>
      baseExpr()[compoundKeyExpr()] &&= throwingRhs()) + "|" + log);
log = "";
print("logical-key-throw=" + observe(() =>
      baseExpr()[throwingKeyExpr()] ||= rhsExpr()) + "|" + log);
log = "";
print("logical-getter-throw=" + observe(() =>
      Function.prototype.caller &&= rhsExpr()) + "|" + log);

identifierOld = 2; log = ""; identifierGetThis = false;
identifierSetThis = false; seenValue = undefined;
const identifierArithmetic = identifierAccessor += rhsExpr();
print("identifier-arithmetic=" + [show(identifierArithmetic), log,
      show(identifierGetThis), show(identifierSetThis), show(seenValue)].join("|"));
identifierOld = 14; log = ""; identifierGetThis = false;
identifierSetThis = false; seenValue = undefined;
const identifierBitwise = identifierAccessor &= rhsExpr();
print("identifier-bitwise=" + [show(identifierBitwise), log,
      show(identifierGetThis), show(identifierSetThis), show(seenValue)].join("|"));
identifierOld = 3; log = ""; identifierGetThis = false;
identifierSetThis = false; seenValue = undefined;
const identifierShift = identifierAccessor <<= rhsExpr();
print("identifier-shift=" + [show(identifierShift), log,
      show(identifierGetThis), show(identifierSetThis), show(seenValue)].join("|"));
identifierOld = 2; log = ""; identifierGetThis = false;
identifierSetThis = false; seenValue = undefined;
const identifierPower = identifierAccessor **= rhsExpr();
print("identifier-power=" + [show(identifierPower), log,
      show(identifierGetThis), show(identifierSetThis), show(seenValue)].join("|"));
identifierOld = 0; log = ""; identifierGetThis = false;
identifierSetThis = false; seenValue = undefined;
const identifierLogicalSet = identifierAccessor ||= rhsExpr();
print("identifier-logical-set=" + [show(identifierLogicalSet), log,
      show(identifierGetThis), show(identifierSetThis), show(seenValue)].join("|"));
identifierOld = 2; log = ""; identifierGetThis = false;
identifierSetThis = false; seenValue = undefined;
const identifierLogicalShort = identifierAccessor ||= rhsExpr();
print("identifier-logical-short=" + [show(identifierLogicalShort), log,
      show(identifierGetThis), show(identifierSetThis), show(seenValue)].join("|"));
identifierOld = undefined; log = ""; seenValue = undefined;
const identifierNamed = identifierAccessor ??= function(){};
print("identifier-name=" + [show(identifierNamed.name),
      show(identifierNamed === seenValue), log].join("|"));
identifierOld = undefined; log = ""; seenValue = undefined;
const identifierParenNamed = (identifierAccessor) ??= function(){};
print("identifier-paren-name=" + [show(identifierParenNamed.name),
      show(identifierParenNamed === seenValue), log].join("|"));
print("identifier-readonly=" + [show(identifierReadonly += 3), show(identifierReadonly),
      observe(() => (function(){ "use strict"; return identifierReadonly += 3; })()),
      observe(() => (function(){ "use strict"; return identifierReadonly ||= 9; })()),
      observe(() => (function(){ "use strict"; return identifierReadonly &&= 9; })())].join("|"));
print("identifier-bitwise-readonly=" + [show(identifierReadonly &= 3),
      show(identifierReadonly),
      observe(() => (function(){ "use strict"; return identifierReadonly |= 1; })())].join("|"));
print("identifier-shift-readonly=" + [show(identifierReadonly >>>= 1),
      show(identifierReadonly),
      observe(() => (function(){ "use strict"; return identifierReadonly <<= 1; })())].join("|"));
print("identifier-power-readonly=" + [show(identifierReadonly **= 3),
      show(identifierReadonly),
      observe(() => (function(){ "use strict"; return identifierReadonly **= 3; })())].join("|"));
log = "";
print("identifier-missing=" + observe(() => identifierMissing += rhsExpr()) + "|" + log);
log = "";
print("identifier-bitwise-missing=" + observe(() => identifierMissingBits &= rhsExpr()) + "|" + log);
log = "";
print("identifier-shift-missing=" + observe(() => identifierMissingShift >>= rhsExpr()) + "|" + log);
log = "";
print("identifier-power-missing=" + observe(() => identifierMissingPower **= rhsExpr()) + "|" + log);
log = "";
print("identifier-getter-throw=" + observe(() => identifierThrowing ||= rhsExpr()) + "|" + log);

log = "";
print("null-logical=" + observe(() => null[compoundKeyExpr()] ||= rhsExpr()) + "|" + log);

log = "";
print("null-set=" + observe(() => null[keyExpr()] = rhsExpr()) + "|" + log);
log = "";
print("null-delete=" + observe(() => delete null[keyExpr()]) + "|" + log);

const originalPrototype = Function.prototype;
const sloppyReadonly = Function.prototype = 1;
print("readonly-sloppy=" + [show(sloppyReadonly),
      show(Function.prototype === originalPrototype), show(delete Function.prototype)].join("|"));
print("readonly-strict-set=" + observe(() => (function(){ "use strict"; return Function.prototype = 1; })()));
print("readonly-strict-delete=" + observe(() => (function(){ "use strict"; return delete Function.prototype; })()));
log = "";
print("readonly-logical-skip=" + observe(() =>
      (function(){ "use strict"; return Function.prototype ||= rhsExpr(); })() === Function.prototype) + "|" + log);
log = "";
print("readonly-logical-set=" + observe(() =>
      (function(){ "use strict"; return Function.prototype &&= rhsExpr(); })()) + "|" + log);
log = "";
print("readonly-bitwise-set=" + observe(() =>
      (function(){ "use strict"; return Function.prototype &= rhsExpr(); })()) + "|" + log);
log = "";
print("readonly-shift-set=" + observe(() =>
      (function(){ "use strict"; return Function.prototype <<= rhsExpr(); })()) + "|" + log);
log = "";
print("readonly-power-set=" + observe(() =>
      (function(){ "use strict"; return Function.prototype **= rhsExpr(); })()) + "|" + log);
log = "";
print("readonly-power-mixed=" + observe(() =>
      (function(){ "use strict"; return Function.prototype **= bigintPowerRhs(); })()) + "|" + log);
log = "";
print("readonly-power-symbol=" + observe(() =>
      (function(){ "use strict"; return Function.prototype **= symbolPowerRhs(); })()) + "|" + log);
log = "";
print("readonly-power-rhs-throw=" + observe(() =>
      (function(){ "use strict"; return Function.prototype **= throwingRhs(); })()) + "|" + log);
print("native-setter=" + observe(() => Function.prototype.caller = 1));

Object.defineProperty(target, "getterOnly", {
    get() { return 1; }, configurable: true
});
const getterOnlySloppy = target.getterOnly = 2;
print("getter-only=" + [show(getterOnlySloppy), show(target.getterOnly),
      observe(() => (function(){ "use strict"; return target.getterOnly = 3; })()),
      show(target.getterOnly)].join("|"));

target.named = function() {};
print("property-name=" + show(target.named.name));

const symbol = Symbol("same");
const otherSymbol = Symbol("same");
target[symbol] = 1;
target[otherSymbol] = 2;
const symbolResult = target[symbol] = 5;
const symbolCompound = target[symbol] += 2;
print("symbol=" + [show(symbolResult), show(symbolCompound),
      show(target[symbol]), show(target[otherSymbol])].join("|"));

let getterCalls = 0;
Object.defineProperty(target, "deletableGetter", {
    get() { getterCalls++; return 1; }, configurable: true
});
print("delete-getter=" + [show(delete target.deletableGetter), show(getterCalls),
      show(Object.hasOwn(target, "deletableGetter"))].join("|"));

const sealed = {};
Object.preventExtensions(sealed);
print("nonextensible=" + [show(sealed.x = 4), show(Object.hasOwn(sealed, "x")),
      observe(() => (function(){ "use strict"; return sealed.x = 5; })())].join("|"));

print("primitive-set=" + [show((1).x = 4),
      observe(() => (function(){ "use strict"; return (1).x = 5; })()),
      show("abc".length = 4),
      observe(() => (function(){ "use strict"; return "abc".length = 5; })())].join("|"));
print("primitive-delete=" + [show(delete "abc"[0]), show(delete "abc"[9]),
      observe(() => (function(){ "use strict"; return delete "abc"[0]; })()),
      show(delete (1).x)].join("|"));
