# HIR schema tools

`contracts/spec/hir.ts` is the HIR schema: TypeScript type definitions in a
small dialect its header describes, together with the binary encoding they
fix. Everything else is derived from it.

| File | Role |
| --- | --- |
| `gen.ts` | Generates `src/engine/hir/schema.rs` (types, writer, reader, dump, `strip`); `--check` fails when the file is stale |
| `io.ts` | The TypeScript reader, writer, dump and `strip`: an interpreter over the parsed schema |
| `test.ts` | Round-trips seeded random programs and compares them with `fixtures/`; `--update` rewrites the fixtures |
| `fixtures/` | Each program's bytes, dump, dump with spans and stripped dump; the Rust tests read the same files |

After editing the schema:

```sh
node tools/hir-schema/gen.ts
node tools/hir-schema/test.ts --update
cargo test --lib engine::hir
```

The Rust and TypeScript sides are written independently, so the fixtures
check each against the other: the Rust reader must read every fixture,
write the same bytes back and print the same dumps.
