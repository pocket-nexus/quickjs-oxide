# Generated Unicode tables

generated holds Unicode data for a pinned upstream version together with its
regeneration instructions. Ordinary builds read these files directly without
running generators.

## Files and subdirectories

- [unicode/](unicode/README.md): the four checksum-pinned Rust Unicode tables.

## Regeneration

The handwritten algorithms in `regexp/src/unicode/` include the tables in
`unicode/` in private table modules; generated files add no public API.

Regenerate with the corresponding `scripts/unicode/generate-unicode-*-tables`
command and the pinned QuickJS source, following the source and license
information in each header. Generators default to `unicode/`. Ordinary builds
consume the checked-in tables without running generators or a reference engine.

To verify an update, generate into a temporary output file and compare it with
the checked-in file; run the Unicode library tests and the normalization
fingerprint check. Test262 metadata belongs in `dev-support/test262/generated/`.
