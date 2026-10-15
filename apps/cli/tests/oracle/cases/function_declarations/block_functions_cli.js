// mode: print
// flags:
// flags: -s
// case: block lexical closure fault
(function blockOuter(){
  {
    function blockInner(){
      missingBlockLexical;
    }
    return blockInner();
  }
})()
// case: Annex outer closure fault
(function annexOuter(){
  {
    function annexInner(){
      missingAnnexOuter;
    }
  }
  return annexInner();
})()
// case: Function constructor block declaration fault
Function("{\n  function dynamicBlockInner(){\n    missingDynamicBlock;\n  }\n  return dynamicBlockInner();\n}")()
