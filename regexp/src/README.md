# Regular expression programs

regexp parses patterns, generates regular-expression instructions, runs
matches and supports bounded interrupt callbacks.

`quickjs-oxide-core`'s `core/src/builtins/regexp.rs` owns JS RegExp objects,
properties, coercions and user callbacks.

The compiler derives a conservative search plan from the parsed expression:
start anchoring, minimum UTF-16 length, an ASCII first-character bitmap, and
a required literal with an optional maximum prefix length. Non-ASCII first
characters remain unfiltered. Proven failures return before acquiring scratch
storage; capture-free pure literals search directly. A required literal in the
middle of a pattern bounds candidate starts only when its prefix has a finite
maximum. Assertions, references and scoped modifiers retain their VM semantics.

Forward repetitions of single-character classes use a fused instruction with
an ASCII bitmap. Greedy rollback retains one endpoint frame; lazy matching
extends by one character only when the continuation fails. Capturing bodies,
backward matching and Unicode Sets string members keep ordinary instructions.
Scans preserve Unicode advancement and interrupt polling. Sticky and proven
start-anchored matches check only their one candidate: pure literals compare
only at that position, and other patterns defer required-literal search until
128 matching steps. Cheap failures do not scan unrelated trailing input, while
expensive backtracking still rejects missing literals. The proof scan shares
the interrupt budget and preserves partial progress when matching resumes.

Execution can reuse one spare set of capture/register/undo/control buffers per
thread, retaining at most 32 KiB of vector capacity. The spare is leased without
holding a borrow across execution, including interrupt callbacks. Oversized
buffers are released; success, failure and interruption clear retained state.

## Files and subdirectories

- [compiler.rs](compiler.rs): typed RegExp parser and compiler.
- [compiler/search.rs](compiler/search.rs): conservative AST search facts.
- [executor.rs](executor.rs): explicit-stack execution of the typed RegExp instruction program.
- [flags.rs](flags.rs): typed flags from pinned QuickJS `libregexp.h` lines 30-38.
- [group_name.rs](group_name.rs): QuickJS-shaped RegExp group-name parsing and lexical capture scans.
- [lib.rs](lib.rs): crate entry, shared interfaces and submodule declarations.
- [opcode.rs](opcode.rs): runtime-independent RegExp instruction IR.
- [unicode/](unicode/README.md): Unicode text rules.
