// mode: print
// case: duplicate object catch names are lexical redefinitions
try{}catch({value,value}){}
// case: duplicate nested array and object catch names are lexical redefinitions
try{}catch([value,{value}]){}
// case: pattern catch binding conflicts with var
try{}catch({value}){var value}
// case: simple catch binding conflicts with same body let
try{}catch(value){let value}
// case: strict object catch pattern rejects eval as a target
'use strict';try{}catch({value:eval}){}
// case: strict simple catch binding rejects eval with its own diagnostic
'use strict';try{}catch(eval){}
// case: sloppy object catch pattern still rejects lexical let
try{}catch({value:let}){}
// case: array catch rest rejects a default
try{}catch([...rest=[]]){}
// case: array catch rest must be last
try{}catch([...rest,last]){}
// case: object catch rest must be last
try{}catch({...rest,last}){}
// case: object catch pattern rejects a non binding leaf
try{}catch({value:true}){}
// case: catch pattern cannot be parenthesized
try{}catch(({value})){}
