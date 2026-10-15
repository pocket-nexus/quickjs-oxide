// Program function declarations against existing global properties, run as a
// sequence of global scripts in one realm (`// ---` separates scripts).
// case: global property matrix
function __qjo_d(name) {
  var value = Object.getOwnPropertyDescriptor(globalThis, name);
  if (value === undefined)
    return 'missing';
  if ('value' in value)
    return 'data|' + typeof value.value + '|' + String(value.value) + '|' +
           value.writable + '|' + value.enumerable + '|' + value.configurable;
  return 'accessor|' + typeof value.get + '|' + typeof value.set + '|' +
         value.enumerable + '|' + value.configurable;
}
// ---
typeof __qjo_new_function+'|'+__qjo_new_function();function __qjo_new_function(){return 1}
// ---
__qjo_d('__qjo_new_function')
// ---
__qjo_new_function.name
// ---
parseInt();function parseInt(){return 19}
// ---
__qjo_d('parseInt')
// ---
Object.defineProperty(globalThis, '__qjo_config_function', {
  value: 3, writable: false, enumerable: false, configurable: true
});
// ---
__qjo_config_function();function __qjo_config_function(){return 10}
// ---
__qjo_d('__qjo_config_function')
// ---
Function.functionGets = 0;
Function.functionSets = 0;
Object.defineProperty(globalThis, '__qjo_accessor_function', {
  get: function () { Function.functionGets++; return 3; },
  set: function () { Function.functionSets++; },
  enumerable: false,
  configurable: true
});
// ---
__qjo_accessor_function();function __qjo_accessor_function(){return 11}
// ---
Function.functionGets + '|' + Function.functionSets + '|' + __qjo_d('__qjo_accessor_function')
// ---
Object.defineProperty(globalThis, '__qjo_fixed_valid_function', {
  value: 3, writable: true, enumerable: true, configurable: false
});
// ---
__qjo_fixed_valid_function();function __qjo_fixed_valid_function(){return 12}
// ---
__qjo_d('__qjo_fixed_valid_function')
// ---
Object.defineProperty(globalThis, '__qjo_fixed_readonly_function', {
  value: 3, writable: false, enumerable: true, configurable: false
});
// ---
function __qjo_fixed_readonly_function(){return 13}
// ---
__qjo_d('__qjo_fixed_readonly_function')
// ---
Object.defineProperty(globalThis, '__qjo_fixed_hidden_function', {
  value: 3, writable: true, enumerable: false, configurable: false
});
// ---
function __qjo_fixed_hidden_function(){return 14}
// ---
__qjo_d('__qjo_fixed_hidden_function')
// ---
Function.fixedAccessorGets = 0;
Function.fixedAccessorSets = 0;
Object.defineProperty(globalThis, '__qjo_fixed_accessor_function', {
  get: function () { Function.fixedAccessorGets++; return 3; },
  set: function () { Function.fixedAccessorSets++; },
  enumerable: true,
  configurable: false
});
// ---
function __qjo_fixed_accessor_function(){return 15}
// ---
Function.fixedAccessorGets + '|' + Function.fixedAccessorSets + '|' + __qjo_d('__qjo_fixed_accessor_function')
// ---
let __qjo_function_priority=1
// ---
Object.defineProperty(globalThis, '__qjo_function_priority', {
  value: 3, writable: false, enumerable: true, configurable: false
});
// ---
function __qjo_function_priority(){return 18}
// ---
__qjo_d('__qjo_function_priority')
// ---
globalThis.__qjo_function_marker = 0;
Object.defineProperty(globalThis, '__qjo_blocked_function', {
  value: 3, writable: false, enumerable: true, configurable: false
});
// ---
__qjo_function_marker=1;function __qjo_fresh_function(){return 1}function __qjo_blocked_function(){return 2}
// ---
__qjo_function_marker+'|'+typeof __qjo_fresh_function
// ---
__qjo_d('__qjo_fresh_function') + '|' + __qjo_d('__qjo_blocked_function')
// ---
Object.defineProperty(globalThis, '__qjo_sealed_existing_function', {
  value: 3, writable: false, enumerable: false, configurable: true
});
globalThis.__qjo_sealed_function_marker = 0;
Object.preventExtensions(globalThis);
// ---
__qjo_sealed_existing_function();function __qjo_sealed_existing_function(){return 16}
// ---
__qjo_d('__qjo_sealed_existing_function')
// ---
__qjo_sealed_function_marker=1;function __qjo_sealed_missing_function(){return 17}
// ---
__qjo_sealed_function_marker+'|'+typeof __qjo_sealed_missing_function
// ---
__qjo_d('__qjo_sealed_missing_function')
