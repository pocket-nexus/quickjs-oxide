// mode: print
// flags:
// flags: -s
// case: fixed global blocks Program function declaration
function NaN(){return 1}
// case: hoisted Program function body fault
hoistedFault();
function hoistedFault(){
missingInHoistedFunction;
}
