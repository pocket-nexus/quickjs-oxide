// Round-trips seeded random programs through the TypeScript writer and
// reader and compares them with the committed fixtures, which the Rust tests
// (`src/engine/hir/tests.rs`) read too: both sides must agree on every byte
// and every dump line.
//
//   node tools/hir-schema/test.ts            check
//   node tools/hir-schema/test.ts --update   rewrite the fixtures

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { type Decl, type Field, type TypeExpr, decl, isNode } from "./schema.ts";
import { SCHEMA, dump, read, strip, write } from "./io.ts";

const dir = fileURLToPath(new URL("./fixtures/", import.meta.url));
const update = process.argv.includes("--update");
// deno-lint-ignore no-explicit-any
type Any = any;

// Mulberry32: a small seeded generator, so the fixtures are reproducible.
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

class Gen {
  next: () => number;
  constructor(seed: number) {
    this.next = rng(seed);
  }
  int(n: number): number {
    return Math.floor(this.next() * n);
  }
  pick<T>(xs: T[]): T {
    return xs[this.int(xs.length)];
  }

  string(): string {
    const pool = ["a", "Z", "_", "é", "\u{1F600}", "\n", "\"", "\\", "\u0000", "\u001f", " ", "\ud800", "\udfff", " "];
    let s = "";
    const n = this.int(6);
    for (let i = 0; i < n; i++) s += this.pick(pool);
    return s;
  }

  f64(): number {
    return this.pick([0, -0, 1, -1.5, 0.1, 1e21, 2 ** 53, 2 ** 53 + 2, 1e-7, NaN, Infinity, -Infinity, 123.456, Number.MAX_VALUE, Number.MIN_VALUE]);
  }

  value(t: TypeExpr, depth: number, strings: number): Any {
    switch (t.t) {
      case "prim":
        switch (t.name) {
          case "u32":
            return this.pick([0, 1, 7, 65536, this.int(1000)]);
          case "i32":
            return this.pick([0, -1, 2147483647, -2147483648, this.int(1000)]);
          case "f64":
            return this.f64();
          case "boolean":
            return this.next() < 0.5;
          case "string":
            return this.string();
        }
        break;
      case "list": {
        const n = depth > 3 ? 0 : this.int(isNode(SCHEMA, t.elem) ? 3 : 4);
        return Array.from({ length: n }, () => this.value(t.elem, depth + 1, strings));
      }
      case "nullable":
        if (depth > 3 || this.next() < 0.4) return null;
        return this.value(t.inner, depth, strings);
      case "named": {
        const d = decl(SCHEMA, t.name);
        if (d.d === "ref") return t.name === "StringRef" ? this.int(strings + 1) : this.int(5);
        if (d.d === "enum") return this.pick(d.members);
        if (d.d === "struct") return this.fields(d.fields, depth + 1, strings, {});
        return this.variant(d as Decl & { d: "union" }, depth + 1, strings);
      }
    }
    throw new Error("unreachable");
  }

  // Past the depth bound, a node takes the variant with the fewest nodes in
  // its fields, so the tree ends.
  variant(d: Decl & { d: "union" }, depth: number, strings: number): Any {
    const nodes = (fields: Field[]) => fields.filter((f) => isNode(SCHEMA, f.type)).length;
    const v = depth > 4 ? d.variants.reduce((a, b) => (nodes(b.fields) < nodes(a.fields) ? b : a)) : this.pick(d.variants);
    return this.fields([...v.fields, ...d.common], depth, strings, { kind: v.kind });
  }

  fields(fields: Field[], depth: number, strings: number, into: Any): Any {
    for (const f of fields) into[f.name] = this.value(f.type, depth, strings);
    return into;
  }

  program(): Any {
    const p = decl(SCHEMA, "Program") as Decl & { d: "struct" };
    const strings = Array.from({ length: 1 + this.int(5) }, () => this.string());
    const out: Any = {};
    for (const f of p.fields) out[f.name] = f.name === "strings" ? strings : this.value(f.type, 0, strings.length);
    out.version = 1;
    return out;
  }
}

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((x, i) => x === b[i]);
}

function check(name: string, actual: string | Uint8Array): void {
  const path = `${dir}${name}`;
  if (update) {
    writeFileSync(path, actual);
    return;
  }
  if (!existsSync(path)) throw new Error(`missing fixture ${name}: run with --update`);
  const expected = readFileSync(path);
  const same = typeof actual === "string" ? expected.toString("utf8") === actual : equalBytes(new Uint8Array(expected), actual);
  if (!same) throw new Error(`fixture ${name} differs`);
}

mkdirSync(dir, { recursive: true });
const SEEDS = [1, 2, 3, 4, 5, 6, 7, 8];
for (const seed of SEEDS) {
  const p = new Gen(seed).program();
  const bytes = write(p);
  const back = read(bytes);
  if (!equalBytes(write(back), bytes)) throw new Error(`seed ${seed}: write(read(write(p))) differs`);
  if (dump(back) !== dump(p)) throw new Error(`seed ${seed}: the dump changed in the round trip`);
  check(`random-${seed}.hir`, bytes);
  check(`random-${seed}.dump`, dump(p));
  check(`random-${seed}.spans.dump`, dump(p, { spans: true }));
  strip(back);
  check(`random-${seed}.strip.dump`, dump(back));
}
console.log(`hir-schema: ${SEEDS.length} programs round-trip${update ? "; fixtures updated" : " and match the fixtures"}`);
