// mode: completion-units
// case: empty script

// case: empty statement
;
// case: empty statement preserves completion
1; ;
// case: empty block preserves completion
1; {}
// case: empty nested block preserves completion
1; {{;}}
// case: block expression updates completion
1; { 2; }
// case: nested block keeps its last expression
{ 1; { 2; {} } }
// case: deep empty blocks preserve completion
7; {{{}}}
// case: false if resets completion
1; if (false) 2;
// case: taken empty if resets completion
1; if (true) {}
// case: taken block updates completion
1; if (true) { 2; }
// case: false branch selects else
if (false) 1; else 2;
// case: true branch skips else
if (true) 1; else 2;
// case: dangling else binds to nearest if
if (true) if (false) 1; else 2;
// case: outer else follows completed nested if
if (false) if (true) 1; else 2; else 3;
// case: taken empty statement
if (true) ; else 2;
// case: empty else statement
if (false) 1; else ;
// case: comma condition
if ((0, 1)) 8; else 9;
// case: nested if resets a previous branch completion
if (true) { 1; if (false) 2; }
// case: function if return false
(function(x){ if (x) return 1; else return 2; })(0)
// case: function if return true
(function(x){ if (x) return 1; else return 2; })(1)
// case: nested branch return
(function(){ if (true) { if (true) { return 42; } return 1; } return 0; })()
// case: dead branch var is function scoped
(function(){ if (false) { var x = 1; } return typeof x; })()
// case: block var remains function scoped
(function(){ { var x = 4; } return x; })()
// case: selected var initializer
(function(flag){ if (flag) { var x = 3; } return typeof x + '|' + x; })(true)
// case: dead throw is not executed
(function(){ if (false) throw 1; return 42; })()
// case: return restricted production in a branch
(function(){ if (true) return
42; return 7; })()
// case: function expression statements stay discarded
(function(flag){ if (flag) 1; else 2; })()
// case: condition and selected branch order
(function(){ var log=''; var c=function(){log=log+'c';return true;}; var y=function(){log=log+'y';}; var n=function(){log=log+'n';}; if(c()) y(); else n(); return log; })()
// case: condition and false branch order
(function(){ var log=''; var c=function(){log=log+'c';return false;}; var y=function(){log=log+'y';}; var n=function(){log=log+'n';}; if(c()) y(); else n(); return log; })()
// case: unselected branch has no effects
(function(){ var x=0; if (true) x=1; else missing(); return x; })()
// case: object condition does not coerce
(function(){ var log=''; var o=function(){}; o.valueOf=function(){log=log+'v';return 0;}; if(o) log=log+'t'; return log; })()
// case: nested condition side effect order
(function(){ var log=''; if((log=log+'a',true)) if((log=log+'b',false)) log=log+'c'; else log=log+'d'; return log; })()
// case: block string is not a directive
{ 'use strict'; '\1'; }
// case: block before string prevents directive prologue
{}; 'use strict'; '\1';
// case: function block string is not a directive
(function(){ { 'use strict'; } return '\1'.charCodeAt(0); })()
// case: block strict spelling leaves legacy octal sloppy
{ 'use strict'; 010; }
// case: if strict spelling leaves legacy octal sloppy
if (true) 'use strict'; 010;
// case: LF participates in if ASI
if (false)
1;
else
2;
// case: CR participates in if ASI
if (false)1;else2;
// case: CRLF participates in if ASI
if (false)
1;
else
2;
// case: line separator participates in if ASI
if (false) 1; else 2;
// case: paragraph separator participates in if ASI
if (false) 1; else 2;
// case: false while resets completion
1; while (false) 2;
// case: empty false while resets completion
1; while (false);
// case: break preserves body completion
while (true) { 3; break; }
// case: nested while resets an earlier body completion
while(true){ 2; while(false) 3; break; }
// case: nested do preserves its final body completion
while(true){ do 2; while(false); break; }
// case: while condition does not become the completion
1; while((5,false));
// case: do condition does not replace its body completion
do 2; while((3,false))
// case: while iteration and continue order
(function(){ var i=0; var sum=0; while(i<5){ i++; if(i===3) continue; sum+=i; } return sum; })()
// case: while continue preserves the current body completion
Function.loopIndex=0; while(Function.loopIndex++<2){ 9; continue; }
// case: nested loop jumps select the nearest loop
(function(){ var i=0; var j=0; while(i<3){ i++; while(true){ j++; break; } continue; j=99; } return i+'|'+j; })()
// case: do body executes once
do 2; while(false)
// case: empty do resets an earlier completion
1; do; while(false)
// case: do break leaves its reset completion
1; do break; while(false)
// case: do continue preserves its body completion
do { 9; continue; } while(false)
// case: do break skips its condition
(function(){ var x=0; do { break; } while(x++); return x; })()
// case: do continue reaches its condition
(function(){ var x=0; do { continue; } while(x++<1); return x; })()
// case: do resets completion for every iteration
Function.loopIndex=0; do { if(Function.loopIndex++===0) 7; } while(Function.loopIndex<2)
// case: do trailing semicolon is optional before same-line source
do {} while(false) 9
// case: do body uses line-terminator ASI
do 1
while(false)
// case: break line terminator starts unreachable source
while(true){ break
missingAfterBreak; }
// case: break CR starts unreachable source
while(true){ breakmissingAfterBreak; }
// case: break CRLF starts unreachable source
while(true){ break
missingAfterBreak; }
// case: break line separator starts unreachable source
while(true){ break missingAfterBreak; }
// case: break paragraph separator starts unreachable source
while(true){ break missingAfterBreak; }
// case: continue block-comment line terminator starts unreachable source
do { continue/*
*/missingAfterContinue; } while(false)
// case: continue CRLF comment starts unreachable source
do { continue/*
*/missingAfterContinue; } while(false)
// case: Function constructor parses while jumps
Function('var i=0; while(i<3)i++; return i')()
// case: Function constructor parses do return
Function('do { return 4; } while(false)')()
// case: false for resets completion
1; for(;false;) 2;
// case: for break preserves body completion
for(;;){ 3; break; }
// case: for continue runs update
(function(){ var sum=0; for(var i=0;i<5;i++){ if(i===2) continue; sum+=i; } return sum; })()
// case: for var declarator list remains function scoped
(function(){ for(var i=0,j=1;i<2;i++,j+=2); return i+'|'+j; })()
// case: for var initializer keeps named evaluation
(function(){ for(var named=function(){};false;); return named.name; })()
// case: for break skips update
(function(){ var i=0; for(;;i++){ break; } return i; })()
// case: for without test reaches update after its body
(function(){ var i=0; for(;;i++){ if(i===3) break; } return i; })()
// case: for without update returns to test
(function(){ var i=0; for(;i<3;){ i++; } return i; })()
// case: for comma clauses preserve order
(function(){ var i=0; var x=0; for(i=0,x=1;i<3;i++,x+=2){} return i+'|'+x; })()
// case: relocated for update keeps internal conditional targets
(function(){ var i=0; var x=0; for(;i<3;i++,x+=i===1?10:1){} return x; })()
// case: for continue preserves the current body completion
Function.loopIndex=0; for(;Function.loopIndex++<2;){ 9; continue; }
// case: for init test and update do not become completion
for(Function.loopIndex=0;Function.loopIndex<1;Function.loopIndex++) 5
// case: nested for jumps select the nearest loop
(function(){ var i=0; var j=0; for(;i<3;i++){ for(;;){ j++; break; } continue; j=99; } return i+'|'+j; })()
// case: Function constructor parses classic for
Function('var sum=0; for(var i=0;i<4;i++)sum+=i; return sum')()
// case: for header line terminators do not insert semicolons
Function.loopIndex=0; for(Function.loopIndex=0
;Function.loopIndex<2
;Function.loopIndex++){} Function.loopIndex
// case: contextual of remains an identifier in classic for
(function(){ var of=9; for(of=0;of<2;of++); return of; })()
// case: sloppy let assignment remains an expression in classic for
(function(){ var let=9; for(let=0;let<2;let++); return let; })()
// case: escaped let member remains an expression in classic for
(function(){ var let=Function; for(l\u0065t.loop=0;l\u0065t.loop<2;l\u0065t.loop++); return let.loop; })()
// case: global sets in for clauses preserve remapped targets
for(loopGlobal=0;loopGlobal<3;loopGlobal++); loopGlobal
// case: for clauses execute in init test body update order
(function(){ var log=''; var i=0; for(log+='i';(log+='t',i<2);(log+='u',i++)){ log+='b'; } return log; })()
// case: return from for body skips update
Function.returnUpdate=0; (function(){ for(;;Function.returnUpdate=1) return 7; })(); Function.returnUpdate
// case: for update does not become completion
1; Function.loopIndex=0; for(;Function.loopIndex++<1;9);
// case: nested for resets an earlier body completion
for(;;){ 2; for(;false;) 3; break; }
// case: plain label preserves completion
plain: 6;
// case: empty labeled statement preserves completion
7; empty: ;
// case: labeled block break preserves completion
outer: { 2; break outer; 3; }
// case: labeled if break exits the regular statement
outer: if(true){ 4; break outer; 5; }
// case: unlabeled break skips a regular labeled statement
while(true){ marked: { 8; break; } 9; }
// case: unlabeled continue skips a regular labeled statement
(function(){ var i=0; var x=0; while(i++<2){ marked: { x++; continue; } } return i+'|'+x; })()
// case: named break crosses a nested loop to a regular label
outer: { while(true){ 10; break outer; } 9; }
// case: labeled break exits an outer for from a nested loop
(function(){ var i=0; outer: for(;i<5;i++){ while(true){ if(i===2) break outer; break; } } return i; })()
// case: labeled continue reaches an outer while condition
(function(){ var i=0; var x=0; outer: while(i++<3){ while(true){ x++; continue outer; } } return i+'|'+x; })()
// case: labeled continue reaches an outer do condition
(function(){ var i=0; var x=0; outer: do { i++; while(true){ x++; continue outer; } } while(i<3); return i+'|'+x; })()
// case: labeled continue reaches an outer for update
(function(){ var i=0; var x=0; outer: for(;i<3;i++){ while(true){ x++; continue outer; } } return i+'|'+x; })()
// case: break can cross the pinned multiple-label regular wrapper
outer: inner: while(true){ 11; break outer; }
// case: continue can target the directly attached inner loop label
(function(){ var i=0; outer: inner: while(i++<2){ continue inner; } return i; })()
// case: escaped label and unescaped target share an identifier value
\u0061: { 12; break a; }
// case: unescaped label and escaped target share an identifier value
a: { 12; break \u0061; }
// case: completed label names can be reused
same: { 1; break same; } same: 2;
// case: line terminator before a label colon is accepted
line
: 13;
// case: sloppy contextual let can be a label
let: { 13; break let; }
// case: sloppy future reserved word can be a label
package: { 13; break package; }
// case: break line terminator keeps the restricted production unlabeled
outer: while(true){ 14; break
outer; }
// case: break CR keeps the restricted production unlabeled
outer: while(true){ 14; breakouter; }
// case: break CRLF keeps the restricted production unlabeled
outer: while(true){ 14; break
outer; }
// case: break line separator keeps the restricted production unlabeled
outer: while(true){ 14; break outer; }
// case: break paragraph separator keeps the restricted production unlabeled
outer: while(true){ 14; break outer; }
// case: break comment terminator keeps the restricted production unlabeled
outer: while(true){ 14; break/*
*/outer; }
// case: function local var remains valid under a label
(function(){ local: var x=15; return x; })()
// case: Function constructor parses labels and named break
Function('var i=0; outer: for(;i<3;i++){if(i===1)break outer;} return i')()
// case: nested function owns an independent same-name label
(function(){ outer: while(true){ return (function(){ outer: { return 16; } })(); } })()
