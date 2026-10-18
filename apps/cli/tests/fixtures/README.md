# Fixture provenance

`inputs/` contains authored JS, module, and C probes. `expected/` contains frozen
observations produced with the pinned QuickJS release identified in each file
name (or its parent suite). Rust integration tests and reference-engine scripts
share these files.

- `dev-support/quickjs-fixture-gates.tsv` records JS probe inputs, expected outputs,
  checksums, driver modes, and the deviation-ledger entry (or `-`) a fixture differs by
  on quickjs-oxide. Run `scripts/quickjs/test-quickjs-fixtures.sh --validate`
  to authenticate the registry, or `--all` to execute it with the pinned engine.
- `dev-support/quickjs-c-oracles.tsv` records C probes and expected transcripts.
  Run `scripts/quickjs/test-quickjs-c-oracles.sh --validate` to authenticate them, or
  `--check` to build and compare them with the reference engine.
- Host and dynamic-import probes use their dedicated `scripts/quickjs/test-*.sh`
  drivers. Unicode C probes feed the generation tools in `scripts/unicode/`.

To update a reference observation, use its driver with the pinned engine, inspect
the behavior change, then update the expected output and its registry checksum
in the same change. Test runs only compare against expected output.
Generated Test262 metadata belongs in `dev-support/test262/generated/`.
