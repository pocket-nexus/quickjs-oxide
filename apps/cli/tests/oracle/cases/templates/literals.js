// mode: completion-units
globalThis.TemplateStringPrototype = String.prototype;
// case: empty no-substitution template
``
// case: plain no-substitution template
`quickjs`
// case: UTF-16 and astral text
`é中🚀`
// case: numeric and Unicode escapes
`\0\x41\u0042\u{1F680}`
// case: lone surrogate escapes
`\uD800x\uDC00`
// case: simple escapes
`\b\t\n\v\f\r`
// case: escaped substitution opener
`\${value}`
// case: escaped template quote
`\``
// case: escaped backslash
`\\`
// case: physical LF normalizes to LF
`a
b`
// case: physical CR normalizes to LF
`ab`
// case: physical CRLF normalizes to LF
`a
b`
// case: physical line separator remains
`a b`
// case: physical paragraph separator remains
`a b`
// case: LF line continuation disappears
`a\
b`
// case: CR line continuation disappears
`a\b`
// case: CRLF line continuation disappears
`a\
b`
// case: line separator continuation disappears
`a\ b`
// case: paragraph separator continuation disappears
`a\ b`
// case: single interpolation
`${42}`
// case: primitive interpolation sequence
`a${1}b${true}c${null}d${void 0}e`
// case: special Number interpolation
`${-0}|${0/0}|${1/0}|${-1/0}`
// case: BigInt interpolation
`x${123456789012345678901234567890n}y`
// case: astral interpolation
`a${"🚀"}b`
// case: adjacent substitutions
`${1}${2}`
// case: empty tail is omitted
`a${1}`
// case: empty head remains the receiver
`${1}z`
// case: raw right brace
`a}b`
// case: escaped interpolation remains text
`a\${1}b`
// case: nested template
`a${`b${2}c`}d`
// case: three nested templates
`a${`b${`c${3}d`}e`}f`
// case: comma expression substitution
`a${(1,2)}b`
// case: conditional substitution
`a${false ? 1 : 2}b`
// case: division inside substitution
`a${8 / 2}b`
// case: division after template
`8` / 2
// case: nested template followed by division
`${`8` / 2}`
// case: function body and call inside substitution
`a${(function(){ return 7; })()}b`
// case: template returned by substitution call
`a${(function(){ return `b${3}c`; })()}d`
// case: substitutions evaluate before native concat coercion
(function(){ var log=''; var a=function(){}, b=function(){}; a.toString=function(){log=log+'a';return 'A';}; b.toString=function(){log=log+'b';return 'B';}; var one=function(){log=log+'1';return a;}; var two=function(){log=log+'2';return b;}; var value=`x${one()}y${two()}z`; return value+'|'+log; })()
// case: no-substitution template skips concat
TemplateStringPrototype.concat=1; `plain`
// case: concat receives primitive receiver and raw arguments
TemplateStringPrototype.concat=function(a,b,c,d,e){'use strict';return typeof this+'|'+this+'|'+typeof a+'|'+a+'|'+typeof b+'|'+b+'|'+typeof c+'|'+c+'|'+typeof d+'|'+d+'|'+typeof e;}; `a${42}b${true}c`
// case: empty later cooked segments are omitted
TemplateStringPrototype.concat=function(a,b,c,d){'use strict';return this+'|'+a+'|'+b+'|'+typeof c+'|'+typeof d;}; `${1}${2}`
// case: concat call waits for every substitution
(function(){var log='';TemplateStringPrototype.concat=function(a,b,c,d){'use strict';log=log+'h';return 'hook:'+this+':'+a+':'+b+':'+c+':'+d;};var one=function(){log=log+'1';return 10;};var two=function(){log=log+'2';return 20;};var value=`a${one()}b${two()}c`;return value+'|'+log;})()
