# Development tools

Implementations have a single owner:

| Directory | Responsibility |
| --- | --- |
| `checks/` | Architecture and provenance checks, oracle inventory, aggregate parity gate |
| `verification/` | Pinned Kani/Verus proofs and their receipts; see `docs/formal-verification.md` |
| `test262/` | Suite preparation, admission generators, diagnostics, metrics and receipt verification |
| `benchmark/` | External workload runners, profiler experiments and build/result provenance |
| `quickjs/` | Pinned reference builds, cache tests, differential fixtures and dynamic-import traces |
| `unicode/` | Unicode table generation and source-fingerprint verification |
| `web/` | Site and docs build, Node/browser checks and Pages deployment tooling |
| `release/` | Version tagging and GitHub Release packaging; see `docs/release.md` |

Commands, CI and generated-file headers use these paths directly. `demo-42.sh`
is a small repository-level example. New tools belong in their owner directory.

Shell scripts coordinate commands and temporary files. Python, Node and AWK
analysis lives in separate source files, including Unicode extraction, Test262
report validation, host metadata checks and site assertions. Helpers stay
with their owner. Admission generators share `test262/test262-admission-data.mjs`. Avoid a generic shared
utilities directory until unrelated owners actually need the same contract.

Handwritten Unicode algorithms live in `regexp/src/unicode/`; generated production
tables live in `regexp/src/unicode/generated/unicode/`. Product builds consume those checked-in
tables without running generators or compiling the QuickJS reference. Test262
generated evidence remains in `dev-support/test262/generated/`.

Tool regression tests live beside the implementation they exercise. Engine unit
and integration tests retain the Cargo layout documented in `tests/README.md`.

Archived Test262 receipts keep their recorded source commit, input list and
fingerprint; their input paths are data, not entry points. Run fingerprints
cover the preparation and runner scripts and the report validator.
