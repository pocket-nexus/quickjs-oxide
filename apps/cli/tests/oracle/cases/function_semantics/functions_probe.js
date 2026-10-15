// mode: print
var f = (0, function(a, b) {});
function bits(d) {
    return Number(d.writable) + "," + Number(d.enumerable) + "," + Number(d.configurable);
}
var nameDesc = Object.getOwnPropertyDescriptor(f, "name");
var lengthDesc = Object.getOwnPropertyDescriptor(f, "length");
print("name=" + nameDesc.value + "|" + bits(nameDesc));
print("length=" + lengthDesc.value + "|" + bits(lengthDesc));
print("lazy-before-get=" + Object.prototype.hasOwnProperty.call(f, "prototype") + "|" + Reflect.ownKeys(f).join(","));
var prototypeDesc = Object.getOwnPropertyDescriptor(f, "prototype");
print("prototype=" + bits(prototypeDesc));
print("keys=" + Reflect.ownKeys(f).join(","));
print("function-prototype-prefix=" + Reflect.ownKeys(Function.prototype).slice(0, 2).join(","));
print("inferred-direct=" + (function(){ var inferred = function(){}; return inferred; })().name);
print("inferred-parenthesized=" + (function(){ var inferred = (((function(){}))); return inferred; })().name);
print("inferred-escaped=" + (function(){ var \u0066 = function(){}; return f; })().name);
print("not-inferred-comma=" + (function(){ var inferred = (0, function(){}); return inferred; })().name);
print("not-inferred-conditional=" + (function(){ var inferred = true ? function(){} : function(){}; return inferred; })().name);
print("not-inferred-logical=" + (function(){ var inferred = 0 || function(){}; return inferred; })().name);
print("named-recursion=" + (function factorial(n){ return n <= 1 ? 1 : n * factorial(n - 1); })(5));
var namedIdentity = function self(){ return self; };
print("named-self-identity=" + (namedIdentity() === namedIdentity));
var namedDescendant = function self(){ return function(){ return self; }; };
print("named-descendant-capture=" + (namedDescendant()() === namedDescendant));
print("named-parameter-shadow=" + (function self(self){ return self; })(7));
print("named-var-shadow=" + (function self(){ var self = 8; return self; })());
var namedSloppy = function self(){ var before = self; var assigned = (self = 9); return (self === before) + "|" + assigned; };
print("named-sloppy-self-assignment=" + namedSloppy());
try { (function self(){ "use strict"; self = 9; })(); } catch (e) { print("named-strict-self-assignment=" + e.name + "|" + e.message); }
var namedMeta = function explicit(a, b){};
var namedNameDesc = Object.getOwnPropertyDescriptor(namedMeta, "name");
var namedLengthDesc = Object.getOwnPropertyDescriptor(namedMeta, "length");
print("named-name=" + namedNameDesc.value + "|" + bits(namedNameDesc));
print("named-length=" + namedLengthDesc.value + "|" + bits(namedLengthDesc));
print("named-before-prototype=" + Object.prototype.hasOwnProperty.call(namedMeta, "prototype") + "|" + Reflect.ownKeys(namedMeta).join(","));
var namedPrototypeDesc = Object.getOwnPropertyDescriptor(namedMeta, "prototype");
print("named-prototype=" + bits(namedPrototypeDesc) + "|" + (namedPrototypeDesc.value.constructor === namedMeta));
print("named-keys=" + Reflect.ownKeys(namedMeta).join(","));
print("constructor=" + (prototypeDesc.value.constructor === f));
print("prototype-chain=" + (Object.getPrototypeOf(prototypeDesc.value) === Object.prototype));
print("lazy-stable=" + (f.prototype === prototypeDesc.value) + "|" + (f.prototype.constructor === f) + "|" + (Object.getPrototypeOf(f.prototype) === Object.prototype));
var compatible = function(){};
Object.defineProperty(compatible, "prototype", {});
print("lazy-compatible-define=" + (compatible.prototype.constructor === compatible));
var incompatible = function(){}, rejected = false;
try { Object.defineProperty(incompatible, "prototype", { configurable: true }); } catch (e) { rejected = e instanceof TypeError; }
print("lazy-incompatible-define=" + rejected);
var explicitF = function(){ return new.target }, explicitG = function(){};
print("explicit-new-target=" + (Reflect.construct(explicitF, [], explicitG) === explicitG));
Object.defineProperty(explicitG, "prototype", { value: null });
print("new-target-prototype-fallback=" + (Object.getPrototypeOf(Reflect.construct(function(){}, [], explicitG)) === Object.prototype));
var baseInstance = new f();
print("construct-prototype=" + (Object.getPrototypeOf(baseInstance) === f.prototype));
print("construct-result-primitive-fallback=" + (function(){ var F=function(){return 1}; return typeof new F() === "object"; })());
print("construct-object-override=" + (function(){ var marker=function(){}; var F=function(){return marker}; return new F() === marker; })());
print("construct-new-target=" + (function(){ var F=function(){return new.target}; return new F() === F; })());
print("call-new-target=" + (function(){ var F=function(){return new.target}; return typeof F() === "undefined"; })());
var fp = Function.prototype;
var fpLengthDesc = Object.getOwnPropertyDescriptor(fp, "length");
var fpNameDesc = Object.getOwnPropertyDescriptor(fp, "name");
print("function-prototype=" + String(fp()) + "|" + bits(fpLengthDesc) + "|" + bits(fpNameDesc) + "|" + (Object.getOwnPropertyDescriptor(fp, "prototype") === undefined));
Object.defineProperty(fp, "length", { value: 99 });
print("native-length-independent=" + fp.length + "|" + (fp() === undefined));
try { Reflect.construct(fp, []); } catch (e) { print("function-prototype-construct-error=" + e.name + "|" + e.message); }
