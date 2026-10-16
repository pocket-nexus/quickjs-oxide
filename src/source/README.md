# Source and text foundations

source stores raw source text, ranges and coordinates, and provides Unicode
character properties, case mapping and normalization. Compilation policy and
runtime objects live elsewhere.

The compiler, regexp and diagnostics use these text rules; exact UTF-16 text
shares the string carrier in value.

## Files and subdirectories

- [coordinates.rs](coordinates.rs): byte offsets and QuickJS source-coordinate mapping.
- [mod.rs](mod.rs): module entry, shared interfaces and submodule declarations.
- [text.rs](text.rs): byte-exact carrier text for dynamically supplied ECMAScript source.
- [unicode.rs](unicode.rs): the Unicode tables of the `oxide-regexp` crate
  (`regexp/src/unicode`) at their historical path, with `JsString` wrappers
  for case conversion and normalization.

`QuickJsSourceLocator` serves one-off diagnostics without allocating; bulk
position queries use the `QuickJsSourceIndex` built by its `index()`. The
index records line and column every 256 raw bytes and is shared across all
child functions, so a query scans at most 255 bytes and a single very long
line does not degrade it. Both use the same LF/UTF-8 continuation counting
rules and keep parsed bytes as-is rather than converting them to standard
UTF-8.
