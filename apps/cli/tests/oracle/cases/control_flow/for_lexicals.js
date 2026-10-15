// case: simple lexical head and outer shadow
((function(){let value=9,sum=0;for(let value=0;value<4;value++)sum+=value;return value+'|'+sum;})())
// case: head declarations initialize left to right
((function(){let result='';for(let left=1,right=left+1;left<2;left++,right++)result=left+'|'+right;return result;})())
// case: head and body lexicals shadow parameters while of remains a name
((function(value){let result='';for(let of=0,value=1;of<1;of++){let value=2;result=of+'|'+value;}return result+'|'+value;})(9))
// case: let without initializer becomes undefined
((function(){for(let value;;){return typeof value+'|'+value;}})())
// case: head anonymous function receives its binding name
((function(){let observed;for(let named=function(){};;){observed=named.name;break;}return observed;})())
// case: normal fallthrough creates per-iteration capture cells
((function(){let first,second,third;for(let value=0;value<3;value++){if(value===0)first=function(){return value};else if(value===1)second=function(){return value};else third=function(){return value};}return first()+'|'+second()+'|'+third();})())
// case: mutating an earlier iteration does not change the next
((function(){let firstRead,firstWrite,secondRead;for(let value=0;value<2;value++){if(value===0){firstRead=function(){return value};firstWrite=function(next){value=next};}else secondRead=function(){return value};}firstWrite(9);return firstRead()+'|'+secondRead();})())
// case: initializer and first body capture different cells
((function(){let initialRead,initialWrite,bodyRead;for(let value=(initialRead=function(){return value},initialWrite=function(next){value=next},0);value<1;value++){bodyRead=function(){return value};}initialWrite(9);return initialRead()+'|'+bodyRead();})())
// case: update and following body capture the same cell
((function(){let updateWrite,bodyRead;for(let value=0;value<2;(updateWrite=updateWrite||function(next){value=next},value++)){if(value===1)bodyRead=function(){return value};}updateWrite(7);return bodyRead();})())
// case: unconditional continue preserves the pinned shared-head-cell quirk
((function(){let first,second,third;for(let value=0;value<3;value++){if(value===0)first=function(){return value};else if(value===1)second=function(){return value};else third=function(){return value};continue;}return first()+'|'+second()+'|'+third();})())
// case: one continue shares only the crossed head cell
((function(){let first,second,third;for(let value=0;value<3;value++){if(value===0)first=function(){return value};else if(value===1)second=function(){return value};else third=function(){return value};if(value===0)continue;}return first()+'|'+second()+'|'+third();})())
// case: normal fallthrough closes a head cell when update is absent
((function(){let first,second;for(let value=0;value<2;){if(value===0)first=function(){return value};else second=function(){return value};value++;}return first()+'|'+second();})())
// case: continue shares the head cell when update is absent
((function(){let first,second;for(let value=0;value<2;){if(value===0)first=function(){return value};else second=function(){return value};value++;continue;}return first()+'|'+second();})())
// case: an absent test keeps the unmoved update cell boundary
((function(){let first,second;for(let value=0;;value++){if(value===0)first=function(){return value};else {second=function(){return value};break;}}return first()+'|'+second();})())
// case: labeled continue closes switch cells but not the head cell
((function(){let first,second;outer:for(let value=0;value<2;value++){switch(0){case 0:const inner=value;const read=function(){return inner+'|'+value};if(value===0){first=read;continue outer;}second=read;break;}}return first()+';'+second();})())
// case: labeled break preserves escaped switch and head values
((function(){let read;outer:for(let value=0;value<3;value++){switch(0){case 0:const inner=value;read=function(){return inner+'|'+value};break outer;}}return read();})())
// case: labeled continue exits an inner lexical for
((function(){let outerFirst,outerSecond,innerRead;outer:for(let outer=0;outer<2;outer++){for(let inner=0;inner<1;inner++){innerRead=function(){return inner};if(outer===0){outerFirst=function(){return outer};continue outer;}outerSecond=function(){return outer};}}return outerFirst()+'|'+outerSecond()+'|'+innerRead();})())
// case: Function constructor lexical for cells
(Function('let first,second;for(let value=0;value<2;value++){if(value===0)first=function(){return value};else second=function(){return value};}return first()+"|"+second();')())
// case: contextual bare let remains an expression
((function(){var let=1;for(let;;){break;}return let;})())
// case: first var declaration scope quirk remains accepted
((function(){{var value;}{for(var value;false;);let value;}return value===undefined;})())
// case: parenthesized in is accepted in a lexical initializer
((function(){for(let value=('name' in Function);false;){}return 1;})())
// case: function body inside a NoIn initializer restores ordinary grammar
((function(){var read;for(let initializer=(read=function(){let value='name' in Function;return value});false;){}return read();})())
// case: classic for nested array binding supports a pattern default
((function(){let result;for(let [[value]=[42]]=[];(result=value,false);){}return result;})())
// case: classic for object binding supports fixed computed and recursive properties
((function(){let key='computed',result;for(let {fixed,[key]:computed,nested:{value=3}}={fixed:1,computed:2,nested:{}};(result=fixed+'|'+computed+'|'+value,false);){}return result;})())
// case: classic for lexical object rest excludes consumed keys
((function(){let result;for(let {skip,...rest}={skip:1,value:2};(result=skip+'|'+rest.value+'|'+('skip' in rest),false);){}return result;})())
// SCRIPT_VALUE_CASES
// case: script lexical for stays local
Function.scopeLog='';for(let value=0;value<3;value++)Function.scopeLog+=value;
(Function.scopeLog+'|'+typeof value)
// case: script lexical for capture survives loop scope exit
Function.saved=undefined;for(let value=0;value<1;value++){Function.saved=function(){return value};}
(Function.saved()+'|'+typeof value)
