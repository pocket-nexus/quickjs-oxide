//! The syntax tree the parser produces, and the scopes and declarations it
//! records while parsing.
//!
//! The tree keeps the source's shape: parentheses, optional chains, patterns
//! and function bodies stay where they were written. Every identifier
//! reference carries the scope it occurs in, and every declaration is
//! registered in the scope arena with the parser's early-error checks, so the
//! HIR builder resolves names without searching the tree again. Positions are
//! UTF-8 byte offsets of the token an operation reports errors at.
// Consumed by the HIR builder, which is not wired to the compile host yet.
#![allow(dead_code)]

pub(crate) mod parser;

use crate::engine::compiler::lexer::Span;
use crate::engine::compiler::names::{NameId, NameTable};
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;
use std::rc::Rc;

/// A byte offset into the source.
pub(crate) type Pos = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ScopeId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DeclId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FuncId(pub(crate) u32);

/// A breakable statement's identity: `break` and `continue` name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LabelId(pub(crate) u32);

impl ScopeId {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl DeclId {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl FuncId {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

// ---- programs ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProgramKind {
    Script,
    Module,
    /// Indirect eval code: a script whose completion value is the result.
    Eval,
}

#[derive(Debug)]
pub(crate) struct Program {
    pub(crate) kind: ProgramKind,
    /// The root function: kind `Script`, `Module` or `Eval`.
    pub(crate) root: Box<Function>,
    pub(crate) names: NameTable,
    pub(crate) scopes: Vec<Scope>,
    pub(crate) decls: Vec<Decl>,
    pub(crate) functions: Vec<FunctionInfo>,
    pub(crate) module: Option<ModuleRecord>,
    /// A `with` statement: parsed for its diagnostics, then rejected.
    pub(crate) has_with: bool,
}

impl Program {
    pub(crate) fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.index()]
    }

    pub(crate) fn decl(&self, id: DeclId) -> &Decl {
        &self.decls[id.index()]
    }

    pub(crate) fn function(&self, id: FuncId) -> &FunctionInfo {
        &self.functions[id.index()]
    }
}

// ---- scopes and declarations ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScopeKind {
    /// A function's var scope: parameters, `var`s and top-level functions.
    FunctionRoot,
    /// The separate parameter environment of a function with a default.
    Parameter,
    FunctionBody,
    ProgramBody,
    Block,
    ClassPrivate,
    If,
    For,
    Switch,
    Catch,
    With,
    /// A named function expression's own name: consulted after the
    /// function's scopes, before its parent's.
    FunctionName,
}

#[derive(Debug)]
pub(crate) struct Scope {
    pub(crate) parent: Option<ScopeId>,
    pub(crate) kind: ScopeKind,
    pub(crate) function: FuncId,
    /// In declaration order.
    pub(crate) decls: Vec<DeclId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclKind {
    /// A `var`, or a function declared at a function's or script's top level.
    Var,
    Let,
    Const,
    /// A class declaration's outer binding.
    Class,
    /// A formal parameter.
    Param,
    /// A `catch (x)` parameter, which a `var x` may redeclare.
    CatchParam,
    /// A function declared in a block (lexical; maybe with an Annex B var).
    ScopedFunction {
        generator_or_async: bool,
    },
    /// A named function expression's own name, inside it.
    FunctionName,
    /// A class's own name, inside the class.
    ClassName,
    /// A module import binding.
    Import,
    /// A hidden binding the parser creates (computed field keys, `*default*`).
    Hidden {
        is_const: bool,
    },
    PrivateField {
        is_static: bool,
    },
    PrivateMethod {
        is_static: bool,
    },
    PrivateGetter {
        is_static: bool,
    },
    PrivateSetter {
        is_static: bool,
    },
    PrivateGetterSetter {
        is_static: bool,
    },
}

impl DeclKind {
    pub(crate) const fn is_lexical(self) -> bool {
        matches!(
            self,
            Self::Let
                | Self::Const
                | Self::Class
                | Self::CatchParam
                | Self::ScopedFunction { .. }
                | Self::ClassName
                | Self::Import
                | Self::Hidden { .. }
        )
    }

    pub(crate) const fn is_private(self) -> bool {
        matches!(
            self,
            Self::PrivateField { .. }
                | Self::PrivateMethod { .. }
                | Self::PrivateGetter { .. }
                | Self::PrivateSetter { .. }
                | Self::PrivateGetterSetter { .. }
        )
    }
}

/// Where a declaration's value lives, as QuickJS decides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Storage {
    /// A frame local (or cell, when captured).
    Local,
    /// A formal parameter's argument slot.
    Argument,
    /// A property of the global object, or a global lexical binding.
    Global,
    /// A module binding.
    Module,
}

#[derive(Debug)]
pub(crate) struct Decl {
    pub(crate) name: NameId,
    pub(crate) kind: DeclKind,
    pub(crate) storage: Storage,
    pub(crate) scope: ScopeId,
    /// The scope the declaration was written in (a `var` in a block lives in
    /// the function root but was written in the block).
    pub(crate) written_in: ScopeId,
    pub(crate) span: Span,
}

/// What a function knows about itself after parsing.
#[derive(Debug)]
pub(crate) struct FunctionInfo {
    pub(crate) parent: Option<FuncId>,
    pub(crate) kind: FunctionKind,
    pub(crate) var_scope: ScopeId,
    pub(crate) body_scope: ScopeId,
    pub(crate) parameter_scope: Option<ScopeId>,
    /// The scope of the parent that contains the function's definition.
    pub(crate) definition_scope: Option<ScopeId>,
    pub(crate) strict: bool,
}

// ---- functions ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FunctionKind {
    Script,
    Module,
    Eval,
    Normal,
    Arrow,
    /// An object-literal or class method (also an accessor, by `role`).
    Method,
    ClassConstructor {
        derived: bool,
    },
    /// The aggregate instance-field or static-element initializer.
    ClassFieldInit {
        is_static: bool,
    },
    StaticBlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MethodRole {
    Method,
    Get,
    Set,
}

#[derive(Debug)]
pub(crate) struct Function {
    pub(crate) id: FuncId,
    pub(crate) kind: FunctionKind,
    pub(crate) role: MethodRole,
    pub(crate) is_async: bool,
    pub(crate) is_generator: bool,
    /// The name written after `function` or `class`.
    pub(crate) name: Option<Ident>,
    /// The binding of that name inside a named function expression.
    pub(crate) self_binding: Option<DeclId>,
    pub(crate) params: Vec<Param>,
    pub(crate) rest: Option<Pattern>,
    pub(crate) simple_params: bool,
    /// The number of formals before the first default or rest (`length`).
    pub(crate) length: u32,
    pub(crate) body: Vec<Stmt>,
    /// An arrow whose body is one expression (`body` is its `return`).
    pub(crate) concise: bool,
    /// A class's synthesized constructor.
    pub(crate) default_ctor: bool,
    pub(crate) strict: bool,
    /// The token that starts the definition (its debug position).
    pub(crate) span: Span,
    /// The source text `Function.prototype.toString` returns.
    pub(crate) source: (Pos, Pos),
}

#[derive(Debug)]
pub(crate) struct Param {
    pub(crate) pattern: Pattern,
    pub(crate) default: Option<Expr>,
}

// ---- references ----

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ident {
    pub(crate) name: NameId,
    pub(crate) span: Span,
}

/// An identifier use: resolved against `scope` and its ancestors.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ref {
    pub(crate) name: NameId,
    pub(crate) span: Span,
    pub(crate) scope: ScopeId,
}

#[derive(Debug)]
pub(crate) enum PropKey {
    /// A fixed key: an identifier name, string or number literal.
    Name(JsString),
    Computed(Box<Expr>),
    Private(Ref),
}

// ---- expressions ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnaryOp {
    Neg,
    Plus,
    Not,
    BitNot,
    TypeOf,
    Void,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Exp,
    Shl,
    Sar,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    InstanceOf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogicalOp {
    And,
    Or,
    Nullish,
}

/// `=`, a compound operator, or a logical assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssignOp {
    Assign,
    Binary(BinaryOp),
    Logical(LogicalOp),
}

#[derive(Debug)]
pub(crate) enum ArrayItem {
    Item(Expr),
    Spread(Expr, Pos),
    Hole,
}

#[derive(Debug)]
pub(crate) enum Arg {
    Arg(Expr),
    Spread(Expr),
}

#[derive(Debug)]
pub(crate) enum ObjProp {
    Init {
        key: PropKey,
        value: Expr,
        key_pos: Pos,
    },
    Shorthand {
        key: JsString,
        value: Ref,
    },
    Method {
        key: PropKey,
        function: Box<Function>,
    },
    Spread {
        value: Expr,
        pos: Pos,
    },
    Proto {
        value: Expr,
        pos: Pos,
    },
}

#[derive(Debug)]
pub(crate) struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum ExprKind {
    Number(f64),
    BigInt(Box<JsBigInt>),
    String(JsString),
    Template {
        quasis: Vec<JsString>,
        exprs: Vec<Expr>,
    },
    TaggedTemplate {
        tag: Box<Expr>,
        cooked: Vec<Option<JsString>>,
        raw: Vec<JsString>,
        exprs: Vec<Expr>,
        /// The first template token.
        pos: Pos,
    },
    RegExp(Box<RegExpLiteral>),
    Null,
    Bool(bool),
    This,
    Ident(Ref),
    Array(Vec<ArrayItem>),
    Object(Vec<ObjProp>),
    Function(Box<Function>),
    Class(Box<Class>),
    Member {
        obj: Box<Expr>,
        prop: PropKey,
        /// `?.` before this access.
        optional: bool,
        /// The `.` or `[`.
        pos: Pos,
    },
    /// `super.x` or `super[x]`.
    SuperMember {
        prop: PropKey,
        pos: Pos,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        optional: bool,
        /// A call written `eval(...)`: a direct eval, which runs as indirect.
        direct_eval: bool,
        /// The `(`.
        pos: Pos,
    },
    New {
        callee: Box<Expr>,
        args: Vec<Arg>,
        /// The `(`, or the token after the callee without arguments.
        pos: Pos,
    },
    SuperCall {
        args: Vec<Arg>,
        pos: Pos,
    },
    /// `import(spec, options)`.
    Import {
        spec: Box<Expr>,
        options: Option<Box<Expr>>,
        pos: Pos,
    },
    ImportMeta,
    NewTarget,
    /// The extent of an optional chain: a `?.` inside short-circuits to here.
    OptionalChain(Box<Expr>),
    Paren(Box<Expr>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        pos: Pos,
    },
    Update {
        increment: bool,
        prefix: bool,
        target: Box<Expr>,
        pos: Pos,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        pos: Pos,
    },
    Logical {
        op: LogicalOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Conditional {
        test: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    /// An assignment to a reference (identifier, member or super member).
    Assign {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
        pos: Pos,
    },
    /// A destructuring assignment `[a, b] = value`.
    AssignPattern {
        pattern: Box<Pattern>,
        value: Box<Expr>,
    },
    Sequence(Vec<Expr>),
    Yield {
        arg: Option<Box<Expr>>,
        delegate: bool,
        pos: Pos,
    },
    Await {
        arg: Box<Expr>,
        pos: Pos,
    },
    /// `#x in obj`.
    PrivateIn {
        name: Ref,
        obj: Box<Expr>,
        pos: Pos,
    },
}

#[derive(Debug)]
pub(crate) struct RegExpLiteral {
    pub(crate) pattern: JsString,
    pub(crate) flags: JsString,
    pub(crate) program: Rc<oxide_regexp::CompiledRegExp>,
}

// ---- classes ----

#[derive(Debug)]
pub(crate) struct Class {
    pub(crate) name: Option<Ident>,
    /// The inner, immutable binding of the class's own name.
    pub(crate) name_binding: Option<DeclId>,
    pub(crate) heritage: Option<Box<Expr>>,
    /// The scope holding the inner name; the heritage is evaluated in it.
    pub(crate) scope: ScopeId,
    /// The scope of the private names, around the body.
    pub(crate) private_scope: ScopeId,
    /// Explicit, or synthesized when the body has none.
    pub(crate) constructor: Box<Function>,
    pub(crate) members: Vec<ClassMember>,
    pub(crate) instance_init: Option<Box<Function>>,
    pub(crate) static_init: Option<Box<Function>>,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum ClassMember {
    Method {
        key: PropKey,
        function: Box<Function>,
        is_static: bool,
    },
    /// A private method or accessor: installed on the class, not defined.
    PrivateMethod {
        decl: DeclId,
        function: Box<Function>,
        is_static: bool,
    },
    /// A computed field key, evaluated with the methods and stored in
    /// `decl` for the initializer.
    FieldKey {
        key: Box<Expr>,
        decl: DeclId,
        is_static: bool,
        pos: Pos,
    },
    /// A private field's name: created with the class.
    PrivateField { decl: DeclId, is_static: bool },
}

/// One element of an aggregate field initializer's body.
#[derive(Debug)]
pub(crate) enum FieldInit {
    Public {
        key: FieldKey,
        value: Option<Expr>,
        span: Span,
    },
    Private {
        decl: DeclId,
        value: Option<Expr>,
        span: Span,
    },
    StaticBlock(Box<Function>),
}

#[derive(Debug)]
pub(crate) enum FieldKey {
    Name(JsString),
    /// The hidden binding holding the computed key.
    Computed(DeclId),
}

// ---- statements ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VarKind {
    Var,
    Let,
    Const,
}

#[derive(Debug)]
pub(crate) struct VarDeclarator {
    pub(crate) target: Pattern,
    pub(crate) init: Option<Expr>,
    /// The `=`, or the name when there is no initializer.
    pub(crate) pos: Pos,
}

#[derive(Debug)]
pub(crate) enum ForInit {
    Var(VarKind, Vec<VarDeclarator>),
    Expr(Expr),
}

/// What a for-in/of head assigns each value to.
#[derive(Debug)]
pub(crate) enum ForTarget {
    Declare(VarKind, Pattern),
    /// An assignment target expression.
    Assign(Expr),
    /// An assignment pattern.
    Pattern(Pattern),
}

#[derive(Debug)]
pub(crate) struct SwitchCase {
    pub(crate) test: Option<Expr>,
    pub(crate) body: Vec<Stmt>,
}

#[derive(Debug)]
pub(crate) struct Catch {
    pub(crate) param: Option<Pattern>,
    /// QuickJS lets a catch pattern have a default.
    pub(crate) param_default: Option<Expr>,
    /// The catch parameter's scope.
    pub(crate) scope: ScopeId,
    pub(crate) body: Vec<Stmt>,
    /// The body's own block scope.
    pub(crate) body_scope: Option<ScopeId>,
}

#[derive(Debug)]
pub(crate) struct Stmt {
    pub(crate) kind: StmtKind,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum StmtKind {
    Expr(Expr),
    Var(VarKind, Vec<VarDeclarator>),
    /// A function declaration; `decl` is its binding.
    Function {
        function: Box<Function>,
        decl: DeclId,
        /// Annex B: the function-root var that also receives it.
        annex_b: Option<DeclId>,
    },
    Class {
        class: Box<Class>,
        decl: DeclId,
    },
    Block {
        body: Vec<Stmt>,
        scope: Option<ScopeId>,
    },
    If {
        test: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
        scope: ScopeId,
    },
    While {
        test: Expr,
        body: Box<Stmt>,
        label: LabelId,
    },
    DoWhile {
        body: Box<Stmt>,
        test: Expr,
        label: LabelId,
    },
    For(Box<ForStmt>),
    ForIn(Box<ForInStmt>),
    ForOf(Box<ForOfStmt>),
    Switch(Box<SwitchStmt>),
    Break(LabelId),
    Continue(LabelId),
    Return(Option<Expr>),
    Throw(Expr),
    Try(Box<TryStmt>),
    Labeled {
        body: Box<Stmt>,
        label: LabelId,
    },
    With {
        object: Expr,
        body: Box<Stmt>,
        scope: ScopeId,
    },
    Debugger,
    Empty,
    /// An element of a class's aggregate field initializer.
    FieldInit(Box<FieldInit>),
    /// `export default value`: the hidden default binding's initialization;
    /// an anonymous function or class value is named `default`.
    ExportDefault {
        value: Expr,
        decl: DeclId,
    },
}

#[derive(Debug)]
pub(crate) struct ForStmt {
    pub(crate) init: Option<ForInit>,
    pub(crate) test: Option<Expr>,
    pub(crate) update: Option<Expr>,
    pub(crate) body: Stmt,
    pub(crate) scope: ScopeId,
    pub(crate) label: LabelId,
}

#[derive(Debug)]
pub(crate) struct ForInStmt {
    pub(crate) target: ForTarget,
    /// Annex B `for (var x = init in o)`.
    pub(crate) init: Option<Expr>,
    pub(crate) object: Expr,
    pub(crate) body: Stmt,
    pub(crate) scope: ScopeId,
    pub(crate) label: LabelId,
}

#[derive(Debug)]
pub(crate) struct ForOfStmt {
    pub(crate) target: ForTarget,
    pub(crate) iterable: Expr,
    pub(crate) body: Stmt,
    pub(crate) scope: ScopeId,
    pub(crate) label: LabelId,
    pub(crate) is_await: bool,
}

#[derive(Debug)]
pub(crate) struct SwitchStmt {
    pub(crate) discriminant: Expr,
    pub(crate) cases: Vec<SwitchCase>,
    pub(crate) scope: ScopeId,
    pub(crate) label: LabelId,
}

#[derive(Debug)]
pub(crate) struct TryStmt {
    pub(crate) block: Vec<Stmt>,
    pub(crate) block_scope: Option<ScopeId>,
    pub(crate) catch: Option<Catch>,
    pub(crate) finally: Option<(Vec<Stmt>, Option<ScopeId>)>,
}

// ---- patterns ----

#[derive(Debug)]
pub(crate) struct Pattern {
    pub(crate) kind: PatternKind,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum PatternKind {
    /// A declared binding.
    Binding(Ref),
    /// An assignment target: an identifier, member or super member.
    Target(Box<Expr>),
    Object {
        props: Vec<PatternProp>,
        rest: Option<Box<Pattern>>,
        /// The rest's `...`.
        rest_pos: Pos,
    },
    Array {
        items: Vec<Option<PatternItem>>,
        rest: Option<Box<Pattern>>,
    },
}

#[derive(Debug)]
pub(crate) struct PatternProp {
    pub(crate) key: PropKey,
    pub(crate) value: Pattern,
    pub(crate) default: Option<Expr>,
    /// The property name token.
    pub(crate) key_pos: Pos,
}

#[derive(Debug)]
pub(crate) struct PatternItem {
    pub(crate) pattern: Pattern,
    pub(crate) default: Option<Expr>,
}

// ---- modules ----

#[derive(Debug, Default)]
pub(crate) struct ModuleRecord {
    pub(crate) requests: Vec<ModuleRequest>,
    pub(crate) imports: Vec<ModuleImport>,
    pub(crate) local_exports: Vec<LocalExport>,
    pub(crate) indirect_exports: Vec<IndirectExport>,
    pub(crate) star_exports: Vec<u32>,
    pub(crate) uses_import_meta: bool,
    pub(crate) has_top_level_await: bool,
}

#[derive(Debug)]
pub(crate) struct ModuleRequest {
    pub(crate) specifier: JsString,
    /// `None` when the request has no `with` clause.
    pub(crate) attributes: Option<Vec<(JsString, JsString)>>,
}

#[derive(Debug)]
pub(crate) enum ImportName {
    Name(JsString),
    Namespace,
}

#[derive(Debug)]
pub(crate) struct ModuleImport {
    pub(crate) request: u32,
    pub(crate) name: ImportName,
    pub(crate) decl: DeclId,
}

#[derive(Debug)]
pub(crate) struct LocalExport {
    pub(crate) local: NameId,
    pub(crate) export: JsString,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) struct IndirectExport {
    pub(crate) export: JsString,
    pub(crate) request: u32,
    pub(crate) import: ImportName,
    pub(crate) span: Span,
}
