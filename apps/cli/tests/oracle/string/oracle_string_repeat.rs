// Pins QuickJS 2026-06-04 `js_string_repeat` (quickjs.c 46144-46187),
// its String prototype table entry (46644), `JS_ToInt64SatFree` (13191-13234),
// `JS_ToStringCheckObject` (13670-13676), and the 30-bit String length cap
// (212). QuickJS converts the receiver first, converts and validates the count
// second, returns the converted source for an empty String or count one, and
// otherwise builds one flat String using exact UTF-16 code-unit repetition.
//
// Raw UTF-16 observations encode code units as hexadecimal before crossing the
// process boundary, so lone surrogates never depend on terminal UTF-8 handling.
