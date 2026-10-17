// Pins QuickJS 2026-06-04 `js_sub_string` (quickjs.c 3964-3991),
// `js_string_substring`/`js_string_substr`/`js_string_slice` (45983-46071),
// their adjacent function-list entries (46641-46643), and the shared
// saturated conversion helpers `JS_ToInt32SatFree`/`JS_ToInt32Clamp`
// (13124-13189).
//
// These vectors intentionally preserve QuickJS's implementation contract:
// substring clamps both indices and swaps them, substr applies a string-length
// offset to a negative start and treats its second argument as a length, while
// slice applies that offset independently to both indices and never swaps.
// All string observations containing unpaired surrogates are encoded as raw
// UTF-16 code units before crossing the process stdout boundary.
