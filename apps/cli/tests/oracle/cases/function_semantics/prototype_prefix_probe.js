// mode: print
function bit(value) { return value ? 1 : 0; }
function show(value) {
    if (value === undefined) return "undefined";
    if (typeof value === "boolean") return String(value);
    if (typeof value === "number") return String(value);
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

const fp = Function.prototype;
const prefix = Reflect.ownKeys(fp).slice(0, 5).map(String).join(",");
console.log("keys=" + prefix);

const caller = Object.getOwnPropertyDescriptor(fp, "caller");
const args = Object.getOwnPropertyDescriptor(fp, "arguments");
const shared = caller.get === caller.set &&
               caller.get === args.get &&
               caller.get === args.set;
console.log("legacy-desc=caller:" + bit(caller.enumerable) + "," + bit(caller.configurable) +
            "|arguments:" + bit(args.enumerable) + "," + bit(args.configurable) +
            "|shared:" + bit(shared));

const thrower = caller.get;
const throwerLength = Object.getOwnPropertyDescriptor(thrower, "length");
const throwerName = Object.getOwnPropertyDescriptor(thrower, "name");
console.log("thrower=length:" + throwerLength.value + "|" +
            bit(throwerLength.writable) + "," + bit(throwerLength.enumerable) + "," + bit(throwerLength.configurable) +
            "|name:" + (throwerName.value === "" ? "<empty>" : throwerName.value) + "|" +
            bit(throwerName.writable) + "," + bit(throwerName.enumerable) + "," + bit(throwerName.configurable) +
            "|keys:" + Reflect.ownKeys(thrower).map(String).join(",") +
            "|extensible:" + bit(Object.isExtensible(thrower)) +
            "|frozen:" + bit(Object.isFrozen(thrower)));

console.log("legacy-sloppy=" + observe(() => caller.get.call(function sloppy() {})));
console.log("legacy-strict=" + observe(() => caller.get.call(function strict() { "use strict"; })));
console.log("legacy-set=" + observe(() => caller.set.call(function sloppy() {}, 1)));
const syntaxGenerator = function* syntaxGenerator() {};
const GeneratorFunction = Object.getPrototypeOf(syntaxGenerator).constructor;
const dynamicGenerator = new GeneratorFunction();
console.log("legacy-generator-syntax=" + observe(() => caller.get.call(syntaxGenerator)));
console.log("legacy-generator-dynamic=" + observe(() => caller.get.call(dynamicGenerator)));

const call = fp.call;
console.log("call-zero=" + observe(() => call.call(function one(a) { return a; })));
console.log("call-this=" + observe(() => call.call(function strictThis() { "use strict"; return this; }, 17)));
console.log("call-args=" + observe(() => call.call(function add(a, b) { return a + b; }, 99, 20, 22)));
console.log("call-throw=" + observe(() => call.call(function fail() { throw "boom"; })));
console.log("call-noncallable=" + observe(() => call.call(1)));

const hasInstanceDescriptor = Object.getOwnPropertyDescriptor(fp, Symbol.hasInstance);
console.log("has-instance-desc=" + bit(hasInstanceDescriptor.writable) + "," +
            bit(hasInstanceDescriptor.enumerable) + "," + bit(hasInstanceDescriptor.configurable));
const hasInstance = hasInstanceDescriptor.value;

const poisonTarget = {};
Object.defineProperty(poisonTarget, "prototype", {
    get() { throw "prototype read"; },
    configurable: true,
});
console.log("has-noncallable-short=" + observe(() => hasInstance.call(poisonTarget, {})));

Object.defineProperty(fp, "prototype", {
    get() { throw "prototype read"; },
    configurable: true,
});
console.log("has-primitive-short=" + observe(() => hasInstance.call(fp, 1)));
delete fp.prototype;

function InstanceTarget() {}
console.log("has-true=" + observe(() => hasInstance.call(InstanceTarget, Object.create(InstanceTarget.prototype))));
console.log("has-false=" + observe(() => hasInstance.call(InstanceTarget, {})));

function BadPrototype() {}
BadPrototype.prototype = 1;
console.log("has-bad-prototype=" + observe(() => hasInstance.call(BadPrototype, {})));
