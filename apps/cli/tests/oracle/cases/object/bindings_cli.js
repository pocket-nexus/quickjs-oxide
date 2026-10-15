// mode: print
// case: direct object pattern requires an initializer
var {x};
// case: reserved shorthand is not a binding target
let {if}={if:1}
// case: string property requires a target
let {"x"}={x:1}
// case: computed property requires a target
let {[x]}={}
// case: method syntax is not a binding property
let {x(){}}={}
// case: shorthand cannot be followed by an object pattern without a colon
let {a{b}}={a:{b:42}}
// case: shorthand cannot be followed by an array pattern without a colon
let {a[b]}={a:[42]}
// case: shorthand cannot take a nested pattern default without a colon
let {a{b}=defaultValue}={a:{b:42}}
// case: string property cannot take a nested pattern without a colon
let {"a"{b}}={}
// case: keyword property cannot take a nested pattern without a colon
let {if{b}}={}
// case: computed property cannot take a nested pattern without a colon
let {["a"]{b}}={}
// case: strict shorthand eval diagnoses after the property name
'use strict';var {eval}={}
// case: strict shorthand arguments diagnoses after the property name
'use strict';var {arguments=1}={}
// case: escaped reserved spelling remains an ordinary property name
var {\u0069f}={}
// case: escaped reserved alias is an invalid binding target
var {value:\u0069f}={}
// case: escaped reserved computed alias is an invalid binding target
var {[0]:\u0069f}={}
// case: malformed object rest keeps syntax priority
let {...rest,value}={}
// case: object rest trailing comma wins before the lexical let target error
let {...let,}={}
// case: object rest lexical let uses the binding diagnostic
let {...let}={}
// case: invalid initializer wins after a valid object rest target
let {...rest}=;
// case: invalid for-of RHS wins after a valid object rest target
for(let {...rest} of ){}
// case: later duplicate declaration wins after a valid object rest binding
let {...value}={},value
