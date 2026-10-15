// mode: print
// case: lexical before malformed block function keeps redefinition priority
(function(){
  { let conflict; function conflict( }
})
// case: strict duplicate block function keeps redefinition priority
(function(){
  'use strict';
  { function duplicate(){} function duplicate( }
})
// case: block function before lexical
(function(){
  { function conflict(){} let conflict; }
})
// case: var before block function
(function(){
  { var conflict; function conflict(){} }
})
// case: block function before var
(function(){
  { function conflict(){} var conflict; }
})
// case: switch cases share one lexical declaration scope
(function(value){
  switch(value){ case 0: let conflict; break; case 1: function conflict(){} }
})
// case: child directive makes its block function eval name strict
(function(){
  { function eval(){ 'use strict'; } }
})
