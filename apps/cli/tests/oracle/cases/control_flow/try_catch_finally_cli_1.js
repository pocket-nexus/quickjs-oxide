// mode: print
// case: try requires catch or finally
try{}
// case: empty catch parameter is invalid
try{}catch(){}
// case: reserved catch parameter is invalid
try{}catch(if){}
// case: catch parameter requires a right parenthesis
try{}catch(e {}
// case: finally requires a block
try{}finally 1
// case: catch cannot appear without try
catch(e){}
// case: finally cannot appear without try
finally{}
// case: catch binding conflicts with same-block let
try{throw 4}catch(e){let e}
// case: catch binding conflicts with same-block const
try{throw 4}catch(e){const e=1}
// case: strict catch binding rejects eval
'use strict';try{}catch(eval){}
// case: catch cannot follow a completed finally
try{}finally{}catch(e){}
