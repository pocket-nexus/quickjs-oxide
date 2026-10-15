// VALUE_CASES
// case: if condition sees the entry closure before the authored Annex closure
(function(){var entry;if((entry=single,true))function single(){return single};return typeof entry+'|'+(entry!==single)+'|'+(entry()===entry)+'|'+(single()===entry)})()
// case: skipped if arm still declares an undefined Annex outer
(function(){if(false)function skipped(){};return typeof skipped+'|'+delete skipped})()
// case: same-name if else keeps the last lexical and first Annex child
(function(){var seen;if((seen=duplicate,true))function duplicate(){return 1}else function duplicate(){return 2};return seen()+'|'+duplicate()+'|'+(seen!==duplicate)})()
// case: executed duplicate else does not replace an existing Annex outer
(function(){var duplicate=7,seen;if((seen=duplicate,false))function duplicate(){return 1}else function duplicate(){return 2};return seen()+'|'+duplicate})()
// case: same-name parameter suppresses only the Annex outer write
(function(parameter){var entry;if((entry=parameter,true))function parameter(){return 4};return typeof entry+'|'+parameter+'|'+entry()})(7)
// case: prior lexical suppresses an if-arm Annex outer without hiding its entry
(function(){var entry;let shadow=7;if((entry=shadow,true))function shadow(){return 8};return entry()+'|'+shadow})()
// case: nested if re-enters fresh lexical closures on every loop iteration
(function(){var i=0,first,second;while(i<2)if((i++,i===1?first=looped:second=looped,true))function looped(){return looped};return (first!==second)+'|'+(first()===first)+'|'+(second()===second)+'|'+(looped()===second)})()
// case: continue closes an escaped if lexical before the next iteration
(function(){var i=0,first,second;while(i<2)if((i++,i===1?first=fresh:second=fresh,false))function fresh(){return fresh}else continue;return (first!==second)+'|'+(first()===first)+'|'+(second()===second)+'|'+typeof fresh})()
// case: label chain adds no lexical scope around its function
(function(){var entry;{a:b:function labelled(){return labelled};entry=labelled}return (entry()===entry)+'|'+(labelled()===entry)+'|'+(entry!==labelled)})()
// case: labelled duplicates share block first-Annex last-lexical behavior
(function(){var entry;{a:function labelled(){return 1};b:function labelled(){return 2};entry=labelled}return entry()+'|'+labelled()+'|'+(entry===labelled)})()
// case: Program labelled function has one global self identity
programLabel:function programLabel(){return programLabel};programLabel()===programLabel
// case: repeated Program labels overwrite the global at each source position
a:function programDuplicate(){return 1};b:function programDuplicate(){return 2};programDuplicate()
// case: Program label source write follows a later direct function hoist
programOrderLabel:function programOrder(){return 1};function programOrder(){return 2};programOrder()
// case: Program var initializer runs after a labelled source write
programVarLabel:function programVarLabel(){return 1};var programVarLabel=function(){return 2};programVarLabel()
// case: FunctionBody label lexical shadows a same-name parameter from entry
(function(parameter){var before=typeof parameter;label:function parameter(){return 4};return before+'|'+parameter()})(7)
// case: FunctionBody arguments label shadows the implicit binding without Annex
(function(){var before=typeof arguments;label:function arguments(){return 6};return before+'|'+arguments()})()
// case: Function constructor supports if-arm and labelled Annex B forms
Function("if(true)function one(){return 10};tag:function two(){return 11};return one()+'|'+two()")()
// case: labels may wrap an if which reopens its own Annex B mask
(function(){outer:inner:if(true)function nested(){return 12};return nested()})()
// case: loop body if reopens Annex B even when the loop never executes
(function(){while(false)if(true)function nestedLoop(){};return typeof nestedLoop+'|'+delete nestedLoop})()
// case: skipped Program arm permits the later lexical initializer
if(false)function laterFalse(){};let laterFalse=7;laterFalse
// case: first Annex global record lets a later label replace an initialized lexical
if(false){function firstAnnexRecord(){return 0}};let firstAnnexRecord=3;label:function firstAnnexRecord(){return 2};typeof firstAnnexRecord+'|'+firstAnnexRecord()
// case: first Annex global record masks a later lexical for a block function
if(false){function maskedBlockLexical(){return 0}};let maskedBlockLexical=3;{function maskedBlockLexical(){return 2}};typeof maskedBlockLexical+'|'+maskedBlockLexical()
// case: first Annex global record masks a later lexical for an if-arm function
if(false){function maskedIfLexical(){return 0}};let maskedIfLexical=3;if(true)function maskedIfLexical(){return 2};typeof maskedIfLexical+'|'+maskedIfLexical()
// case: first Annex global record masks repeated Program lexicals
if(false)function repeatedLexical(){};let repeatedLexical=1;let repeatedLexical=2;repeatedLexical
// case: first Annex global record masks a later Program var
if(false)function varAfterLexical(){};let varAfterLexical=1;var varAfterLexical=2;varAfterLexical
// case: first lexical kind controls mutation after masked duplicate declarations
if(false)function firstLetControls(){};let firstLetControls=1;const firstLetControls=2;firstLetControls=3;firstLetControls
// case: first Annex descriptor controls direct reads before and after lexical initialization
if(false)function maskedDirectRead(){};var before=maskedDirectRead+'|'+typeof maskedDirectRead;let maskedDirectRead=1;before+'|'+maskedDirectRead+'|'+typeof maskedDirectRead
// case: first Annex descriptor relays through nested reads before and after lexical initialization
if(false)function maskedNestedRead(){};var read=function(){return typeof maskedNestedRead+'|'+maskedNestedRead};var before=read();let maskedNestedRead=1;before+'|'+read()
// ERROR_CASES
// case: executed Program if Annex write observes a later lexical TDZ
if(true)function laterIfLexical(){};let laterIfLexical;
// case: Program labelled Annex write observes a later lexical TDZ
label:function laterLabelLexical(){};let laterLabelLexical;
// case: first Annex global record exposes a later const to the next Annex write
if(false){function maskedConstLexical(){return 0}};const maskedConstLexical=3;{function maskedConstLexical(){return 2}}
// case: first masked const remains read-only after a duplicate let initializer
if(false)function firstConstControls(){};const firstConstControls=1;let firstConstControls=2;firstConstControls=3
// case: var initializer still observes a masked Program const
if(false)function constVarInitializer(){};const constVarInitializer=1;var constVarInitializer=2
// case: label before masked lexical and var still fails at the authored TDZ write
label:function labelLexicalVar(){};let labelLexicalVar=1;var labelLexicalVar;
// case: first Annex descriptor keeps a later lexical in TDZ for direct writes
if(false)function maskedDirectWrite(){};maskedDirectWrite=2;let maskedDirectWrite=1
// case: first Annex descriptor relay keeps a later lexical in TDZ for nested writes
if(false)function maskedNestedWrite(){};var write=function(){maskedNestedWrite=2};write();let maskedNestedWrite=1
// case: first Annex descriptor keeps an initialized const read-only for direct writes
if(false)function maskedDirectConstWrite(){};const maskedDirectConstWrite=1;maskedDirectConstWrite=2
// case: first Annex descriptor relay keeps an initialized const read-only
if(false)function maskedNestedConstWrite(){};const maskedNestedConstWrite=1;(function(){maskedNestedConstWrite=2})()
// WITH_BOUNDARY_CASES
// case: with directly containing a function remains a QuickJS syntax error
(function(){with(Function)function directWith(){}})()
// case: with containing an if would reopen Annex B once with is implemented
Function.withNested=7;with(Function)if(true)function withNested(){return 13};withNested()+'|'+Function.withNested
