# Planned work

This page collects the remaining memory and embedding-target work. It describes
proposals, not available APIs or CLI options. The current implementation is in
[Heap](heap.md), [Execution](performance/execution.md),
[Architecture](architecture.md) and the [core guide](../core/README.md).
Each item needs a consumer or a measured benefit before implementation; there
is no delivery schedule.

## Already implemented

The heap has a reserved-bytes budget, a 1 MiB minimum and growth 2, bitmap arena
allocation, value-range splitting and coalescing, and shrinking of trailing free
capacity after collection. Value ranges include both powers of two and three
times powers of two. The compiler emits `ConcatN` for eligible concatenation
chains. The CLI uses a startup symbol-order file on x86_64 Linux, and the release
profile uses `panic = "abort"`.

Dynamic code and its inline caches are reclaimed when unreachable; this closed
[#156](https://github.com/pocket-nexus/quickjs-oxide/issues/156). These mechanisms
are described in the implementation guides rather than retained as future
commits. The original core replacement issue
[#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) is also closed;
the target work below remains deferred.

## Peak memory

[#158](https://github.com/pocket-nexus/quickjs-oxide/issues/158) remains open.
Its target is a geometric mean peak RSS no greater than 1.3× QuickJS across the
37 fixed-work programs, with elapsed time within 2% of the agreed base. The
[latest Publish measurement](../apps/web/docs/performance.md#memory) is still
above that memory target. The old implementation sequence and its estimated
per-program savings are superseded by the mechanisms above and current
measurements.

Further changes start with a fresh memory table separating process RSS,
compile-phase peak, retained code, heap reservation and live bytes after a
collection. Compiler allocation is tracked in
[#154](https://github.com/pocket-nexus/quickjs-oxide/issues/154). Keep growth 2
and the 1 MiB minimum as the default; a different trigger needs its own measured
tradeoff. Record each change against the same committed base, including churn
workloads, and keep failed gates in the report.

## Heap cap policy

A cap and a catchable, preallocated out-of-memory error already exist. Remaining
work is to clamp reservation steps to the space left under the cap, verify the
trigger's behavior near it, and allow the register file to shrink at an idle
runtime boundary. A configurable initial register reservation is a proposal.

Acceptance includes 100 allocation-failure/recovery cycles under `gc-stress`,
with the heap usable after every catch. Run the 37 programs at reduced work,
fixed probes and a churn ring under 32, 24 and 16 MiB caps (8 MiB for a ring
sized to 4 MiB live). Record completion or out-of-memory, collections, pauses,
reserved bytes, peak RSS and time against an uncapped run. At 32 MiB, workloads
whose live set is below half the cap must complete; RSS must remain within the
cap plus the measured process floor plus 10%. Report workloads that cannot fit.

## Code images and heap snapshots

These are separate proposed artifacts: a code image contains lowered units and
constants without runtime heap state; a heap snapshot contains a quiescent
runtime's arenas and stores. Host state stays outside a snapshot. Proposed
`load_image`, `snapshot` and `from_snapshot` APIs, and `--compile`, `--snapshot`
and `--load-snapshot` CLI options, are not implemented.

The design keeps indices stable and uses a little-endian format with fixed-width
indices and a build fingerprint. Rebuild hash tables at load, start inline
caches empty, and run the bytecode publication validator on loaded images.
Snapshot creation requires no active frames, jobs, kept objects, live handles,
agent threads or waiters. Loading validates every length and index.

Acceptance: deterministic snapshots; identical formats across 32- and 64-bit
builds; corpus and suite outputs preserved after loading; truncation and byte
mutation tests return errors without panics. Measure the realm alone and after
AWFY, TypeScript and Mandreel setup. A realm snapshot should load in fewer than
half the instructions of cold initialization; code images in fewer than a tenth
of compilation's instructions. Record artifact sizes and elapsed time as well.

## 32-bit and builds without the standard library

WebAssembly is already a supported 32-bit build. Native 32-bit validation is
proposed on `i686-unknown-linux-musl`: audit conversions and public widths, run
the corpus, fixed probes, suite programs and focused Test262 vector, and record
record sizes on both widths. Image and snapshot tests must cross-load artifacts
between i686 and x86_64 once those formats exist.

The core currently uses `std`. A proposed `std` feature split would use `alloc`,
the core's own hash table in both builds, and a host-supplied math provider for
transcendentals. Preserve exact rounding and default-host behavior. Threaded
agents and blocking waits need an explicit policy for targets without threads;
any observable change belongs in the deviation ledger and conformance checks.
The RegExp crate's thread-local scratch cache also needs a policy without `std`.

Acceptance includes a `thumbv7em-none-eabi` core build without default features,
host-side tests of that configuration, and the normal instruction and
conformance gates for shared implementations. Console SDK integration, nightly
`-Zbuild-std` recipes and device host services belong to the eventual port.

## Optional generational collection

The heap remains non-moving. The proposed experiment uses sticky mark bits,
an allocation log and a remembered set, with write barriers in the heap's write
entry points. Extend the verifier and stress mode to cover minor collections,
weak edges and every old-to-young store before evaluating performance.

The experiment's go/no-go remains: Splay's collector share below 15% and its
elapsed time reduced by at least 15%; no V8 v7 program above the normal 2%
instruction gate; every other program's time within 3% of the base. Build it
behind a temporary feature until measured, then adopt or remove it. The
[Claim checks](performance/benchmarking.md#claim) also apply; these margins do
not excuse layout noise. Other items do not depend on this experiment.

## Validation and reporting

Follow [the agent guide](../AGENTS.md) and
[Benchmarking](performance/benchmarking.md) for each change's required checks.
Commit the candidate before building measurement binaries, fix sample counts
and order in advance, and measure serially. Report base and candidate identities,
commands, A/A noise, tables and any missed gate in the pull request; keep raw
results out of commits. Add CI coverage when a proposed capability actually
lands, rather than documenting future jobs as existing checks.
