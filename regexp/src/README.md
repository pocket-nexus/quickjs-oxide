# Regular expression programs

regexp parses patterns, generates regular-expression instructions, runs
matches and supports bounded interrupt callbacks.

`quickjs-oxide-core`'s `core/src/builtins/regexp.rs` owns JS RegExp objects,
properties, coercions and user callbacks.

The compiler records an exact first UTF-16 unit when it can prove one after
zero-width instructions. Non-sticky searches skip candidates with a different
unit before entering the matcher, preserving Unicode index advancement and
interrupt polling. Case-folded or surrogate literals and ambiguous prefixes
keep the general search path; sticky matching never searches ahead.

## Files and subdirectories

- [compiler.rs](compiler.rs): typed RegExp parser and compiler.
- [executor.rs](executor.rs): explicit-stack execution of the typed RegExp instruction program.
- [flags.rs](flags.rs): typed flags from pinned QuickJS `libregexp.h` lines 30-38.
- [group_name.rs](group_name.rs): QuickJS-shaped RegExp group-name parsing and lexical capture scans.
- [lib.rs](lib.rs): crate entry, shared interfaces and submodule declarations.
- [opcode.rs](opcode.rs): runtime-independent RegExp instruction IR.
- [unicode/](unicode/README.md): Unicode text rules.
