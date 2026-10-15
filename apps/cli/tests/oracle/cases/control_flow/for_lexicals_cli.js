// mode: print
// flags:
// flags: -s
// case: head self initializer is in the temporal dead zone
(function forHeadTdz(){
  for(let value=value;false;){}
})()
// case: later declarator is in the temporal dead zone
(function forLaterTdz(){
  for(let first=second,second=1;false;){}
})()
// case: initializer closure observes a later declarator TDZ
(function forClosureTdz(){
  for(let read=function forRead(){return value},value=read();false;){}
})()
// case: relocated const update is readonly
(function forConstUpdate(){
  for(const value=0;value<1;value=1){}
})()
// case: captured const remains readonly in the relocated update
(function forCapturedConstUpdate(){
  let read;
  for(const value=0;value<1;value=1){
    read=function(){return value};
  }
})()
// case: conditional update true branch keeps its readonly site
(function forConstConditionalUpdate(flag){
  for(const value=0;value<1;flag?value=1:value=2){}
})(true)
// case: conditional update false branch keeps its readonly site
(function forConstConditionalUpdate(flag){
  for(const value=0;value<1;flag?value=1:value=2){}
})(false)
// case: continue reaches the relocated readonly update
(function forConstContinue(){
  for(const value=0;value<1;value=1){
    continue;
  }
})()
// case: const body write is readonly
(function forConstBody(){
  for(const value=0;;){
    value=1;
  }
})()
// case: captured const writer stays readonly after break
(function forConstWriterOuter(){let write;for(const value=0;value<1;){write=function forConstWriterInner(){value=1};break;}write();})()
// case: Function constructor head TDZ
Function('for(let value=value;false;){}')()
// case: first var scope quirk still resolves an initializer through the later lexical
(function forVarScopeTdz(){
  {var value;}
  {for(var value=0;false;);let value;}
})()
