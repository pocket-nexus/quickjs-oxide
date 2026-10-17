// Pins QuickJS 2026-06-04 `js_string_pad` (quickjs.c 46073-46142), the
// `padEnd`/`padStart` GenericMagic entries and selectors (46647-46648),
// `JS_ToInt32SatFree`/`JS_ToInt32Sat` (13124-13172), and the 30-bit String
// length cap (212). QuickJS uses magic 1 for padEnd and 0 for padStart.
//
// The exact ordering matters: convert the receiver, saturate the target length,
// return immediately when the source is already long enough, then convert the
// optional filler. An empty converted filler also returns before the length-cap
// check. Padding repeats and truncates raw UTF-16 code units, so it may retain
// lone surrogates or split an astral pair.
