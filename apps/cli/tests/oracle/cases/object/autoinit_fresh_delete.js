// mode: print
// case: Object.assign
var deleted=delete Object.assign;
print([deleted,'assign' in Object,Object.prototype.hasOwnProperty.call(Object,'assign'),typeof Object.assign].join('|'));
// case: descriptors
var a=delete Object.getOwnPropertyDescriptor,b=delete Object.getOwnPropertyDescriptors;
print([a,b,'getOwnPropertyDescriptor' in Object,'getOwnPropertyDescriptors' in Object,
  typeof Object.getOwnPropertyDescriptor,typeof Object.getOwnPropertyDescriptors].join('|'));
// case: enumeration
var a=delete Object.keys,b=delete Object.values,c=delete Object.entries;
print([a,b,c,'keys' in Object,'values' in Object,'entries' in Object,
  typeof Object.keys,typeof Object.values,typeof Object.entries].join('|'));
// case: extensibility
var a=delete Object.isExtensible,b=delete Object.preventExtensions;
print([a,b,'isExtensible' in Object,'preventExtensions' in Object,
  typeof Object.isExtensible,typeof Object.preventExtensions].join('|'));
// case: Object.fromEntries
var deleted=delete Object.fromEntries;
print([deleted,'fromEntries' in Object,Object.prototype.hasOwnProperty.call(Object,'fromEntries'),typeof Object.fromEntries].join('|'));
// case: Object.hasOwn
var deleted=delete Object.hasOwn;
print([deleted,'hasOwn' in Object,Object.prototype.hasOwnProperty.call(Object,'hasOwn'),typeof Object.hasOwn].join('|'));
// case: integrity
var a=delete Object.seal,b=delete Object.freeze,c=delete Object.isSealed,d=delete Object.isFrozen;
print([a,b,c,d,'seal' in Object,'freeze' in Object,'isSealed' in Object,'isFrozen' in Object,
  typeof Object.seal,typeof Object.freeze,typeof Object.isSealed,typeof Object.isFrozen].join('|'));
// case: Object.is
var deleted=delete Object.is;
print([deleted,'is' in Object,Object.prototype.hasOwnProperty.call(Object,'is'),typeof Object.is].join('|'));
