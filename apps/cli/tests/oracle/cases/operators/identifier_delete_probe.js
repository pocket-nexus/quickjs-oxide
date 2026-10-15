// mode: print
function ownGlobal(name) {
    return Object.prototype.hasOwnProperty.call(globalThis, name);
}

print("missing=" + delete __qjo_delete_missing + "|" +
      typeof __qjo_delete_missing);

__qjo_delete_assigned = 7;
print("assigned=" + delete __qjo_delete_assigned + "|" +
      typeof __qjo_delete_assigned + "|" + ownGlobal("__qjo_delete_assigned"));

print("intrinsics=" + delete undefined + "|" + delete NaN + "|" +
      delete Infinity + "|" + typeof undefined + "|" + typeof NaN + "|" +
      typeof Infinity);

Object.defineProperty(globalThis, "__qjo_delete_config_data", {
    value: 11, writable: true, enumerable: true, configurable: true
});
print("config-data=" + delete __qjo_delete_config_data + "|" +
      ownGlobal("__qjo_delete_config_data"));

Object.defineProperty(globalThis, "__qjo_delete_fixed_data", {
    value: 13, writable: true, enumerable: true, configurable: false
});
print("fixed-data=" + delete __qjo_delete_fixed_data + "|" +
      ownGlobal("__qjo_delete_fixed_data") + "|" + __qjo_delete_fixed_data);

var __qjo_delete_gets = 0;
var __qjo_delete_sets = 0;
function deleteGetter() { __qjo_delete_gets++; return 19; }
function deleteSetter(value) { __qjo_delete_sets++; }

Object.defineProperty(globalThis, "__qjo_delete_config_accessor", {
    get: deleteGetter, set: deleteSetter, enumerable: true, configurable: true
});
print("config-accessor=" + delete __qjo_delete_config_accessor + "|" +
      ownGlobal("__qjo_delete_config_accessor") + "|" +
      __qjo_delete_gets + "|" + __qjo_delete_sets);

Object.defineProperty(globalThis, "__qjo_delete_fixed_accessor", {
    get: deleteGetter, set: deleteSetter, enumerable: true, configurable: false
});
print("fixed-accessor=" + delete __qjo_delete_fixed_accessor + "|" +
      ownGlobal("__qjo_delete_fixed_accessor") + "|" +
      __qjo_delete_gets + "|" + __qjo_delete_sets);

var globalPrototype = Object.getPrototypeOf(globalThis);
Object.defineProperty(globalPrototype, "__qjo_delete_inherited", {
    value: 17, writable: true, enumerable: true, configurable: true
});
print("inherited=" + delete __qjo_delete_inherited + "|" +
      ownGlobal("__qjo_delete_inherited") + "|" + __qjo_delete_inherited);
delete globalPrototype.__qjo_delete_inherited;

print("scopes=" + (function(argument) {
    var local = 2;
    var closure = function() { return delete local; };
    var self = (function named() { return delete named; });
    return (delete argument) + "|" + (delete local) + "|" + closure() + "|" +
           self() + "|" + (delete arguments);
})(1));

print("references=" + (function() {
    var value = 1;
    var parenthesized = delete (value);
    var comma = delete (0, value);
    var assigned = delete (value = 7);
    var afterAssignment = value;
    var post = delete value++;
    var afterPost = value;
    var prefix = delete ++value;
    return parenthesized + "|" + comma + "|" + assigned + "|" +
           afterAssignment + "|" + post + "|" + afterPost + "|" + prefix +
           "|" + value;
})());

print("typeof-basic=" + typeof delete __qjo_delete_type_missing + "|" +
      delete typeof __qjo_delete_type_missing);

Object.defineProperty(globalThis, "__qjo_delete_typeof", {
    get: deleteGetter, set: deleteSetter, enumerable: true, configurable: true
});
print("delete-typeof=" + delete typeof __qjo_delete_typeof + "|" +
      __qjo_delete_gets + "|" + ownGlobal("__qjo_delete_typeof"));

Object.defineProperty(globalThis, "__qjo_typeof_delete", {
    get: deleteGetter, set: deleteSetter, enumerable: true, configurable: true
});
print("typeof-delete=" + typeof delete __qjo_typeof_delete + "|" +
      __qjo_delete_gets + "|" + ownGlobal("__qjo_typeof_delete"));

print("Function-local=" +
      Function("var local = 1; return delete local + '|' + local")());
print("Function-argument=" +
      Function("argument", "return delete argument + '|' + argument")(2));
print("Function-missing=" + Function("return delete __qjo_function_missing")());
