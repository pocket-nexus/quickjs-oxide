# Regular expression programs

regexp parses patterns, generates regular-expression instructions, runs
matches and supports bounded interrupt callbacks.

The root crate's `src/engine/builtins/regexp` owns JS RegExp objects,
properties, coercions and user callbacks.

## Files and subdirectories

- [compiler.rs](compiler.rs): typed RegExp parser and compiler.
- [executor.rs](executor.rs): explicit-stack execution of the typed RegExp instruction program.
- [flags.rs](flags.rs): typed flags from pinned QuickJS `libregexp.h` lines 30-38.
- [group_name.rs](group_name.rs): QuickJS-shaped RegExp group-name parsing and lexical capture scans.
- [lib.rs](lib.rs): crate entry, shared interfaces and submodule declarations.
- [opcode.rs](opcode.rs): runtime-independent RegExp instruction IR.
- [unicode/](unicode/README.md): Unicode text rules.
