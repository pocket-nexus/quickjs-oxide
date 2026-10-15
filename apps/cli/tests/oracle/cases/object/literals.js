// mode: completion
// case: data, computed, proto and spread lowering
(function(){
  var out = [];
  function add(name, value) { out.push(name + "=" + value); }

  var value = 3;
  var fixed = {2:"two", a:1, value, if:4, "a":5};
  var desc = Object.getOwnPropertyDescriptor(fixed, "a");
  add("fixed", fixed[2] + "|" + fixed.value + "|" + fixed.a + "|" +
      desc.writable + "|" + desc.enumerable + "|" + desc.configurable + "|" +
      Object.keys(fixed).join(","));

  var log = "";
  var key = {};
  Object.defineProperty(key, Symbol.toPrimitive, {
    value: function(hint) { log += "key(" + hint + ")"; return "computed"; }
  });
  function rhs() { log += "rhs"; return 7; }
  var computed = {[key]: rhs()};
  add("computed-order", log + "|" + computed.computed);

  var computedThrowLog = "";
  var throwingKey = {};
  Object.defineProperty(throwingKey, Symbol.toPrimitive, {
    value: function(){ computedThrowLog += "key"; throw "key-error"; }
  });
  try { ({[throwingKey]:(computedThrowLog += "rhs")}); }
  catch (error) { computedThrowLog += "|" + error; }
  add("computed-throw", computedThrowLog);

  var described = Symbol("named");
  var missing = Symbol();
  var empty = Symbol("");
  var named = {
    plain: function(){},
    [described]: function(){},
    [missing]: function(){},
    [empty]: function(){}
  };
  add("names", named.plain.name + "|" + named[described].name + "|" +
      named[missing].name + "|" + named[empty].name);

  var proto = {marker:9};
  var protoObject = {__proto__: proto};
  var protoNull = {__proto__: null};
  var protoPrimitive = {__proto__: 1};
  var __proto__ = 11;
  var protoShorthand = {__proto__};
  var protoComputed = {["__proto__"]: 12};
  add("proto", protoObject.marker + "|" +
      Object.hasOwn(protoObject, "__proto__") + "|" +
      (Object.getPrototypeOf(protoNull) === null) + "|" +
      (Object.getPrototypeOf(protoPrimitive) === Object.prototype) + "|" +
      protoShorthand.__proto__ + "|" + protoComputed.__proto__);

  var symbol = Symbol("spread");
  var sourceProto = {later:"from-prototype"};
  var source = Object.create(sourceProto);
  Object.defineProperty(source, "first", {
    enumerable: true,
    configurable: true,
    get: function(){ log += "get-first"; delete source.later; return "first"; }
  });
  source.later = "own-later";
  source[symbol] = "symbol";
  Object.defineProperty(source, "hidden", {value:"hidden", enumerable:false});
  log = "";
  var spread = {before:0, ...source, after:1};
  add("spread", spread.first + "|" + spread.later + "|" + spread[symbol] + "|" +
      Object.hasOwn(spread, "hidden") + "|" + log + "|" +
      Object.keys(spread).join(","));

  var setterHits = 0;
  Object.defineProperty(Object.prototype, "throughSetter", {
    configurable: true,
    set: function(){ setterHits++; }
  });
  var definedSpread = {...{throughSetter:42}};
  delete Object.prototype.throughSetter;
  add("spread-define", setterHits + "|" + Object.hasOwn(definedSpread, "throughSetter") +
      "|" + definedSpread.throughSetter);

  var spreadThrowLog = "";
  var throwingSource = {};
  Object.defineProperty(throwingSource, "a", {
    enumerable: true,
    get: function(){ spreadThrowLog += "a"; return 1; }
  });
  Object.defineProperty(throwingSource, "b", {
    enumerable: true,
    get: function(){ spreadThrowLog += "b"; throw "spread-error"; }
  });
  Object.defineProperty(throwingSource, "c", {
    enumerable: true,
    get: function(){ spreadThrowLog += "c"; return 3; }
  });
  try { ({...throwingSource}); }
  catch (error) { spreadThrowLog += "|" + error; }
  add("spread-throw", spreadThrowLog);

  var primitiveSpread = {..."ab", ...1, ...null, ...undefined};
  add("primitive-spread", Object.keys(primitiveSpread).join(","));

  return out.join("\n");
})()
// case: duplicate __proto__ is an early error
({__proto__:1,__proto__:2})
