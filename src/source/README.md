# Source and text foundations

source holds what the front end reads before it parses: the exact source
bytes, byte offsets, ranges and QuickJS's line and column rules, and the
Unicode identifier properties the lexer asks for. Parsing policy lives in
the compiler; the engine's own strings and objects live in `oxide-core`.

The front end compiles from a `SourceText` and records positions with these
types; the register lowering turns the resulting pc-to-source tables into
the core's position tables, from which errors take their `stack`. Exact
UTF-16 text shares the `JsString` carrier in value.

## Files and subdirectories

- [coordinates.rs](coordinates.rs): byte offsets and QuickJS source-coordinate mapping.
- [mod.rs](mod.rs): module entry, `SourceLocation` and `SourceSpan`, and submodule declarations.
- [text.rs](text.rs): byte-exact carrier text for dynamically supplied ECMAScript source.
- [unicode.rs](unicode.rs): the Unicode tables of the `oxide-regexp` crate
  (`regexp/src/unicode`), re-exported: the identifier properties
  the lexer uses, and `JsString` wrappers for case conversion and
  normalization.

`SourceText` keeps the input's exact bytes beside an equal-width UTF-8
carrier for the lexer, so surrogate encodings and malformed bytes survive
parsing; the function source text the lowering stores for
`Function.prototype.toString` is the original bytes.

`QuickJsSourceLocator` serves one-off diagnostics without allocating; bulk
position queries use the `QuickJsSourceIndex` built by its `index()`. The
index records line and column every 256 raw bytes and is shared across all
child functions, so a query scans at most 255 bytes and a single very long
line does not degrade it. Both use the same LF/UTF-8 continuation counting
rules and count the parsed bytes as they are.
