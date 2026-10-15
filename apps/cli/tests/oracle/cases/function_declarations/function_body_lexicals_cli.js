// mode: print
// flags:
// flags: -s
// case: nested block temporal dead zone read
(function blockTdz(){
  {
    return value;
    let value = 1;
  }
})()
// case: nested block typeof temporal dead zone read
(function blockTypeofTdz(){ { return typeof value; let value = 1; } })()
// case: nested block direct const write
(function blockReadonly(){ { const value = 1; value = 2; } })()
// case: captured block const write after scope exit
(function blockReadonlyOuter(){ let write; { const value = 1; write = function blockReadonlyInner(){ value = 2; }; } return write; })()()
// case: block exit participates in late readonly source mapping
(function blockReadonlyMarker(){
  {
    value = 1;
    const value = 2;
  }
})()
// case: block late readonly mapping crosses a lowered scope exit
(function blockReadonlyAfterScope(){
  {
    value = 1;
    const value = 2;
  }
  99;
})()
// case: block branch removal shares lowered scope label state
(function blockReadonlyBranches(flag){
  {
    if (flag) value = 1; else value = 2;
    const value = 3;
    99;
  }
})(false)
// case: switch case selector observes the shared temporal dead zone
(function switchSelectorTdz(){
  switch (0) {
    case value:
      let value = 0;
      return value;
  }
})()
// case: unselected switch declaration creates a cross-case temporal dead zone
(function switchCaseTdz(){ switch (1) { case 0: let value = 0; break; case 1: return value; } })()
// case: switch direct const write
(function switchReadonly(){ switch (0) { case 0: const value = 1; value = 2; } })()
// case: captured switch const write after scope exit
(function switchReadonlyOuter(){ let write; switch (0) { case 0: const value = 1; write = function switchReadonlyInner(){ value = 2; }; break; } return write; })()()
// case: switch exit participates in late readonly source mapping
(function switchReadonlyMarker(){
  switch (0) {
    case 0:
      value = 1;
      const value = 2;
  }
})()
// case: switch late readonly mapping observes the following case operation
(function switchReadonlyAfterWrite(){
  switch (0) {
    case 0:
      value = 1;
      const value = 2;
      99;
  }
})()
// case: switch case label bounds late readonly source mapping
(function switchReadonlyCaseLabel(){
  switch (0) {
    case 0:
      value = 1;
    case 1:
      const value = 2;
      99;
  }
})()
