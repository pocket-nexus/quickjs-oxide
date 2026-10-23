# Formal verification

This work covers scalar representation, bounded arena schedules, a source-bound
Verus proof of `Arena::mark`, an inductive bitmap model, and initial storage,
collector and rooting checks. **The five-stage acceptance criteria are not all
met:** the complete production-arena refinement remains open.
Neither the abstract model nor bounded Kani schedules establish that refinement.
The engine's ECMAScript semantics are outside these proofs.

## Run the scalar proofs

The supported proof target is `x86_64-unknown-linux-gnu`. Install the version
recorded in `scripts/verification/kani-version`:

```sh
cargo install --locked kani-verifier --version "$(cat scripts/verification/kani-version)"
cargo kani setup
python3 -m unittest discover -s scripts/verification -p 'test_*.py'
python3 scripts/verification/phase1.py
```

The pinned bundle is Kani **0.68.0**, using CBMC **6.11.0** and Rust
**1.100.0-nightly (8925ea358 2026-08-20)**. It compiled the current workspace
and verified all 12 harnesses. Product builds continue to use the repository's
Rust **1.99.0**; Kani installs its own verifier toolchain without changing
`rust-toolchain.toml` or the Cargo dependencies.

The runner checks the Kani version, invokes it with all default safety and
unwinding checks enabled, and validates its structured JSON result. It requires
exactly the expected 12 harnesses, successful completion, no unresolved checks,
and reachable, passing explicit assertions in each proof. Missing, duplicated,
failed, or vacuous harness results fail the run. Each harness has a configurable local
time budget, defaulting to one hour. For example, append
`--harness-timeout 30m` or `--harness-timeout 1h`. This is a per-harness
budget, not a bound on the total group duration. A timeout is not a proof.

Each invocation creates a new `target/verification/phase1-*` directory with:

- `run.log`: complete compiler and verifier output, including warnings;
- `results.json`: Kani's harnesses, checks, target and tool versions;
- `provenance.json`: the command, HEAD, working-tree status, and SHA-256 hashes
  of the core/regexp sources, their manifests, workspace manifest, lockfile,
  toolchain file and verification scripts.

Raw results stay out of commits. Formal verification runs **locally**, without
GitHub solver jobs or a CI-time acceptance requirement. The `Verification support
checks` workflow only checks formatting and the Python receipt/inventory tests.
Ordinary storage and collector regression tests remain in the existing Engine CI.
Retain local receipts on both success and failure, and report the source commit,
command, budgets and outcome in the PR. Changing the harness inventory requires
updating the runner's expected set and this document.

## Properties and input domains

The value harnesses live in `core/src/value_kani_tests.rs` and the range
harnesses in `core/src/heap/values_kani_tests.rs`. Their parent modules include
them only under `cfg(test)` and `cfg(kani)`. They have access to the real private
constructors/helpers; no implementation is copied or stubbed.

| Harness | Symbolic input domain | Property |
| --- | --- | --- |
| `int32_round_trip` | Every `i32` | Exact integer round trip, exact widening to double, correct classification and no heap/special accessor alias |
| `bool_round_trip` | Both booleans | Boolean round trip, correct singleton, numeric/heap/sentinel separation |
| `special_values_are_distinct` | Selector 0–2 for Undefined, Null, Empty | Correct kind, distinct sentinel predicates, no boolean/number/heap classification |
| `heap_tags_and_payloads_round_trip` | Every `u32` index and all four heap tags | Exact payload round trip, mutually exclusive heap accessors, no numeric/boolean/sentinel alias; ScopeId-to-ObjectId preserves the index |
| `number_bits_and_classification` | Every `u64`, converted with `f64::from_bits` | Correct Number classification; bits below `0xFFF9_0000_0000_0000` preserved, others canonicalized to `0x7FF8_0000_0000_0000`; IEEE NaN classification preserved |
| `class_sizes_and_inverse` | Classes 0 and 2–62 | Size equals the independent power-of-two/three-times-power-of-two model; `class_of` inverts it and rounding preserves it |
| `rounding_is_minimal` | Requested length 1–2³¹; every alternative supported class | Result is a supported size, covers the request, and is no larger than any supported size covering it |
| `floor_is_maximal` | Every positive `usize` on x86-64; every alternative supported class | Result is the largest supported size fitting the available slots, capped at 2³¹ |
| `next_class_is_the_successor` | Requested length 0–3×2²⁹; every alternative supported class | Result is the smallest supported size strictly above the rounded `max(request, 1)` |
| `range_log_round_trip` | Every `u32` start and supported nonempty size | Compact log encoding decodes to exactly the original start and length |
| `range_exponent_round_trip` | Every `u32` start; exponents 0–31 | Power-of-two ranges round trip through both exponent and log encodings |
| `empty_range_is_canonical` | Every `u32` start with length zero | Both empty encodings use 255 and decode to `Range::EMPTY`, discarding the irrelevant start; `class_len(0) == 0` |

`kani::any()` is symbolic, not random sampling. Selectors only enumerate the
constructor kinds. Size restrictions are explicit **proof preconditions**:

- `values.rs` documents the supported largest class as 2³¹ and leaves class 1
  unused. `class_for` requires a positive request; `class_len` handles zero.
- The successor domain stops at 3×2²⁹ so the next class remains supported.
- `Range::from_log` and `from_exponent` consume codes produced by the appropriate
  encoder, not arbitrary untrusted bytes. Empty ranges deliberately canonicalize
  their start. `exponent()` is specified here only for powers of two or empty.
- An arbitrary `start` tests encoding, not containment in an allocated heap.

This phase **does not prove all callers establish these preconditions**. In
particular, requests above the supported range and arbitrary malformed log
bytes are outside the claims. Caller admission and allocation arithmetic belong
in later phases; do not cite these proofs as validation of untrusted requests.

The independent size model uses `2_u32.pow(exponent)` and multiplication, rather
than the production packed-class bit expression. Minimality/maximality quantify
over an arbitrary alternative class, avoiding a bounded enumeration of heap
sizes. The model's `pow` needs at most five binary exponent digits; its harnesses
use unwind 7. Other harnesses use the runner's default unwind 2. Unwinding
assertions remain enabled, so insufficient unfolding fails instead of silently
discarding executions. The proven scalar domains are not restricted to a small
sample or a small arena.

## Trust boundary and interpretation

- Proofs depend on the specifications and assumptions, Kani's Rust translation
  and standard-library models, CBMC and its solver. They do not verify rustc,
  LLVM, generated machine code, or the verifier itself.
- Float claims concern boxing and bit reinterpretation. They preserve signed
  zero and applicable NaN payloads; they do not specify NaN payloads produced by
  arbitrary floating-point arithmetic or prove JavaScript numeric operators.
- A round-tripping heap ID need not be allocated, rooted, or live. Nothing here
  establishes object identity after reuse, GC reachability or safepoint safety.
- The Kani build can warn about unsupported constructs elsewhere in the crate
  (currently caller location and a foreign function). Reachable unsupported
  constructs still fail verification; no ignore/stubbing option is enabled.
  Compile warnings from Kani's injected assertion macros are retained in logs.
- Internal checks in impossible branches may be `Unreachable`; explicit proof
  assertions must be reachable and successful. This detects vacuous harnesses,
  but cannot establish that the human-written specification is the right one.
- These results apply to the recorded target and default feature configuration.
  No wasm32, other architecture, or `gc-stress` proof is claimed.

## Production arena schedules

Run the two configurations **serially within one checkout**. Kani's force-build
can remove another invocation's intermediate files. Do not run a second Kani
invocation in this checkout while a long verification is in progress.

```sh
python3 scripts/verification/stateful.py arena
python3 scripts/verification/stateful.py arena-stress
```

The six harnesses in `arena_kani_tests.rs` exercise production functions, without
stubs. The independent model uses slot contents and direct expected counts;
it does not implement the arena's bitmap search or cursor logic.

| Harness | Domain and checked behavior |
| --- | --- |
| `arena_free_and_reuse` | Two slots, arbitrary u8 payloads/external-byte amounts, either freed slot; lowest-index reuse, content frame, live/free/extra accounting |
| `arena_write_frame_and_accounting` | Either of two live slots, arbitrary replacement; paired `get_mut`/`adjust_extra` preserves the other slot and accounting |
| `arena_mark_and_sweep` | Either of two live slots marked twice; freshness, exactly the unmarked record delivered once, survivor contents and accounting |
| `arena_sweep_all_and_empty` | Two unmarked records; descending callbacks, all records freed, subsequent empty sweep and shrink |
| `arena_sweep_quarantine_cycle` | Two collections and intervening allocations; delayed reuse under `gc-stress`, immediate reuse normally, old quarantine released next cycle |
| `arena_multiword_reuse` | A full 130-slot fixture, any freed index 0–129; reuse across both 63/64 and 127/128 word boundaries |

These are recorded schedules, not all schedules of a given length. Most use
unwind 8 or 12; the 130-slot fixture uses 140. Insufficient unfolding fails.
The fixture does not establish arbitrary multiword construction or sweeping.

## Source-bound Verus mark proof

```sh
python3 scripts/verification/install_verus.py
python3 scripts/verification/verus.py --verus target/verification/tools/verus-x86-linux/verus --mutations
```

Verus is pinned to **0.2026.10.18.7d82994**, Rust **1.99.1**. The installer
checks the Linux x86-64 release archive's SHA-256 before extraction. Neither
Cargo dependencies nor the product toolchain change.

`verus.py` extracts `bit` and the complete `mark` body from `arena.rs` into
`arena_mark.rs.in`. The proof has no vector-length bound. Its obligations are:

- Precondition: the ID denotes a live record and the bitmap/vector lengths
  satisfy the stated structural representation invariant.
- Postcondition: exactly the selected mark bit is set; the returned Boolean
  says whether that bit was clear beforehand.
- Frame: all records, live/quarantine bits, counters and cursor are unchanged;
  every other mark word is unchanged by the sequence-update postcondition.
- The structural invariant and the target's liveness are preserved.

This is an explicitly audited source projection, not a Verus build of the
entire engine. `Option::is_some_and` has no usable specification in this Verus
release: its exact source spelling is checked before translation to an equivalent
`match`. The diagnostic-only `check` body is checked before translation to an
unconditional liveness assertion (a stronger executable check than release).
The projected quarantine field also represents a ghost frame for normal builds.
Changes to either translation boundary fail the runner and require review.
No `assume`, `admit`, external body, or unchecked contract is introduced;
`--no-cheating` is required. Verus/vstd and the audited translations are trusted.

The runner requires successful whole-file verification and the expected proof
count (5 for mark, 15 for the model). Negative controls replace setting the mark
bit with clearing it and replace free's live mask with all ones. Both must fail
a proof obligation, not merely compilation. Generated source, exact commands,
source hashes, verifier JSON and diagnostics are retained with each receipt.

## Induction and the remaining refinement gap

`verus/arena_model.rs` proves a live/reusable/quarantined bitmap partition:
live and quarantine bits lie within valid slots and are disjoint. Marks are
**not** required to be a subset of live bits: production free/shrink can leave
stale marks until `clear_marks`.

The constructor establishes this invariant. Append, reuse, free, mark,
clear-marks, sweep, quarantine release, trimming reusable slots and payload
writes preserve it under their explicit legal-operation preconditions. The
proof composes these transitions over arbitrary finite sequences and gives a
frame theorem for unrelated bitmap words. This theorem has no sequence-length
or word-count bound, but concerns the **abstract transition system**. It does
not prove production counters, cursor selection, payload destruction, vector
capacity arithmetic or the full Rust loops refine those transitions.

Experimental production induction harnesses are also provided:

```sh
python3 scripts/verification/stateful.py arena-inductive
python3 scripts/verification/stateful.py arena-inductive-stress
```

They quantify over canonical states with at most two `Item` slots, record
capacity 0 or 4, matching bitmap capacities, arbitrary u8 payload/extra values,
legal live/quarantine partitions and stale marks. Separate steps cover the
constructor, allocation, free, paired write/accounting, mark/clear, sweep and
shrink. Even if all these steps pass, their induction applies only to this
restricted state space, not arbitrary arena capacities or arbitrary `Record`
implementations. The initial symbolic sweep attempt exhausted verifier memory;
these groups are experimental, not completed proofs.

Production induction is **not complete**. It needs a production correspondence for
all relevant mutations, including cursor/free-count bookkeeping and shrink,
plus the actual `Record` and caller obligations. In particular `get_mut` and
`adjust_extra` are separate methods; mutations changing `extra_bytes()` must
be specified as a paired transaction. The API alone does not enforce that
protocol. This is a proof obligation, not a demonstrated caller defect.

## Storage, collector and rooting coverage

Accepted proof groups and executable model tests:

```sh
python3 scripts/verification/stateful.py storage
python3 scripts/verification/stateful.py rooting
cargo test -p quickjs-oxide-core --lib model_tests
cargo test -p quickjs-oxide-core --lib --features gc-stress model_tests
```

| Layer | Proof/test evidence | Remaining obligations |
| --- | --- | --- |
| Storage | Kani: fixed request pairs (1, 3), (2, 4), symbolic contents, non-overlap, free/reuse poisoning and used-slot accounting; growth 2→6 and slot growth 4→8. Runtime model: all 16 request pairs in 1–4 through coalescing, growth and complete sweep. | Arbitrary request sizes/sequences, formal coalescing/sweep partition preservation, allocation failure and cap admission |
| Collector | Runtime independent reachability model: all 64 directed-graph/root-set combinations on two scope objects, followed by root removal and another full collection. Four WeakMap/key root combinations and 32 two-map ephemeron-chain/root-order combinations. | **Collector Kani proofs have not passed**; general payload/graph/fixpoint proof, weak refs/finalizers, code/shape/module root combinations, concurrency |
| Rooting | Kani: symbolic Int32 payloads through nested root-stack scopes and two handle slots with replacement/release/reuse, preserving the other handle. Runtime: actual collection keeps a handled object until release. | Every VM/host caller roots all values across safepoints; reentry, borrowed references, stale handles, error/unwind cleanup |

The executable tests use the complete production `collect()` path, including
weak processing and reclamation. They are exhaustive over the listed finite
fixtures, **not formal proofs of the collector**. They run both normally and
under `gc-stress`. The independent closure is not the engine's stress verifier.

Collector proofs run locally through the real production entry points. To give
each harness a full hour, run:

```sh
python3 scripts/verification/stateful.py collector --harness-timeout 1h --verbose
```

The group runs serially and can therefore take two hours. To investigate one
harness, add `--harness strong_graph_marking` or
`--harness ephemeron_requires_reachable_map_and_key`. A subset run validates an
exact subset inventory and is not evidence that the entire group passed.
`provenance.json` records the selected harnesses and per-harness budget.
`--verbose` retains verifier commands and regular structured-result output; it
does not use Kani's old output mode for proof acceptance.

They check production mark bits against the two-object transitive closure and
one ephemeron fixture before reclamation. Manual collection mode excludes
automatic safepoints; timing/budget policy is not specified. No function is
stubbed. Initial runs used a five-minute budget and timed out, including the
one-map ephemeron harness. These short runs do not establish that a longer local
verification cannot complete. An earlier complete-collection attempt also failed to verify.
These are **open proof obligations**, not successful coverage or implementation
counterexamples. Kani's atomic operations are modeled sequentially, so no
concurrency claim is possible here.

The 64 free-list constructor, drop and capacity-fold loops get an explicit
unwind 65. `stateful.py` derives their IDs from current compiler loop output;
it does not raise every nested loop's bound. The loop-inspection invocation is
**not verification**, even though Kani's old-format trailer prints "success".
Only the subsequent structured verification report is accepted. Default safety
and unwinding checks remain enabled.

Accepted stateful receipts require the exact inventory, completion, no failed
or unresolved check, and every explicit proof assertion reachable and passing,
as in the scalar proofs. Only experimental induction groups allow unreachable conditional
invariant assertions (for example in an empty-state constructor), while still
requiring at least one reachable property. Tests reject wholly vacuous results
and require every proof in the source tree to belong to an explicit group.

### Defect found by the independent collector tests

`Heap::verify()` calls `mark_from_roots`, leaving reachable weak objects in
`gc.weak`. Previously the next mark traversal did not reset that scratch list.
If a WeakMap then lost its root while its key stayed rooted, the stale map
could incorrectly keep its value alive. Normal explicit verification produced
extra retention; automatic post-collection verification under `gc-stress`
reported "live but unreachable". This is a reproduced implementation defect,
not a Kani counterexample.

`verification_does_not_root_a_dead_weak_map_in_the_next_collection` fails on the
old implementation in a normal build. The fix clears `gc.weak` at the start of
every mark traversal, before discovering the current reachable weak objects.
Repeated verification, root removal and the next collection are covered. All
six new storage/collector model test functions pass in normal and stress modes.

## Status and acceptance

| Area | Implementation status | Acceptance boundary |
| --- | --- | --- |
| Scalar encodings | 12 production Kani scalar harnesses | Documented scalar domains only; callers' admission is not proved |
| Arena schedules | 6 production Kani schedules in normal and stress configurations | Fixed schedules plus the 130-slot reuse fixture |
| Arena mark | Source-bound Verus mark proof and mutation controls | Audited translations and stated caller preconditions |
| Arena induction | Abstract induction proved; production induction/refinement experimental | **Complete production correspondence remains unproved** |
| Heap and rooting | Storage/root-container proofs and separate full-collector finite model tests; weak-work-list defect fixed | Collector formal proofs and complete caller protocol remain open |

Proof code and model tests are test-gated. The weak-work-list fix changes the
production collector, so this PR requires both the Sweep and memory checks
under `AGENTS.md`; it is no longer in the no-benchmark tier. Results and the
exact measured commits/commands are reported in the PR, with raw receipts
outside version control.

## Tool references

- [Kani installation](https://model-checking.github.io/kani/install-guide.html)
- [Loop unwinding and bounded proofs](https://model-checking.github.io/kani/tutorial-loop-unwinding.html)
- [Supported Rust features](https://model-checking.github.io/kani/rust-feature-support.html)
- [Verus guide](https://verus-lang.github.io/verus/guide/)
- [Creusot guide](https://guide.creusot.rs/)
