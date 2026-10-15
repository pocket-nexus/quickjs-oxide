// mode: print
// flags:
// flags: -s
// case: hoisted body declaration fault
(function bodyDeclarationOuter(){
  return bodyDeclarationInner();
  function bodyDeclarationInner(){
    missingBodyDeclaration;
  }
})()
// case: last duplicate body declaration owns the stack location
(function duplicateBodyDeclaration(){
  function duplicateTarget(){ missingFirstBodyDeclaration; }
  function duplicateTarget(){ missingSecondBodyDeclaration; }
  return duplicateTarget();
})()
// case: Function constructor body declaration owns its dynamic stack
Function("function bodyInner(){\n  missingDynamicBody;\n}\nreturn bodyInner()")()
