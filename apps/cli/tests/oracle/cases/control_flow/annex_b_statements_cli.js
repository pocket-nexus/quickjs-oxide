// mode: print
// flags:
// flags: -s
// case: if-arm Annex outer closure fault
(function singleOuter(){
  if(true) function singleInner(){
    missingSingleAnnex;
  }
  return singleInner();
})()
// case: label-chain lexical closure fault
(function labelOuter(){
  first:second:function labelInner(){
    missingLabelChain;
  }
  return labelInner();
})()
// case: Function constructor if-arm fault
Function("if(true) function dynamicSingle(){\n  missingDynamicSingle;\n}\nreturn dynamicSingle();")()
// case: duplicate if Annex-first closure fault
(function duplicateOuter(){
  if((Function.savedDuplicateIf=duplicateIf,true))
    function duplicateIf(){ missingFirstIfTarget; }
  else
    function duplicateIf(){ missingSecondIfTarget; }
  return duplicateIf();
})()
// case: duplicate if lexical-last closure fault
(function duplicateOuter(){
  if((Function.savedDuplicateIf=duplicateIf,true))
    function duplicateIf(){ missingFirstIfTarget; }
  else
    function duplicateIf(){ missingSecondIfTarget; }
  return Function.savedDuplicateIf();
})()
