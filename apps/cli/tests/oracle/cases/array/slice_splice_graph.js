// mode: print
var implemented=['at','with','concat','every','some','forEach','map','filter','reduce','reduceRight','fill','find','findIndex','findLast','findLastIndex','indexOf','lastIndexOf','includes','join','toString','toLocaleString','pop','push','shift','unshift','reverse','toReversed','sort','toSorted','slice','splice','toSpliced','copyWithin','values','keys','entries'];
var own=Reflect.ownKeys(Array.prototype),names=[];
for(var i=0;i<own.length;i++)if(implemented.indexOf(own[i])>=0)names[names.length]=own[i];
function bits(descriptor){return 'D'+Number(descriptor.writable)+Number(descriptor.enumerable)+Number(descriptor.configurable)}
function metadata(name){
  var descriptor=Object.getOwnPropertyDescriptor(Array.prototype,name),fn=descriptor.value,constructable;
  try{Reflect.construct(function(){},[],fn);constructable=true}catch(error){constructable=false}
  return name+':'+fn.name+':'+fn.length+':'+bits(descriptor)+':'+
    bits(Object.getOwnPropertyDescriptor(fn,'name'))+':'+
    bits(Object.getOwnPropertyDescriptor(fn,'length'))+':'+
    (typeof fn==='function')+':'+(Object.getPrototypeOf(fn)===Function.prototype)+':'+constructable;
}
print('keys='+names.join(','));
print('meta='+metadata('slice'));
print('meta='+metadata('splice'));
print('meta='+metadata('toSpliced'));
