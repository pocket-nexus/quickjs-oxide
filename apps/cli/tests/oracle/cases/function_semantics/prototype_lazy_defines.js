// mode: print
// case: call configurable
const fp = Function.prototype;
const accepted = Reflect.defineProperty(fp, "call", { configurable: true });
const descriptor = Object.getOwnPropertyDescriptor(fp, "call");
const forwarded = Reflect.apply(descriptor.value, function one(a) { return a; }, [undefined, 7]);
console.log("call-configurable=accepted:" + (accepted ? 1 : 0) +
            "|kind:data|value:" + typeof descriptor.value + ":" + forwarded +
            "|desc:" + (descriptor.writable ? 1 : 0) + "," +
            (descriptor.enumerable ? 1 : 0) + "," + (descriptor.configurable ? 1 : 0));
// case: call enumerable
const fp = Function.prototype;
const accepted = Reflect.defineProperty(fp, "call", { enumerable: true });
const descriptor = Object.getOwnPropertyDescriptor(fp, "call");
const forwarded = Reflect.apply(descriptor.value, function one(a) { return a; }, [undefined, 7]);
console.log("call-enumerable=accepted:" + (accepted ? 1 : 0) +
            "|kind:data|value:" + typeof descriptor.value + ":" + forwarded +
            "|desc:" + (descriptor.writable ? 1 : 0) + "," +
            (descriptor.enumerable ? 1 : 0) + "," + (descriptor.configurable ? 1 : 0));
// case: call accessor
const fp = Function.prototype;
function replacementGetter() { return 73; }
const accepted = Reflect.defineProperty(fp, "call", { get: replacementGetter });
const descriptor = Object.getOwnPropertyDescriptor(fp, "call");
console.log("call-accessor=accepted:" + (accepted ? 1 : 0) +
            "|kind:accessor|get:" + typeof descriptor.get +
            "|set:" + String(descriptor.set) +
            "|desc:" + (descriptor.enumerable ? 1 : 0) + "," +
            (descriptor.configurable ? 1 : 0) + "|value:" + fp.call);
// case: @@hasInstance configurable rejection
const fp = Function.prototype;
const accepted = Reflect.defineProperty(fp, Symbol.hasInstance, { configurable: true });
const descriptor = Object.getOwnPropertyDescriptor(fp, Symbol.hasInstance);
console.log("has-instance-configurable=accepted:" + (accepted ? 1 : 0) +
            "|kind:data|value:" + typeof descriptor.value +
            "|desc:" + (descriptor.writable ? 1 : 0) + "," +
            (descriptor.enumerable ? 1 : 0) + "," + (descriptor.configurable ? 1 : 0));
