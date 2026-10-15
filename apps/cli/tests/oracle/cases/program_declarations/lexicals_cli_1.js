// mode: print
// case: duplicate Program lexical
let value=1;
let value=2;
// case: Program const requires initializer
const value;
// case: strict Program eval binding
'use strict';
let eval=1;
// case: strict Program arguments binding
'use strict';
const arguments=1;
// case: Program let cannot bind itself
let let=1;
