// mode: print
// case: strict direct identifier
"use strict";
delete victim
// case: strict parenthesized identifier
"use strict";
delete (victim)
// case: strict Function constructor body
Function("\"use strict\"; delete victim")
