// mode: print
var implemented=['at','with','indexOf','lastIndexOf','includes','values','keys','entries'];
var own=Reflect.ownKeys(Array.prototype),names=[];
for(var i=0;i<own.length;i++)
  if(implemented.indexOf(own[i])>=0)names[names.length]=own[i];
function bits(descriptor) {
  return 'D'+Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable);
}
var descriptor=Object.getOwnPropertyDescriptor(Array.prototype,'with'),fn=descriptor.value;
var constructable;
try { Reflect.construct(function(){},[],fn); constructable=true; }
catch(error) { constructable=false; }
print('keys='+names.join(','));
print('meta=with:'+fn.name+':'+fn.length+':'+bits(descriptor)+':'+
      bits(Object.getOwnPropertyDescriptor(fn,'name'))+':'+
      bits(Object.getOwnPropertyDescriptor(fn,'length'))+':'+
      (typeof fn==='function')+':'+(Object.getPrototypeOf(fn)===Function.prototype)+':'+constructable);
