# quickjs-oxide-regexp

The regular-expression engine of quickjs-oxide and the checksum-pinned
Unicode 17 tables it and the String intrinsics use, as a workspace crate the
root crate and `quickjs-oxide-core` depend on.

The crate has `#![forbid(unsafe_code)]` and no dependencies. Its API works on
UTF-16 code units (`&[u16]`, or Latin-1 `&[u8]` where a function takes
`CodeUnits`) and plain `u32` code points; it never sees an engine string. The
front end validates regular-expression literals through `compile`;
`crate::source::unicode` (`src/source/unicode.rs`) in the root crate
re-exports the Unicode functions with thin `JsString` wrappers. RegExp
objects are `quickjs-oxide-core`'s (`core/src/builtins/regexp.rs`).

| Module | Content |
| --- | --- |
| crate root | `compile`, `CompiledRegExp`, `RegExpFlags`, `Instruction`, `execute` and its interrupt and Latin-1 forms, `RegExpMatch`, the error types, `javascript_compile_error_message` |
| `unicode` | `is_id_start`, `is_id_continue`, `CodeUnits`, `CodeUnitBuf`, `StringError`, `MAX_STRING_LEN` |
| `unicode::case` | `convert_case_with_limit`, `regexp_canonicalize`, `regexp_canonicalize_range_pairs` |
| `unicode::normalize` | `NormalizationForm`, `normalize`, `normalize_with_limit`, `normalize_code_points` |
| `unicode::property` | the property sets of RegExp `\p{…}` escapes |

The `test-hooks` feature exposes the thread-local allocation-failure hooks of
the case and normalization buffers to the root crate's String tests.

```sh
cargo test -p quickjs-oxide-regexp
```

Sources: [`src/`](src/README.md).
