// mode: print
// case: duplicate body lexical
(function(){
  let value;
  let value;
})
// case: var before body lexical
(function(){
  var value;
  let value;
})
// case: var after body lexical
(function(){
  let value;
  var value;
})
// case: parameter conflicts with body lexical
(function(value){
  let value;
})
// case: const requires initializer
(function(){
  const value;
})
// case: duplicate name in one declaration list
(function(){
  let value, value;
})
// case: let cannot bind itself
(function(){
  let let = 1;
})
// case: strict eval lexical name
(function(){
  'use strict';
  let eval = 1;
})
// case: single statement lexical declaration
(function(){
  if (true) let value = 1;
})
// case: duplicate nested block lexical
(function(){
  { let value; const value = 1; }
})
// case: var before nested block lexical
(function(){
  { var value; let value; }
})
// case: var after nested block lexical
(function(){
  { let value; var value; }
})
// case: descendant var conflicts with parent block lexical
(function(){
  { let value; { var value; } }
})
// case: duplicate switch lexical across cases
(function(){
  switch (0) { case 0: let value; break; case 1: const value = 1; }
})
// case: switch var before lexical across cases
(function(){
  switch (0) { case 0: var value; break; case 1: let value; }
})
// case: switch var after lexical across cases
(function(){
  switch (0) { case 0: let value; break; case 1: var value; }
})
