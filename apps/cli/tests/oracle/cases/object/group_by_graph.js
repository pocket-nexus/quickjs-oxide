// mode: print
function bits(descriptor) {
  return 'D:'+Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable);
}
function isConstructor(value) {
  try { Reflect.construct(function(){}, [], value); return true; }
  catch (_) { return false; }
}
function callableMeta(value) {
  return value.name+':'+value.length+':' +
    (Object.getPrototypeOf(value)===Function.prototype)+':' +
    (typeof value==='function')+':'+isConstructor(value)+':' +
    Object.getOwnPropertyNames(value).join(',');
}
var selected=['length','name','prototype','create','getPrototypeOf','setPrototypeOf',
  'defineProperty','defineProperties','getOwnPropertyNames','getOwnPropertySymbols','groupBy'];
print('ctor-prefix='+Reflect.ownKeys(Object).filter(function(key){
  return selected.indexOf(key)>=0;
}).map(String).join(','));
print('groupBy='+callableMeta(Object.groupBy)+':' +
  bits(Object.getOwnPropertyDescriptor(Object,'groupBy')));
print('length='+bits(Object.getOwnPropertyDescriptor(Object.groupBy,'length')));
print('name='+bits(Object.getOwnPropertyDescriptor(Object.groupBy,'name')));
