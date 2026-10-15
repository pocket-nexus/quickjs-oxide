// mode: print
// case: missing condition left parenthesis wins over later lex error
if true 1; "unterminated
// case: missing condition right parenthesis wins over later lex error
if (true 0; "unterminated
// case: missing consequent
if (true)
// case: else cannot be a consequent
if (true) else 1;
// case: orphan else
else 1;
// case: unterminated then block
if (true) {
// case: unterminated nonempty block
if (true) { 1;
// case: stray right brace
if (true) {} }
// case: stray right brace after root expression
1 }
// case: stray right brace after if expression
if (true) 1 }
// case: stray right brace after nested if expression
if (false) if (true) 1 }
// case: missing else branch
if (true) {} else
// case: return remains an early error in a dead branch
if (false) return 1;
// case: escaped reserved binding remains an early error in a dead branch
(function(){ if (false) { var \u0069f=1; } })()
// case: raw malformed escape wins before later lex error
if (true) \u{}; "unterminated
// case: dead return wins before a later lex error
if (false) { return 1; "unterminated
// case: dead branch still scans its reached lexical error
if (false) { "unterminated
// case: strict program directive reaches a block legacy escape
"use strict"; { "\1"; }
// case: strict function directive reaches a block legacy escape
(function(){ "use strict"; { return "\1"; } })()
// case: LF throw restriction reports the following token
if (true) throw
1;
// case: CR throw restriction uses QuickJS debug coordinates
if (true) throw1;
// case: line separator throw restriction uses QuickJS debug coordinates
if (true) throw 1;
// case: break outside a loop
break
// case: continue outside a loop
continue
// case: outer loop is not visible inside a nested function
while(false) (function(){ break; })
// case: outer do loop is not visible inside a nested function
do (function(){ continue; }); while(false)
// case: do requires while
do {}
// case: do preserves QuickJS non-ASCII expect diagnostic
do{} false
// case: while requires a left parenthesis
while true;
// case: while requires a right parenthesis
while (true;
// case: while requires a body
while (true)
// case: do expression body still requires ASI
do 1 while(false)
// case: do condition requires a left parenthesis
do{}while false
// case: unimplemented break label is not silently discarded
while(true){ break missing; }
// case: unimplemented continue label is not silently discarded
while(true){ continue missing; }
// case: return remains an early error in a dead loop body
while(false) return 1;
// case: break scans the following lexical error first
break "unterminated
// case: in-loop break scans the following lexical error first
while(false){ break "unterminated
// case: outside break LF uses restricted-production ASI
break
missingLabel
// case: outside break CR keeps QuickJS debug coordinates
breakmissingLabel
// case: outside break CRLF advances at LF
break
missingLabel
// case: outside break line separator keeps QuickJS debug coordinates
break missingLabel
// case: outside break paragraph separator keeps QuickJS debug coordinates
break missingLabel
// case: outside continue comment terminator uses restricted-production ASI
continue/*
*/missingLabel
// case: for requires a left parenthesis
for true;
// case: for initializer requires its semicolon
for(1 2;;);
// case: for test requires its semicolon
for(;true 2;);
// case: for requires a right parenthesis
for(;;
// case: for requires a body
for(;;)
// case: outer for is not visible inside a nested function
for(;false;) (function(){ break; })
// case: outer for continue is not visible inside a nested function
for(;false;) (function(){ continue; })
// case: return remains an early error in a dead for body
for(;false;) return 1;
// case: for body continue label is not silently discarded
for(;;){ continue missing; }
// case: top-level in with later semicolons stays classic for
for(Function.item in Function;;);
// case: top-level of with later semicolons stays classic for
for(Function.item of Function;;);
// case: for pre-scan does not let a later lexical error win
for(true token; ; "unterminated
// case: duplicate regular label is rejected
duplicate: { duplicate: 1; }
// case: duplicate active loop label is rejected
duplicate: while(false){ duplicate: 1; }
// case: escaped and unescaped active label names are duplicates
duplicate: { d\u0075plicate: 1; }
// case: duplicate label wins before later lexical failure
duplicate: { duplicate: "unterminated
// case: continue cannot target a regular labeled statement
regular: { continue regular; }
// case: outer label in a multiple-label chain is not a continue target
outer: inner: while(true){ continue outer; }
// case: missing named break target remains an early error
while(true){ break missing; }
// case: missing named continue target remains an early error
for(;;){ continue missing; }
// case: outer label is not visible inside a nested function
outer: while(false) (function(){ break outer; })
// case: outer continue label is not visible inside a nested function
outer: for(;false;) (function(){ continue outer; })
// case: label requires a statement
missing:
// case: continue line terminator does not consume a regular label name
regular: { continue
regular; }
// case: resolved break target still scans the following lexical error
outer: while(true){ break outer "unterminated
// case: unknown break label wins before a later lexical error
while(true){ break missing "unterminated
// case: escaped reserved word is not consumed as a break label
while(true){ break \u0069f; }
// case: escaped reserved word is not accepted as a statement label
\u0069f: 1;
