# Compiler

compiler turns complete JavaScript source into unpublished function and module
drafts: lexing and parsing, declaration and scope handling, name resolution,
closure capture and stack-instruction generation. Changes to syntax, binding
rules or compile-time rewrites start here.

The entry point only orchestrates requests. `options` holds configuration,
`parser` owns syntax, diagnostics and construction state, and `model` holds
artifacts shared between phases. FunctionBuilder's consuming `finish` checks
temporary control state and yields the IR; `resolution` resolves names,
`lowering` emits stack code, `relocation` handles target relocation, `flow`
provides shared required validation, and `optimize` performs bounded local
rewrites and source-position projection.

The pipeline uses linear FunctionIr/IrOp and emits stack code after parsing
and binding resolution. Declaration order, stable binding identity, exception
regions and source positions carry through every phase; name indexes are
lookup aids and do not decide shadowing, duplicate declarations or eval
semantics.

compiler uses the exact source representation from source and the code
contracts from code. It does not select host providers or own executing
frames; runtime linking, roots and the publication transaction belong to code
and heap. Runtime identities of objects and Symbols are never ordinary
compile-time constants.

See [workspace architecture](../../../docs/architecture.md) for phase
boundaries.

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
evaluation order follow the existing contracts and oracle, and parser resource
limits and stack-exhaustion behavior are checked against the oracle.
