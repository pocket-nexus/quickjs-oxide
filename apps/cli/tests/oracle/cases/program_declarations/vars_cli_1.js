// mode: print
// case: Program var followed by Program lexical
var clash;
let clash;
// case: Program lexical followed by Program var
let clash;
var clash;
// case: strict Program var eval binding
'use strict';
var eval=1;
// case: strict Program var arguments binding
'use strict';
var arguments=1;
