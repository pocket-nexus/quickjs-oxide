# Compiler

compiler is the front end and the register lowering. The front end turns
complete JavaScript source into an `UnlinkedFunction` tree (or an
`UnlinkedModule`) of QuickJS-shaped stack code: lexing and parsing,
declaration and scope handling, name resolution, closure capture and
stack-instruction generation. The lowering in [`register`](register/mod.rs) turns
that tree into `oxide-core` register code and publishes it into the
running runtime. Changes to syntax, binding rules or compile-time rewrites
start in the front end; changes to the register code it produces start in
the lowering.

The entry point only orchestrates requests. `options` holds configuration,
`parser` owns syntax, diagnostics and construction state, and `model` holds
artifacts shared between phases. FunctionBuilder's consuming `finish` checks
temporary control state and yields the IR; `resolution` resolves names,
`lowering` emits stack code, `relocation` handles target relocation, `flow`
keeps the stack-size computation and the block entries `optimize` reads, and
`optimize` performs bounded local rewrites and source-position projection.

The pipeline uses linear FunctionIr/IrOp and emits stack code after parsing
and binding resolution. Declaration order, stable binding identity, exception
regions and source positions carry through every phase; name indexes are
lookup aids and do not decide shadowing, duplicate declarations or eval
semantics.

## The front end as the lowering's input

The lowering reads only what the front end leaves in the unlinked tree:
the stack instructions, whose operand-stack depth is static at every
instruction (`compute_stack_size` in `code/bytecode.rs`; `register/analysis.rs`
keeps the depth and the active catch regions per instruction), the constants,
each function's metadata, its arguments, locals and closure variables with
their sources, and its debug record (file name, pc-to-source table, source
text). The stack depth becomes the register allocation (register 0 is `this`,
then parameters, locals and one temporary per depth); captured bindings
become cells, and the front end's `CloseLocal` (QuickJS's `close_loc`)
becomes `CloseCell`. A
function past the 256-register window keeps its further locals and deeper
temporaries in a spill scope (the `register-stress` feature lowers every
function that way, with two temporaries in registers, for the corpora).
The lowering also checks the surface the core runs: a `with` statement is
an early `SyntaxError`, and a direct `eval` runs as indirect `eval`, so the
lowering compiles indirect eval code (deviation ledger, `TARGET-*`).

compiler uses the exact source representation from source and the stack
instruction and unlinked-record contracts from code. The lowering hands each unit to the core's
`CodeStore::publish`, which validates it, and to `instantiate`, which
creates the function objects. The API's compile host
(`api/core/compile.rs`) calls the front end and the lowering for scripts,
modules and code compiled at run time. Compile-time constants are
primitives, strings, regular-expression sources, template sites and child
functions.

See [workspace architecture](../../../docs/architecture.md) for the stages
from source to running code.

## Front-end contracts

Optimizations preserve exact source bytes, UTF-16 strings, token spans, ASI
and `line_terminator_before`, the lexical goal/context, escaped identifiers
and private-name identity. Name interning does not decide scoping or
shadowing. Lookahead caches preserve each probe's swallow-or-propagate error
policy and never turn a speculative failure into a committed error; cache
identity includes source, offset, goal and context and is never reused across
lexical contexts. Rescans keep the observed line breaks and token starts,
template continuations are never rescanned with the division goal, and
lookahead does not mutate committed state early.

Template cooked/raw values, tagged/untagged invalid-escape behavior and the
first error position are preserved. Escaped identifiers are checked by their
decoded names; private-name identity and length limits that include `#` are
preserved. Lazy text handling still reports malformed bytes and length errors
at scan time. Diagnostic phase, message text, position, first error and
evaluation order follow the existing contracts and oracle.

`stack_guard` bounds parser recursion. Each recursive grammar edge enters
through `parse_recursion` (or `enter_recursion`/`leave_recursion` for headed
statements) and charges a weight calibrated to QuickJS's nesting depths;
every committed token advance also checks the native stack against the
thread's floor. Both report a catchable `SyntaxError: stack overflow`. A new
recursive production enters through `parse_recursion` with the closest frame.
