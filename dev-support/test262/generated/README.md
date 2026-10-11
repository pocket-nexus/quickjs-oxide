# Generated conformance metadata

This directory holds the active generated Test262 manifests, source inventories,
closure/edge tables, and diagnostic ledgers. They describe conformance inputs
for the pinned corpus revision; they are not Cargo test targets.

Their producers are `scripts/test262/generate-test262-<topic>.mjs`. Match a
file's `test262-<topic>` prefix to its producer (for example `module-default-a`,
`module-static-negative-a`, or `import-meta-a`). Run without a mode flag, each
producer verifies its frozen output against the pinned corpus at
`target/oracle/quickjs-2026-06-04/test262` (override with `--suite DIR`);
`--admissions` emits its `admissions.tsv` rows. Inspect a producer's argument
handling before regenerating files.

`scripts/checks/check-test262-artifact-inventory.mjs` checks that tracked
artifacts have consumers. `scripts/test262/test-test262.sh --spec
dev-support/test262/current.conf --check` authenticates the current conformance
spec and receipts.
