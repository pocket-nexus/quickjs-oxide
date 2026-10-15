// mode: print
// case: direct temporal dead zone read
(function lexicalTdz(){ return value; let value = 1; })()
// case: typeof temporal dead zone read
(function lexicalTypeofTdz(){ return typeof value; let value = 1; })()
// case: direct const write
(function lexicalReadonly(){ const value = 1; value = 2; })()
// case: captured const write
(function lexicalReadonlyOuter(){ const value = 1; return function lexicalReadonlyInner(){ value = 2; }; })()()
// case: const write before declaration inherits the final dead source marker
(function lexicalReadonlyBefore(){
  value = 1;
  const value = 2;
})()
// case: const self-write initializer inherits the declaration marker
(function lexicalReadonlySelf(){
  const value = (value = 1);
})()
// case: following return supersedes a const self-write marker
(function lexicalReadonlyReturn(){
  const value = (value = 1);
  return 0;
})()
// case: branch join bounds a late readonly throw marker
(function lexicalReadonlyBranch(flag){
  if (flag) value = 1;
  const value = 2;
  return 0;
})(true)
// case: dead forward branch references are removed before late throw mapping
(function lexicalReadonlyDeadBranch(){
  value = 1;
  if (false) 2;
  const value = 3;
  99;
})()
// case: return marker wins over a nested readonly assignment marker
(function lexicalReadonlyReturnExpression(){
  return 0 + (value = 2);
  const value = 1;
})()
// case: earlier branch throw removal updates a later branch join
(function lexicalReadonlyBranches(flag){
  if (flag) value = 1; else value = 2;
  const value = 3;
  99;
})(false)
// case: constant branch folding releases the untaken readonly label
(function lexicalReadonlyConstantBranch(){
  if (true) value = 1; else value = 2;
  const value = 3;
  99;
})()
// case: unreferenced physical labels still block adjacent constant folding
(function lexicalReadonlyConditionalLabel(flag){
  if (false ? true : false) { if (flag) value = 1; } else { value = 2; }
  const value = 9;
  99;
})(false)
// case: conditional goto inversion preserves the inner readonly join
(function lexicalReadonlyInversion(){
  if (true) { if (false ? true : false); else value = 1; } else { value = 3; }
  const value = 9;
  99;
})()
