// Generates `src/engine/hir/schema.rs` from `contracts/spec/hir.ts`.
//
//   node tools/hir-schema/gen.ts           write the generated file
//   node tools/hir-schema/gen.ts --check   fail when it is stale

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { emitRust } from "./rust.ts";
import { parseSchema, schemaVersion } from "./schema.ts";

const root = fileURLToPath(new URL("../../", import.meta.url));
const schemaPath = `${root}contracts/spec/hir.ts`;
const rustPath = `${root}src/engine/hir/schema.rs`;

const source = readFileSync(schemaPath, "utf8");
const raw = emitRust(parseSchema(source), schemaVersion(source));
const rust = execFileSync("rustfmt", ["--edition", "2024", "--emit", "stdout"], { input: raw, encoding: "utf8", maxBuffer: 1 << 28 });

if (process.argv.includes("--check")) {
  let current = "";
  try {
    current = readFileSync(rustPath, "utf8");
  } catch {
    // A missing file is stale.
  }
  if (current !== rust) {
    console.error(`${rustPath} is stale: run node tools/hir-schema/gen.ts`);
    process.exit(1);
  }
} else {
  writeFileSync(rustPath, rust);
}
