// case: simple let and const execution
((function(){ let value; let base = 40, delta = 1; value = base + delta; value++; const answer = value; return answer; })())
// case: uninitialized let becomes undefined
((function(){ let value; const result = value === undefined; return result; })())
// case: closure created before declaration observes initialization
((function(){ const read = function(){ return later; }; let later = 42; return read(); })())
// case: transitive closure relay preserves one lexical cell
((function(){ let value = 40; const make = function(){ return function step(){ value += 1; return value; }; }; const step = make(); return step() + step(); })())
// case: Function constructor lexical body
(Function('let value = 40; const delta = 2; return value + delta;')())
// case: Function constructor lexical closure relay
((function(){ const step = Function('let value = 40; return function(){ value += 1; return value; };')(); return step() + step(); })())
// case: declaration lists initialize from left to right
((function(){ let log = '', a = (log += 'a', 1), b = (log += a, 2), c = (log += b, 3); return log + '|' + a + '|' + b + '|' + c; })())
// case: lexical anonymous functions receive contextual names
((function(){ let first = function(){}, second = function named(){}; const third = function(){}; return first.name + '|' + second.name + '|' + third.name; })())
// case: lexical delete is statically false
((function(){ const deleted = delete value; let value = 1; return deleted; })())
// case: short-circuited const logical assignment does not write
((function(){ const value = 0; return value &&= missing; })())
// case: sloppy eval and arguments lexical names remain ordinary bindings
((function(){ let arguments = 20; const eval = 22; return arguments + eval; })())
// case: dynamic Function does not capture caller lexical state
((function(){ let callerValue = 1; return Function('return typeof callerValue')(); })())
// case: nested block lexicals shadow without changing the outer binding
((function(){ let value = 1, inner; { let value = 40; const delta = 2; value += delta; inner = value; } return value + '|' + inner; })())
// case: captured block lexical outlives its scope
((function(){ let read; { const value = 42; read = function(){ return value; }; } return read(); })())
// case: block continue closes one capture before the next scope entry
((function(){ let first, second; let i = 0; while (i < 2) { let value = ++i; let read = function(){ return value; }; if (i === 1) { first = read; continue; } second = read; } return first() * 10 + second(); })())
// case: labeled block break preserves its captured lexical
((function(){ let read; done: { const value = 42; read = function(){ return value; }; break done; } return read(); })())
// case: nested block lexical may shadow a parameter
((function(value){ { let value = 42; return value; } })(1))
// case: QuickJS keeps only the first sibling var declaration scope
((function(){ { var value; } { var value; let value; } return value === undefined; })())
// case: Function constructor nested lexical closure
(Function('let read; { const value = 42; read = function(){ return value; }; } return read();')())
// case: nested array lexical bindings compose defaults elisions and rest
((function(){const [[first]=[40],,[...[,second]]]=[undefined,0,[1,2]];return first+second;})())
// case: function body object lexical binding
((function(){const [{value}] = [{value:1}];return value;})())
// case: nested block object lexical binding
((function(){{const [{value}] = [{value:2}];return value;}})())
// case: switch object lexical binding
((function(){switch(0){case 0:const [{value}] = [{value:3}];return value;}})())
// case: function body object rest lexical binding
((function(){const {skip,...rest}={skip:1,value:2};return skip+'|'+rest.value+'|'+('skip' in rest);})())
// case: nested block object rest lexical binding
((function(){{const [{skip,...rest}]=[{skip:2,value:3}];return skip+'|'+rest.value+'|'+('skip' in rest);}})())
// case: switch object rest lexical binding
((function(){switch(0){case 0:const {skip,...rest}={skip:3,value:4};return skip+'|'+rest.value+'|'+('skip' in rest);}})())
// case: switch fallthrough shares one lexical environment
((function(){ let read; switch (0) { case 0: let value = 40; read = function(){ return value; }; case 1: value++; break; } return read(); })())
// case: switch lexical shadows without changing the outer binding
((function(){ let value = 1, inner; switch (0) { case 0: let value = 42; inner = value; break; } return value + '|' + inner; })())
// case: switch continue closes one capture before the next scope entry
((function(){ let first, second; let i = 0; outer: while (i < 2) { switch (0) { case 0: let value = ++i; let read = function(){ return value; }; if (i === 1) { first = read; continue outer; } second = read; break; } } return first() * 10 + second(); })())
// SCRIPT_SCOPE_VALUE_CASES
// case: script nested block lexical
Function.scopeLog = ''; { let value = 40; const delta = 2; Function.scopeLog += value + delta; }
(Function.scopeLog + '|' + typeof value)
// case: script nested switch lexical
Function.scopeLog = ''; switch (0) { case 0: let value = 40; const delta = 2; Function.scopeLog += value + delta; break; }
(Function.scopeLog + '|' + typeof value)
// case: script captured block cell survives scope exit
Function.saved = undefined; { let value = 40; Function.saved = function(){ return ++value; }; }
(Function.saved() * 100 + Function.saved())
// case: script captured switch cell survives scope exit
Function.saved = undefined; switch (0) { case 0: let value = 40; Function.saved = function(){ return ++value; }; break; }
(Function.saved() * 100 + Function.saved())
