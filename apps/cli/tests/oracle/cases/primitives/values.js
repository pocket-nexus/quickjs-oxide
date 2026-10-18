// mode: completion-units
// CASES
// Arithmetic, precedence, and primitive numeric coercion.
// case: arithmetic precedence
(1 + 2 * 3
)
// case: parenthesized arithmetic
((1 + 2) * 3
)
// case: subtraction and division
(7 - 10 / 2
)
// case: signed remainder
(-7 % 4
)
// case: unary plus string coercion
(+' 42 '
)
// case: subtraction coercion
('9' - true
)
// case: numeric literal precision
(9007199254740991 + 1
)
// case: invalid hexadecimal separator string
(+'0x1_'
)
// case: invalid hexadecimal sign string
(+'0x+1'
)
// case: non-ecmascript infinity spelling
(+'inf'
)
// case: fraction-only numeric string
(+'.125'
)
// case: trailing-dot exponent numeric string
(+'1.e2'
)
// case: signed exponent numeric string
(+'-1.5e+2'
)
// case: non-ascii whitespace after number
(0 +1
)
// Exponentiation lives above multiplicative expressions, associates to the
// right, and retains QuickJS's libc-pow special cases.
// case: number exponentiation
(2 ** 10
)
// case: exponentiation is right associative
(2 ** 3 ** 2
)
// case: power binds before multiplication
(2 * 3 ** 2
)
// case: power result binds into multiplication
(2 ** 3 * 4
)
// case: power accepts unary rhs
(2 ** -2
)
// case: parenthesized negative power base
((-2) ** 3
)
// case: parenthesized power under unary minus
(-(2 ** 2)
)
// case: parenthesized power inside unary rhs
(2 ** -(2 ** 3)
)
// case: nan to the zero power
((0 / 0) ** 0
)
// case: undefined to the zero power
((void 0) ** 0
)
// case: one to nan power is nan
(1 ** (0 / 0)
)
// case: one to infinite power is nan
(1 ** (1 / 0)
)
// case: negative one to negative infinite power is nan
((-1) ** (-1 / 0)
)
// case: negative zero odd positive power
((-0) ** 3
)
// case: negative zero even positive power
((-0) ** 2
)
// case: negative zero odd negative power
((-0) ** -3
)
// case: negative zero even negative power
((-0) ** -2
)
// case: negative base fractional power
((-2) ** 0.5
)
// case: minimum positive power subnormal
(2 ** -1074
)
// case: positive power underflow
(2 ** -1075
)
// case: negative power underflow keeps sign
((-2) ** -1075
)
// case: negative infinity odd power
((-1 / 0) ** 3
)
// case: negative infinity negative odd power
((-1 / 0) ** -3
)
// case: negative infinity fractional power
((-1 / 0) ** 0.5
)
// case: power primitive conversion
('2' ** true
)
// case: power expression and coercion order
((function(){ var log = ''; var left = function(){}; var right = function(){}; left.valueOf = function(){ log = log + 'l'; return 2; }; right.valueOf = function(){ log = log + 'r'; return 3; }; var evalLeft = function(){ log = log + 'L'; return left; }; var evalRight = function(){ log = log + 'R'; return right; }; var result = evalLeft() ** evalRight(); return result + '|' + log; })()
)
// case: right associative power evaluation and coercion order
((function(){ var log = ''; var a = function(){}, b = function(){}, c = function(){}; a.valueOf = function(){ log = log + 'a'; return 2; }; b.valueOf = function(){ log = log + 'b'; return 3; }; c.valueOf = function(){ log = log + 'c'; return 2; }; var evalA = function(){ log = log + 'A'; return a; }; var evalB = function(){ log = log + 'B'; return b; }; var evalC = function(){ log = log + 'C'; return c; }; var result = evalA() ** evalB() ** evalC(); return result + '|' + log; })()
)
// case: power object operands both convert to BigInt
((function(){ var log = ''; var left = function(){}, right = function(){}; left.valueOf = function(){ log = log + 'l'; return 2n; }; right.valueOf = function(){ log = log + 'r'; return 10n; }; return (left ** right) + '|' + log; })()
)
// QuickJS bitwise operators apply ToNumeric, then signed ToInt32 for
// Numbers or infinite-width two's-complement operations for BigInts.
// case: bitwise not zero
(~0
)
// case: bitwise not modulo 2^32
(~4294967296
)
// case: bitwise not nan
(~(0 / 0)
)
// case: bitwise not infinity
(~(1 / 0)
)
// case: bitwise not signed boundary
(~2147483648
)
// case: bitwise not unsigned boundary
(~4294967295
)
// case: bitwise fractional ToInt32
(-1.9 & 3.7
)
// case: bitwise positive wrap
(4294967297 & -1
)
// case: bitwise negative wrap
(-2147483649 | 0
)
// case: bitwise exact integer wrap
(9007199254740991 | 0
)
// case: bitwise huge finite wrap
(1e300 | 1
)
// case: bitwise removes negative zero
(-0 ^ 0
)
// case: bitwise primitive coercion
('7' ^ true
)
// case: bitwise precedence
(1 | 2 ^ 3 & 4
)
// case: equality binds before bitwise
(1 | 2 === 3
)
// case: bitwise binds before logical
(0 || 1 | 2
)
// case: bitwise binds inside nullish rhs
(null ?? 1 | 2
)
// case: bitwise expression and coercion order
((function(){ var log = ''; var left = function(){}; var right = function(){}; left.valueOf = function(){ log = log + 'l'; return 6; }; right.valueOf = function(){ log = log + 'r'; return 3; }; var evalLeft = function(){ log = log + 'L'; return left; }; var evalRight = function(){ log = log + 'R'; return right; }; var result = evalLeft() & evalRight(); return result + '|' + log; })()
)
// Shift operators share QuickJS's ordered ToNumeric path. Number counts
// are masked to five bits, while >>> preserves an unsigned 32-bit result.
// case: shift left number
(1 << 3
)
// case: shift right signed number
(-8 >> 2
)
// case: shift right unsigned number
(-1 >>> 0
)
// case: shift count masks at 32
(1 << 33
)
// case: negative number shift count wraps
(1 << -1
)
// case: shift count masks uint32 max
(1 << 4294967295
)
// case: shift count wraps at uint32
(1 << 4294967296
)
// case: signed shift converts lhs
(4294967295 >> 0
)
// case: unsigned shift converts lhs
(4294967295 >>> 0
)
// case: nan shifts as zero
((0 / 0) << 5
)
// case: nan shift count is zero
(7 >> (0 / 0)
)
// case: infinity shifts as zero
((1 / 0) >>> 0
)
// case: infinity shift count is zero
(7 >>> (1 / 0)
)
// case: shift removes negative zero
(-0 << 0
)
// case: fractional shift conversion
(-9.9 >> 1.9
)
// case: string and boolean shift conversion
('7' << true
)
// case: additive binds before shift
(1 + 2 << 3
)
// case: shift rhs includes additive
(16 >> 1 + 1
)
// case: shift binds before relational
(1 << 2 < 5
)
// case: shift binds before bitwise
(8 >> 1 & 3
)
// case: shift operators are left associative
(64 >> 2 >> 1
)
// case: shift binds inside nullish rhs
(1 ?? 2 << 3
)
// case: shift expression and coercion order
((function(){ var log = ''; var left = function(){}; var right = function(){}; left.valueOf = function(){ log = log + 'l'; return 8; }; right.valueOf = function(){ log = log + 'r'; return 1; }; var evalLeft = function(){ log = log + 'L'; return left; }; var evalRight = function(){ log = log + 'R'; return right; }; var result = evalLeft() >>> evalRight(); return result + '|' + log; })()
)
// Number observations that String(number) alone cannot distinguish.
// case: positive infinity
(1 / 0
)
// case: negative infinity
(-1 / 0
)
// case: nan
(0 / 0
)
// case: negative zero literal
(-0
)
// case: negative zero arithmetic
(0 * -1
)
// Addition and strings, including astral and unpaired UTF-16 units.
// case: string and number concatenation
('answer: ' + 42
)
// case: string and null concatenation
('value=' + null
)
// case: astral utf16 concatenation
('\uD83D' + '\uDE80'
)
// case: unpaired utf16 concatenation
('\uD800' + 'x'
)
// case: literal astral character
('a🚀' + 'z'
)
// Abstract and strict equality.
// case: abstract numeric string equality
('42' == 42
)
// case: strict numeric string inequality
('42' !== 42
)
// case: abstract null undefined equality
(null == void 0
)
// case: strict null undefined inequality
(null !== void 0
)
// case: abstract boolean equality
(false == 0
)
// case: nan is not equal to itself
(0 / 0 != 0 / 0
)
// case: number representations are strictly equal
(1 === 1.0
)
// case: trimmed radix string equality
('\uFEFF 0x10 ' == 16
)
// Numeric and UTF-16 lexicographic relational comparison.
// case: numeric less than
(2 < 10
)
// case: relational numeric coercion
('10' < 2
)
// case: lexicographic string comparison
('10' < '2'
)
// case: utf16 string comparison
('\uD800' < '\uE000'
)
// case: nan relational comparison
(0 / 0 >= 0
)
// Logical operators preserve the selected operand, including the sign of zero.
// case: short circuit and
(0 && -0
)
// case: short circuit or
(1 || -0
)
// case: logical and selected value
('left' && 'right'
)
// case: logical or selected value
('' || 'fallback'
)
// Nullish coalescing uses exact null/undefined checks and preserves identity.
// case: nullish null selects rhs
(null ?? 42
)
// case: nullish undefined selects rhs
(void 0 ?? 'fallback'
)
// case: nullish false preserves lhs
(false ?? true
)
// case: nullish zero preserves lhs
(0 ?? 1
)
// case: nullish negative zero preserves lhs
(-0 ?? 1
)
// case: nullish empty string preserves lhs
('' ?? 'fallback'
)
// case: nullish chain selects last
(null ?? void 0 ?? 'last'
)
// case: nullish short circuit skips rhs
((function(){ var calls = 0; var value = 1 ?? (calls = 1); return value + calls; })()
)
// case: nullish chain stops at zero
((function(){ var calls = 0; var value = null ?? (calls = calls + 1, 0) ?? (calls = 99); return value + calls; })()
)
// case: nullish arithmetic precedence
(null ?? 1 + 2 * 3
)
// case: nullish conditional precedence
(0 ?? 1 ? 2 : 3
)
// case: parenthesized logical before nullish
((false || 4) ?? 5
)
// case: parenthesized logical after nullish
(null ?? (false || 6)
)
// case: nullish anonymous function is not named
((function(){ var inferred; inferred = null ?? function(){}; return inferred.name; })()
)
// case: nullish result call drops member receiver
((Function.__nullishMethod = function(){ return this === Function; }, (Function.__nullishMethod ?? Function)())
)
// case: nullish rhs member call keeps receiver
((Function.__nullishRhs = function(){ return this === Function; }, null ?? Function.__nullishRhs())
)
// Identifier compound assignment shares QuickJS's late scope get/set path.
// case: identifier arithmetic compound local matrix
((function(){ var value = 20; value += 2; value -= 4; value *= 3; value /= 2; value %= 5; return value; })()
)
// case: identifier arithmetic compound argument
((function(value){ value += 2; return value; })(3)
)
// case: identifier arithmetic compound closure
((function(value){ return (function(){ value += 2; return value; })() + (function(){ value += 3; return value; })(); })(1)
)
// case: identifier arithmetic compound global
((__qjo_identifier_compound = 1, __qjo_identifier_compound += 2)
)
// case: identifier arithmetic compound is right associative
((function(){ var left = 1, right = 2; var result = left += right *= 3; return result + left + right; })()
)
// case: identifier logical and taken
((function(value){ value &&= 9; return value; })(2)
)
// case: identifier logical and short
((function(value){ value &&= 9; return value; })(0)
)
// case: identifier logical or taken
((function(value){ value ||= 9; return value; })(0)
)
// case: identifier logical or short
((function(value){ value ||= 9; return value; })(2)
)
// case: identifier logical nullish taken
((function(value){ value ??= 9; return value; })(null)
)
// case: identifier logical nullish false short
((function(value){ value ??= 9; return value; })(false)
)
// case: identifier logical function object short
(typeof (Function ||= 1)
)
// case: identifier logical direct name inference
((function(){ var named; named ??= function(){}; return named.name; })()
)
// case: identifier logical parenthesized lhs has no name inference
((function(){ var named; (named) ??= function(){}; return named.name; })()
)
// case: identifier logical parenthesized assignment still infers
((function(){ var named; (named ??= function(){}); return named.name; })()
)
// case: identifier simple parenthesized lhs has no name inference
((function(){ var named; (named) = function(){}; return named.name; })()
)
// case: identifier logical right associative inner name
((function(){ var a = 0, b = 0; a ||= b ||= function(){}; return a.name + '|' + b.name; })()
)
// case: identifier logical comma rhs has no name inference
((function(){ var named; named ||= (0, function(){}); return named.name; })()
)
// case: sloppy private name arithmetic ignores write
((function named(){ var result = named += ''; return typeof result + '|' + typeof named; })()
)
// case: sloppy private name postfix update ignores write
((function named(){ var before = named; named.valueOf = function(){ return 4; }; var old = named++; return old + '|' + (named === before) + '|' + typeof named; })()
)
// case: sloppy private name logical names rhs and ignores write
((function named(){ var before = named; var result = named &&= function(){}; return (named === before) + '|' + result.name; })()
)
// case: strict private name logical short
((function named(){ 'use strict'; var before = named; return (named ||= 1) === before; })()
)
// case: captured sloppy private name ignores write
((function named(){ return function(){ named += ''; return typeof named; }; })()()
)
// case: identifier bitwise compound matrix
((function(){ var value = 14; value &= 11; value ^= 3; value |= 4; return value; })()
)
// case: identifier bitwise compound is right associative
((function(){ var left = 1, right = 3; var result = left |= right &= 2; return result * 100 + left * 10 + right; })()
)
// case: fixed member bitwise compound matrix
((function(){ Function.__qjo_bits = 14; Function.__qjo_bits &= 11; Function.__qjo_bits ^= 3; return Function.__qjo_bits |= 4; })()
)
// case: computed member bitwise compound converts key once
((function(){ var log = ''; var key = function(){}; key.toString = function(){ log = log + 'k'; return '__qjo_computed_bits'; }; Function.__qjo_computed_bits = 14; var result = Function[key] &= 11; return result + '|' + Function.__qjo_computed_bits + '|' + log; })()
)
// case: bitwise compound does not infer anonymous function names
((function(){ var names = ''; Function.prototype.valueOf = function(){ names = names + this.name + '|'; return 1; }; var direct = 0, paren = 0; direct |= function(){}; (paren) |= function(){}; Function.__qjo_name = 0; Function.__qjo_name |= function(){}; delete Function.prototype.valueOf; return names; })()
)
// case: identifier shift compound matrix
((function(){ var value = 8; value <<= 2; value >>= 1; value >>>= 2; return value; })()
)
// case: identifier shift compound is right associative
((function(){ var left = 1, right = 3; var result = left <<= right >>= 1; return result * 100 + left * 10 + right; })()
)
// case: bigint shift compound assignment
((function(){ var value = 1n; value <<= 65n; value >>= 64n; return value; })()
)
// case: fixed member shift compound matrix
((function(){ Function.__qjo_shift = -8; Function.__qjo_shift >>= 1; return Function.__qjo_shift >>>= 1; })()
)
// case: computed member shift compound converts key once
((function(){ var log = ''; var key = function(){}; key.toString = function(){ log = log + 'k'; return '__qjo_computed_shift'; }; Function.__qjo_computed_shift = 3; var result = Function[key] <<= 2; return result + '|' + Function.__qjo_computed_shift + '|' + log; })()
)
// case: shift compound does not infer anonymous function names
((function(){ var names = ''; Function.prototype.valueOf = function(){ names = names + this.name + '|'; return 1; }; var direct = 1, paren = 1; direct <<= function(){}; (paren) >>= function(){}; Function.__qjo_shift_name = 1; Function.__qjo_shift_name >>>= function(){}; delete Function.prototype.valueOf; return names; })()
)
// case: identifier exponent compound
((function(value){ value **= 3; return value; })(2)
)
// case: identifier exponent compound is right associative
((function(){ var left = 2, right = 3; var result = left **= right **= 2; return result + left + right; })()
)
// case: bigint exponent compound assignment
((function(){ var value = 2n; value **= 100n; return value; })()
)
// case: fixed member exponent compound
((function(){ Function.__qjo_power = 2; var result = Function.__qjo_power **= 3; return result + Function.__qjo_power; })()
)
// case: computed member exponent compound converts key once
((function(){ var log = ''; var key = function(){}; key.toString = function(){ log = log + 'k'; return '__qjo_computed_power'; }; Function.__qjo_computed_power = 2; var result = Function[key] **= 3; return result + '|' + Function.__qjo_computed_power + '|' + log; })()
)
// case: exponent compound does not infer anonymous function names
((function(){ var names = ''; Function.prototype.valueOf = function(){ names = names + this.name + '|'; return 1; }; var direct = 1, paren = 1; direct **= function(){}; (paren) **= function(){}; Function.__qjo_power_name = 1; Function.__qjo_power_name **= function(){}; delete Function.prototype.valueOf; return names; })()
)
// Update expressions retain QuickJS's dedicated inc/dec and post-inc/dec
// semantics: postfix returns the converted old Numeric while prefix
// returns the replacement written through the original Reference.
// case: identifier update value and conversion matrix
((function(){ var value = '01'; var a = value++; var b = ++value; var c = value--; var d = --value; return typeof a + '|' + a + '|' + b + '|' + c + '|' + d + '|' + value; })()
)
// case: postfix object returns converted old numeric
((function(){ var value = function(){}; value.valueOf = function(){ return 4; }; var original = value; var old = value++; return (old === original) + '|' + typeof old + '|' + old + '|' + value; })()
)
// case: bigint update matrix
((function(){ var value = 4n; var old = value--; var replacement = ++value; return old * 100n + replacement * 10n + value; })()
)
// case: bigint decrement subtracts one outside int64
((function(){ var out = []; [2n ** 63n, -(2n ** 63n), 2n ** 64n, -(2n ** 64n), 2n ** 100n].forEach(function (start) { var a = start, b = start; var old = b--; out.push(String(--a - start), String(old - start), String(b - start)); }); return out.join(','); })()
)
// case: captured identifier updates share one cell
((function(value){ var update = function(){ return value++; }; return update() * 100 + (++value) * 10 + value; })(2)
)
// case: global identifier postfix update
((__qjo_update_global = '01', __qjo_update_global++ * 10 + __qjo_update_global)
)
// case: fixed member update preserves old and new values
((function(){ Function.__qjo_update = '01'; var old = Function.__qjo_update++; return old + '|' + ++Function.__qjo_update; })()
)
// case: computed member update converts key once
((function(){ var log = ''; var key = function(){}; key.toString = function(){ log = log + 'k'; return '__qjo_computed_update'; }; Function.__qjo_computed_update = '01'; var old = Function[key]++; return typeof old + '|' + old + '|' + Function.__qjo_computed_update + '|' + log; })()
)
// case: prefix and postfix updates bind before power
((function(){ var value = 2; return (++value ** 2) * 100 + (value++ ** 2) * 10 + value; })()
)
// case: postfix line terminator starts prefix statement
((function(){ var x = 1, y = 2; var result = x
++y; return result * 100 + x * 10 + y; })()
)
// case: postfix CRLF starts prefix statement
((function(){ var x = 1, y = 2; var result = x
++y; return result * 100 + x * 10 + y; })()
)
// case: postfix line-separator starts prefix statement
((function(){ var x = 1, y = 2; var result = x ++y; return result * 100 + x * 10 + y; })()
)
// case: postfix paragraph-separator starts prefix statement
((function(){ var x = 1, y = 2; var result = x ++y; return result * 100 + x * 10 + y; })()
)
// case: postfix block-comment line starts prefix statement
((function(){ var x = 1, y = 2; var result = x/*
*/++y; return result * 100 + x * 10 + y; })()
)
// case: postfix survives comment without line terminator
((function(){ var value = 1; var old = value/**/++; return old * 10 + value; })()
)
// case: prefix accepts a line terminator
((function(){ var value = 1; return ++
value; })()
)
// case: parenthesized references remain update targets
((function(){ var value = 1; var old = (value)++; return old * 100 + (++(value)) * 10 + value; })()
)
// Conditional selection and associativity.
// case: truthy conditional
('x' ? -0 : 1
)
// case: falsy conditional
(0 ? 1 : 'no'
)
// case: nested conditional
(false ? 0 : true ? 1 : 2
)
// Every currently materialized JavaScript primitive typeof result.
// case: typeof undefined
(typeof void 0
)
// case: typeof null
(typeof null
)
// case: typeof boolean
(typeof false
)
// case: typeof number
(typeof (1 / 0)
)
// case: typeof string
(typeof 'oxide'
)
// case: typeof global intrinsic
(typeof Error
)
// case: inherited global object lookup
(typeof toString
)
// case: typeof missing global
(typeof __qjo_missing_global
)
// case: typeof parenthesized missing global
(typeof ((__qjo_missing_global))
)
// Source-level functions use late identifier resolution and runtime FClosure.
// case: anonymous function parameters and direct call
((function(a, b) { return a + b; })(20, 22)
)
// case: missing function argument is undefined
((function(a) { return typeof a; })()
)
// case: extra function argument preserves declared slot
((function(a) { return a; })(42, 99)
)
// case: sloppy escaped contextual parameter and reference
((function(impl\u0065ments) { return impl\u0065ments; })(1)
)
// case: ordinary function fallthrough
((function() { 1; })()
)
// case: return restricted production ASI
((function() { return
42; })()
)
// case: transitive captured parameter
((function(a) { return function() { return function(b) { return a + b; }; }; })(20)()(22)
)
// case: transitive captured function local
((function() { var a = 20; return function() { return function(b) { return a + b; }; }; })()()(22)
)
// case: var binding is hoisted before initialization
((function() { return x; var x = 1; })()
)
// case: named function private recursion
((function fact(n) { return n ? n * fact(n - 1) : 1; })(5)
)
// case: dynamic Function anonymous wrapper self binding
((function(f) { return f() === f; })(function anonymous() { return anonymous; })
)
// case: named function transitive private capture
((function(f) { return f()()() === f; })(function named() { return function() { return function() { return named; }; }; })
)
// case: sloppy named function assignment preserves private binding
((function(f) { return f() === f; })(function named() { named = 1; return named; })
)
// case: nested strict write inherits sloppy function-name binding behavior
((function(f) { return f()() === f; })(function named() { return function() { 'use strict'; named = 1; return named; }; })
)
// case: named function parameter shadows private binding
((function named(named) { return named; })(42)
)
// case: named function var shadows private binding
((function named() { var named = 42; return named; })()
)
// case: named function binding is not visible outside
((function named() {}), typeof named
)
// case: typeof function object
(typeof (function() {})
)
// QuickJS short/heap BigInt normalization and primitive integration.
// case: short bigint addition
(1n + 2n
)
// case: heap bigint multiplication
(123456789012345678901234567890n * 98765432109876543210n
)
// case: bigint exponentiation
(2n ** 100n
)
// case: negative bigint exponentiation
((-2n) ** 3n
)
// case: zero bigint to zero power
(0n ** 0n
)
// case: zero bigint huge exponent shortcut
(0n ** 999999999999999999999999999999999999999n
)
// case: one bigint huge exponent shortcut
(1n ** 999999999999999999999999999999999999999n
)
// case: negative one bigint huge odd exponent shortcut
((-1n) ** 999999999999999999999999999999999999999n
)
// case: quickjs power of two allocation success boundary
(typeof (2n ** 1048574n)
)
// case: quickjs negative power allocation sign boundary
(typeof ((-2n) ** 1048575n)
)
// case: quickjs four power allocation success boundary
(typeof (4n ** 524287n)
)
// case: quickjs generic bigint square allocation boundary
(typeof (((1n << 524286n) + 1n) ** 2n)
)
// case: quickjs nominal bigint power identity
(((1n << 1048574n) ** 1n) >> 1048574n
)
// case: quickjs extended bigint zeroth power shortcut
((1n << 1048575n) ** 0n
)
// case: bigint division truncates toward zero
(-7n / 3n
)
// case: bigint remainder follows dividend
(-7n % 3n
)
// case: bigint unary negation
(-9223372036854775808n
)
// case: bigint bitwise not
(~0n
)
// case: bigint bitwise not negative one
(~-1n
)
// case: bigint negative xor
(-1n ^ 255n
)
// case: bigint negative and sign extension
(-5n & 3n
)
// case: bigint negative or sign extension
(-5n | 2n
)
// case: bigint negative xor sign extension
(-5n ^ 2n
)
// case: heap bigint bitwise and
(123456789012345678901234567890n & -1n
)
// case: heap bigint bitwise compound matrix
((function(){ var value = -1n; value &= 123456789012345678901234567890n; value ^= 15n; value |= 2n; return value; })()
)
// case: bigint left shift
(1n << 65n
)
// case: negative bigint arithmetic right shift
(-9n >> 2n
)
// case: bigint negative left count reverses
(8n << -1n
)
// case: bigint negative right count reverses
(8n >> -2n
)
// case: bigint huge right shift saturates positive
(123456789n >> 999999999999999999999999999999n
)
// case: bigint huge right shift saturates negative
(-1n >> 999999999999999999999999999999n
)
// case: zero bigint huge left shift stays zero
(0n << 999999999999999999999999999999n
)
// case: zero bigint reverse huge right shift stays zero
(0n >> -999999999999999999999999999999n
)
// case: quickjs bigint shift sign-limb extension boundary
(typeof (1n << 1048575n)
)
// case: quickjs bigint shift boundary round trip
((1n << 1048575n) >> 1048575n
)
// case: quickjs bigint extended value right shifts one limb
(((1n << 1048575n) >> 64n) >> 1048511n
)
// case: quickjs bigint extended value negative count shifts one limb
(((1n << 1048575n) << -64n) >> 1048511n
)
// case: quickjs negative bigint shift boundary
(typeof (-1n << 1048575n)
)
// case: quickjs bigint add sign-limb extension boundary
(typeof ((1n << 1048574n) + (1n << 1048574n))
)
// case: quickjs bigint negation sign-limb extension boundary
(typeof -(-1n << 1048575n)
)
// case: quickjs max allocation bigint additive identity
(((1n << 1048574n) + 0n) >> 1048574n
)
// case: bigint string concatenation
(1n + ' oxide'
)
// case: bigint number abstract equality
(1n == 1
)
// case: bigint boolean abstract equality
(0n == false
)
// case: bigint string abstract equality
(255n == '0xff'
)
// case: bigint number precision inequality
(9007199254740993n == 9007199254740993
)
// case: bigint fractional comparison
(2n < 2.5
)
// case: negative bigint fractional comparison
(-2n > -2.5
)
// case: bigint string relational conversion
(10n < '11'
)
// case: bigint logical selection
(0n || 2n
)
// case: typeof bigint
(typeof 1n
)
// FUTURE_RESERVED_VALUE_CASES
// case: always-reserved data property IdentifierNames
(({enum:20,export:22}).enum+({enum:20,export:22}).export
)
// case: always-reserved method property IdentifierNames
(({extends(){return 20},import(){return 22}}).extends()+({extends(){return 20},import(){return 22}}).import()
)
// case: escaped always-reserved property IdentifierNames
(({en\u0075m:20,exp\u006frt:22}).en\u0075m+({en\u0075m:20,exp\u006frt:22}).exp\u006frt
)
