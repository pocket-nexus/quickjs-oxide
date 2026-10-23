# Formal verification

The first phase verifies scalar value representation and value-heap size/range
encoding in the **production Rust functions**, using Kani. It does not verify
the whole engine, its ECMAScript semantics, allocation, or garbage collection.

## Run phase 1

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
failed, or vacuous harness results fail the run. Each harness has a 60-second
timeout; a timeout is not a proof.

Each invocation creates a new `target/verification/phase1-*` directory with:

- `run.log`: complete compiler and verifier output, including warnings;
- `results.json`: Kani's harnesses, checks, target and tool versions;
- `provenance.json`: the command, HEAD, working-tree status, and SHA-256 hashes
  of the core/regexp sources, their manifests, workspace manifest, lockfile,
  toolchain file and verification scripts.

Raw results stay out of commits. The `Formal verification (phase 1)` workflow
runs on relevant pull requests, main pushes, and manual dispatch, and uploads
these receipts even on failure. Changing the harness inventory requires updating
the runner's expected set and this document.

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

## Staged implementation

Each phase should be a separately reviewable PR with actual proof receipts and
an explicit list of remaining assumptions. The following phases are planned,
not completed by the phase-1 proofs.

| Phase | Work | Completion criterion |
| --- | --- | --- |
| 1 — implemented | Kani for Value and size-class/range encoding | The 12 named scalar properties pass over the documented domains; assumptions and provenance are recorded |
| 2 — next | Kani for small arena operation sequences | Production arena agrees with an independent slot-array model for fixed, recorded capacity/sequence bounds; normal and quarantine behavior are covered |
| 3 — planned | Select Verus or Creusot through one arena operation | An operation's precondition, representation invariant, frame condition and postcondition are proved against executable production code or a proved correspondence |
| 4 — planned | Constructor and all relevant arena mutations | Initialization establishes the invariant and every operation preserves it under discharged caller obligations, supporting induction over arbitrary legal finite sequences |
| 5 — planned | Value heap, collector and rooting protocol | Separate coverage reports for allocation/partition invariants, collector reachability including ephemerons, and callers' root/safepoint obligations |

Phase 2 should compare allocation, marking, sweep and reuse against a simple
model, including content preservation and free/live counters. Bounds are part
of the result; they must not be described as arbitrary-sequence correctness.
Exercise multiword bitmap boundaries explicitly before increasing sequence
lengths. A proof on a few slots does not cover the cursor crossing bit 63.

Phases 3–4 need a model of live, reusable and quarantined slots, and must show
that existing live records survive unrelated changes. Proving only an abstract
arena does not verify the current Rust implementation. Trusted external
contracts and unverified bodies must be inventoried rather than hidden.

Phase 5 separates collector correctness from complete rooting by callers. The
existing stress verifier reuses `mark_from_roots`; the formal reachability model
must be independent of that traversal. Host reentry, VM registers, weak maps,
dynamic code and other root sources need explicit coverage.

Phase 1 changes only test-gated Rust, verification tooling, CI and documentation;
it takes the repository's no-benchmark tier. Future production changes must run
the Sweep and the additional memory/compile/scaling checks required by
`AGENTS.md`, even if their motivation is verification.

## Tool references

- [Kani installation](https://model-checking.github.io/kani/install-guide.html)
- [Loop unwinding and bounded proofs](https://model-checking.github.io/kani/tutorial-loop-unwinding.html)
- [Supported Rust features](https://model-checking.github.io/kani/rust-feature-support.html)
- [Verus guide](https://verus-lang.github.io/verus/guide/)
- [Creusot guide](https://guide.creusot.rs/)
