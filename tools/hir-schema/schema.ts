// Reads `contracts/spec/hir.ts` into the model both emitters use: the dialect
// and the binary layout it fixes are described at the top of that file.

export type Prim = "u32" | "i32" | "f64" | "boolean" | "string";

export type TypeExpr =
  | { t: "prim"; name: Prim }
  | { t: "named"; name: string }
  | { t: "list"; elem: TypeExpr }
  | { t: "nullable"; inner: TypeExpr };

export interface Field {
  name: string;
  type: TypeExpr;
  typed: boolean;
}

export interface Variant {
  kind: string;
  fields: Field[];
}

export type Decl =
  | { d: "ref"; name: string }
  | { d: "enum"; name: string; members: string[]; names: string[] }
  | { d: "union"; name: string; variants: Variant[]; common: Field[]; kinds: number | null }
  | { d: "struct"; name: string; fields: Field[] };

export interface Schema {
  decls: Map<string, Decl>;
  order: string[];
  // Unions stored inline as `Program` table records, not as nodes.
  inline: Set<string>;
}

const PRIMS = new Set(["u32", "i32", "f64", "boolean", "string"]);
const SKIPPED = new Set(["u32", "i32", "f64", "Typed"]);

interface Token {
  k: "id" | "str" | "num" | "p" | "doc";
  v: string;
}

function tokenize(src: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) {
      i++;
    } else if (src.startsWith("/**", i)) {
      const end = src.indexOf("*/", i);
      out.push({ k: "doc", v: src.slice(i + 3, end) });
      i = end + 2;
    } else if (src.startsWith("//", i)) {
      const end = src.indexOf("\n", i);
      i = end < 0 ? src.length : end;
    } else if (src.startsWith("/*", i)) {
      i = src.indexOf("*/", i) + 2;
    } else if (c === '"') {
      const end = src.indexOf('"', i + 1);
      out.push({ k: "str", v: src.slice(i + 1, end) });
      i = end + 1;
    } else if (/[A-Za-z_$]/.test(c)) {
      let j = i;
      while (j < src.length && /[A-Za-z0-9_$]/.test(src[j])) j++;
      out.push({ k: "id", v: src.slice(i, j) });
      i = j;
    } else if (/[0-9]/.test(c)) {
      let j = i;
      while (j < src.length && /[0-9A-Fa-fx]/.test(src[j])) j++;
      out.push({ k: "num", v: src.slice(i, j) });
      i = j;
    } else {
      out.push({ k: "p", v: c });
      i++;
    }
  }
  return out;
}

type Raw =
  | { r: "prim"; name: Prim }
  | { r: "named"; name: string; arg: Raw | null }
  | { r: "null" }
  | { r: "str"; v: string }
  | { r: "obj"; fields: RawField[] }
  | { r: "list"; elem: Raw }
  | { r: "union"; members: Raw[] }
  | { r: "and"; left: Raw; right: Raw };

interface RawField {
  name: string;
  type: Raw;
}

class Parser {
  toks: Token[];
  i = 0;
  constructor(toks: Token[]) {
    this.toks = toks;
  }
  peek(): Token | undefined {
    return this.toks[this.i];
  }
  next(): Token {
    const t = this.toks[this.i++];
    if (!t) throw new Error("unexpected end of schema");
    return t;
  }
  is(v: string): boolean {
    const t = this.peek();
    return t !== undefined && (t.k === "p" || t.k === "id") && t.v === v;
  }
  expect(v: string): void {
    const t = this.next();
    if (t.v !== v) throw new Error(`schema: expected '${v}', found '${t.v}'`);
  }
  id(): string {
    const t = this.next();
    if (t.k !== "id") throw new Error(`schema: expected a name, found '${t.v}'`);
    return t.v;
  }

  type(): Raw {
    const left = this.union();
    if (this.is("&")) {
      this.next();
      return { r: "and", left, right: this.union() };
    }
    return left;
  }
  union(): Raw {
    if (this.is("|")) this.next();
    const members = [this.postfix()];
    while (this.is("|")) {
      this.next();
      members.push(this.postfix());
    }
    return members.length === 1 ? members[0] : { r: "union", members };
  }
  postfix(): Raw {
    let t = this.primary();
    while (this.is("[")) {
      this.next();
      this.expect("]");
      t = { r: "list", elem: t };
    }
    return t;
  }
  primary(): Raw {
    const t = this.next();
    if (t.k === "p" && t.v === "(") {
      const inner = this.type();
      this.expect(")");
      return inner;
    }
    if (t.k === "p" && t.v === "{") {
      const fields: RawField[] = [];
      while (!this.is("}")) {
        const name = this.id();
        this.expect(":");
        fields.push({ name, type: this.type() });
        if (this.is(";") || this.is(",")) this.next();
      }
      this.next();
      return { r: "obj", fields };
    }
    if (t.k === "str") return { r: "str", v: t.v };
    if (t.k === "id") {
      if (t.v === "null") return { r: "null" };
      if (PRIMS.has(t.v)) return { r: "prim", name: t.v as Prim };
      let arg: Raw | null = null;
      if (this.is("<")) {
        this.next();
        arg = this.type();
        this.expect(">");
      }
      return { r: "named", name: t.v, arg };
    }
    throw new Error(`schema: unexpected '${t.v}'`);
  }
}

function tag(doc: string | null, name: string): string | null {
  if (!doc) return null;
  const m = new RegExp(`@${name}\\s+([^@*]*)`).exec(doc);
  return m ? m[1].trim() : null;
}

function field(raw: RawField, where: string): Field {
  let t = raw.type;
  let typed = false;
  if (t.r === "named" && t.name === "Typed") {
    if (!t.arg) throw new Error(`schema: Typed without an argument in ${where}`);
    typed = true;
    t = t.arg;
  }
  return { name: raw.name, type: typeExpr(t, `${where}.${raw.name}`), typed };
}

function typeExpr(t: Raw, where: string): TypeExpr {
  switch (t.r) {
    case "prim":
      return { t: "prim", name: t.name };
    case "named":
      if (t.arg) throw new Error(`schema: generic type in ${where}`);
      return { t: "named", name: t.name };
    case "list":
      return { t: "list", elem: typeExpr(t.elem, where) };
    case "union": {
      const rest = t.members.filter((m) => m.r !== "null");
      if (rest.length === 1 && t.members.length === 2) {
        return { t: "nullable", inner: typeExpr(rest[0], where) };
      }
      throw new Error(`schema: an inline union in ${where} needs a name`);
    }
    default:
      throw new Error(`schema: an inline ${t.r} type in ${where} needs a name`);
  }
}

function variants(t: Raw, name: string): Variant[] {
  const members = t.r === "union" ? t.members : [t];
  return members.map((m) => {
    if (m.r !== "obj") throw new Error(`schema: union ${name} has a member that is not an object`);
    const [k, ...rest] = m.fields;
    if (!k || k.name !== "kind" || k.type.r !== "str") {
      throw new Error(`schema: union ${name} has a member without a leading kind`);
    }
    return { kind: k.type.v, fields: rest.map((f) => field(f, `${name}.${k.type.r === "str" ? k.type.v : ""}`)) };
  });
}

export function parseSchema(src: string): Schema {
  const p = new Parser(tokenize(src));
  const decls = new Map<string, Decl>();
  const order: string[] = [];
  let doc: string | null = null;
  while (p.peek()) {
    const t = p.next();
    if (t.k === "doc") {
      doc = t.v;
      continue;
    }
    if (t.v === "export") continue;
    let decl: Decl | null = null;
    if (t.v === "interface") {
      const name = p.id();
      const body = p.primary();
      if (body.r !== "obj") throw new Error(`schema: interface ${name} has no body`);
      decl = { d: "struct", name, fields: body.fields.map((f) => field(f, name)) };
    } else if (t.v === "type") {
      const name = p.id();
      if (p.is("<")) {
        while (!p.is(">")) p.next();
        p.next();
      }
      p.expect("=");
      const body = p.type();
      if (p.is(";")) p.next();
      if (SKIPPED.has(name)) {
        doc = null;
        continue;
      }
      if (body.r === "prim" && body.name === "u32") {
        decl = { d: "ref", name };
      } else if (body.r === "union" && body.members.every((m) => m.r === "str")) {
        const members = body.members.map((m) => (m.r === "str" ? m.v : ""));
        const given = tag(doc, "names");
        const names = given ? given.split(/\s+/) : members.map(pascal);
        if (names.length !== members.length) throw new Error(`schema: @names of ${name} does not match its members`);
        decl = { d: "enum", name, members, names };
      } else {
        let common: Field[] = [];
        let core = body;
        if (body.r === "and") {
          if (body.right.r !== "obj") throw new Error(`schema: ${name}'s shared fields are not an object`);
          common = body.right.fields.map((f) => field(f, name));
          core = body.left;
        }
        const kinds = tag(doc, "kinds");
        decl = { d: "union", name, variants: variants(core, name), common, kinds: kinds === null ? null : Number(kinds) };
      }
    } else {
      throw new Error(`schema: unexpected '${t.v}' at the top level`);
    }
    doc = null;
    decls.set(decl.name, decl);
    order.push(decl.name);
  }

  const program = decls.get("Program");
  if (!program || program.d !== "struct") throw new Error("schema: no Program interface");
  const inline = new Set<string>();
  for (const f of program.fields) {
    if (f.type.t === "list" && f.type.elem.t === "named") {
      const d = decls.get(f.type.elem.name);
      if (d && d.d === "union") inline.add(d.name);
    }
  }
  const schema: Schema = { decls, order, inline };
  check(schema);
  return schema;
}

function check(s: Schema): void {
  const used = new Map<number, string>();
  for (const name of s.order) {
    const d = s.decls.get(name)!;
    if (d.d === "union" && !s.inline.has(name)) {
      if (d.kinds === null) throw new Error(`schema: node union ${name} has no @kinds`);
      d.variants.forEach((v, i) => {
        const k = d.kinds! + i;
        if (used.has(k)) throw new Error(`schema: kind ${k} of ${name} is also ${used.get(k)}'s`);
        used.set(k, name);
      });
    }
    const fields = d.d === "struct" ? d.fields : d.d === "union" ? [...d.variants.flatMap((v) => v.fields), ...d.common] : [];
    for (const f of fields) words(s, f.type);
  }
}

export function pascal(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export function snake(s: string): string {
  return s.replace(/[A-Z]/g, (c) => "_" + c.toLowerCase());
}

export function decl(s: Schema, name: string): Decl {
  const d = s.decls.get(name);
  if (!d) throw new Error(`schema: unknown type ${name}`);
  return d;
}

export function isNode(s: Schema, t: TypeExpr): boolean {
  if (t.t !== "named") return false;
  const d = decl(s, t.name);
  return d.d === "union" && !s.inline.has(d.name);
}

// The fixed word size of a value of type `t`.
export function words(s: Schema, t: TypeExpr): number {
  switch (t.t) {
    case "prim":
      return t.name === "f64" || t.name === "string" ? 2 : 1;
    case "list":
      return 2;
    case "nullable": {
      const inner = t.inner;
      if (inner.t === "list" || (inner.t === "prim" && inner.name === "string")) {
        throw new Error("schema: nullable lists and strings are not supported");
      }
      if (inner.t === "prim" && inner.name === "f64") return 3;
      if (inner.t === "named" && decl(s, inner.name).d === "struct") return 1 + words(s, inner);
      return 1;
    }
    case "named": {
      const d = decl(s, t.name);
      if (d.d === "struct") return d.fields.reduce((n, f) => n + words(s, f.type), 0);
      if (d.d === "union" && s.inline.has(d.name)) throw new Error(`schema: ${d.name} is a table record and cannot be nested`);
      return 1;
    }
  }
}

// Whether the dump prints a value of this type on its parent's line.
export function simple(s: Schema, t: TypeExpr): boolean {
  switch (t.t) {
    case "prim":
      return true;
    case "list":
      return simple(s, t.elem);
    case "nullable":
      return simple(s, t.inner);
    case "named": {
      const d = decl(s, t.name);
      return d.d === "ref" || d.d === "enum" || d.name === "Span";
    }
  }
}

// The node span: a node union's shared `span` field goes to the span section.
export function nodeSpan(s: Schema, d: Decl): boolean {
  return d.d === "union" && !s.inline.has(d.name) && d.common.some((f) => f.name === "span" && f.type.t === "named" && f.type.name === "Span");
}

export function variantFields(s: Schema, d: Decl & { d: "union" }, v: Variant): Field[] {
  const common = nodeSpan(s, d) ? d.common.filter((f) => f.name !== "span") : d.common;
  return [...v.fields, ...common];
}

export function hasNodeSpan(s: Schema, d: Decl & { d: "union" }): boolean {
  return nodeSpan(s, d);
}

export function schemaVersion(src: string): number {
  const m = /@schemaVersion\s+(\d+)/.exec(src);
  return m ? Number(m[1]) : 1;
}
