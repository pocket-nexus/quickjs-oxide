// mode: print
// case: Program function followed by let
function clash(){}
let clash;
// case: Program function followed by const
function clash(){}
const clash=1;
// case: strict Program function named eval
'use strict';
function eval(){}
// case: strict Program function named arguments
'use strict';
function arguments(){}
