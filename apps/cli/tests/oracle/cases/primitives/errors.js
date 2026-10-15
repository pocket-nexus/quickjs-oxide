// RUNTIME_ERROR_CASES
// case: strict variable binding syntax message
(function(){ 'use strict'; var eval; })
// case: strict argument binding syntax message
(function(arguments){ 'use strict'; })
// case: strict function-name syntax message
(function eval(){ 'use strict'; })
// case: strict reserved identifier syntax message
(function(){ 'use strict'; var impl\u0065ments; })
// case: logical before nullish syntax message
1 || 2 ?? 3
// case: nullish before logical syntax message
1 ?? 2 || 3
// case: unparenthesized unary power syntax message
-2 ** 2
// case: prefix update invalid operand
++1
// case: postfix update invalid operand
1++
// case: strict eval prefix update early error
(function(){ 'use strict'; ++eval; })
// case: strict arguments postfix update early error
(function(){ 'use strict'; arguments++; })
// case: missing global postfix update
__qjo_missing_update_global++
// case: strict private name postfix update
(function named(){ 'use strict'; return named++; })()
// case: identifier compound missing global
__qjo_missing_compound += 1
// case: identifier logical compound missing global
__qjo_missing_logical ||= 1
// case: strict private name arithmetic write
(function named(){ 'use strict'; named += 1; })()
// case: strict private name logical write
(function named(){ 'use strict'; named &&= 1; })()
// case: captured strict private name logical write
(function named(){ 'use strict'; return function(){ named &&= 1; }; })()()
// case: strict eval compound early error
(function(){ 'use strict'; eval += 1; })
// case: strict eval simple assignment early error
(function(){ 'use strict'; eval = 1; })
// case: strict arguments compound early error
(function(){ 'use strict'; (arguments) ??= 1; })
// case: strict arguments simple assignment early error
(function(){ 'use strict'; (arguments) = 1; })
// case: mixed BigInt arithmetic
1n + 1
// case: mixed BigInt bitwise left
1n & 1
// case: mixed BigInt bitwise right
1 | 1n
// case: mixed BigInt shift left
1n << 1
// case: mixed BigInt shift right
1 >> 1n
// case: unsigned shift rejects two BigInts
1n >>> 0n
// case: unsigned shift rejects bigint lhs
1n >>> 0
// case: unsigned shift rejects bigint rhs
1 >>> 0n
// case: oversized bigint left shift
1n << 1048576n
// case: mixed BigInt exponent lhs
1n ** 1
// case: mixed BigInt exponent rhs
1 ** 1n
// case: negative BigInt exponent
2n ** -1n
// case: zero BigInt negative exponent
0n ** -1n
// case: power-of-two BigInt allocation boundary
2n ** 1048575n
// case: negative power-of-two BigInt allocation boundary
(-2n) ** 1048576n
// case: BigInt exponent mathematical size guard
2n ** 1048577n
// case: BigInt exponent int32 guard
3n ** 2147483648n
// case: four power BigInt allocation boundary
4n ** 524288n
// case: extended BigInt power identity allocation guard
(1n << 1048575n) ** 1n
// case: negative exponent precedes extended base allocation guard
(1n << 1048575n) ** -1n
// case: nominal BigInt square preallocation guard
(1n << 1048574n) ** 2n
// case: postfix BigInt update allocation guard
(function(){ var value = 1n << 1048575n; return value++; })()
// case: prefix BigInt decrement allocation guard
(function(){ var value = 1n << 1048575n; return --value; })()
// case: generic BigInt square next allocation boundary
((1n << 524287n) + 1n) ** 2n
// case: nonzero bigint reverse huge right shift overflows
1n >> -999999999999999999999999999999n
// case: extended bigint zero left shift allocation guard
(1n << 1048575n) << 0n
// case: extended bigint zero right shift allocation guard
(1n << 1048575n) >> 0n
// case: extended bigint sub-limb right shift allocation guard
(1n << 1048575n) >> 63n
// case: extended bigint bitwise allocation guard
(1n << 1048575n) & -1n
// case: extended bigint arithmetic allocation guard
(1n << 1048575n) + 0n
// case: extended bigint string allocation guard
'' + (1n << 1048575n)
// case: max allocation bigint multiply preallocation guard
(1n << 1048574n) * 0n
// case: max allocation bigint divide preallocation guard
(1n << 1048574n) / 1n
// case: max allocation bigint remainder preallocation guard
(1n << 1048574n) % 1n
// case: BigInt unary plus
+1n
// case: strict private name bitwise write
(function named(){ 'use strict'; named &= 1; })()
// case: strict eval bitwise compound early error
(function(){ 'use strict'; eval &= 1; })
// case: strict arguments bitwise compound early error
(function(){ 'use strict'; (arguments) ^= 1; })
// case: strict private name shift write
(function named(){ 'use strict'; named <<= 1; })()
// case: strict eval shift compound early error
(function(){ 'use strict'; eval >>= 1; })
// case: strict arguments shift compound early error
(function(){ 'use strict'; (arguments) >>>= 1; })
// case: strict private name exponent write
(function named(){ 'use strict'; named **= 1; })()
// case: strict eval exponent compound early error
(function(){ 'use strict'; eval **= 1; })
// case: strict arguments exponent compound early error
(function(){ 'use strict'; (arguments) **= 1; })
// case: call non-callable
(1)()
// case: construct non-callable
new 1
// case: throw line terminator
throw
9
// case: typeof comma expression still resolves its identifier
typeof (0, __qjo_missing_global)
// case: strict named function assignment is read only
(function named() { 'use strict'; named = 1; })()
// case: captured strict named function assignment is read only
(function named() { 'use strict'; return function() { named = 1; }; })()()
// FUTURE_RESERVED_ERROR_CASES
// case: sloppy always-reserved enum statement
enum;
// case: strict always-reserved export statement
'use strict'; export;
// case: always-reserved extends expression
1 + extends;
// case: bare import expression
import;
// case: always-reserved variable binding
var enum;
// case: always-reserved formal binding
function f(import){}
// case: escaped always-reserved reference
exp\u006frt;
// case: escaped always-reserved binding
var ext\u0065nds;
// case: direct eval reserved statement
eval("enum;")
// case: strict direct eval reserved statement
eval("'use strict'; export;")
// case: indirect eval reserved statement
(0, eval)("extends;")
// case: eval bare import
eval("import;")
// case: empty dynamic import
import()
// case: spread dynamic import
import(...source)
// case: adjacent dynamic import arguments
import(source extra)
// case: three dynamic import arguments
import(source, {}, extra)
// case: dynamic import as new target
new import(source)
// case: empty dynamic import as new target
new import()
// case: dynamic import as new target across comment trivia
new/*gap*/import(source)
// case: dynamic import as new target across newline trivia
new
import(source)
// case: dynamic import as new target before call trivia
new import /* gap */ (source)
// case: nested dynamic import as new target across trivia
(() => new/*gap*/import(source))
// case: dynamic import assignment target
import(source) = 1
// case: dynamic import postfix update
import(source)++
// case: dynamic import prefix update
++import(source)
// case: script import.meta
import.meta
// case: script import property other than meta
import.foo
// case: script escaped import meta
import.\u006deta
// case: optional call directly on import
import?.(source)
// case: tagged template directly on import
import`source`
// case: optional tagged template directly after import call
import('fixture')?.`tag`
// case: tagged template after an import optional chain
import('fixture')?.then`tag`
// case: optional chain after parenthesized dynamic import new target
new (import('fixture'))?.then
// case: valid dynamic import defers to a later syntax error
import('fixture'); enum;
// case: valid dynamic import defers to a private field early error
import('fixture'); class C { method(){ return this.#missing; } }
// case: valid dynamic import defers to a private-in early error
import('fixture'); class C { method(object){ return #missing in object; } }
// case: direct eval import defers to a private field early error
eval("import('fixture'); class C { method(){ return this.#missing; } }")
// case: direct eval import.meta
eval("import.meta")
// case: indirect eval malformed dynamic import
(0, eval)("import(...source)")
// case: eval escaped always-reserved reference
eval("imp\\u006frt;")
// case: escaped dynamic import keyword
im\u0070ort('module')
// case: typeof bare dynamic import keyword
typeof import;
// case: strict dynamic import options yield identifier
'use strict'; import('module', yield);
