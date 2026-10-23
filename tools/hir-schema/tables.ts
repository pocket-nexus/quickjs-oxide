// Section tags of the binary form (see the header of contracts/spec/hir.ts).

import type { Field } from "./schema.ts";

export const TABLES: Record<string, number> = {
  files: 1,
  strings: 2,
  consts: 3,
  types: 4,
  layouts: 5,
  descriptors: 6,
  decls: 7,
  functions: 8,
  modules: 9,
  nodes: 10,
  spans: 11,
  lists: 12,
  program: 13,
};

// The section tag of a `Program` table field, or null for the fields the
// program section holds.
export function tableField(f: Field): number | null {
  return f.type.t === "list" && f.name in TABLES ? TABLES[f.name] : null;
}
