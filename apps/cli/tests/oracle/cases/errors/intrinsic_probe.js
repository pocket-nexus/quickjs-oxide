// mode: print
function bits(d) {
    return Number(d.writable) + "," + Number(d.enumerable) + "," + Number(d.configurable);
}
var ep = Error.prototype, tp = TypeError.prototype;
print("error-keys=" + Reflect.ownKeys(Error).join(","));
print("type-error-keys=" + Reflect.ownKeys(TypeError).join(","));
print("error-prototype-keys=" + Reflect.ownKeys(ep).join(","));
print("type-error-prototype-keys=" + Reflect.ownKeys(tp).join(","));
print("constructor-links=" + bits(Object.getOwnPropertyDescriptor(Error, "prototype")) + "|" + bits(Object.getOwnPropertyDescriptor(ep, "constructor")));
print("chains=" + (Object.getPrototypeOf(Error) === Function.prototype) + "|" + (Object.getPrototypeOf(TypeError) === Error) + "|" + (Object.getPrototypeOf(ep) === Object.prototype) + "|" + (Object.getPrototypeOf(tp) === ep));
print("metadata=" + Error.length + "|" + TypeError.length + "|" + Error.name + "|" + TypeError.name);
var empty = Error(), numbered = Error(42), typed = new TypeError("x");
var caused = Error(undefined, { cause: undefined });
print("instances=" + (Object.getPrototypeOf(empty) === ep) + "|" + (Object.getPrototypeOf(typed) === tp) + "|" + Object.hasOwn(empty, "message") + "|" + Error.isError(empty));
print("messages=" + Object.hasOwn(empty, "message") + "|" + numbered.message + "|" + Object.hasOwn(caused, "cause") + "|" + String(caused.cause));
print("cause-bits=" + bits(Object.getOwnPropertyDescriptor(caused, "cause")));
print("to-string=" + ep.toString.call(empty) + "|" + ep.toString.call(numbered) + "|" + ep.toString.call(typed) + "|" + ep.toString.call({ name: "", message: "m" }));
print("is-error=" + Error.isError(empty) + "|" + Error.isError(ep) + "|" + Error.isError(Object.create(ep)));
print("method-bits=" + bits(Object.getOwnPropertyDescriptor(Error, "isError")) + "|" + bits(Object.getOwnPropertyDescriptor(ep, "toString")));
print("object-prefix=" + Reflect.ownKeys(Object.prototype).join(",") + "|" + bits(Object.getOwnPropertyDescriptor(Object.prototype, "toString")) + "|" + bits(Object.getOwnPropertyDescriptor(Object.prototype, "valueOf")));
print("object-message=" + Error({}).message + "|" + Error({ [Symbol.toStringTag]: "X" }).message);
print("object-tags=" + Object.prototype.toString.call(null) + "|" + Object.prototype.toString.call(undefined) + "|" + Object.prototype.toString.call({}) + "|" + Object.prototype.toString.call(function(){}) + "|" + Object.prototype.toString.call(Error()));
try { Error(Symbol()); } catch (e) { print("symbol-message=" + e.name + "|" + e.message); }
