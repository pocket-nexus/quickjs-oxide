// Differential lock for pinned QuickJS 2026-06-04 scoped RegExp modifiers.
// The parser and state restoration behavior lives in `libregexp.c`
// `re_parse_modifiers` / `re_parse_term` (1810-1950). These vectors avoid
// unrelated unsupported RegExp features so the modifier slice can close
// independently while still probing literal, constructor, Unicode, capture,
// quantifier, and stateful execution integration.
