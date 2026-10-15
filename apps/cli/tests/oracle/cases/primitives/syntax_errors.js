// mode: completion-name
// case: strict mode reaches earlier directive escapes
'\1'; 'use strict'; 0
// case: strict mode rejects legacy octal numbers
'use strict'; 010
// case: conditional consequent excludes comma
true ? 1, 2 : 3
// case: logical or cannot precede nullish
1 || 2 ?? 3
// case: logical and cannot precede nullish
1 && 2 ?? 3
// case: nullish cannot precede logical or
1 ?? 2 || 3
// case: nullish cannot precede logical and
1 ?? 2 && 3
// case: unparenthesized negative power lhs
-2 ** 2
// case: unparenthesized positive power lhs
+2 ** 2
// case: unparenthesized logical-not power lhs
!2 ** 2
// case: unparenthesized bitwise-not power lhs
~2 ** 2
// case: unparenthesized typeof power lhs
typeof 2 ** 2
// case: unparenthesized void power lhs
void 2 ** 2
// case: unparenthesized delete power lhs
delete Function ** 2
// case: unparenthesized unary power rhs chain
2 ** -2 ** 3
// case: unparenthesized unary containing postfix update power lhs
(function(value){ return -value++ ** 2; })(2)
// case: prefix update rejects a power expression operand
(function(value){ return ++(value ** 2); })(2)
// case: postfix update rejects a power expression operand
(function(value){ return (value ** 2)++; })(2)
// case: prefix update rejects a value operand
++1
// case: postfix update rejects a value operand
1++
// case: adjacent numeric expressions need ASI
1 2
// case: malformed hexadecimal literal
0x
// case: legacy octal cannot have a fraction
01.1
// case: legacy octal cannot have an exponent
01e1
// case: legacy leading-zero decimal cannot use separators
08.1_2
// case: unary plus rejects BigInt
+1n
// case: mixed numeric addition rejects BigInt
1n + 1
// case: BigInt division by zero
1n / 0n
// case: strict function rejects future reserved parameter
(function(implements) { 'use strict'; return implements; })(1)
// case: strict function rejects eval var binding
(function() { 'use strict'; var eval = 1; return eval; })()
// case: strict function rejects future reserved identifier reference
(function() { 'use strict'; return impl\u0065ments; })()
// case: strict function rejects eval expression name
(function eval() { 'use strict'; })
// case: escaped reserved word is not an identifier
(function(\u0069f) { return \u0069f; })(1)
