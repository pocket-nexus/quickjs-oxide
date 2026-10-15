// mode: print
function bits(descriptor){return 'D:'+Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable)}
function isConstructor(value){try{Reflect.construct(function(){},[],value);return true}catch(_){return false}}
function meta(name){
  var descriptor=Object.getOwnPropertyDescriptor(Object,name),value=descriptor.value;
  return value.name+':'+value.length+':' +(Object.getPrototypeOf(value)===Function.prototype)+':' +
    (typeof value==='function')+':'+isConstructor(value)+':' +Object.getOwnPropertyNames(value).join(',')+':'+bits(descriptor);
}
var selected=['length','name','prototype','create','getPrototypeOf','setPrototypeOf','defineProperty',
  'defineProperties','getOwnPropertyNames','getOwnPropertySymbols','groupBy','keys','values','entries'];
print('prefix='+Reflect.ownKeys(Object).filter(function(key){return selected.indexOf(key)>=0}).join(','));
print('keys='+meta('keys'));
print('values='+meta('values'));
print('entries='+meta('entries'));
print('identity='+(Object.keys===Object.keys)+':' +(Object.values===Object.values)+':' +
  (Object.entries===Object.entries)+':' +(Object.keys!==Object.values)+':' +(Object.values!==Object.entries));
['keys','values','entries'].forEach(function(name){var fn=Object[name];
  print(name+'-props='+bits(Object.getOwnPropertyDescriptor(fn,'length'))+':' +bits(Object.getOwnPropertyDescriptor(fn,'name')));
});
