# Test262 baseline

Test262 is the primary broad conformance signal for quickjs-oxide. The suite
revision and the authenticated QuickJS 2026-06-04 patch and configuration are
pinned in [`compat/upstream.toml`](../compat/upstream.toml) and mirrored by
[`dev-support/test262/current.conf`](../dev-support/test262/current.conf).

## Official metrics

<!-- current-test262-metrics:start -->
Metrics are reported in this order:

1. **Full pass:** 80,010 / 102,037 (78.413%). Every frozen Test262 variant is in
   the denominator.
2. **Eligible coverage:** 80,060 / 102,037 (78.462%). This measures how much of
   the full vector the current profile admits to execution.
3. **Runnable pass quality:** 80,010 / 80,060 (99.938%). This is useful for
   diagnosing admitted behavior, but it must not replace either coverage
   metric above.

The frozen outcome summary is:

```text
fail-parse=7 fail-runtime=43 pass=80010 skipped-config-exclude=6700 skipped-feature=11775 unsupported-feature=847 unsupported-module=121 unsupported-negative-provenance=2534
```
<!-- current-test262-metrics:end -->

## Reproduce

One script consumes one inert data spec:

```sh
./scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --check
./scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --runner-provenance
./scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --focused
TEST262_WORKERS=2 ./scripts/test262/test-test262.sh \
  --spec dev-support/test262/current.conf --full
```

`--check` authenticates the in-tree upstream pin, profile, negative-diagnostic
contract and phase/type-only exemption ledger, focused manifest, and frozen
TSV/JSONL receipts. `--runner-provenance` builds the Rust runner with the
current source fingerprint, verifies the embedded binding, and executes no
Test262 cases. `--focused` replays the 6,844-variant focused vector and requires
the result body, everything after the identity line, to be byte-identical;
public CI runs it on every pull request, every push to main, and a
`test262-focused` dispatch. The focused vector covers dependency-closed
private-callable, static import-attributes, and static JSON module cases;
source-authenticated dynamic-import and top-level-await syntax, runtime, graph,
and rejection cohorts; and the dependency-free module local-binding family.
`--full` runs all 102,037 variants and compares the complete result body against
the frozen `full_*_body_sha256` hashes; the scheduled `test262-full` job and the
`test262-receipt` dispatch cover it.

## Promotion

Every spec key refers to in-tree data. When outcomes change, only frozen data
and hashes are updated.

- Focused outcomes: regenerate the frozen focused receipts, update
  `focused_tsv_sha256`/`focused_jsonl_sha256` and their line counts, and set
  `engine_semantics_sha256` to the receipt identity reported by
  `test262-engine-fingerprint.mjs --worktree`.
- Full outcomes: dispatch `test262-receipt`; the capture step prints
  `full_tsv_body_sha256` and `full_jsonl_body_sha256` for `current.conf`.
- Every gate then replays the frozen vectors and compares result bodies
  directly.

Negative admissions are fail-closed: an expected failure counts only when its
exact path is present in the audited-negative data. Every admitted path/variant
belongs to exactly one diagnostic class. Exact contracts require the
pinned-QuickJS phase, type, message, and `exact` or `absent` location policy. A
frozen ledger identifies the 2,586 variants that check phase and type only.
Schema-v5 receipts authenticate both data files and record expected/actual
diagnostic fields. New negative admissions add exact contracts. The semantic
rule registry is separately authenticated, and scheduled QuickJS differential CI
replays every exact variant against QuickJS 2026-06-04 so the stored message and
location stay tied to the QuickJS oracle.

Archived milestone profiles, gates, and result vectors are release assets
inventoried in
[`dev-support/test262/archive/index.tsv`](../dev-support/test262/archive/index.tsv).
