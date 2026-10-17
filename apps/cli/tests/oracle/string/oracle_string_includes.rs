// Pins QuickJS 2026-06-04 `js_string_includes` (quickjs.c 45516-45578),
// `js_is_regexp` (47487-47499), and the adjacent function-list entries
// (46634-46636). The three methods deliberately share one magic-selected
// kernel but retain distinct position defaults and search ranges.
//
// Proxy, TypedArray, and genuine RegExp values participate in the same
// differential. Module namespace conversion remains a separate
// module/object-model boundary.
