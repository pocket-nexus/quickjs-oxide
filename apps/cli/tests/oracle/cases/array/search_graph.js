// mode: print
var selected=['at','indexOf','lastIndexOf','includes'];
var implemented=['at','indexOf','lastIndexOf','includes','values','keys','entries'];
var own=Reflect.ownKeys(Array.prototype),names=[];
for(var i=0;i<own.length;i++)
  if(implemented.indexOf(own[i])>=0)names[names.length]=own[i];
print('keys='+names.join(','));
var metadata=[];
for(var i=0;i<selected.length;i++) {
  var name=selected[i],descriptor=Object.getOwnPropertyDescriptor(Array.prototype,name),fn=descriptor.value;
  var constructor;
  try { Reflect.construct(function(){},[],fn); constructor=true; }
  catch(error) { constructor=false; }
  metadata[metadata.length]=name+':'+fn.name+':'+fn.length+':D:'+
    Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable)+':'+
    (typeof fn==='function')+':'+(Object.getPrototypeOf(fn)===Function.prototype)+':'+constructor;
}
print('meta='+metadata.join('|'));
