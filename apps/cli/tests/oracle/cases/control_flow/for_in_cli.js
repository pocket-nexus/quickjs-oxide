// mode: print
// case: strict var initializer is rejected
(function(){'use strict';for(var k=1 in {a:1}){}})()
// case: lexical initializer is rejected
for(let k=1 in {a:1}){}
// case: ordinary assignment initializer is rejected
var k;for(k=1 in {a:1}){}
// case: call expression is not a for-in assignment target
for(f() in {}) {}
