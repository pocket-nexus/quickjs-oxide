// mode: print
function bit(value) { return value ? 1 : 0; }
function show(value) {
    if (value === undefined) return "undefined";
    if (typeof value === "boolean" || typeof value === "number") return String(value);
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
// case: basic apply
const fp = Function.prototype;
const keys = Reflect.ownKeys(fp).slice(0, 6).map(String).join(",");
console.log("keys=" + keys);
const applyDescriptor = Object.getOwnPropertyDescriptor(fp, "apply");
const apply = applyDescriptor.value;
const lengthDescriptor = Object.getOwnPropertyDescriptor(apply, "length");
const nameDescriptor = Object.getOwnPropertyDescriptor(apply, "name");
console.log("apply-desc=" + bit(applyDescriptor.writable) + "," +
            bit(applyDescriptor.enumerable) + "," + bit(applyDescriptor.configurable) +
            "|length:" + lengthDescriptor.value + "|" + bit(lengthDescriptor.writable) + "," +
            bit(lengthDescriptor.enumerable) + "," + bit(lengthDescriptor.configurable) +
            "|name:" + nameDescriptor.value + "|" + bit(nameDescriptor.writable) + "," +
            bit(nameDescriptor.enumerable) + "," + bit(nameDescriptor.configurable));

function zeroTarget(a) {
    "use strict";
    return typeof this === "undefined" && typeof a === "undefined";
}
function strictThis() { "use strict"; return this; }
function forwarded(a, b) { "use strict"; return this + a * 10 + b; }
console.log("zero=" + observe(() => Reflect.apply(apply, zeroTarget, [])));
console.log("null-array=" + observe(() => Reflect.apply(apply, strictThis, [17, null])));
console.log("undefined-array=" + observe(() => Reflect.apply(apply, strictThis, [18, undefined])));
console.log("forward=" + observe(() => Reflect.apply(apply, forwarded,
            [100, { 0: 2, 1: 3, length: 2 }])));
// case: early apply errors
const apply = Function.prototype.apply;
const poison = {};
Object.defineProperty(poison, "length", {
    get() { throw "length throw"; },
    configurable: true,
});
console.log("noncallable-first=" + observe(() => Reflect.apply(apply, {}, [undefined, poison])));
console.log("primitive-array=" + observe(() => Reflect.apply(apply, function () {}, [undefined, 1])));
console.log("length-throw=" + observe(() => Reflect.apply(apply, function () {}, [undefined, poison])));
// case: apply ToPrimitive
const apply = Function.prototype.apply;
function sum(a, b) { return (a == null ? 0 : a) + (b == null ? 0 : b); }
function arrayLike(length) { return { 0: 10, 1: 20, length }; }

let hintSeen = "none";
const numberHint = {
    [Symbol.toPrimitive](hint) { hintSeen = hint; return 2; }
};
console.log("primitive-hint=" + observe(() => Reflect.apply(apply, sum,
            [undefined, arrayLike(numberHint)])) + "|hint:" + hintSeen);

const objectResult = { [Symbol.toPrimitive]() { return {}; } };
console.log("primitive-object=" + observe(() => Reflect.apply(apply, sum,
            [undefined, arrayLike(objectResult)])));

const noncallable = { [Symbol.toPrimitive]: 1 };
console.log("primitive-noncallable=" + observe(() => Reflect.apply(apply, sum,
            [undefined, arrayLike(noncallable)])));

const throwing = { [Symbol.toPrimitive]() { throw "primitive throw"; } };
console.log("primitive-throw=" + observe(() => Reflect.apply(apply, sum,
            [undefined, arrayLike(throwing)])));
// case: apply ordinary conversion
const apply = Function.prototype.apply;
function sum(a, b) { return (a == null ? 0 : a) + (b == null ? 0 : b); }
let order = 0;
const lengthObject = {
    valueOf() { order = order * 10 + 1; return lengthObject; },
    toString() { order = order * 10 + 2; return "2"; },
};
const arrayLike = { 0: 10, 1: 20, length: lengthObject };
console.log("ordinary-order=" + observe(() => Reflect.apply(apply, sum,
            [undefined, arrayLike])) + "|order:" + order);
// case: apply ToLength
const apply = Function.prototype.apply;
function sum(a, b, c) {
    return (a == null ? 0 : a) + (b == null ? 0 : b) + (c == null ? 0 : c);
}
function run(length) {
    return observe(() => Reflect.apply(apply, sum,
        [undefined, { 0: 10, 1: 20, 2: 30, length }]));
}
console.log("length-nan=" + run(NaN));
console.log("length-negative=" + run(-2));
console.log("length-fraction=" + run(2.9));
console.log("length-infinity=" + run(Infinity));
console.log("length-string=" + run("2.9"));
console.log("length-bigint=" + run(1n));
// case: apply indexed Get
const apply = Function.prototype.apply;
let order = 0;
const ordered = { length: 3 };
for (const [index, digit] of [[0, 1], [1, 2], [2, 3]]) {
    Object.defineProperty(ordered, index, {
        get() { order = order * 10 + digit; return order; },
        configurable: true,
    });
}
function encode(a, b, c) { return a * 10000 + b * 100 + c; }
console.log("index-order=" + observe(() => Reflect.apply(apply, encode,
            [undefined, ordered])) + "|order:" + order);

function holeTarget(a, b, c) {
    return a * 100 + (typeof b === "undefined" ? 10 : 0) + c;
}
console.log("index-hole=" + observe(() => Reflect.apply(apply, holeTarget,
            [undefined, { 0: 1, 2: 3, length: 3 }])));

const inherited = Object.create({ 1: 2 });
inherited[0] = 1;
inherited.length = 2;
console.log("index-inherited=" + observe(() => Reflect.apply(apply,
            function (a, b) { return a * 10 + b; }, [undefined, inherited])));

const accessor = { length: 1 };
Object.defineProperty(accessor, 0, { get() { return 7; }, configurable: true });
console.log("index-accessor=" + observe(() => Reflect.apply(apply,
            function (a) { return a; }, [undefined, accessor])));

const throwing = { length: 2 };
Object.defineProperty(throwing, 0, { get() { throw "index throw"; }, configurable: true });
Object.defineProperty(throwing, 1, { get() { throw "late index read"; }, configurable: true });
console.log("index-throw=" + observe(() => Reflect.apply(apply,
            function () {}, [undefined, throwing])));
// case: apply argument boundary
const apply = Function.prototype.apply;
function target() { return 1; }
console.log("boundary-65534=" + observe(() => Reflect.apply(apply, target,
            [undefined, { length: 65534 }])));
console.log("boundary-65535=" + observe(() => Reflect.apply(apply, target,
            [undefined, { length: 65535 }])));
