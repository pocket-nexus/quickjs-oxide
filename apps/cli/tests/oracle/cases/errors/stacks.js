// mode: print
// case: nested VM fault and tail-call sites
(function outer(){ return (function inner(){ return 1n + 1; })(); })()
// case: eager Error constructor stack
throw (function outer(){ return (function inner(){ return new Error("boom"); })(); })()
// case: parse location
1 +
// case: strict missing assignment inherits the LHS site
"use strict"; missing = 1
// case: assignment inherits an RHS identifier marker
"use strict"; missing = Error
// case: assignment inherits an RHS operator marker
"use strict"; missing = 1 + 2
// case: assignment inherits a private self-binding marker
(function f(){ "use strict"; missing = f; })()
// case: fixed member null fault
null.x
// case: computed member undefined fault
(void 0)['x']
// case: non-tail method call site
(function(){ var value = Function.name(); return value; })()
// case: tail method call site
(function(){ return Function.name(); })()
// case: template concat coercion inherits the last substitution site
Function.x=function(){}; Function.x.toString=function(){throw new Error("x")};
`a${Function.x}b`
// case: fixed member assignment fault
null.x = 1
// case: computed member assignment fault
null['x'] = 1
// case: fixed member delete fault
delete null.x
// case: computed member delete fault
delete (void 0)['x']
// case: strict fixed member assignment rejection
"use strict";
Function.prototype = 1
// case: strict fixed member delete rejection
"use strict";
delete Function.prototype
// case: native member setter fault
Function.prototype.caller = 1
// case: strict fixed compound assignment rejection
"use strict";
Function.prototype += 1
// case: strict computed compound assignment rejection
"use strict";
Function['prototype'] += 1
// case: binary bitwise mixed type uses operator site
(function(){ return 1n & 1; })()
// case: binary shift mixed type uses operator site
(function(){ return 1n << 1; })()
// case: unsigned shift BigInt rejection uses operator site
(function(){ return 1n >>> 0n; })()
// case: oversized BigInt shift uses operator site
(function(){ return 1n << 1048576n; })()
// case: binary exponent mixed type uses operator site
(function(){ return 1n ** 1; })()
// case: negative BigInt exponent uses operator site
(function(){ return 2n ** -1n; })()
// case: oversized BigInt exponent uses operator site
(function(){ return 2n ** 1048575n; })()
// case: strict fixed bitwise assignment rejection uses operator site
"use strict";
Function.prototype &= 1
// case: strict computed bitwise assignment rejection uses operator site
"use strict";
Function['prototype'] ^= 1
// case: strict fixed shift assignment rejection uses operator site
"use strict";
Function.prototype <<= 1
// case: strict computed shift assignment rejection uses operator site
"use strict";
Function['prototype'] >>>= 1
// case: strict fixed exponent assignment rejection uses operator site
"use strict";
Function.prototype **= 1
// case: strict computed exponent assignment rejection uses operator site
"use strict";
Function['prototype'] **= 1
// case: bitwise compound getter fault keeps member site
Function.prototype.caller |= 1
// case: shift compound getter fault keeps member site
Function.prototype.caller >>= 1
// case: exponent compound getter fault keeps member site
Function.prototype.caller **= 1
// case: bitwise compound mixed type uses operator site
Function.__qjo_bit_error = 1n; Function.__qjo_bit_error &= 1
// case: compound nullish pre-key fault
null[true] += 1
// case: compound getter fault keeps member site
Function.prototype.caller += 1
// case: strict fixed logical assignment rejection
"use strict";
Function.prototype &&= 1
// case: strict computed logical assignment rejection
"use strict";
Function['prototype'] &&= 1
// case: logical assignment put inherits RHS call site
"use strict";
Function.prototype &&= (function(){ return 1; })()
// case: logical nullish pre-key fault
null[true] ||= 1
// case: logical getter fault keeps member site
Function.prototype.caller &&= 1
// case: nullish RHS getter keeps member site
null ?? Function.prototype.caller
// case: nullish RHS call throw uses call site
null ?? (function(){ throw new Error('boom'); })()
// case: nullish RHS identifier keeps identifier site
void 0 ?? missingNullishRhs
// case: logical before nullish parse location
1 || 2 ?? 3
// case: nullish before logical parse location
1 ?? 2 || 3
// case: identifier compound missing read keeps identifier site
missingIdentifierCompound += 1
// case: identifier bitwise compound missing read keeps identifier site
missingIdentifierBitwise &= 1
// case: identifier shift compound missing read keeps identifier site
missingIdentifierShift >>>= 1
// case: identifier exponent compound missing read keeps identifier site
missingIdentifierPower **= 1
// case: strict private identifier arithmetic write uses operator site
(function named(){ 'use strict'; named += 1; })()
// case: strict private identifier bitwise write uses operator site
(function named(){ 'use strict'; named &= 1; })()
// case: strict private identifier shift write uses operator site
(function named(){ 'use strict'; named <<= 1; })()
// case: strict private identifier exponent write uses operator site
(function named(){ 'use strict'; named **= 1; })()
// case: strict private identifier logical write uses identifier site
(function named(){ 'use strict'; named &&= 1; })()
// case: identifier logical write inherits RHS call site
(function named(){ 'use strict'; named &&= (function(){ return 1; })(); })()
// case: strict eval compound early error uses RHS site
(function(){ 'use strict'; eval += 1; })
// case: strict parenthesized arguments compound early error uses RHS site
(function(){ 'use strict'; (arguments) ??= 1; })
// case: strict eval exponent compound early error uses RHS site
(function(){ 'use strict'; eval **= 1; })
// case: postfix update missing read keeps identifier site
missingIdentifierUpdate++
// case: prefix update missing read keeps identifier site
++missingIdentifierUpdate
// case: strict fixed postfix update rejection uses operator site
"use strict";
Function.prototype++
// case: strict computed prefix update rejection uses operator site
"use strict";
++Function['prototype']
// case: postfix update getter fault keeps member site
Function.prototype.caller++
// case: postfix update nullish pre-key fault
null[true]++
// case: strict private postfix update write
(function named(){ 'use strict'; named++; })()
// case: strict private prefix update write
(function named(){ 'use strict'; ++named; })()
// case: return marker supersedes strict private postfix update
(function named(){ 'use strict'; return named++; })()
// case: return marker supersedes strict private prefix update
(function named(){ 'use strict'; return ++named; })()
// case: postfix BigInt update allocation error uses operator site
(function(){ var value = 1n << 1048575n; return value++; })()
// case: CR remains a debug column
(function f(){
return 1n + 1;
})()
// case: CRLF advances only at LF
(function f(){
return 1n + 1;
})()
// case: line separator remains a debug column
(function f(){ return 1n + 1; })()
// case: UTF-8 columns count lead bytes
(function f(){ "é🙂中"; return 1n + 1; })()
// case: while condition fault keeps its expression site
while(
null.x) {}
// case: while body fault keeps its expression site
while(true){
null.x;}
// case: do body fault keeps its expression site
do {
null.x;} while(false)
// case: do continue condition fault keeps its expression site
do { continue; } while(
null.x)
// case: for initializer fault keeps its expression site
for(
null.x;;);
// case: for test fault keeps its expression site
for(;
null.x;);
// case: for update fault keeps its expression site
for(;;
null.x);
// case: for body fault keeps its expression site
for(;true;){
null.x;}
// case: regular labeled body fault keeps its expression site
outer: {
null.x;}
// case: labeled while continue returns to the condition fault site
Function.i=0; outer: while(Function.i++===0 ? true :
null.x){ continue outer; }
// case: labeled do continue reaches its condition fault site
outer: do { continue outer; } while(
null.x)
// case: labeled for continue reaches the relocated update fault site
Function.i=0; outer: for(;Function.i++<1;
null.x){ continue outer; }
// case: switch discriminant fault keeps its expression site
switch(
null.x){}
// case: switch case-test fault keeps its expression site
switch(0){case
null.x:;}
// case: switch body fault keeps its expression site
switch(1){case 1:
null.x;}
// case: switch return fault keeps its operator site
(function(){switch(1){case 1:return 1n + 1;}})()
// case: switch terminal read-only write abandons its discriminant
(function named(){'use strict';switch(1){case 1:named=1;}})()
// case: invalid in uses the operator site
'key'
in 1
// case: invalid instanceof uses the operator site
1
instanceof 2
// case: in key conversion preserves the throwing function frame
Function[Symbol.toPrimitive]=function key(){throw new Error('key')};
Function in Function
// case: instanceof bad prototype preserves the native hasInstance frame
Function.bad=function Bad(){}; Function.bad.prototype=1;
Function instanceof Function.bad
// case: unary bitwise coercion uses operator site
~Symbol()
