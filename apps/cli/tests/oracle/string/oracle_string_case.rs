// Pins QuickJS 2026-06-04 `js_string_toLowerCase`, its Unicode 17 case
// conversion/property tables, and the four GenericMagic String prototype
// entries (quickjs.c 46215-46304 and 46656-46659; libunicode.c 51-190 and
// 347-376). The two locale-named variants deliberately share the ordinary
// lower/upper kernels and never inspect their locale arguments.
//
// Every raw String observation is encoded as hexadecimal UTF-16 code units
// before crossing the process boundary, preserving NUL and lone surrogates.
