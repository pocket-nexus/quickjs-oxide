// mode: print
// case: body lexical before function declaration
(function(){
  let conflict;
  function conflict(){}
})
// case: body function declaration before lexical
(function(){
  function conflict(){}
  let conflict;
})
// case: strict body function named eval
(function(){
  'use strict';
  function eval(){}
})
// case: strict body function named arguments
(function(){
  'use strict';
  function arguments(){}
})
// case: declared function directive makes its eval name strict
(function(){
  function eval(){ 'use strict'; }
})
// case: declared function directive rejects duplicate parameters
(function(){
  function child(a,a){ 'use strict'; }
})
