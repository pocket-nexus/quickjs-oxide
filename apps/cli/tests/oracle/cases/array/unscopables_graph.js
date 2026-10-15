// mode: print
var prototype=Array.prototype,own=Reflect.ownKeys(prototype),symbols=[];
for(var i=0;i<own.length;i++) {
  if(own[i]===Symbol.iterator)symbols[symbols.length]='iterator';
  if(own[i]===Symbol.unscopables)symbols[symbols.length]='unscopables';
}
function bits(descriptor) {
  return 'D'+Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable);
}
var outer=Object.getOwnPropertyDescriptor(prototype,Symbol.unscopables),value=outer.value;
var keys=Reflect.ownKeys(value),entries=[];
for(var i=0;i<keys.length;i++) {
  var descriptor=Object.getOwnPropertyDescriptor(value,keys[i]);
  entries[entries.length]=keys[i]+':'+descriptor.value+':'+bits(descriptor);
}
print('symbols='+symbols.join(',')+':own='+own.length);
print('outer='+bits(outer)+':null-proto='+(Object.getPrototypeOf(value)===null)+
      ':array='+Array.isArray(value));
print('keys='+keys.join(','));
print('entries='+entries.join('|'));
