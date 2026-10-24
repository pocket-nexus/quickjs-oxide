# GC verification

This package implements the first GC verification boundary: a least-fixed-point
proof, a production phase driver checked against explicit adapter contracts,
production bitmap/entry decisions, source proofs of root storage, Value decoding,
arena marking and heap-value queueing, and independent checks of the real heap
representation. **It is not an end-to-end proof of the entire collector or
of every caller's rooting discipline.** The remaining obligations are part of
the result, not implicit axioms hidden inside the proof.

## Run

Linux x86-64, Python 3 and rustup:

```sh
python3 scripts/verification/gc.py --install --mutations --output target/verification/proofs.json
python3 -m unittest discover -s scripts/verification -p 'test_*.py'
cargo test --locked -p quickjs-oxide-core
cargo test --locked --release -p quickjs-oxide-core --features gc-stress
cargo test --locked --features gc-stress --test embedding_api symbol_rope_description_survives_collection_without_other_string_roots
python3 scripts/verification/gc_mutations.py --output target/verification/runtime-mutations.json
```

`--require-complete` additionally requires every implementation and caller
obligation in the ledger to be discharged. **It currently exits 2**, even when
all of the implemented proofs pass. Receipts always include `complete`,
`open_obligations`, the working-tree dirty flag and source hashes. The explicit
finite-heap/host-memory/stop-the-world preconditions in `GC-RESOURCES` remain
part of the theorem's domain; adding an arbitrary `assumption` does not pass
this gate. This checks the ledger, not automatic coverage: there is still no
composed source theorem for `Heap::collect`, and relabeling a ledger item would
not prove it.

An existing pinned verifier can be supplied with `--verus /path/to/verus` or
`VERUS`. `toolchain.json` pins the official Verus archive by version and SHA-256.
Its Rust 1.99.1 toolchain is separate from the engine's Rust 1.99.0. No dependency,
lockfile or engine toolchain change is needed. The verifier and results stay in
`target/` or temporary directories. GitHub Actions runs the same checks in
`.github/workflows/gc-verification.yml`.

The command fails on missing tools, wrong versions, timeouts, extraction drift,
proof failures, or deliberately broken proofs that unexpectedly verify. A parse
error or failed compilation does not count as a rejected mutation.

## What is proved

`model.rs` defines a finite node universe, roots, strong edges and triples
`(map, key, value)`. A set is closed when it contains the roots, every strong
successor of its elements, and every ephemeron value whose map and key both
belong to the set. Reachability is the intersection of all such closed sets,
restricted to the node universe. The specification does not call the collector.

The proved obligations are:

- Reachability is closed and is a subset of every closed set.
- Initial work, node discovery/strong expansion, waiting-key activation,
  duplicate removal, and weak-map scanning preserve the work invariant.
- Waiting triples retain their live source map as ghost provenance. The
  executable table stores only key -> values.
- Empty pending work and complete map scanning imply exact reachability.
- Abstract execution terminates with a lexicographic measure: unmarked nodes,
  unscanned nodes, then pending work. This is an unbounded finite-heap theorem,
  not an enumeration of small heaps. Queue multiplicities and real container
  operations are representation obligations.
- Sweep intersection retains exactly that reachable set; adding finalization
  roots already in it does not change the fixed point; pruned references cannot
  name subsequently reused slots.
- Concrete non-vacuity examples cover a self-supporting weak cycle, a dead map
  with a live key, and discovering another map through an ephemeron value.
- A subsequent collection starts from the new graph and roots, independently
  of the previous abstract work state.

`storage.rs` proves interval split/coalesce, disjoint space transfer between
allocated/free/quarantined sets, and preservation of a retained root-stack
prefix. It does **not** prove the complete `Arena` or `ValueHeap` implementation.
Root completeness is an explicit precondition of `safepoint_preserves_needed`;
the lemma must not be presented as proof that all engine callers root correctly.

## Connection to production code

`core/src/heap/gc_kernel.rs` is compiled normally by Cargo. The proof runner
extracts the exact bodies of `mark_word`, `dead_word`, `ephemeron_action`, and
`close_ephemerons` from this file. There is no independently maintained copy of
their executable bodies. The first three are verified for all inputs against
bit-precise/Boolean postconditions. The phase driver receives only ghost
annotations and is verified against `driver.rs.in`.

The driver's three adapter obligations are:

| Operation | Obligation |
| --- | --- |
| `scan` | Preserve the graph and work invariant. Either queue new marking work, or establish complete closure with no pending work. |
| `drain` | Preserve graph and invariant, empty the queue, and strictly decrease the remaining-work rank. |
| `clear` | Preserve marked nodes and graph, empty the pending and waiting state. |

`WeakPass` in `gc.rs` implements the production operations. Its complete
refinement to those contracts is **not yet deductively proved**. The contract is
an explicit assumption at that boundary. The model's semantic operations and
the checks below provide evidence, not a replacement for that missing proof.
The driver also assumes its adapter calls terminate; its decreases clause does
not prove termination of arbitrary trait implementations.

### Additional production-source proofs

`roots.rs.in`, `value.rs.in` and `arena.rs.in` receive production methods from
`gc.rs`, `value.rs` and `arena.rs`, selected by `scripts/verification/source.py`.
They prove the following contracts for arbitrary sequence lengths permitted by
their premises and every representable value/index, rather than a bounded
collection of test heaps:

- Every `RootStack` operation has an exact sequence contract. Restoring a valid
  mark preserves precisely the preceding prefix. `Handles::create`, `get`,
  `set`, `release` and `live_count` preserve the bijection between empty slots
  and a duplicate-free free list; allocating a handle preserves other active
  handles and their values. Both reuse and append have exact slot/free-list
  postconditions. The count equals the number of active slots. Composition
  witnesses exercise create/release/reuse and save/push/restore over arbitrary
  input values and retained prefixes.
- `Value::kind`, `is_heap`, the four heap constructors and their decoders agree
  on the arena namespace and the full `u32` payload. The classification proof
  covers every `u64` bit pattern, including primitive and reserved tags.
- `push_heap_values` terminates and appends exactly the heap-valued subsequence,
  retaining the previous queue and preserving order and multiplicity.
- `Arena::new`, `is_live`, `check`, `get`, `get_mut`, `mark`, `is_marked` and
  `clear_marks` have source proofs. Marking is fresh exactly when its one bit
  was clear; every other bit, all record contents and metadata are preserved.
  Clearing terminates and clears all marks. Mutable access frames the one
  selected record. Both normal and `gc-stress` layouts are verified, including
  preservation of the quarantine vector.

The contracts state their remaining premises. Marks must belong to an adequate
root prefix; handles must be active and nonempty, and table lengths must remain
representable. `Handles::values`, its derived `Default`, lifetime/heap ownership
discipline, arena allocation/sweep invariant preservation and the complete
tracer are not discharged by these proofs. In particular, decoding an arena
index does not prove that it still names a live record.

The source frontend adds ghost contracts, proof blocks, loop invariants and
named returns, narrows visibility inside the proof module, and instantiates the
production identifier macro's record declarations. Constants with executable
initializers receive a checked executable/specification correspondence. Arena
methods that do not use `Record` are checked for arbitrary `T`, a stronger bound.
For `Arena::check` only, it erases the panic formatting arguments after checking
the exact assertion condition; Verus still proves that condition, so the erased
failure branch is unreachable under the contract. A negative proof removes the
liveness premise in the stress configuration and must fail. No collector call,
branch condition, mutation or successful-path statement is replaced with a
trusted summary. This small source frontend, like the original kernel extractor,
is part of the trusted base.

The production changes needed by these proofs replace two optional bitmap
lookups with explicit matches, bitmap fill and heap-value filtering with explicit
loops, and a Boolean-to-integer `From` call with a cast. This avoids unsupported
standard-library specifications without adding local axioms. The loop proofs
cover bounds, overflow, termination and their complete output sequences.

The runner rejects local `assume`, `assume_specification`, `admit`, external
body/type/trait/function specifications, and uninterpreted/axiomatic escapes.
This does not eliminate the trusted base: Verus, its
standard library, SMT solver, rustc/LLVM, source extraction, the specifications,
and the stated adapter/host assumptions remain trusted. The adapter contract
cannot be counted as a proved implementation merely because it has no body.

## Independent representation audit

`gc_audit.rs` is compiled only in core unit tests and `gc-stress` builds. Before
collection it enumerates allocated records into a typed graph, preserving arena
identity. It computes reachability with a strong-edge worklist and repeated full
ephemeron scans. It never uses the production marker, the key-indexed waiting
table, its discovery cursor, or `Job::trace` to compute expected results.

The graph includes objects, shapes, cells, strings/ropes, symbols, BigInts,
function/bytecode/internal records, transient code units and regexp programs.
Object kinds and internal tags are exhaustively matched. It covers properties,
elements, scope parents, captures/home objects/field initializers, bytecode
children and constants, prototype/key links, namespace keys, and internal slots
including suspended registers. WeakMap entries are conditional edges; WeakSet
and WeakRef targets are not strong edges; registry callbacks and held values
are strong. Prototype validity cells are retained by their live owners.

Roots independently cover the explicit stack, handles, fixed roots, active
register prefix and frames, pinned/held atoms, pending code, cached closure and
arguments shapes, the static cell, kept objects, rejections, modules, and every
current job variant. A missing root in both the program's state and this root
policy still requires a caller/rooting proof; heap graph agreement cannot find
a Rust local that was never rooted.

The audit compares marks before weak processing, checks jobs and caches before
reuse, and compares survivors after sweep. `verify()` also checks the independent
graph, instead of relying only on re-running the same traversal. Stable adapter
boundaries check live-map provenance, outstanding work, activation and cleanup.
The graph audit increases stress-test cost; release builds contain none of it.

## Behavioral and negative checks

`gc_tests.rs` includes weak cycles, delayed maps, symbol keys, finalization job
lifetimes, an independent lost-edge detector, and 96 deterministic generated
heap graphs across three successive collections. These are tests, not an
exhaustive theorem. Arena tests cross 63/64/65/129 slot boundaries while checking
unique ownership, exact callback releases, counters and retained payloads. Value
heap tests repeatedly split, release and rebuild storage, checking every slot's
ownership and surviving values. Existing handle, cap, capture, cache reuse,
host-reentry and stress tests remain in the suite.

The independent audit exposed a missing production edge: a Symbol description
can be an unflattened rope, so marking only its string record fails to retain
the rope halves. The collector now visits that description through the string
worklist. Unit and public embedding regressions cover it. The pre-fix unit test
fails in the independent mark comparison, before sweep can recycle the halves.

Proof mutations remove map liveness, drop key activation, carry stale marks,
invert the executable sweep mask, or omit the production driver's drain/clear
operations. Further source mutations destroy a retained root prefix, fail to
release a handle, exclude Symbol values, invert heap-value filtering, leave mark
bits set, write the wrong arena bit, or omit a live-access precondition. Runtime
mutations remove an actual strong
trace edge, drop actual waiting-value activation, or retain the previous weak
work list. Runtime mutations run in an isolated source copy after a passing
baseline; they must fail a selected behavioral test with the expected GC
diagnostic. The user's worktree is not mutated.

## Obligation ledger and maintenance

[`obligations.json`](obligations.json) records proved-source, proved-model,
runtime-checked, caller-obligation and assumption boundaries. There is no claim
of full collector verification until the representation, allocator, contents
and rooting obligations are discharged. In particular, there is no proof here
of OOM recovery, process RSS, host callback termination, or all mutator safepoints.

When adding a record field or root source, update the independent graph policy,
its behavioral test and the ledger. When changing weak processing, update the
work invariant and adapter contracts together. When changing an executable
kernel, rerun the source proofs and negative checks. Do not weaken a contract or
add an assumption merely to make a proof pass.

Engine changes also require the repository's Sweep and memory runs from clean,
committed base/candidate worktrees. Verification results are not performance
measurements. Retain exact commands, commits, binary hashes and benchmark
tables outside version control as specified in `AGENTS.md`.
