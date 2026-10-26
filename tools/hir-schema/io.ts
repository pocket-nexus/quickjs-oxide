// The TypeScript reader, writer, dump and strip: one interpreter over the
// parsed schema, independent of the generated Rust, so the two check each
// other through the fixtures. Values are plain objects shaped as in
// `contracts/spec/hir.ts`.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  type Decl,
  type Field,
  type Schema,
  type TypeExpr,
  decl,
  hasNodeSpan,
  isNode,
  parseSchema,
  schemaVersion,
  simple,
  variantFields,
  words,
} from "./schema.ts";
import { TABLES, tableField } from "./tables.ts";

export const SCHEMA_PATH = fileURLToPath(new URL("../../contracts/spec/hir.ts", import.meta.url));
const SOURCE = readFileSync(SCHEMA_PATH, "utf8");
export const SCHEMA: Schema = parseSchema(SOURCE);
export const SCHEMA_VERSION = schemaVersion(SOURCE);

type Union = Decl & { d: "union" };
type Struct = Decl & { d: "struct" };
// deno-lint-ignore no-explicit-any
type Any = any;

const program = decl(SCHEMA, "Program") as Struct;

function union(name: string): Union {
  return decl(SCHEMA, name) as Union;
}

function variantOf(u: Union, kind: string): { index: number; fields: Field[] } {
  const index = u.variants.findIndex((v) => v.kind === kind);
  if (index < 0) throw new Error(`${u.name} has no kind ${kind}`);
  return { index, fields: variantFields(SCHEMA, u, u.variants[index]) };
}

// ---- writer ----

class Writer {
  nodes: number[] = [];
  spans: number[] = [];
  lists: number[] = [];
  count = 0;

  node(words: number[], span: Any | null): number {
    this.nodes.push(...words);
    if (span) this.spans.push(span.file, span.start, span.end);
    else this.spans.push(0, 0, 0);
    return ++this.count;
  }

  list(out: number[], items: number[], len: number): void {
    out.push(this.lists.length, len);
    for (const x of items) this.lists.push(x);
  }

  string(out: number[], s: string): void {
    out.push(this.lists.length, s.length);
    for (let i = 0; i < s.length; i += 2) {
      const high = i + 1 < s.length ? s.charCodeAt(i + 1) : 0;
      this.lists.push((s.charCodeAt(i) | (high << 16)) >>> 0);
    }
  }
}

const f64buf = new DataView(new ArrayBuffer(8));

function pushF64(out: number[], x: number): void {
  f64buf.setFloat64(0, x, true);
  out.push(f64buf.getUint32(0, true), f64buf.getUint32(4, true));
}

function scalar(t: TypeExpr, v: Any): number {
  if (t.t === "prim") {
    if (t.name === "boolean") return v ? 1 : 0;
    return v >>> 0;
  }
  if (t.t === "named") {
    const d = decl(SCHEMA, t.name);
    if (d.d === "ref") return v >>> 0;
    if (d.d === "enum") {
      const i = d.members.indexOf(v);
      if (i < 0) throw new Error(`${v} is not a ${d.name}`);
      return i;
    }
  }
  throw new Error(`not a scalar: ${JSON.stringify(t)}`);
}

function enc(w: Writer, t: TypeExpr, v: Any, out: number[]): void {
  switch (t.t) {
    case "prim":
      if (t.name === "f64") pushF64(out, v);
      else if (t.name === "string") w.string(out, v);
      else out.push(scalar(t, v));
      return;
    case "list": {
      const buf: number[] = [];
      for (const x of v) enc(w, t.elem, x, buf);
      w.list(out, buf, v.length);
      return;
    }
    case "nullable": {
      const inner = t.inner;
      if (inner.t === "prim" && inner.name === "f64") {
        if (v === null) out.push(0, 0, 0);
        else {
          out.push(1);
          pushF64(out, v);
        }
      } else if (inner.t === "named" && decl(SCHEMA, inner.name).d === "struct") {
        if (v === null) for (let i = 0; i < words(SCHEMA, t); i++) out.push(0);
        else {
          out.push(1);
          enc(w, inner, v, out);
        }
      } else if (isNode(SCHEMA, inner)) {
        out.push(v === null ? 0 : node(w, union((inner as { name: string }).name), v));
      } else {
        out.push(v === null ? 0 : scalar(inner, v) + 1);
      }
      return;
    }
    case "named": {
      const d = decl(SCHEMA, t.name);
      if (d.d === "ref" || d.d === "enum") out.push(scalar(t, v));
      else if (d.d === "struct") for (const f of d.fields) enc(w, f.type, v[f.name], out);
      else if (SCHEMA.inline.has(d.name)) {
        const { index, fields } = variantOf(d, v.kind);
        out.push(index);
        for (const f of fields) enc(w, f.type, v[f.name], out);
      } else out.push(node(w, d, v));
      return;
    }
  }
}

function node(w: Writer, u: Union, v: Any): number {
  const { index, fields } = variantOf(u, v.kind);
  const out = [u.kinds! + index];
  for (const f of fields) enc(w, f.type, v[f.name], out);
  return w.node(out, hasNodeSpan(SCHEMA, u) ? v.span : null);
}

// FNV-1a over the UTF-16LE bytes of each input, each followed by two zero
// bytes.
export function fingerprint(inputs: string[]): number {
  let hash = 0x811c9dc5;
  const byte = (b: number) => {
    hash = Math.imul(hash ^ b, 0x01000193) >>> 0;
  };
  for (const s of inputs) {
    for (let i = 0; i < s.length; i++) {
      const u = s.charCodeAt(i);
      byte(u & 0xff);
      byte(u >> 8);
    }
    byte(0);
    byte(0);
  }
  return hash;
}

export function write(p: Any): Uint8Array {
  const w = new Writer();
  const head: number[] = [];
  for (const f of program.fields) if (!tableField(f)) enc(w, f.type, p[f.name], head);
  const sections: [number, number[]][] = [];
  for (const f of program.fields) {
    const tag = tableField(f);
    if (!tag || f.type.t !== "list") continue;
    const out = [p[f.name].length];
    for (const x of p[f.name]) enc(w, f.type.elem, x, out);
    sections.push([tag, out]);
  }
  sections.push([TABLES.nodes, w.nodes], [TABLES.spans, w.spans], [TABLES.lists, w.lists], [TABLES.program, head]);
  sections.sort((a, b) => a[0] - b[0]);
  let size = 16;
  for (const [, ws] of sections) size += 16 + Math.ceil((ws.length * 4) / 16) * 16;
  const bytes = new Uint8Array(size);
  const view = new DataView(bytes.buffer);
  bytes.set([0x48, 0x49, 0x52, 0x31]);
  view.setUint32(4, SCHEMA_VERSION, true);
  view.setUint32(8, sections.length, true);
  view.setUint32(12, fingerprint(p.unit.fingerprintInputs), true);
  let at = 16;
  for (const [tag, ws] of sections) {
    view.setUint16(at, tag, true);
    view.setUint32(at + 4, ws.length * 4, true);
    at += 16;
    for (const x of ws) {
      view.setUint32(at, x, true);
      at += 4;
    }
    at = Math.ceil(at / 16) * 16;
  }
  return bytes;
}

// ---- reader ----

class Cur {
  w: Uint32Array;
  at: number;
  constructor(w: Uint32Array, at: number) {
    this.w = w;
    this.at = at;
  }
  u(): number {
    if (this.at >= this.w.length) throw new Error("malformed HIR: truncated record");
    return this.w[this.at++];
  }
  flag(): boolean {
    const x = this.u();
    if (x > 1) throw new Error("malformed HIR: a boolean is neither 0 nor 1");
    return x === 1;
  }
  f64(): number {
    f64buf.setUint32(0, this.u(), true);
    f64buf.setUint32(4, this.u(), true);
    return f64buf.getFloat64(0, true);
  }
}

class Reader {
  nodes: Uint32Array;
  offsets: number[] = [];
  spans: Uint32Array;
  lists: Uint32Array;
  constructor(nodes: Uint32Array, spans: Uint32Array, lists: Uint32Array) {
    this.nodes = nodes;
    this.spans = spans;
    this.lists = lists;
    let i = 0;
    while (i < nodes.length) {
      this.offsets.push(i);
      const size = nodeWords(nodes[i]);
      if (size === null) throw new Error("malformed HIR: unknown node kind");
      i += size;
    }
    if (i !== nodes.length) throw new Error("malformed HIR: the last node is truncated");
  }

  list(c: Cur, size: number): [Cur, number] {
    const start = c.u();
    const len = c.u();
    if (start + len * size > this.lists.length) throw new Error("malformed HIR: a list runs past the list section");
    return [new Cur(this.lists, start), len];
  }

  string(c: Cur): string {
    const start = c.u();
    const len = c.u();
    if (start + Math.ceil(len / 2) > this.lists.length) throw new Error("malformed HIR: a string runs past the list section");
    let s = "";
    for (let i = 0; i < len; i++) {
      const word = this.lists[start + (i >> 1)];
      s += String.fromCharCode(i & 1 ? word >>> 16 : word & 0xffff);
    }
    return s;
  }
}

const KIND_SIZE = new Map<number, number>();
const KIND_UNION = new Map<number, Union>();
for (const name of SCHEMA.order) {
  const d = decl(SCHEMA, name);
  if (d.d !== "union" || SCHEMA.inline.has(name)) continue;
  d.variants.forEach((v, i) => {
    KIND_SIZE.set(d.kinds! + i, 1 + variantFields(SCHEMA, d, v).reduce((a, f) => a + words(SCHEMA, f.type), 0));
    KIND_UNION.set(d.kinds! + i, d);
  });
}

function nodeWords(kind: number): number | null {
  return KIND_SIZE.get(kind) ?? null;
}

function fromScalar(t: TypeExpr, x: number): Any {
  if (t.t === "prim") {
    if (t.name === "boolean") return x !== 0;
    if (t.name === "i32") return x | 0;
    return x;
  }
  if (t.t === "named") {
    const d = decl(SCHEMA, t.name);
    if (d.d === "ref") return x;
    if (d.d === "enum") {
      if (x >= d.members.length) throw new Error("malformed HIR: an enumeration value is out of range");
      return d.members[x];
    }
  }
  throw new Error(`not a scalar: ${JSON.stringify(t)}`);
}

function dec(r: Reader, t: TypeExpr, c: Cur, limit: number): Any {
  switch (t.t) {
    case "prim":
      if (t.name === "f64") return c.f64();
      if (t.name === "string") return r.string(c);
      if (t.name === "boolean") return c.flag();
      return fromScalar(t, c.u());
    case "list": {
      const [lc, n] = r.list(c, words(SCHEMA, t.elem));
      const out = [];
      for (let i = 0; i < n; i++) out.push(dec(r, t.elem, lc, limit));
      return out;
    }
    case "nullable": {
      const inner = t.inner;
      if (inner.t === "prim" && inner.name === "f64") {
        const present = c.flag();
        const x = c.f64();
        return present ? x : null;
      }
      if (inner.t === "named" && decl(SCHEMA, inner.name).d === "struct") {
        const present = c.flag();
        const x = dec(r, inner, c, limit);
        return present ? x : null;
      }
      const x = c.u();
      if (x === 0) return null;
      if (isNode(SCHEMA, inner)) return readNode(r, union((inner as { name: string }).name), x, limit);
      return fromScalar(inner, x - 1);
    }
    case "named": {
      const d = decl(SCHEMA, t.name);
      if (d.d === "ref" || d.d === "enum") return fromScalar(t, c.u());
      if (d.d === "struct") {
        const v: Any = {};
        for (const f of d.fields) v[f.name] = dec(r, f.type, c, limit);
        return v;
      }
      if (SCHEMA.inline.has(d.name)) {
        const k = c.u();
        const variant = d.variants[k];
        if (!variant) throw new Error(`malformed HIR: unknown ${d.name} kind`);
        const v: Any = { kind: variant.kind };
        for (const f of variant.fields) v[f.name] = dec(r, f.type, c, limit);
        return v;
      }
      return readNode(r, d, c.u(), limit);
    }
  }
}

function readNode(r: Reader, u: Union, n: number, limit: number): Any {
  if (n === 0) throw new Error("malformed HIR: a node reference is 0");
  const index = n - 1;
  if (index >= limit) throw new Error("malformed HIR: a node reference does not point to an earlier node");
  const at = r.offsets[index];
  const kind = r.nodes[at];
  if (KIND_UNION.get(kind) !== u) throw new Error(`malformed HIR: a node is not a ${u.name}`);
  const variant = u.variants[kind - u.kinds!];
  const c = new Cur(r.nodes, at + 1);
  const v: Any = { kind: variant.kind };
  for (const f of variantFields(SCHEMA, u, variant)) v[f.name] = dec(r, f.type, c, index);
  if (hasNodeSpan(SCHEMA, u)) {
    const s = 3 * index;
    v.span = s + 3 <= r.spans.length ? { file: r.spans[s], start: r.spans[s + 1], end: r.spans[s + 2] } : { file: 0, start: 0, end: 0 };
  }
  return v;
}

export function read(bytes: Uint8Array): Any {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 16 || String.fromCharCode(...bytes.subarray(0, 4)) !== "HIR1") throw new Error("malformed HIR: no HIR1 magic");
  if (view.getUint32(4, true) !== SCHEMA_VERSION) throw new Error("malformed HIR: another schema version");
  const count = view.getUint32(8, true);
  const sections = new Map<number, Uint32Array>();
  let at = 16;
  for (let i = 0; i < count; i++) {
    if (at + 16 > bytes.length) throw new Error("malformed HIR: truncated section header");
    const tag = view.getUint16(at, true);
    const len = view.getUint32(at + 4, true);
    if (len % 4 !== 0) throw new Error("malformed HIR: a section is not whole words");
    if (at + 16 + len > bytes.length) throw new Error("malformed HIR: a section runs past the end");
    const words = new Uint32Array(len / 4);
    for (let j = 0; j < words.length; j++) words[j] = view.getUint32(at + 16 + 4 * j, true);
    if (!sections.has(tag)) sections.set(tag, words);
    at += 16 + Math.ceil(len / 16) * 16;
  }
  const empty = new Uint32Array(0);
  const r = new Reader(sections.get(TABLES.nodes) ?? empty, sections.get(TABLES.spans) ?? empty, sections.get(TABLES.lists) ?? empty);
  const limit = r.offsets.length;
  const headWords = sections.get(TABLES.program);
  if (!headWords) throw new Error("malformed HIR: no program section");
  const head = new Cur(headWords, 0);
  const p: Any = {};
  for (const f of program.fields) {
    const tag = tableField(f);
    if (!tag) {
      p[f.name] = dec(r, f.type, head, limit);
      continue;
    }
    if (f.type.t !== "list") continue;
    const ws = sections.get(tag);
    const out = [];
    if (ws) {
      const c = new Cur(ws, 0);
      const n = c.u();
      for (let i = 0; i < n; i++) out.push(dec(r, f.type.elem, c, limit));
    }
    p[f.name] = out;
  }
  // Keep the schema's field order, whatever order the sections came in.
  const ordered: Any = {};
  for (const f of program.fields) ordered[f.name] = p[f.name];
  return ordered;
}

// ---- dump ----

const TABLE_HEADS: Record<string, string> = {
  files: "file",
  strings: "string",
  consts: "const",
  types: "type",
  layouts: "layout",
  descriptors: "descriptor",
  decls: "decl",
  functions: "fn",
  modules: "module",
};

export interface DumpOptions {
  spans?: boolean;
}

function fmtF64(x: number): string {
  if (Number.isNaN(x)) return "NaN";
  if (x === Infinity) return "Infinity";
  if (x === -Infinity) return "-Infinity";
  if (Object.is(x, -0)) return "-0";
  if (Number.isInteger(x) && Math.abs(x) <= 2 ** 53) return String(x);
  return x.toExponential().replace("e+", "e");
}

class Dumper {
  strings: string[];
  spans: boolean;
  out: string[] = [];
  constructor(strings: string[], spans: boolean) {
    this.strings = strings;
    this.spans = spans;
  }

  fmt(t: TypeExpr, v: Any): string {
    switch (t.t) {
      case "prim":
        if (t.name === "f64") return fmtF64(v);
        if (t.name === "string") return JSON.stringify(v);
        return String(v);
      case "list":
        return `[${v.map((x: Any) => this.fmt(t.elem, x)).join(" ")}]`;
      case "nullable":
        return v === null ? "null" : this.fmt(t.inner, v);
      case "named": {
        if (t.name === "StringRef") return v < this.strings.length ? JSON.stringify(this.strings[v]) : `#${v}?`;
        if (t.name === "Span") return `${v.file}:${v.start}-${v.end}`;
        const d = decl(SCHEMA, t.name);
        if (d.d === "ref") return `#${v}`;
        return String(v);
      }
    }
  }

  isDefault(t: TypeExpr, v: Any): boolean {
    if (t.t === "list") return v.length === 0;
    if (t.t === "nullable") return v === null;
    return v === 0 || v === false;
  }

  // One record: its head and simple fields on a line, each complex field on
  // the next lines, its value two columns further in.
  record(ind: number, head: string, fields: Field[], v: Any): void {
    const parts: string[] = [];
    const children: [string, () => void][] = [];
    for (const f of fields) {
      const t = f.type;
      const x = v[f.name];
      if (t.t === "named" && t.name === "Span" && !this.spans) continue;
      if (f.typed && this.isDefault(t, x)) continue;
      if (t.t === "nullable") {
        if (x === null) continue;
        if (simple(SCHEMA, t)) parts.push(`${f.name}:${this.fmt(t.inner, x)}`);
        else children.push([f.name, () => this.value(t.inner, x, ind + 4, "")]);
      } else if (simple(SCHEMA, t)) {
        parts.push(`${f.name}:${this.fmt(t, x)}`);
      } else if (t.t === "list") {
        if (x.length === 0) parts.push(`${f.name}:[]`);
        else children.push([f.name, () => x.forEach((e: Any) => this.value(t.elem, e, ind + 4, ""))]);
      } else {
        children.push([f.name, () => this.value(t, x, ind + 4, "")]);
      }
    }
    this.out.push(" ".repeat(ind) + [head, ...parts].join(" ") + "\n");
    for (const [name, child] of children) {
      this.out.push(" ".repeat(ind + 2) + name + ":\n");
      child();
    }
  }

  value(t: TypeExpr, v: Any, ind: number, head: string): void {
    if (t.t !== "named") throw new Error("complex dump of a non-named type");
    const d = decl(SCHEMA, t.name);
    if (d.d === "struct") {
      this.record(ind, head || d.name, d.fields, v);
    } else if (d.d === "union") {
      const variant = d.variants.find((x) => x.kind === v.kind)!;
      this.record(ind, head ? `${head} ${v.kind}` : v.kind, [...variant.fields, ...d.common], v);
    }
  }
}

export function dump(p: Any, options: DumpOptions = {}): string {
  const d = new Dumper(p.strings, options.spans ?? false);
  d.record(0, "program", program.fields.filter((f) => !tableField(f)), p);
  for (const f of program.fields) {
    if (!tableField(f) || f.type.t !== "list") continue;
    const elem = f.type.elem;
    p[f.name].forEach((x: Any, i: number) => {
      const head = `${TABLE_HEADS[f.name]} #${i}`;
      if (simple(SCHEMA, elem)) d.out.push(`${head} ${d.fmt(elem, x)}\n`);
      else d.value(elem, x, 0, head);
    });
  }
  return d.out.join("");
}

// ---- strip ----

function stripValue(t: TypeExpr, v: Any): void {
  if (v === null) return;
  switch (t.t) {
    case "list":
      for (const x of v) stripValue(t.elem, x);
      return;
    case "nullable":
      stripValue(t.inner, v);
      return;
    case "named": {
      const d = decl(SCHEMA, t.name);
      if (d.d === "struct") stripFields(d.fields, v);
      else if (d.d === "union") stripFields([...d.variants.find((x) => x.kind === v.kind)!.fields, ...d.common], v);
    }
  }
}

function stripFields(fields: Field[], v: Any): void {
  for (const f of fields) {
    if (!f.typed) {
      stripValue(f.type, v[f.name]);
      continue;
    }
    const t = f.type;
    v[f.name] = t.t === "list" ? [] : t.t === "nullable" ? null : t.t === "prim" && t.name === "boolean" ? false : 0;
  }
}

// Resets every piece of type information to its default.
export function strip(p: Any): void {
  stripFields(program.fields, p);
}
