# Generated conformance metadata

This directory holds the active generated Test262 manifests, source inventories,
closure/edge tables, and diagnostic ledgers formerly stored at `tests/test262-*`.
The relocation preserves their bytes, source hashes, and pinned corpus revision.
They describe conformance inputs; they are not ordinary Cargo test targets.

Their producer scripts are `scripts/test262/generate-test262-*.mjs`. Match a file's
`test262-<topic>` prefix to its producer (for example `module-default-a`,
`module-static-negative-a`, or `import-meta-a`). Existing producer `--check` modes
verify the frozen output without rewriting it. Use the pinned corpus prepared by
the conformance tooling, and inspect generator usage before regenerating files.

Track a generated artifact only when the runtime or a gate independent of its
producer reads it directly, or when `dev-support/test262/current.conf` pins its
exact bytes. Generator-only manifests, inventories, closure tables, and diagnostic
candidates must instead be regenerated into a temporary directory by the producer's
test. A new cohort therefore has a zero-file tracked-artifact budget unless a
concrete consumer or `current.conf` entry is added in the same change. Existing
producer-owned files are migrated to this rule when their generator is next changed;
removing them all at once would mix unrelated cohort contracts into one review.

`scripts/checks/check-test262-artifact-inventory.mjs` checks that tracked artifacts have
consumers. `scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --check`
authenticates the current conformance spec and receipts. A structural change can
make a historical engine fingerprint stale without invalidating those receipts;
only a new conformance run establishes results for the new source revision.
