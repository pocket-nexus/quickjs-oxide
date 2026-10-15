// mode: print
// case: statement before first clause
switch(0){1;}
// case: keyword before first clause
switch(0){else:1;}
// case: duplicate default
switch(0){default:1;default:2;}
// case: missing case expression
switch(0){case:1;}
// case: case keyword cannot be a case expression
switch(0){case case:;}
// case: default keyword cannot be a case expression
switch(0){case default:;}
// case: missing case colon
switch(0){case 0 1;}
// case: continue cannot target switch
switch(0){case 0:continue;}
// case: continue cannot target regular switch label
label:switch(0){case 0:continue label;}
// case: missing discriminant close paren
switch(0 {case 0:1;}
// case: missing switch body brace
switch(0) case 0:1;
// case: escaped case is not a clause
switch(0){c\u0061se 0:1;}
// case: case outside switch
case 0:1;
// case: default outside switch
default:1;
