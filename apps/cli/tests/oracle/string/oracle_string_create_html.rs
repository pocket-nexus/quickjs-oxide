// Pins QuickJS 2026-06-04 Annex-B `js_string_CreateHTML` and its selector/tag
// table (quickjs.c 46546-46615), plus the thirteen GenericMagic String
// prototype entries (46661-46674). QuickJS converts the receiver first. The
// four attribute variants then use `JS_ToStringCheckObject` for argv[0], so a
// missing, explicit undefined, or null attribute is rejected. The other nine
// variants ignore every argument. Attribute text replaces only raw U+0022
// with the six ASCII code units of `&quot;`; source text is never escaped.
//
// Every raw String observation is encoded as hexadecimal UTF-16 code units
// before crossing the process boundary, preserving NUL and lone surrogates.
