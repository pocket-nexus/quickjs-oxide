// Pins QuickJS 2026-06-04 `js_string_trim` (quickjs.c 46189-46213), its
// GenericMagic table entries and alias entries (46649-46653),
// `JS_ToStringCheckObject` (13670-13676), `js_sub_string` (3964-3990), and
// `lre_is_space`/`char_range_s` (libunicode.h 140-166; libunicode.c
// 1885-1917). The magic bitmask uses one for the leading end and two for the
// trailing end. `trimRight` copies the `trimEnd` function value and `trimLeft`
// copies the `trimStart` function value, so aliases share canonical callable
// identity and names initially while their writable properties mutate
// independently afterward.
//
// Raw UTF-16 observations encode code units as hexadecimal before crossing the
// process boundary, so lone surrogates never depend on terminal UTF-8 handling.
