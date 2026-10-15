// mode: print
function bits(descriptor) {
  return 'D:'+Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable);
}
function isConstructor(value) {
  try { Reflect.construct(function(){}, [], value); return true; }
  catch (_) { return false; }
}
function callableMeta(owner,name) {
  var descriptor=Object.getOwnPropertyDescriptor(owner,name),value=descriptor.value;
  return value.name+':'+value.length+':' +
    (Object.getPrototypeOf(value)===Function.prototype)+':' +
    (typeof value==='function')+':'+isConstructor(value)+':' +
    Object.getOwnPropertyNames(value).join(',')+':'+bits(descriptor);
}
var selected=['length','at','charCodeAt','charAt','concat','codePointAt',
  'isWellFormed','toWellFormed','indexOf','lastIndexOf','includes','endsWith',
  'startsWith','toString','valueOf'];
print('keys='+Reflect.ownKeys(String.prototype).filter(function(key){
  return selected.indexOf(key)>=0;
}).map(String).join(','));
print('indexOf='+callableMeta(String.prototype,'indexOf'));
print('lastIndexOf='+callableMeta(String.prototype,'lastIndexOf'));
print('identity='+(String.prototype.indexOf===String.prototype.indexOf)+':' +
  (String.prototype.lastIndexOf===String.prototype.lastIndexOf)+':' +
  (String.prototype.indexOf!==String.prototype.lastIndexOf));
print('index-props='+bits(Object.getOwnPropertyDescriptor(String.prototype.indexOf,'length'))+':' +
  bits(Object.getOwnPropertyDescriptor(String.prototype.indexOf,'name')));
print('last-props='+bits(Object.getOwnPropertyDescriptor(String.prototype.lastIndexOf,'length'))+':' +
  bits(Object.getOwnPropertyDescriptor(String.prototype.lastIndexOf,'name')));
