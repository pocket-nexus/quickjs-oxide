// HIR: the front end's whole delivery to the middle end.
//
// This file is the schema. `tools/hir-schema/gen.ts` reads it and generates
// the Rust types, binary reader and writer, text dump and `strip`
// (`src/engine/hir/schema.rs`) and the same reader, writer and dump in
// TypeScript (`tools/hir-schema/generated/hir-io.ts`). Edit this file, then
// run `node tools/hir-schema/gen.ts`; `--check` fails when a generated file
// is stale.
//
// The dialect is a subset of TypeScript type syntax:
//
// - `type XRef = u32` is an index into table `X`, from 0. `TypeRef` is the
//   exception: 0 is `unknown`, and `n` is `types[n - 1]`.
// - `type E = "a" | "b"` is an enumeration. Punctuation members are named by
//   an `@names` tag, one name per member.
// - `type U = | { kind: "a"; ... } | ...`, optionally `( ... ) & { ... }` for
//   fields every variant has, is a tagged union. A union that is the element
//   type of a `Program` table is stored inline in its table; every other
//   union is a node of the node table and carries an `@kinds` tag, the kind
//   number of its first variant. Variants are numbered in order: new
//   variants go at the end.
// - `interface S { ... }` is a record stored inline in its parent.
// - Field types are `u32`, `i32`, `f64`, `boolean`, `string`, a named type,
//   `T[]` and `T | null`.
// - `Typed<T>` marks type information: `strip` resets it to its default (0,
//   `null`, `false` or the empty list).
//   JavaScript source produces only defaults, so for any input
//   `strip(hir(x)) == hir(x)`.
//
// Binary form, little-endian: a 16-byte header (magic "HIR1", schema version
// u32, section count u32, FNV-1a hash of the unit's fingerprint inputs), then
// sections. A section is tag u16, reserved u16, payload byte length u32, 8
// zero bytes, and a payload of u32 words padded to 16 bytes. Every value has
// a fixed word size: a scalar, reference, enumeration or node reference is
// one word; `f64` two (its bits, low word first); `string` and `T[]` two (the
// start and length in the list section; a string is UTF-16 code units, two
// per word); `T | null` one word (0, else the value plus 1) or, for an `f64`
// or record, a presence word and the value's words. A node reference is the
// node's index plus 1. A node is its kind word and its fields' words, in
// schema order, variant fields before shared ones; its `span` goes to the
// span section instead, three words per node, zero for nodes without one.
// Children are written before their parent.
//
// Sections: 1 files, 2 strings, 3 consts, 4 types, 5 layouts,
// 6 descriptors, 7 decls, 8 functions, 9 modules (each a count word and the
// records), 10 nodes, 11 spans, 12 lists, 13 program (version, unit, entry).
// A reader skips sections it does not know and rejects node kinds it does
// not know. The span section may be absent.

export type u32 = number;
export type i32 = number;
export type f64 = number;
export type Typed<T> = T;

export type FileRef = u32;
export type StringRef = u32;
export type ConstRef = u32;
export type TypeRef = u32;
export type LayoutRef = u32;
export type DescriptorRef = u32;
export type DeclRef = u32;
export type FunctionRef = u32;
export type ModuleRef = u32;

export interface Program {
  version: u32;
  unit: Unit;
  entry: Entry;
  files: File[];
  strings: string[];
  consts: Const[];
  types: Typed<Type[]>;
  layouts: Typed<Layout[]>;
  descriptors: Typed<Descriptor[]>;
  decls: Decl[];
  functions: Function[];
  modules: Module[];
}

export interface Unit {
  name: string;
  fingerprintInputs: string[];
}

/** @kinds 1 */
export type Entry =
  | { kind: "script"; top: FunctionRef }
  | { kind: "module"; module: ModuleRef }
  | { kind: "eval"; top: FunctionRef };

export interface File {
  path: string;
  hash: u32;
}

// UTF-8 byte offsets into `file`. A node's `start` is its debug position:
// the token an error at the node is reported at (an operator, the `(` of a
// call, the `.` of a member access), else the node's first token.
export interface Span {
  file: FileRef;
  start: u32;
  end: u32;
}

export type Const =
  | { kind: "number"; value: f64 }
  | { kind: "bigint"; digits: StringRef }
  | { kind: "regexp"; source: StringRef; flags: StringRef };

// ---- types ----

export type NumRepr = "f64" | "f32" | "i32" | "u32" | "i16" | "u16" | "i8" | "u8";

export type Type =
  | { kind: "number"; repr: NumRepr }
  | { kind: "string" }
  | { kind: "boolean" }
  | { kind: "null" }
  | { kind: "undefined" }
  | { kind: "literal"; value: Literal }
  | { kind: "record"; layout: LayoutRef }
  | { kind: "shape"; fields: ShapeField[] }
  | { kind: "array"; elem: TypeRef }
  | { kind: "fn"; params: TypeRef[]; rest: TypeRef | null; ret: TypeRef; async: boolean }
  | { kind: "class"; layout: LayoutRef; proto: TypeRef | null }
  | { kind: "union"; members: TypeRef[]; tag: StringRef | null }
  | { kind: "dynamic" }
  | { kind: "nominal"; base: TypeRef; name: StringRef };

/** @kinds 16 */
export type Literal =
  | { kind: "const"; value: ConstRef }
  | { kind: "str"; value: StringRef }
  | { kind: "bool"; value: boolean };

export interface ShapeField {
  name: StringRef;
  ty: TypeRef;
  optional: boolean;
}

export interface Layout {
  name: StringRef;
  fields: LayoutField[];
}

export interface LayoutField {
  name: StringRef;
  ty: TypeRef;
  readonly: boolean;
}

export type Descriptor =
  | { kind: "number"; repr: NumRepr; min: f64 | null; max: f64 | null }
  | { kind: "string"; maxBytes: u32 | null }
  | { kind: "boolean" }
  | { kind: "null" }
  | { kind: "undefined" }
  | { kind: "literal"; value: Literal }
  | { kind: "optional"; inner: DescriptorRef }
  | { kind: "array"; elem: DescriptorRef; maxLen: u32 | null }
  | { kind: "record"; layout: LayoutRef; fields: DescriptorRef[] }
  | { kind: "tagged"; tag: StringRef; variants: TaggedVariant[] }
  | { kind: "nominal"; base: DescriptorRef; name: StringRef };

export interface TaggedVariant {
  value: StringRef;
  body: DescriptorRef;
}

// ---- declarations ----

// `functionName` is a named function expression's own name, `hidden` a
// binding the front end introduces (a computed field key, a module's
// default export). `this`, `arguments`, `newTarget`, `homeObject` and
// `activeFunction` are a non-arrow function's implicit bindings; an arrow
// refers to its enclosing function's.
export type DeclKind =
  | "var" | "let" | "const" | "param" | "function" | "class" | "catch" | "import" | "global"
  | "this" | "arguments" | "newTarget" | "implicit" | "functionName" | "hidden"
  | "homeObject" | "activeFunction"
  | "privateField" | "privateMethod" | "privateGetter" | "privateSetter" | "privateAccessor";

export interface Decl {
  name: StringRef;
  kind: DeclKind;
  owner: FunctionRef;
  capturedBy: FunctionRef[];
  assigned: boolean;
  tdz: boolean;
  ty: Typed<TypeRef>;
  span: Span;
  // A private name of the class constructor rather than of instances.
  isStatic: boolean;
}

// ---- functions ----

export type FunctionKind =
  | "normal" | "arrow" | "method" | "getter" | "setter" | "constructor" | "derivedConstructor"
  | "classFieldInit" | "staticBlock" | "module" | "script" | "eval";

export interface Function {
  name: StringRef | null;
  kind: FunctionKind;
  role: MethodRole;
  async: boolean;
  generator: boolean;
  strict: boolean;
  params: Param[];
  rest: Pattern | null;
  simpleParams: boolean;
  body: Block;
  decls: DeclRef[];
  captures: DeclRef[];
  inner: FunctionRef[];
  usesThis: boolean;
  usesArguments: boolean;
  usesNewTarget: boolean;
  hasDirectEval: boolean;
  homeObject: boolean;
  sig: Typed<Signature | null>;
  effects: Typed<Effects | null>;
  span: Span;
  // `Function.length`.
  length: u32;
  // The source `Function.prototype.toString` returns.
  source: SourceRange;
  // A named function expression's own binding.
  self: DeclRef | null;
  // The binding the arguments object initializes.
  argumentsDecl: DeclRef | null;
  // With a parameter environment (a default), each parameter's binding there
  // and the body binding it is copied to before the body runs.
  paramCopies: ParamCopy[];
  // An arrow whose body is one expression.
  concise: boolean;
  // A class's synthesized constructor.
  defaultCtor: boolean;
}

export interface SourceRange {
  start: u32;
  end: u32;
}

export interface ParamCopy {
  from: DeclRef;
  to: DeclRef;
}

export interface Param {
  pattern: Pattern;
  default: Expr | null;
}

export interface Signature {
  params: TypeRef[];
  rest: TypeRef | null;
  ret: TypeRef;
  thisTy: TypeRef;
}

export type CallKnowledge = "none" | "known" | "unknown";

export interface Effects {
  reads: DeclRef[];
  writes: DeclRef[];
  heapReads: boolean;
  heapWrites: boolean;
  allocates: boolean;
  mayThrow: boolean;
  maySuspend: boolean;
  calls: CallKnowledge;
}

// ---- modules ----

export interface Module {
  file: FileRef;
  top: FunctionRef;
  requests: ModuleRequest[];
  imports: Import[];
  exports: Export[];
  reexports: Reexport[];
  // `export * from`: request indexes.
  starExports: u32[];
  dynamic: boolean;
  usesImportMeta: boolean;
  hasTopLevelAwait: boolean;
}

export interface ModuleRequest {
  specifier: StringRef;
  // A `with { ... }` clause, possibly empty.
  hasAttributes: boolean;
  attributes: ImportAttribute[];
}

export interface ImportAttribute {
  key: StringRef;
  value: StringRef;
}

// `name` null is a namespace import.
export interface Import {
  request: u32;
  name: StringRef | null;
  decl: DeclRef;
}

export interface Export {
  name: StringRef;
  decl: DeclRef;
}

// `export { name as as } from` (`name` null: `export * as as from`).
export interface Reexport {
  request: u32;
  name: StringRef | null;
  as: StringRef;
}

// ---- expressions ----

/** @names Neg Plus Not BitNot TypeOf Void Delete */
export type UnaryOp = "-" | "+" | "!" | "~" | "typeof" | "void" | "delete";

/** @names Inc Dec */
export type UpdateOp = "++" | "--";

/** @names Add Sub Mul Div Rem Exp Shl Sar Shr BitAnd BitOr BitXor Eq Ne StrictEq StrictNe Lt Le Gt Ge In InstanceOf */
export type BinOp =
  | "+" | "-" | "*" | "/" | "%" | "**" | "<<" | ">>" | ">>>" | "&" | "|" | "^"
  | "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" | "in" | "instanceof";

/** @names And Or Nullish */
export type LogicalOp = "&&" | "||" | "??";

/** @names Assign Add Sub Mul Div Rem Exp Shl Sar Shr BitAnd BitOr BitXor And Or Nullish */
export type AssignOp =
  | "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "**=" | "<<=" | ">>=" | ">>>=" | "&=" | "|=" | "^="
  | "&&=" | "||=" | "??=";

/** @kinds 256 */
export type Expr = (
  | { kind: "const"; value: ConstRef }
  | { kind: "str"; value: StringRef }
  | { kind: "bool"; value: boolean }
  | { kind: "null" }
  | { kind: "undefined" }
  | { kind: "template"; quasis: StringRef[]; exprs: Expr[] }
  | { kind: "taggedTemplate"; tag: Expr; site: u32; cooked: (StringRef | null)[]; raw: StringRef[]; exprs: Expr[] }
  | { kind: "array"; items: ArrayItem[] }
  | { kind: "object"; props: ObjProp[] }
  | { kind: "fn"; fn: FunctionRef }
  | { kind: "class"; cls: Class }
  | { kind: "decl"; decl: DeclRef }
  | { kind: "this"; decl: DeclRef }
  | { kind: "newTarget"; decl: DeclRef }
  // `decl`: the module's `import.meta` binding, an implicit declaration of
  // the module function.
  | { kind: "importMeta"; decl: DeclRef }
  | { kind: "superMember"; prop: PropKey; this: DeclRef; home: DeclRef }
  | { kind: "member"; obj: Expr; prop: PropKey; optional: boolean; layout: Typed<LayoutRef | null>; field: Typed<u32 | null> }
  | { kind: "unary"; op: UnaryOp; e: Expr }
  | { kind: "update"; op: UpdateOp; prefix: boolean; target: Target }
  | { kind: "binary"; op: BinOp; l: Expr; r: Expr; numeric: Typed<NumRepr | null> }
  | { kind: "logical"; op: LogicalOp; l: Expr; r: Expr }
  | { kind: "cond"; test: Expr; then: Expr; else: Expr }
  | { kind: "assign"; op: AssignOp; target: Target; value: Expr }
  | { kind: "seq"; exprs: Expr[] }
  | { kind: "call"; callee: Expr; args: Arg[]; optional: boolean; direct: Typed<FunctionRef | null>; intrinsic: Typed<StringRef | null>; directEval: boolean }
  | { kind: "new"; callee: Expr; args: Arg[]; direct: Typed<FunctionRef | null> }
  | { kind: "superCall"; args: Arg[]; this: DeclRef; newTarget: DeclRef; func: DeclRef }
  | { kind: "import"; spec: Expr; options: Expr | null }
  | { kind: "yield"; e: Expr | null; delegate: boolean }
  | { kind: "await"; e: Expr }
  | { kind: "decode"; desc: DescriptorRef; e: Expr }
  | { kind: "scratch"; elem: TypeRef; body: FunctionRef }
  | { kind: "optionalChain"; e: Expr }
  | { kind: "privateIn"; name: DeclRef; obj: Expr }
  // NamedEvaluation: an anonymous function or class value named `name`.
  | { kind: "named"; name: StringRef; e: Expr }
) & { ty: Typed<TypeRef>; span: Span };

/** @kinds 512 */
export type PropKey =
  | { kind: "name"; name: StringRef }
  | { kind: "computed"; e: Expr }
  | { kind: "private"; name: DeclRef };

/** @kinds 528 */
export type Arg =
  | { kind: "arg"; e: Expr }
  | { kind: "spread"; e: Expr };

/** @kinds 544 */
export type ArrayItem =
  | { kind: "item"; e: Expr }
  | { kind: "spread"; e: Expr }
  | { kind: "hole" };

// A target's `start` is the identifier, or the member's `.` or `[`.
/** @kinds 560 */
export type Target = (
  | { kind: "decl"; decl: DeclRef }
  | { kind: "member"; obj: Expr; prop: PropKey; layout: Typed<LayoutRef | null>; field: Typed<u32 | null> }
  | { kind: "pattern"; p: Pattern }
  | { kind: "superMember"; prop: PropKey; this: DeclRef; home: DeclRef }
) & { span: Span };

export type MethodRole = "method" | "get" | "set";

/** @kinds 576 */
export type ObjProp =
  | { kind: "init"; key: PropKey; value: Expr; shorthand: boolean }
  | { kind: "method"; key: PropKey; fn: FunctionRef; role: MethodRole }
  | { kind: "spread"; e: Expr }
  | { kind: "proto"; value: Expr };

// A class: its inner name binding, heritage, constructor (written or
// synthesized), the elements evaluated with it in order, and QuickJS's two
// aggregate initializers, one for instance fields and one for static
// fields and static blocks.
export interface Class {
  name: DeclRef | null;
  extends: Expr | null;
  ctor: FunctionRef;
  members: ClassMember[];
  instanceInit: FunctionRef | null;
  staticInit: FunctionRef | null;
  layout: Typed<LayoutRef | null>;
  span: Span;
}

/** @kinds 592 */
export type ClassMember =
  | { kind: "method"; key: PropKey; fn: FunctionRef; static: boolean; role: MethodRole }
  | { kind: "privateMethod"; decl: DeclRef; fn: FunctionRef; static: boolean; role: MethodRole }
  // A computed field key, evaluated here and kept in `decl`.
  | { kind: "fieldKey"; key: Expr; decl: DeclRef; static: boolean }
  // A private field name, created with the class.
  | { kind: "privateField"; decl: DeclRef; static: boolean };

/** @kinds 608 */
export type FieldKey =
  | { kind: "name"; name: StringRef }
  | { kind: "computed"; decl: DeclRef };

// ---- statements ----

export type DeclaringKind = "var" | "let" | "const";

/** @kinds 768 */
export type Stmt = (
  | { kind: "expr"; e: Expr }
  | { kind: "declare"; decl: DeclRef; init: Expr | null }
  | { kind: "destructure"; pattern: Pattern; init: Expr; declaring: DeclaringKind | null }
  | { kind: "block"; body: Block }
  | { kind: "if"; test: Expr; then: Stmt; else: Stmt | null }
  | { kind: "while"; test: Expr; body: Stmt; label: u32 }
  | { kind: "doWhile"; body: Stmt; test: Expr; label: u32 }
  | { kind: "for"; init: Stmt | null; test: Expr | null; update: Expr | null; body: Stmt; label: u32; perIteration: DeclRef[] }
  // `init`: Annex B `for (var x = init in o)`.
  | { kind: "forIn"; target: ForTarget; init: Expr | null; obj: Expr; body: Stmt; label: u32 }
  | { kind: "forOf"; target: ForTarget; iter: Expr; body: Stmt; label: u32; await: boolean; arrayFast: Typed<boolean> }
  | { kind: "switch"; disc: Expr; cases: SwitchCase[]; label: u32; exhaustive: Typed<boolean> }
  | { kind: "break"; label: u32 }
  | { kind: "continue"; label: u32 }
  | { kind: "return"; e: Expr | null }
  | { kind: "throw"; e: Expr }
  // `catchDefault`: QuickJS lets a catch pattern have a default.
  | { kind: "try"; body: Block; catchParam: Pattern | null; catchBody: Block | null; finally: Block | null; catchDefault: Expr | null }
  | { kind: "labeled"; label: u32; body: Stmt }
  // `annexB`: the function-root var a sloppy block function also sets.
  | { kind: "fnDecl"; decl: DeclRef; fn: FunctionRef; annexB: DeclRef | null }
  | { kind: "classDecl"; decl: DeclRef; cls: Class }
  | { kind: "debugger" }
  | { kind: "empty" }
  // In an aggregate field initializer: a field of `this`, or a static block.
  | { kind: "defineField"; key: FieldKey; value: Expr | null }
  | { kind: "definePrivate"; decl: DeclRef; value: Expr | null }
  | { kind: "staticBlock"; fn: FunctionRef }
) & { span: Span };

export interface Block {
  stmts: Stmt[];
  decls: DeclRef[];
}

export interface SwitchCase {
  test: Expr | null;
  body: Stmt[];
}

/** @kinds 800 */
export type ForTarget =
  | { kind: "assign"; t: Target }
  | { kind: "declare"; pattern: Pattern };

// ---- patterns ----

/** @kinds 1024 */
export type Pattern = (
  | { kind: "decl"; decl: DeclRef }
  | { kind: "target"; t: Target }
  // `restAt`: the rest's `...`.
  | { kind: "object"; props: PatternProp[]; rest: Pattern | null; restAt: u32 }
  | { kind: "array"; items: PatternItem[]; rest: Pattern | null }
) & { ty: Typed<TypeRef>; span: Span };

// `keyAt`: the property name's first token.
export interface PatternProp {
  key: PropKey;
  value: Pattern;
  default: Expr | null;
  keyAt: u32;
}

/** @kinds 1040 */
export type PatternItem =
  | { kind: "item"; p: Pattern; default: Expr | null }
  | { kind: "hole" };
