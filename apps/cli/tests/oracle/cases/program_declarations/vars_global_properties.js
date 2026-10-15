// Program `var` declarations against existing global properties, run as a
// sequence of global scripts in one realm (`// ---` separates scripts).
// case: global property and preflight matrix
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
var __qjo_var_new
// ---
__qjo_d('__qjo_var_new')
// ---
delete __qjo_var_new
// ---
Object.defineProperty(globalThis, '__qjo_var_config', {
  value: 10, writable: true, enumerable: false, configurable: true
});
// ---
var __qjo_var_config
// ---
__qjo_d('__qjo_var_config')
// ---
var __qjo_var_config=20
// ---
__qjo_d('__qjo_var_config')
// ---
__qjo_var_config
// ---
delete __qjo_var_config
// ---
__qjo_d('__qjo_var_config')
// ---
Object.defineProperty(globalThis, '__qjo_var_fixed', {
  value: 7, writable: false, enumerable: false, configurable: false
});
// ---
var __qjo_var_fixed=8
// ---
__qjo_d('__qjo_var_fixed')
// ---
'use strict';var __qjo_var_fixed=9
// ---
__qjo_d('__qjo_var_fixed')
// ---
Function.varGets = 0;
Function.varSets = 0;
Function.varSeen = 'none';
Object.defineProperty(globalThis, '__qjo_var_accessor', {
  get: function () { Function.varGets++; return 4; },
  set: function (value) { Function.varSets++; Function.varSeen = value; },
  enumerable: false,
  configurable: true
});
// ---
var __qjo_var_accessor
// ---
Function.varGets + '|' + Function.varSets + '|' + Function.varSeen + '|' + __qjo_d('__qjo_var_accessor')
// ---
var __qjo_var_accessor=12
// ---
Function.varGets + '|' + Function.varSets + '|' + Function.varSeen + '|' + __qjo_d('__qjo_var_accessor')
// ---
globalThis.__qjo_var_atomic_marker=0;let __qjo_var_existing_lex=9
// ---
__qjo_var_atomic_marker=1;var __qjo_var_fresh_before=(__qjo_var_atomic_marker=2),__qjo_var_existing_lex=(__qjo_var_atomic_marker=3),__qjo_var_fresh_after=(__qjo_var_atomic_marker=4)
// ---
__qjo_var_atomic_marker+'|'+typeof __qjo_var_fresh_before+'|'+typeof __qjo_var_fresh_after+'|'+__qjo_var_existing_lex
// ---
__qjo_d('__qjo_var_fresh_before') + '|' + __qjo_d('__qjo_var_fresh_after')
// ---
globalThis.__qjo_var_sealed_marker = 0;
globalThis.__qjo_var_sealed_existing = 5;
Object.preventExtensions(globalThis);
// ---
__qjo_var_sealed_marker=1;var __qjo_var_sealed_existing=6,__qjo_var_sealed_missing=7
// ---
__qjo_var_sealed_marker+'|'+__qjo_var_sealed_existing+'|'+typeof __qjo_var_sealed_missing
// ---
__qjo_d('__qjo_var_sealed_missing')
// ---
var __qjo_var_sealed_existing=8
// ---
__qjo_d('__qjo_var_sealed_existing')
