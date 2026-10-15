// mode: print
// STACK_CASES
// case: fixed leaf getter fault points at its assignment target
(function outer(){var value,source={};Object.defineProperty(source,'p',{get:function fixedGetter(){throw new Error('get')}});({p:value}=source)})()
// case: computed leaf getter fault points at its assignment target
(function outer(){var value,key='p',source={};Object.defineProperty(source,'p',{get:function computedGetter(){throw new Error('get')}});({[key]:value}=source)})()
// case: rest copy getter fault points at its rest target
(function outer(){var rest,source={};Object.defineProperty(source,'p',{enumerable:true,get:function restGetter(){throw new Error('copy')}});({...rest}=source)})()
// case: computed target ToPropertyKey fault points at its member target
(function outer(){var target={},key={};key[Symbol.toPrimitive]=function targetKey(){throw new Error('key')};({p:target[key]}={p:1})})()
// case: computed target Put fault points at its member target
(function outer(){var target={},key='value';Object.defineProperty(target,key,{set:function targetPut(){throw new Error('put')}});({p:target[key]}={p:1})})()
// PARSER_CASES
// case: object assignment rest cannot have a trailing comma
var rest;({...rest,}={})
// case: object assignment rest must be the final property
var rest,value;({...rest,value}={})
// case: object assignment rest cannot carry an initializer
var rest;({...rest={}}={})
// case: object assignment rest requires a target
({...}={})
// case: invalid logical leaf reports at its following delimiter
var value;({p:true&&value}={})
// case: invalid call leaf reports at its following delimiter
var value;({p:value()}={})
// case: invalid new leaf reports at its following delimiter
var Value;({p:new Value}={})
// case: invalid nested object leaf is diagnosed recursively
var value;({p:{q:true&&value}}={})
// case: invalid nested array rest keeps its own trailing-comma diagnostic
var rest;({p:[...rest,]}={})
// case: invalid nested object rest keeps its rest-last diagnostic
var rest,value;({p:{...rest,value}}={})
// case: object assignment pattern cannot be a compound target
var value;({value}+={value:1})
// case: object assignment pattern cannot be a logical-and target
var value;({value}&&={value:1})
// case: strict object assignment rejects eval shorthand
'use strict';var value;({eval}={value:1})
// case: strict object assignment rejects arguments alias targets
'use strict';var value;({p:arguments}={p:1})
