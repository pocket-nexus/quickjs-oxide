// mode: print
// case: strict if-arm function
(function(){'use strict';if(true)function strictIf(){}})
// case: strict if context error precedes a malformed function header
(function(){'use strict';if(true)function malformed(})
// case: direct while-body function
(function(){while(false)function whileBody(){}})
// case: direct do-body function
(function(){do function doBody(){} while(false)})
// case: direct for-body function
(function(){for(;false;)function forBody(){}})
// case: if arm cannot forward function permission through a label
(function(){if(true)label:function labelledIf(){}})
// case: strict labelled function
(function(){'use strict';label:function strictLabel(){}})
// case: prior Program var blocks a labelled function
var priorVar;label:function priorVar(){}
// case: prior Program var conflict precedes a malformed labelled child
var priorMalformed;label:function priorMalformed(}
// case: prior Program function blocks a labelled function
function priorFunction(){}label:function priorFunction(){}
// case: prior Program lexical blocks a labelled function
let priorLexical;label:function priorLexical(){}
// case: prior body lexical blocks a labelled function
(function(){let bodyLexical;label:function bodyLexical(){}})
// case: prior body var blocks a labelled function
(function(){var bodyVar;label:function bodyVar(){}})
// case: labelled body function blocks a later lexical
(function(){label:function laterBodyLexical(){};let laterBodyLexical})
// case: duplicate label chain
(function(){same:same:if(true)function duplicateLabel(){}})
// case: declared function directive validates its if-arm name strictly
(function(){if(true)function eval(){'use strict';}})
// case: if-arm generator is not an Annex B ordinary function
if(true)function* singleGenerator(){}
// case: labelled generator is not an Annex B ordinary function
label:function* labelledGenerator(){}
// case: if-arm async declaration requires the other-declaration mask
if(true)async function singleAsync(){}
// case: labelled async declaration requires the other-declaration mask
label:async function labelledAsync(){}
