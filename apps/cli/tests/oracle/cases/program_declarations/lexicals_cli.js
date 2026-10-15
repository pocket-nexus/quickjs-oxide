// mode: print
// flags:
// flags: -s
// case: read before Program lexical initialization
value;
let value=1;
// case: typeof before Program lexical initialization
typeof value;
let value=1;
// case: Program const assignment
const value=1;
value=2;
// case: fixed global property blocks a lexical declaration
let NaN=1;
// case: declaration preflight keeps source-order conflict priority
let first=function(){return Infinity},NaN=1,Infinity=2;
