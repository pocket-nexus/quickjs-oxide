// mode: print
// case: object binding rest cannot have a trailing comma
let {...rest,}={}
// case: object binding rest must be the final property
let {...rest,value}={}
// case: object binding rest cannot carry an initializer
let {...rest=defaultValue}={}
// case: object binding rest requires a binding identifier
let {...{value}}={}
// case: object binding rest rejects a member target
let {...rest.value}={}
// case: strict object binding rest rejects eval
(function(){'use strict';var {...eval}={}})()
// case: lexical object rest participates in duplicate-binding errors
let {value,...value}={}
// case: object binding declarations still require an initializer
let {...rest};
// case: for-of lexical object rest cannot have a declaration initializer
for(let {...rest}={} of []){}
