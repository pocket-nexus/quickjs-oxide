// mode: print
// case: duplicate head lexical
(function(){
  for(let value=0,value=1;false;){}
})
// case: const head requires initializer
(function(){
  for(const value;false;){}
})
// case: descendant body var conflicts with head lexical
(function(){
  for(let value=0;value<1;value++){var value;}
})
// case: nested switch var conflicts with head lexical
(function(){
  for(let value=0;value<1;value++){switch(0){case 0:var value;}}
})
// case: strict eval head binding
(function(){
  'use strict';
  for(let eval=0;false;){}
})
// case: strict arguments head binding
(function(){
  'use strict';
  for(let arguments=0;false;){}
})
// case: let cannot bind itself in a head
(function(){
  for(let let=0;false;){}
})
// case: unparenthesized in stops a lexical NoIn initializer
(function(){
  for(let value='name' in Function;false;){}
})
