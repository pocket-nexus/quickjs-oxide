# Test262 dev support

`current.conf` is the single authoritative Test262 gate specification. It pins
the current profile, focused manifest and receipts, full-corpus metrics, line
counts, summaries, SHA-256 hashes, and the current negative-diagnostic contract.
It also pins the exact admission catalog by repository path, line count, and
SHA-256. The gate parses it as inert `key=value` data.

The focused manifest is a sorted set of source paths. Its receipt is a
full `(path, variant)` vector, so one path may produce multiple metadata-driven
variants. The vector includes source-authenticated static JSON and top-level-await module
graphs, the exact dynamic-import Script-goal cohort, and the complete
dependency-free module local-binding family. Generator canaries keep
unauthenticated module graphs out of the vector.

`admissions.tsv` is the hash-pinned source of exact Module-goal graphs,
dynamic-import Script-goal graphs, dependency-free module roots, `$262.agent`
host paths, and supplemental feature contracts. Its strict 16-column TSV
schema records source hashes, complete metadata shapes, graph edges and closure
sizes, lookup priority, execution goal, and host cohort policy. Every
`dynamic-import-root` also declares a closed root policy. An
`initial-import-tree` must already contain the authenticated import opcode;
`runtime-compiled-import` authorizes a later runtime compilation and requires
the initial tree not to contain it; `parse-rejected` admits only an exact,
request-free, one-file `negative=parse/SyntaxError` root and hard-fails if it
reaches bytecode publication. Parse-negative roots cannot claim either
bytecode policy. A missing or unknown policy fails closed. Both coordinator and
isolated worker parse and authenticate the same file; malformed, unsorted,
duplicate, open-graph, mixed-goal, or checksum-drifted data fails closed. The
cohort generators emit `--admissions` rows and compare their owned group
against this file in normal check mode.

`negative-diagnostics.tsv` is a strict, source-authenticated overlay for
negative variants whose QuickJS failure reason and location are part of the
gate. Its path/variant rows carry the pinned source hash, phase/type, semantic
rule, exact QuickJS message, and an `exact` or `absent` location policy. The
runner validates the table and source metadata before scheduling tests. A
contracted variant passes only when phase, type, message, and location all
match. Test262 paths and source hashes stay in dev-support data, outside the
production parser.

`negative-diagnostic-rules.tsv` is the complete registry for the semantic
`rule` column and names the corresponding pinned QuickJS source anchor. The
authenticated audit tool rejects unknown or unused rules. Scheduled
differential CI replays every exact contract through pinned QuickJS using the
same Script/Module goal and strict-prefix policy and compares error type,
message, line, and column. Parse contracts use an isolated command-line source;
Module resolution and runtime contracts execute the authenticated root path
inside the pinned suite through `run-test262 -N --module`, so QuickJS loads the
real dependency graph. Runtime dependency-rejection contracts additionally
require an exact first source frame from their sole direct dependency; a root
throw with the same type and message is rejected. The Oxide runner reads only
own data properties, including the Error stack, when recovering the matching
location and separately observes compilation, linking, and execution to enforce
the declared phase. An absent location is represented by two empty TSV fields.

`negative-diagnostic-exemptions.tsv` is the frozen ledger of variants that check
phase and type only. Every audited negative variant belongs to exactly one of
the two files: an exact contract or this ledger. Overlap and missing rows are
fatal. New admissions extend the exact contract; an exemption leaves the ledger
only by becoming an exact contract.

The gate authenticates the current worktree with a canonical engine-semantics
fingerprint computed by `scripts/test262/test262-engine-fingerprint.mjs`. It
hashes sorted repository paths and exact contents of the files and trees listed
in `engine_semantics_files` and `engine_semantics_trees`: `Cargo.toml`,
`Cargo.lock`, the profile and upstream pins, the central preparation, gate and
report-verification scripts, and the `adapters`, `apps`, `conformance`,
`core` (the new core the engine links), `examples`, `src` and `tests` trees, plus the pinned `rust-toolchain.toml`. `current.conf` and `dev-support/` lie
outside that coverage and are authenticated separately, which avoids a circular
checksum. `test-test262.sh` fixes the coverage contract independently of
profile data and fails if an unlisted implicit build input such as `build.rs`,
`rust-toolchain` or `.cargo/config.toml` exists.

The gate always builds the Rust runner from the authenticated worktree. It
embeds the workspace fingerprint at compile time, verifies the binding before
execution, and runs a private byte-identical copy so another Cargo invocation
cannot replace workers during a long run. Set `CARGO_TARGET_DIR` to reuse
Cargo's authenticated build cache; the gate rejects a `TEST262_RUNNER`
override.

Use the one parameterized entry point:

```sh
./scripts/test262/test-test262.sh --check
./scripts/test262/test-test262.sh --runner-provenance
./scripts/test262/test-test262.sh --focused
TEST262_WORKERS=2 ./scripts/test262/test-test262.sh --full
node scripts/test262/audit-negative-diagnostics.mjs --suite /path/to/test262 \
  --qjs /path/to/pinned/qjs
```

For new Script-goal admissions, prepare a bytewise-sorted candidate TSV with
the header `path\tvariant\trule`, then generate source-authenticated rows with:

```sh
node scripts/test262/audit-negative-diagnostics.mjs \
  --generate /path/to/candidates.tsv --output /path/to/contracts.tsv \
  --suite /path/to/test262 --qjs /path/to/pinned/qjs \
  --oxide target/debug/qjs
```

Generation first inserts a leading runtime sentinel and requires both engines
to report the same source diagnostic one line later, proving that parsing fails
before execution. It then succeeds only when the original pinned QuickJS and
Oxide diagnostics match exactly. The candidate file assigns reviewed semantic
rules explicitly. Module candidates use the runner's module path; this CLI
helper covers Script-goal candidates. Scheduled differential CI runs the checked-in smoke
cohort plus runtime-deception, metadata, and A/A regression checks.

`--check` authenticates the spec, pinned inputs, and frozen receipts without
building or running the engine. `--focused` and `--full` stamp new receipts with
the current workspace fingerprint and compare their result bodies with the
frozen hashes. The gate recomputes that fingerprint after building the runner
and after execution, so concurrent source edits fail the run instead of
producing a mixed-source receipt.

The repository holds the current profile and focused receipt plus the small
semantic ledgers used by runner unit tests.
`scripts/checks/check-test262-artifact-inventory.mjs` requires every tracked
`dev-support/test262/generated/test262-*` artifact to be referenced by the
current spec, runner, or a generator. Archived milestone profiles, gates, result
vectors, baselines, and manifests are public `test262-history-*` release
assets; `archive/index.tsv` is their authenticated inventory.

The `$262` realm and agent host lives behind the non-default `test262-host`
feature; the central gate enables it when it builds `run-test262`. Feature-specific checks
can be run with `cargo test --locked --features test262-host --lib --bins`.

Official progress reports lead with full pass and eligible coverage. Runnable
pass rate is secondary. A new admission must be expressed in profile data and
must validate the correct negative phase/type and QuickJS diagnostic rule. The
central gate stays generic over cohorts and fixtures.

Active generated manifests and ledgers live in [`generated/`](generated/README.md).
Handwritten test inputs and frozen QuickJS observations live in
[`apps/cli/tests/fixtures/`](../../apps/cli/tests/fixtures/README.md).
