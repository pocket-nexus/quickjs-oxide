// mode: print
function bit(value){return value?1:0}
function bits(descriptor){return 'D:'+bit(descriptor.writable)+bit(descriptor.enumerable)+bit(descriptor.configurable)}
function isConstructor(value){try{Reflect.construct(function(){},[],value);return true}catch(_){return false}}
function meta(name){
  var descriptor=Object.getOwnPropertyDescriptor(Object,name),value=descriptor.value;
  return value.name+':'+value.length+':' +(Object.getPrototypeOf(value)===Function.prototype)+':' +
    (typeof value==='function')+':'+isConstructor(value)+':' +Object.getOwnPropertyNames(value).join(',')+':'+bits(descriptor);
}
var selected=['length','name','prototype','create','getPrototypeOf','setPrototypeOf','defineProperty',
  'defineProperties','getOwnPropertyNames','getOwnPropertySymbols','groupBy','keys','values','entries',
  'isExtensible','preventExtensions','getOwnPropertyDescriptor','getOwnPropertyDescriptors','is','assign'];
print('prefix='+Reflect.ownKeys(Object).filter(function(key){return selected.indexOf(key)>=0}).join(','));
print('assign='+meta('assign'));
print('identity='+(Object.assign===Object.assign));
var fn=Object.assign;
print('assign-props='+bits(Object.getOwnPropertyDescriptor(fn,'length'))+':' +bits(Object.getOwnPropertyDescriptor(fn,'name')));
