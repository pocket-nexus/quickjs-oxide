//! AST to HIR: name resolution and the binding facts the middle end uses.
//!
//! Every identifier is resolved against the scope the parser recorded for
//! it, through enclosing functions, to a declaration; an unresolved name is
//! a `global`. A use from an inner function adds the declaration to the
//! captures of every function between the two. `this`, `arguments`,
//! `new.target`, the home object and the active function are implicit
//! declarations of the nearest non-arrow function. A lexical declaration
//! needs a TDZ check when a use may precede its initialization: a use in
//! another function, a use before it in the source, or any use of a
//! declaration in a `switch`.

use super::schema::{
    self, Arg, ArrayItem, AssignOp, BinOp, Block, Class, ClassMember, Const, ConstRef, Decl,
    DeclKind, DeclRef, DeclaringKind, Entry, Expr, ExprKind, FieldKey, File, FileRef, ForTarget,
    Function, FunctionKind, FunctionRef, Import, ImportAttribute, LogicalOp, MethodRole, Module,
    ModuleRef, ModuleRequest, ObjProp, Param, ParamCopy, Pattern, PatternItem, PatternKind,
    PatternProp, Program, PropKey, Reexport, SourceRange, Span, Stmt, StmtKind, StringRef,
    SwitchCase, Target, TargetKind, TypeRef, UnaryOp, Unit, UpdateOp,
};
use crate::engine::api::error::Error;
use crate::engine::ast::names::{NameId, NameTable};
use crate::engine::ast::{self, DeclId, FuncId, ProgramKind, Ref, ScopeId, ScopeKind};
use crate::engine::value::JsString;
use oxide_core::FxHashMap;
use std::rc::Rc;

/// The HIR of one program, and what the in-process middle end needs beside
/// it.
pub(crate) struct Built {
    pub(crate) program: Program,
    /// The compiled pattern of each regular-expression constant.
    pub(crate) regexps: FxHashMap<u32, Rc<oxide_regexp::CompiledRegExp>>,
}

/// Builds the HIR of a parsed program.
///
/// # Errors
/// Returns the early errors that need every declaration: a private name that
/// no enclosing class declares.
pub(crate) fn build(program: ast::Program, path: &str) -> Result<Built, Error> {
    let ast::Program {
        kind,
        root,
        names,
        scopes,
        decls,
        functions,
        module,
        has_with: _,
    } = program;
    let mut builder = Builder::new(&names, &scopes, &decls, &functions);
    builder.files.push(File {
        path: JsString::try_from_utf8(path)?,
        hash: 0,
    });
    let top = builder.function(*root)?;
    let entry = match kind {
        ProgramKind::Script => Entry::Script { top },
        ProgramKind::Eval => Entry::Eval { top },
        ProgramKind::Module => Entry::Module {
            module: ModuleRef(0),
        },
    };
    let modules = match module {
        Some(record) => vec![builder.module(record, top)?],
        None => Vec::new(),
    };
    builder.finish_decls();
    let Builder {
        files,
        strings,
        consts,
        hir_decls,
        hir_functions,
        regexps,
        ..
    } = builder;
    Ok(Built {
        program: Program {
            version: super::schema::SCHEMA_VERSION,
            unit: Unit {
                name: JsString::try_from_utf8(path)?,
                fingerprint_inputs: Vec::new(),
            },
            entry: Box::new(entry),
            files,
            strings,
            consts,
            types: Vec::new(),
            layouts: Vec::new(),
            descriptors: Vec::new(),
            decls: hir_decls,
            functions: hir_functions
                .into_iter()
                .map(|function| function.ok_or_else(|| Error::internal("a function was not built")))
                .collect::<Result<_, _>>()?,
            modules,
        },
        regexps,
    })
}

#[derive(Default, Clone, Copy)]
struct Implicit {
    this: Option<DeclRef>,
    arguments: Option<DeclRef>,
    new_target: Option<DeclRef>,
    home_object: Option<DeclRef>,
    active_function: Option<DeclRef>,
}

#[derive(Default)]
struct FunctionFacts {
    captures: Vec<DeclRef>,
    inner: Vec<FunctionRef>,
    implicit: Implicit,
    self_binding: Option<(NameId, DeclRef)>,
    uses_this: bool,
    uses_arguments: bool,
    uses_new_target: bool,
    has_direct_eval: bool,
}

/// What a lexical declaration's TDZ decision needs.
#[derive(Clone, Copy, Default)]
struct Readiness {
    /// The source position after which the binding is initialized.
    ready: Option<u32>,
    /// A declaration whose scope a jump can enter past it (a `switch`).
    always: bool,
}

struct Builder<'a> {
    names: &'a NameTable,
    scopes: &'a [ast::Scope],
    ast_decls: &'a [ast::Decl],
    ast_functions: &'a [ast::FunctionInfo],
    files: Vec<File>,
    strings: Vec<JsString>,
    string_index: FxHashMap<JsString, u32>,
    name_strings: FxHashMap<NameId, StringRef>,
    consts: Vec<Const>,
    number_index: FxHashMap<u64, u32>,
    regexps: FxHashMap<u32, Rc<oxide_regexp::CompiledRegExp>>,
    hir_decls: Vec<Decl>,
    /// AST declaration to HIR declaration (the first `ast_decls.len()`).
    decl_owner: Vec<FuncId>,
    readiness: Vec<Readiness>,
    /// Uses: declaration, using function, position.
    uses: Vec<(DeclRef, FuncId, u32)>,
    globals: FxHashMap<NameId, DeclRef>,
    /// Each scope's declarations by name: the last declared.
    scope_names: FxHashMap<(u32, NameId), DeclId>,
    /// The module's `import.meta` binding, once used.
    import_meta: Option<DeclRef>,
    hir_functions: Vec<Option<Function>>,
    facts: Vec<FunctionFacts>,
    /// Declarations each function owns, AST ones first.
    owned: Vec<Vec<DeclRef>>,
    current: Vec<FuncId>,
    template_sites: u32,
}

const NO_FILE: FileRef = FileRef(0);

fn span(start: u32, end: u32) -> Span {
    Span {
        file: NO_FILE,
        start,
        end: end.max(start),
    }
}

fn ast_span(span: impl Into<ast::Span>) -> Span {
    let span = span.into();
    self::span(span.start, span.end)
}

fn expr(kind: ExprKind, span: Span) -> Expr {
    Expr {
        kind,
        ty: TypeRef(0),
        span,
    }
}

/// Each scope's declarations by name, the last of a name winning.
fn scope_names(scopes: &[ast::Scope], decls: &[ast::Decl]) -> FxHashMap<(u32, NameId), DeclId> {
    let mut map = FxHashMap::default();
    for (index, scope) in scopes.iter().enumerate() {
        for &decl in &scope.decls {
            map.insert((index as u32, decls[decl.index()].name), decl);
        }
    }
    map
}

fn stmt(kind: StmtKind, span: Span) -> Stmt {
    Stmt { kind, span }
}

impl<'a> Builder<'a> {
    fn new(
        names: &'a NameTable,
        scopes: &'a [ast::Scope],
        ast_decls: &'a [ast::Decl],
        ast_functions: &'a [ast::FunctionInfo],
    ) -> Self {
        let mut builder = Self {
            names,
            scopes,
            ast_decls,
            ast_functions,
            files: Vec::new(),
            strings: Vec::new(),
            string_index: FxHashMap::default(),
            name_strings: FxHashMap::default(),
            consts: Vec::new(),
            number_index: FxHashMap::default(),
            regexps: FxHashMap::default(),
            hir_decls: Vec::with_capacity(ast_decls.len()),
            decl_owner: Vec::with_capacity(ast_decls.len()),
            readiness: Vec::new(),
            uses: Vec::new(),
            globals: FxHashMap::default(),
            scope_names: scope_names(scopes, ast_decls),
            import_meta: None,
            hir_functions: (0..ast_functions.len()).map(|_| None).collect(),
            facts: (0..ast_functions.len())
                .map(|_| FunctionFacts::default())
                .collect(),
            owned: vec![Vec::new(); ast_functions.len()],
            current: Vec::new(),
            template_sites: 0,
        };
        // AST declarations keep their indexes.
        for (index, decl) in ast_decls.iter().enumerate() {
            let owner = scopes[decl.scope.index()].function;
            let kind = match decl.kind {
                ast::DeclKind::Var => DeclKind::Var,
                ast::DeclKind::Let => DeclKind::Let,
                ast::DeclKind::Const | ast::DeclKind::ClassName => DeclKind::Const,
                ast::DeclKind::Class => DeclKind::Class,
                ast::DeclKind::Param => DeclKind::Param,
                ast::DeclKind::CatchParam => DeclKind::Catch,
                ast::DeclKind::ScopedFunction { .. } => DeclKind::Function,
                ast::DeclKind::FunctionName => DeclKind::FunctionName,
                ast::DeclKind::Import => DeclKind::Import,
                ast::DeclKind::Hidden { .. } => DeclKind::Hidden,
                ast::DeclKind::PrivateField { .. } => DeclKind::PrivateField,
                ast::DeclKind::PrivateMethod { .. } => DeclKind::PrivateMethod,
                ast::DeclKind::PrivateGetter { .. } => DeclKind::PrivateGetter,
                ast::DeclKind::PrivateSetter { .. } => DeclKind::PrivateSetter,
                ast::DeclKind::PrivateGetterSetter { .. } => DeclKind::PrivateAccessor,
            };
            let is_static = matches!(
                decl.kind,
                ast::DeclKind::PrivateField { is_static: true }
                    | ast::DeclKind::PrivateMethod { is_static: true }
                    | ast::DeclKind::PrivateGetter { is_static: true }
                    | ast::DeclKind::PrivateSetter { is_static: true }
                    | ast::DeclKind::PrivateGetterSetter { is_static: true }
            );
            let name = builder.name(decl.name);
            builder.hir_decls.push(Decl {
                name,
                kind,
                owner: FunctionRef(owner.0),
                captured_by: Vec::new(),
                assigned: false,
                tdz: false,
                ty: TypeRef(0),
                span: ast_span(decl.span),
                is_static,
            });
            builder.decl_owner.push(owner);
            builder.readiness.push(Readiness {
                ready: None,
                always: scopes[decl.scope.index()].kind == ScopeKind::Switch,
            });
            builder.owned[owner.index()].push(DeclRef(index as u32));
        }
        builder
    }

    // ---- tables ----

    fn string(&mut self, value: JsString) -> StringRef {
        if let Some(&index) = self.string_index.get(&value) {
            return StringRef(index);
        }
        let index = self.strings.len() as u32;
        self.strings.push(value.clone());
        self.string_index.insert(value, index);
        StringRef(index)
    }

    fn name(&mut self, name: NameId) -> StringRef {
        if let Some(&reference) = self.name_strings.get(&name) {
            return reference;
        }
        let value = JsString::try_from_utf8(self.names.name(name))
            .unwrap_or_else(|_| JsString::from_static(""));
        let reference = self.string(value);
        self.name_strings.insert(name, reference);
        reference
    }

    fn number(&mut self, value: f64) -> ConstRef {
        // Byte-swapped: an integer's low bits are zero, and the hasher
        // buckets by the low bits.
        let bits = value.to_bits().swap_bytes();
        if let Some(&index) = self.number_index.get(&bits) {
            return ConstRef(index);
        }
        let index = self.consts.len() as u32;
        self.consts.push(Const::Number { value });
        self.number_index.insert(bits, index);
        ConstRef(index)
    }

    fn push_const(&mut self, value: Const) -> ConstRef {
        let index = self.consts.len() as u32;
        self.consts.push(value);
        ConstRef(index)
    }

    fn new_decl(&mut self, name: StringRef, kind: DeclKind, owner: FuncId, at: Span) -> DeclRef {
        let index = self.hir_decls.len() as u32;
        self.hir_decls.push(Decl {
            name,
            kind,
            owner: FunctionRef(owner.0),
            captured_by: Vec::new(),
            assigned: false,
            tdz: false,
            ty: TypeRef(0),
            span: at,
            is_static: false,
        });
        self.decl_owner.push(owner);
        self.readiness.push(Readiness::default());
        self.owned[owner.index()].push(DeclRef(index));
        DeclRef(index)
    }

    // ---- resolution ----

    fn current(&self) -> FuncId {
        *self.current.last().expect("a function is being built")
    }

    /// Records a use of `decl` from the current function, with its captures.
    fn use_decl(&mut self, decl: DeclRef, at: u32) -> DeclRef {
        let current = self.current();
        let owner = self.decl_owner[decl.0 as usize];
        self.uses.push((decl, current, at));
        if self.hir_decls[decl.0 as usize].kind == DeclKind::Global || owner == current {
            return decl;
        }
        let mut function = current;
        while function != owner {
            let facts = &mut self.facts[function.index()];
            if !facts.captures.contains(&decl) {
                facts.captures.push(decl);
            }
            let captured_by = &mut self.hir_decls[decl.0 as usize].captured_by;
            if !captured_by.contains(&FunctionRef(function.0)) {
                captured_by.push(FunctionRef(function.0));
            }
            match self.ast_functions[function.index()].parent {
                Some(parent) => function = parent,
                None => break,
            }
        }
        decl
    }

    fn lookup_in_scope(&self, scope: ScopeId, name: NameId) -> Option<DeclId> {
        self.scope_names.get(&(scope.0, name)).copied()
    }

    /// The declaration a binding pattern or declarator declares: resolved
    /// like a use, but neither a use nor an assignment.
    fn resolve_declaration(&mut self, reference: Ref) -> DeclRef {
        let uses = self.uses.len();
        let decl = self.resolve(reference);
        self.uses.truncate(uses);
        decl
    }

    /// The declaration `reference` names, as QuickJS resolves it.
    fn resolve(&mut self, reference: Ref) -> DeclRef {
        let name = reference.name;
        let mut scope = Some(reference.scope);
        let mut function = self.scopes[reference.scope.index()].function;
        loop {
            while let Some(current) = scope {
                if let Some(decl) = self.lookup_in_scope(current, name)
                    && !self.ast_decls[decl.index()].kind.is_private()
                {
                    return self.use_decl(DeclRef(decl.0), pos(reference.span));
                }
                scope = self.scopes[current.index()].parent;
            }
            let info = &self.ast_functions[function.index()];
            if self.names.name(name) == "arguments" && has_arguments(info.kind) {
                let decl = self.implicit_arguments(function);
                return self.use_decl(decl, pos(reference.span));
            }
            if let Some((self_name, decl)) = self.facts[function.index()].self_binding
                && self_name == name
            {
                return self.use_decl(decl, pos(reference.span));
            }
            match info.parent {
                Some(parent) => {
                    scope = info.definition_scope;
                    function = parent;
                }
                None => {
                    let decl = self.global(name);
                    return self.use_decl(decl, pos(reference.span));
                }
            }
        }
    }

    /// A private name: through scopes and functions, to a class's.
    fn resolve_private(&mut self, reference: Ref) -> Result<DeclRef, Error> {
        let name = reference.name;
        let mut scope = Some(reference.scope);
        let mut function = self.scopes[reference.scope.index()].function;
        loop {
            while let Some(current) = scope {
                if let Some(decl) = self.lookup_in_scope(current, name)
                    && self.ast_decls[decl.index()].kind.is_private()
                {
                    return Ok(self.use_decl(DeclRef(decl.0), pos(reference.span)));
                }
                scope = self.scopes[current.index()].parent;
            }
            let info = &self.ast_functions[function.index()];
            match info.parent {
                Some(parent) => {
                    scope = info.definition_scope;
                    function = parent;
                }
                None => {
                    return Err(crate::engine::ast::parser::syntax_atom_error_without_span(
                        "undefined private field '",
                        self.names.name(name),
                        "'",
                    )?);
                }
            }
        }
    }

    fn global(&mut self, name: NameId) -> DeclRef {
        if let Some(&decl) = self.globals.get(&name) {
            return decl;
        }
        let string = self.name(name);
        let decl = self.new_decl(string, DeclKind::Global, FuncId(0), span(0, 0));
        self.globals.insert(name, decl);
        decl
    }

    /// The nearest function that is not an arrow.
    fn non_arrow(&self, mut function: FuncId) -> FuncId {
        while self.ast_functions[function.index()].kind == ast::FunctionKind::Arrow {
            match self.ast_functions[function.index()].parent {
                Some(parent) => function = parent,
                None => break,
            }
        }
        function
    }

    fn implicit_arguments(&mut self, function: FuncId) -> DeclRef {
        if let Some(decl) = self.facts[function.index()].implicit.arguments {
            return decl;
        }
        let name = self.string(JsString::from_static("arguments"));
        let decl = self.new_decl(name, DeclKind::Arguments, function, span(0, 0));
        let facts = &mut self.facts[function.index()];
        facts.implicit.arguments = Some(decl);
        facts.uses_arguments = true;
        decl
    }

    fn implicit(&mut self, kind: DeclKind, at: u32) -> DeclRef {
        let function = self.non_arrow(self.current());
        let existing = {
            let implicit = &self.facts[function.index()].implicit;
            match kind {
                DeclKind::This => implicit.this,
                DeclKind::NewTarget => implicit.new_target,
                DeclKind::HomeObject => implicit.home_object,
                DeclKind::ActiveFunction => implicit.active_function,
                _ => None,
            }
        };
        let decl = match existing {
            Some(decl) => decl,
            None => {
                let text = match kind {
                    DeclKind::This => "<this>",
                    DeclKind::NewTarget => "<new_target>",
                    DeclKind::HomeObject => "<home_object>",
                    _ => "<this_active_func>",
                };
                let name = self.string(JsString::from_static(text));
                let decl = self.new_decl(name, kind, function, span(0, 0));
                let facts = &mut self.facts[function.index()];
                match kind {
                    DeclKind::This => {
                        facts.implicit.this = Some(decl);
                        facts.uses_this = true;
                    }
                    DeclKind::NewTarget => {
                        facts.implicit.new_target = Some(decl);
                        facts.uses_new_target = true;
                    }
                    DeclKind::HomeObject => facts.implicit.home_object = Some(decl),
                    _ => facts.implicit.active_function = Some(decl),
                }
                decl
            }
        };
        self.use_decl(decl, at)
    }

    fn mark_assigned(&mut self, decl: DeclRef) {
        self.hir_decls[decl.0 as usize].assigned = true;
    }

    fn set_ready(&mut self, decl: DeclRef, at: u32) {
        let readiness = &mut self.readiness[decl.0 as usize];
        if readiness.ready.is_none() {
            readiness.ready = Some(at);
        }
    }

    fn finish_decls(&mut self) {
        let mut tdz = vec![false; self.hir_decls.len()];
        for &(decl, function, at) in &self.uses {
            let index = decl.0 as usize;
            let owner = self.decl_owner[index];
            let readiness = self.readiness[index];
            if function != owner
                || readiness.always
                || readiness.ready.is_none_or(|ready| at < ready)
            {
                tdz[index] = true;
            }
        }
        for (index, (decl, tdz)) in self.hir_decls.iter_mut().zip(tdz).enumerate() {
            // A parameter environment's bindings are lexical: a default may
            // read a later one.
            let parameter_environment = decl.kind == DeclKind::Param
                && self
                    .ast_decls
                    .get(index)
                    .is_some_and(|d| d.storage == ast::Storage::Local);
            decl.tdz = tdz
                && (parameter_environment
                    || matches!(
                        decl.kind,
                        DeclKind::Let
                            | DeclKind::Const
                            | DeclKind::Class
                            | DeclKind::Hidden
                            | DeclKind::Import
                    ));
            if decl.kind == DeclKind::Const {
                decl.assigned = false;
            }
        }
    }

    // ---- functions ----

    fn function(&mut self, function: ast::Function) -> Result<FunctionRef, Error> {
        let id = function.id;
        if let Some(&parent) = self.current.last() {
            self.facts[parent.index()].inner.push(FunctionRef(id.0));
        }
        if let Some(decl) = function.self_binding {
            let name = self.ast_decls[decl.index()].name;
            self.facts[id.index()].self_binding = Some((name, DeclRef(decl.0)));
        }
        self.current.push(id);
        let result = self.function_body(function);
        self.current.pop();
        let built = result?;
        self.hir_functions[id.index()] = Some(built);
        Ok(FunctionRef(id.0))
    }

    fn function_body(&mut self, function: ast::Function) -> Result<Function, Error> {
        let ast::Function {
            id,
            kind,
            role,
            is_async,
            is_generator,
            name,
            self_binding,
            params,
            rest,
            rest_default,
            simple_params,
            length,
            body,
            concise,
            default_ctor,
            strict,
            span: function_span,
            source,
        } = function;
        let info = &self.ast_functions[id.index()];
        let (var_scope, body_scope, parameter_scope) =
            (info.var_scope, info.body_scope, info.parameter_scope);
        let mut hir_params = Vec::with_capacity(params.len());
        for param in params {
            let inferred = binding_name(&param.pattern);
            let pattern = self.pattern(param.pattern, Some(DeclaringKind::Var))?;
            let default = match param.default {
                Some(default) => Some(Box::new(self.named_value(default, inferred)?)),
                None => None,
            };
            hir_params.push(Param {
                pattern: Box::new(pattern),
                default,
            });
        }
        let rest = match rest {
            Some(rest) => Some(Box::new(self.pattern(rest, Some(DeclaringKind::Var))?)),
            None => None,
        };
        let rest_default = match rest_default {
            Some(default) => Some(self.boxed(default)?),
            None => None,
        };
        let stmts = self.statements(body)?;
        let body_decls = self.scope_decls(body_scope);
        let param_copies = match parameter_scope {
            Some(parameter_scope) => self.scopes[parameter_scope.index()]
                .decls
                .iter()
                .filter_map(|from| {
                    let name = self.ast_decls[from.index()].name;
                    self.lookup_in_scope(var_scope, name).map(|to| ParamCopy {
                        from: DeclRef(from.0),
                        to: DeclRef(to.0),
                    })
                })
                .collect(),
            None => Vec::new(),
        };
        let arguments_decl = if has_arguments(kind) {
            let arguments = self.names.lookup("arguments");
            let declared = arguments.and_then(|name| {
                let decl = self.lookup_in_scope(var_scope, name)?;
                (self.ast_decls[decl.index()].kind == ast::DeclKind::Var).then_some(DeclRef(decl.0))
            });
            declared.or(self.facts[id.index()].implicit.arguments)
        } else {
            None
        };
        let facts = std::mem::take(&mut self.facts[id.index()]);
        let hir_kind = match kind {
            ast::FunctionKind::Script => FunctionKind::Script,
            ast::FunctionKind::Module => FunctionKind::Module,
            ast::FunctionKind::Eval => FunctionKind::Eval,
            ast::FunctionKind::Normal => FunctionKind::Normal,
            ast::FunctionKind::Arrow => FunctionKind::Arrow,
            ast::FunctionKind::Method => match role {
                ast::MethodRole::Method => FunctionKind::Method,
                ast::MethodRole::Get => FunctionKind::Getter,
                ast::MethodRole::Set => FunctionKind::Setter,
            },
            ast::FunctionKind::ClassConstructor { derived: false } => FunctionKind::Constructor,
            ast::FunctionKind::ClassConstructor { derived: true } => {
                FunctionKind::DerivedConstructor
            }
            ast::FunctionKind::ClassFieldInit { .. } => FunctionKind::ClassFieldInit,
            ast::FunctionKind::StaticBlock => FunctionKind::StaticBlock,
        };
        let name = name.map(|name| self.name(name.name));
        let uses_this = facts.uses_this;
        Ok(Function {
            name,
            kind: hir_kind,
            role: method_role(role),
            async_: is_async,
            generator: is_generator,
            strict,
            params: hir_params,
            rest,
            rest_default,
            simple_params,
            body: Block {
                stmts,
                decls: body_decls,
            },
            decls: std::mem::take(&mut self.owned[id.index()]),
            captures: facts.captures,
            inner: facts.inner,
            uses_this,
            uses_arguments: facts.uses_arguments,
            uses_new_target: facts.uses_new_target,
            has_direct_eval: facts.has_direct_eval,
            home_object: facts.implicit.home_object.is_some(),
            sig: None,
            effects: None,
            span: ast_span(function_span),
            length,
            source: SourceRange {
                start: source.0,
                end: source.1,
            },
            self_: self_binding.map(|decl| DeclRef(decl.0)),
            arguments_decl,
            param_copies,
            concise,
            default_ctor,
        })
    }

    fn scope_decls(&self, scope: ScopeId) -> Vec<DeclRef> {
        self.scopes[scope.index()]
            .decls
            .iter()
            .filter(|decl| !self.ast_decls[decl.index()].kind.is_private())
            .map(|decl| DeclRef(decl.0))
            .collect()
    }

    // ---- statements ----

    fn statements(&mut self, statements: Vec<ast::Stmt>) -> Result<Vec<Stmt>, Error> {
        let mut out = Vec::with_capacity(statements.len());
        for statement in statements {
            if let Some(statement) = self.statement(statement)? {
                out.push(statement);
            }
        }
        Ok(out)
    }

    fn block(
        &mut self,
        statements: Vec<ast::Stmt>,
        scope: Option<ScopeId>,
    ) -> Result<Block, Error> {
        Ok(Block {
            stmts: self.statements(statements)?,
            decls: scope
                .map(|scope| self.scope_decls(scope))
                .unwrap_or_default(),
        })
    }

    fn boxed_statement(&mut self, statement: ast::Stmt) -> Result<Box<Stmt>, Error> {
        let at = ast_span(statement.span);
        Ok(Box::new(
            self.statement(statement)?
                .unwrap_or_else(|| stmt(StmtKind::Empty, at)),
        ))
    }

    /// Wraps `statement` in a block holding the declarations of `scope`.
    fn scoped(&self, statement: Stmt, scope: ScopeId) -> Stmt {
        let decls = self.scope_decls(scope);
        if decls.is_empty() {
            return statement;
        }
        let at = statement.span.clone();
        stmt(
            StmtKind::Block {
                body: Block {
                    stmts: vec![statement],
                    decls,
                },
            },
            at,
        )
    }

    fn statement(&mut self, statement: ast::Stmt) -> Result<Option<Stmt>, Error> {
        use ast::StmtKind as S;
        let at = ast_span(statement.span);
        let kind = match statement.kind {
            S::Expr(value) => StmtKind::Expr {
                e: Box::new(self.expr(value)?),
            },
            S::Var(kind, declarators) => {
                return self.var_statement(kind, declarators, at).map(Some);
            }
            S::Function {
                function,
                decl,
                annex_b,
            } => {
                let decl = DeclRef(decl.0);
                let function = self.function(*function)?;
                StmtKind::FnDecl {
                    decl,
                    fn_: function,
                    annex_b: annex_b.map(|decl| DeclRef(decl.0)),
                }
            }
            S::Class { class, decl } => {
                let decl = DeclRef(decl.0);
                let end = pos_end(class.span);
                let cls = Box::new(self.class(*class)?);
                self.set_ready(decl, end);
                StmtKind::ClassDecl { decl, cls }
            }
            S::Block { body, scope } => StmtKind::Block {
                body: self.block(body, scope)?,
            },
            S::If {
                test,
                then,
                otherwise,
                scope,
            } => {
                let built = StmtKind::If {
                    test: Box::new(self.expr(test)?),
                    then: self.boxed_statement(*then)?,
                    else_: match otherwise {
                        Some(otherwise) => Some(self.boxed_statement(*otherwise)?),
                        None => None,
                    },
                };
                return Ok(Some(self.scoped(stmt(built, at), scope)));
            }
            S::While { test, body, label } => StmtKind::While {
                test: Box::new(self.expr(test)?),
                body: self.boxed_statement(*body)?,
                label: label.0,
            },
            S::DoWhile { body, test, label } => StmtKind::DoWhile {
                body: self.boxed_statement(*body)?,
                test: Box::new(self.expr(test)?),
                label: label.0,
            },
            S::For(for_statement) => return self.for_statement(*for_statement, at).map(Some),
            S::ForIn(for_in) => return self.for_in_statement(for_in, at).map(Some),
            S::ForOf(for_of) => return self.for_of_statement(for_of, at).map(Some),
            S::Switch(switch) => return self.switch_statement(switch, at).map(Some),
            S::Break(label) => StmtKind::Break { label: label.0 },
            S::Continue(label) => StmtKind::Continue { label: label.0 },
            S::Return(value) => StmtKind::Return {
                e: match value {
                    Some(value) => Some(Box::new(self.expr(value)?)),
                    None => None,
                },
            },
            S::Throw(value) => StmtKind::Throw {
                e: Box::new(self.expr(value)?),
            },
            S::Try(try_statement) => self.try_statement(try_statement)?,
            S::Labeled { body, label } => StmtKind::Labeled {
                label: label.0,
                body: self.boxed_statement(*body)?,
            },
            S::With { .. } => {
                return Err(Error::internal("a with statement reached the HIR builder"));
            }
            S::Debugger => StmtKind::Debugger,
            S::Empty => StmtKind::Empty,
            S::FieldInit(field) => self.field_init(field)?,
            S::ExportDefault { value, decl } => {
                let decl = DeclRef(decl.0);
                let end = pos_end(value.span);
                let value = self.named_value_string(value, JsString::from_static("default"))?;
                self.set_ready(decl, end);
                StmtKind::Declare {
                    decl,
                    init: Some(Box::new(value)),
                }
            }
        };
        Ok(Some(stmt(kind, at)))
    }

    #[inline(never)]
    fn for_in_statement(&mut self, for_in: Box<ast::ForInStmt>, at: Span) -> Result<Stmt, Error> {
        let ast::ForInStmt {
            target,
            init,
            object,
            body,
            scope,
            label,
        } = *for_in;
        let target = self.for_target(target)?;
        let init = match init {
            Some(init) => Some(Box::new(self.expr(init)?)),
            None => None,
        };
        let object_end = pos_end(object.span);
        let obj = Box::new(self.expr(object)?);
        self.ready_scope(scope, object_end);
        let built = StmtKind::ForIn {
            target: Box::new(target),
            init,
            obj,
            body: self.boxed_statement(body)?,
            label: label.0,
        };
        Ok(self.scoped(stmt(built, at), scope))
    }

    #[inline(never)]
    fn for_of_statement(&mut self, for_of: Box<ast::ForOfStmt>, at: Span) -> Result<Stmt, Error> {
        let ast::ForOfStmt {
            target,
            iterable,
            body,
            scope,
            label,
            is_await,
        } = *for_of;
        let target = self.for_target(target)?;
        let iterable_end = pos_end(iterable.span);
        let iter = Box::new(self.expr(iterable)?);
        self.ready_scope(scope, iterable_end);
        let built = StmtKind::ForOf {
            target: Box::new(target),
            iter,
            body: self.boxed_statement(body)?,
            label: label.0,
            await_: is_await,
            array_fast: false,
        };
        Ok(self.scoped(stmt(built, at), scope))
    }

    // Boxed so `statement`'s frame holds only the pointer.
    #[allow(clippy::boxed_local)]
    #[inline(never)]
    fn switch_statement(&mut self, switch: Box<ast::SwitchStmt>, at: Span) -> Result<Stmt, Error> {
        let ast::SwitchStmt {
            discriminant,
            cases,
            scope,
            label,
        } = *switch;
        let disc = Box::new(self.expr(discriminant)?);
        let mut hir_cases = Vec::with_capacity(cases.len());
        for case in cases {
            hir_cases.push(SwitchCase {
                test: match case.test {
                    Some(test) => Some(Box::new(self.expr(test)?)),
                    None => None,
                },
                body: self.statements(case.body)?,
            });
        }
        let built = StmtKind::Switch {
            disc,
            cases: hir_cases,
            label: label.0,
            exhaustive: false,
        };
        Ok(self.scoped(stmt(built, at), scope))
    }

    #[inline(never)]
    fn try_statement(&mut self, try_statement: Box<ast::TryStmt>) -> Result<StmtKind, Error> {
        let ast::TryStmt {
            block,
            block_scope,
            catch,
            finally,
        } = *try_statement;
        let body = self.block(block, block_scope)?;
        let (catch_param, catch_body, catch_default) = match catch {
            Some(catch) => {
                let param = match catch.param {
                    Some(param) => Some(Box::new(self.pattern(param, Some(DeclaringKind::Let))?)),
                    None => None,
                };
                let default = match catch.param_default {
                    Some(default) => Some(Box::new(self.expr(default)?)),
                    None => None,
                };
                let mut catch_body = self.block(catch.body, catch.body_scope)?;
                let mut decls = self.scope_decls(catch.scope);
                decls.append(&mut catch_body.decls);
                catch_body.decls = decls;
                (param, Some(catch_body), default)
            }
            None => (None, None, None),
        };
        let finally = match finally {
            Some((statements, scope)) => Some(self.block(statements, scope)?),
            None => None,
        };
        Ok(StmtKind::Try {
            body: Box::new(body),
            catch_param,
            catch_body: catch_body.map(Box::new),
            finally: finally.map(Box::new),
            catch_default,
        })
    }

    // Boxed so `statement`'s frame holds only the pointer.
    #[allow(clippy::boxed_local)]
    #[inline(never)]
    fn field_init(&mut self, field: Box<ast::FieldInit>) -> Result<StmtKind, Error> {
        Ok(match *field {
            ast::FieldInit::Public { key, value, span } => {
                let (key, inferred) = match key {
                    ast::FieldKey::Name(name) => {
                        let string = self.string(name.clone());
                        (FieldKey::Name { name: string }, Some(name))
                    }
                    ast::FieldKey::Computed(decl) => {
                        let decl = DeclRef(decl.0);
                        self.use_decl(decl, pos(span));
                        (FieldKey::Computed { decl }, None)
                    }
                };
                let value = match value {
                    Some(value) => Some(Box::new(match inferred {
                        Some(name) => self.named_value_string(value, name)?,
                        None => self.expr(value)?,
                    })),
                    None => None,
                };
                StmtKind::DefineField {
                    key: Box::new(key),
                    value,
                }
            }
            ast::FieldInit::Private { decl, value, span } => {
                let decl = DeclRef(decl.0);
                self.use_decl(decl, pos(span));
                let name = self.ast_decls[decl.0 as usize].name;
                let value = match value {
                    Some(value) => Some(Box::new(self.named_value(value, Some(name))?)),
                    None => None,
                };
                StmtKind::DefinePrivate { decl, value }
            }
            ast::FieldInit::StaticBlock(function) => StmtKind::StaticBlock {
                fn_: self.function(*function)?,
            },
        })
    }

    /// Marks every declaration of a loop head's scope ready at `at`.
    fn ready_scope(&mut self, scope: ScopeId, at: u32) {
        for decl in self.scope_decls(scope) {
            self.set_ready(decl, at);
        }
    }

    fn var_statement(
        &mut self,
        kind: ast::VarKind,
        declarators: Vec<ast::VarDeclarator>,
        at: Span,
    ) -> Result<Stmt, Error> {
        let mut out = Vec::new();
        for declarator in declarators {
            let declaring = match kind {
                ast::VarKind::Var => DeclaringKind::Var,
                ast::VarKind::Let => DeclaringKind::Let,
                ast::VarKind::Const => DeclaringKind::Const,
            };
            let end = declarator
                .init
                .as_ref()
                .map_or(pos_end(declarator.target.span), |init| pos_end(init.span));
            let site = span(declarator.pos, end);
            match declarator.target.kind {
                ast::PatternKind::Binding(reference) => {
                    let decl = self.resolve_declaration(reference);
                    let init = match declarator.init {
                        Some(init) => Some(Box::new(self.named_value(init, Some(reference.name))?)),
                        None => None,
                    };
                    if kind == ast::VarKind::Var {
                        if init.is_none() {
                            continue;
                        }
                        self.mark_assigned(decl);
                    }
                    self.set_ready(decl, end);
                    out.push(stmt(StmtKind::Declare { decl, init }, site));
                }
                _ => {
                    let target_span = declarator.target.span;
                    let pattern = self.pattern(declarator.target, Some(declaring))?;
                    let init = declarator.init.ok_or_else(|| {
                        Error::internal("a destructuring declaration has no initializer")
                    })?;
                    let init = self.expr(init)?;
                    let mut bound = Vec::new();
                    pattern_decls(&pattern, &mut bound);
                    for decl in bound {
                        self.set_ready(decl, end);
                    }
                    let _ = target_span;
                    out.push(stmt(
                        StmtKind::Destructure {
                            pattern: Box::new(pattern),
                            init: Box::new(init),
                            declaring: Some(declaring),
                        },
                        site,
                    ));
                }
            }
        }
        Ok(match out.len() {
            1 => out.pop().expect("one statement"),
            0 => stmt(StmtKind::Empty, at),
            _ => stmt(
                StmtKind::Block {
                    body: Block {
                        stmts: out,
                        decls: Vec::new(),
                    },
                },
                at,
            ),
        })
    }

    fn for_statement(&mut self, for_statement: ast::ForStmt, at: Span) -> Result<Stmt, Error> {
        let ast::ForStmt {
            init,
            test,
            update,
            body,
            scope,
            label,
        } = for_statement;
        let mut per_iteration = Vec::new();
        let init = match init {
            Some(ast::ForInit::Var(kind, declarators)) => {
                if kind != ast::VarKind::Var {
                    per_iteration = self.scope_decls(scope);
                }
                Some(Box::new(self.var_statement(
                    kind,
                    declarators,
                    at.clone(),
                )?))
            }
            Some(ast::ForInit::Expr(value)) => {
                let value_span = span_of(&value);
                Some(Box::new(stmt(
                    StmtKind::Expr {
                        e: Box::new(self.expr(value)?),
                    },
                    value_span,
                )))
            }
            None => None,
        };
        let test = match test {
            Some(test) => Some(Box::new(self.expr(test)?)),
            None => None,
        };
        let update = match update {
            Some(update) => Some(Box::new(self.expr(update)?)),
            None => None,
        };
        let body = self.boxed_statement(body)?;
        let built = StmtKind::For {
            init,
            test,
            update,
            body,
            label: label.0,
            per_iteration,
        };
        Ok(self.scoped(stmt(built, at), scope))
    }

    fn for_target(&mut self, target: ast::ForTarget) -> Result<ForTarget, Error> {
        Ok(match target {
            ast::ForTarget::Declare(kind, pattern) => {
                let declaring = match kind {
                    ast::VarKind::Var => DeclaringKind::Var,
                    ast::VarKind::Let => DeclaringKind::Let,
                    ast::VarKind::Const => DeclaringKind::Const,
                };
                let pattern = self.pattern(pattern, Some(declaring))?;
                if kind == ast::VarKind::Var {
                    let mut bound = Vec::new();
                    pattern_decls(&pattern, &mut bound);
                    for decl in bound {
                        self.mark_assigned(decl);
                    }
                }
                ForTarget::Declare {
                    pattern: Box::new(pattern),
                }
            }
            ast::ForTarget::Assign(target) => ForTarget::Assign {
                t: Box::new(self.target(target)?),
            },
            ast::ForTarget::Pattern(pattern) => {
                let at = ast_span(pattern.span);
                let pattern = self.pattern(pattern, None)?;
                let _ = at;
                ForTarget::Assign {
                    t: Box::new(Target {
                        kind: TargetKind::Pattern {
                            p: Box::new(pattern),
                        },
                        span: at,
                    }),
                }
            }
        })
    }

    // ---- classes ----

    fn class(&mut self, class: ast::Class) -> Result<Class, Error> {
        let ast::Class {
            name: _,
            name_binding,
            heritage,
            scope: _,
            private_scope: _,
            constructor,
            members,
            instance_init,
            static_init,
            span: class_span,
        } = class;
        let end = pos_end(class_span);
        let extends = match heritage {
            Some(heritage) => Some(Box::new(self.expr(*heritage)?)),
            None => None,
        };
        let mut hir_members = Vec::with_capacity(members.len());
        for member in members {
            hir_members.push(match member {
                ast::ClassMember::Method {
                    key,
                    function,
                    is_static,
                } => {
                    let role = method_role(function.role);
                    let key = self.prop_key(key)?;
                    ClassMember::Method {
                        key: Box::new(key),
                        fn_: self.function(*function)?,
                        static_: is_static,
                        role,
                    }
                }
                ast::ClassMember::PrivateMethod {
                    decl,
                    function,
                    is_static,
                } => {
                    let role = method_role(function.role);
                    ClassMember::PrivateMethod {
                        decl: DeclRef(decl.0),
                        fn_: self.function(*function)?,
                        static_: is_static,
                        role,
                    }
                }
                ast::ClassMember::FieldKey {
                    key,
                    decl,
                    is_static,
                    pos: _,
                } => {
                    let key_end = pos_end(key.span);
                    let key = self.expr(*key)?;
                    let decl = DeclRef(decl.0);
                    self.set_ready(decl, key_end);
                    ClassMember::FieldKey {
                        key: Box::new(key),
                        decl,
                        static_: is_static,
                    }
                }
                ast::ClassMember::PrivateField { decl, is_static } => ClassMember::PrivateField {
                    decl: DeclRef(decl.0),
                    static_: is_static,
                },
            });
        }
        let ctor = self.function(*constructor)?;
        let instance_init = match instance_init {
            Some(function) => Some(self.function(*function)?),
            None => None,
        };
        let static_init = match static_init {
            Some(function) => Some(self.function(*function)?),
            None => None,
        };
        let name = name_binding.map(|decl| DeclRef(decl.0));
        if let Some(name) = name {
            self.set_ready(name, end);
        }
        Ok(Class {
            name,
            extends,
            ctor,
            members: hir_members,
            instance_init,
            static_init,
            layout: None,
            span: ast_span(class_span),
        })
    }

    // ---- expressions ----

    /// `value` with NamedEvaluation by `name` when it is an anonymous
    /// function or class (through parentheses).
    fn named_value(&mut self, value: ast::Expr, name: Option<NameId>) -> Result<Expr, Error> {
        match name {
            Some(name) if is_anonymous_definition(&value) => {
                let string = self.name(name);
                self.wrap_named(value, string)
            }
            _ => self.expr(value),
        }
    }

    fn named_value_string(&mut self, value: ast::Expr, name: JsString) -> Result<Expr, Error> {
        if is_anonymous_definition(&value) {
            let string = self.string(name);
            return self.wrap_named(value, string);
        }
        self.expr(value)
    }

    fn wrap_named(&mut self, value: ast::Expr, name: StringRef) -> Result<Expr, Error> {
        let at = span_of(&value);
        let inner = self.expr(value)?;
        Ok(expr(
            ExprKind::Named {
                name,
                e: Box::new(inner),
            },
            at,
        ))
    }

    fn exprs(&mut self, values: Vec<ast::Expr>) -> Result<Vec<Expr>, Error> {
        values.into_iter().map(|value| self.expr(value)).collect()
    }

    fn boxed(&mut self, value: ast::Expr) -> Result<Box<Expr>, Error> {
        Ok(Box::new(self.expr(value)?))
    }

    fn args(&mut self, args: Vec<ast::Arg>) -> Result<Vec<Arg>, Error> {
        args.into_iter()
            .map(|arg| {
                Ok(match arg {
                    ast::Arg::Arg(value) => Arg::Arg {
                        e: self.boxed(value)?,
                    },
                    ast::Arg::Spread(value) => Arg::Spread {
                        e: self.boxed(value)?,
                    },
                })
            })
            .collect()
    }

    fn prop_key(&mut self, key: ast::PropKey) -> Result<PropKey, Error> {
        Ok(match key {
            ast::PropKey::Name(name) => PropKey::Name {
                name: self.string(name),
            },
            ast::PropKey::Computed(value) => PropKey::Computed {
                e: self.boxed(*value)?,
            },
            ast::PropKey::Private(reference) => PropKey::Private {
                name: self.resolve_private(*reference)?,
            },
        })
    }

    fn expr(&mut self, value: ast::Expr) -> Result<Expr, Error> {
        use ast::ExprKind as E;
        let node_span = ast_span(value.span);
        let at = |position: u32| span(position, node_span.end);
        let (kind, span) = match value.kind {
            E::Number(value) => (
                ExprKind::Const {
                    value: self.number(value),
                },
                node_span,
            ),
            E::BigInt(value) => {
                let digits = value
                    .to_string_radix(10)
                    .map_err(|error| Error::internal(error.to_string()))?;
                let digits = self.string(JsString::try_from_utf8(&digits)?);
                (
                    ExprKind::Const {
                        value: self.push_const(Const::Bigint { digits }),
                    },
                    node_span,
                )
            }
            E::String(value) => (
                ExprKind::Str {
                    value: self.string(value),
                },
                node_span,
            ),
            E::Template { quasis, exprs } => (
                ExprKind::Template {
                    quasis: quasis.into_iter().map(|quasi| self.string(quasi)).collect(),
                    exprs: self.exprs(exprs)?,
                },
                node_span,
            ),
            E::TaggedTemplate {
                tag,
                cooked,
                raw,
                exprs,
                pos,
            } => {
                let site = self.template_sites;
                self.template_sites += 1;
                (
                    ExprKind::TaggedTemplate {
                        tag: self.boxed(*tag)?,
                        site,
                        cooked: Box::new(
                            cooked
                                .into_iter()
                                .map(|part| part.map(|part| self.string(part)))
                                .collect(),
                        ),
                        raw: Box::new(raw.into_iter().map(|part| self.string(part)).collect()),
                        exprs: self.exprs(exprs)?,
                    },
                    at(pos),
                )
            }
            E::RegExp(literal) => {
                let source = self.string(literal.pattern);
                let flags = self.string(literal.flags);
                let constant = self.push_const(Const::Regexp { source, flags });
                self.regexps.insert(constant.0, literal.program);
                (ExprKind::Const { value: constant }, node_span)
            }
            E::Null => (ExprKind::Null, node_span),
            E::Bool(value) => (ExprKind::Bool { value }, node_span),
            E::This => (
                ExprKind::This {
                    decl: self.implicit(DeclKind::This, node_span.start),
                },
                node_span,
            ),
            E::Ident(reference) => (
                ExprKind::Decl {
                    decl: self.resolve(reference),
                },
                node_span,
            ),
            E::Array(items) => {
                let mut hir_items = Vec::with_capacity(items.len());
                for item in items {
                    hir_items.push(match item {
                        ast::ArrayItem::Item(value) => ArrayItem::Item {
                            e: self.boxed(value)?,
                        },
                        ast::ArrayItem::Spread(value, _) => ArrayItem::Spread {
                            e: self.boxed(value)?,
                        },
                        ast::ArrayItem::Hole => ArrayItem::Hole,
                    });
                }
                (ExprKind::Array { items: hir_items }, node_span)
            }
            E::Object(props) => (
                ExprKind::Object {
                    props: self.object(props)?,
                },
                node_span,
            ),
            E::Function(function) => (
                ExprKind::Fn {
                    fn_: self.function(*function)?,
                },
                node_span,
            ),
            E::Class(class) => (
                ExprKind::Class {
                    cls: Box::new(self.class(*class)?),
                },
                node_span,
            ),
            E::Member {
                obj,
                prop,
                optional,
                pos,
            } => (
                ExprKind::Member {
                    obj: self.boxed(*obj)?,
                    prop: Box::new(self.prop_key(prop)?),
                    optional,
                    layout: None,
                    field: None,
                },
                at(pos),
            ),
            E::SuperMember { prop, pos } => {
                let this = self.implicit(DeclKind::This, pos);
                let home = self.implicit(DeclKind::HomeObject, pos);
                (
                    ExprKind::SuperMember {
                        prop: Box::new(self.prop_key(prop)?),
                        this,
                        home,
                        super_at: node_span.start,
                    },
                    at(pos),
                )
            }
            E::Call {
                callee,
                args,
                optional,
                direct_eval,
                pos,
            } => {
                if direct_eval {
                    let current = self.current();
                    self.facts[current.index()].has_direct_eval = true;
                }
                (
                    ExprKind::Call {
                        callee: self.boxed(*callee)?,
                        args: self.args(args)?,
                        optional,
                        direct: None,
                        intrinsic: None,
                        direct_eval,
                    },
                    at(pos),
                )
            }
            E::New { callee, args, pos } => (
                ExprKind::New {
                    callee: self.boxed(*callee)?,
                    args: self.args(args)?,
                    direct: None,
                },
                at(pos),
            ),
            E::SuperCall { args, pos } => {
                let this = self.implicit(DeclKind::This, pos);
                let new_target = self.implicit(DeclKind::NewTarget, pos);
                let func = self.implicit(DeclKind::ActiveFunction, pos);
                (
                    ExprKind::SuperCall {
                        args: self.args(args)?,
                        this,
                        new_target,
                        func,
                        super_at: node_span.start,
                    },
                    at(pos),
                )
            }
            E::Import { spec, options, pos } => (
                ExprKind::Import {
                    spec: self.boxed(*spec)?,
                    options: match options {
                        Some(options) => Some(self.boxed(*options)?),
                        None => None,
                    },
                },
                at(pos),
            ),
            E::ImportMeta => {
                let decl = match self.import_meta {
                    Some(decl) => decl,
                    None => {
                        let name = self.string(JsString::from_static("import.meta"));
                        let decl = self.new_decl(name, DeclKind::Implicit, FuncId(0), span(0, 0));
                        self.import_meta = Some(decl);
                        decl
                    }
                };
                (
                    ExprKind::ImportMeta {
                        decl: self.use_decl(decl, node_span.start),
                    },
                    node_span,
                )
            }
            E::NewTarget => (
                ExprKind::NewTarget {
                    decl: self.implicit(DeclKind::NewTarget, node_span.start),
                },
                node_span,
            ),
            E::OptionalChain(inner) => (
                ExprKind::OptionalChain {
                    e: self.boxed(*inner)?,
                },
                node_span,
            ),
            E::Paren(inner) => return self.expr(*inner),
            E::Unary {
                op,
                expr: operand,
                pos,
            } => {
                let op = match op {
                    ast::UnaryOp::Neg => UnaryOp::Neg,
                    ast::UnaryOp::Plus => UnaryOp::Plus,
                    ast::UnaryOp::Not => UnaryOp::Not,
                    ast::UnaryOp::BitNot => UnaryOp::BitNot,
                    ast::UnaryOp::TypeOf => UnaryOp::TypeOf,
                    ast::UnaryOp::Void => UnaryOp::Void,
                    ast::UnaryOp::Delete => UnaryOp::Delete,
                };
                (
                    ExprKind::Unary {
                        op,
                        e: self.boxed(*operand)?,
                    },
                    at(pos),
                )
            }
            E::Update {
                increment,
                prefix,
                target,
                pos,
            } => (
                ExprKind::Update {
                    op: if increment {
                        UpdateOp::Inc
                    } else {
                        UpdateOp::Dec
                    },
                    prefix,
                    target: Box::new(self.assignment_target(*target)?),
                },
                at(pos),
            ),
            E::Binary {
                op,
                left,
                right,
                pos,
            } => (
                ExprKind::Binary {
                    op: binary_op(op),
                    l: self.boxed(*left)?,
                    r: self.boxed(*right)?,
                    numeric: None,
                },
                at(pos),
            ),
            E::Logical { op, left, right } => (
                ExprKind::Logical {
                    op: logical_op(op),
                    l: self.boxed(*left)?,
                    r: self.boxed(*right)?,
                },
                node_span,
            ),
            E::Conditional {
                test,
                then,
                otherwise,
            } => (
                ExprKind::Cond {
                    test: self.boxed(*test)?,
                    then: self.boxed(*then)?,
                    else_: self.boxed(*otherwise)?,
                },
                node_span,
            ),
            E::Assign {
                op,
                target,
                value,
                pos,
            } => {
                // NamedEvaluation applies to `=` and logical assignment of a
                // plain (unparenthesized) identifier.
                let name = match (&op, &target.kind) {
                    (
                        ast::AssignOp::Assign | ast::AssignOp::Logical(_),
                        ast::ExprKind::Ident(reference),
                    ) => Some(reference.name),
                    _ => None,
                };
                let target = self.assignment_target(*target)?;
                let value = self.named_value(*value, name)?;
                let at_span = match op {
                    ast::AssignOp::Binary(_) => at(pos),
                    _ => node_span,
                };
                (
                    ExprKind::Assign {
                        op: assign_op(op),
                        target: Box::new(target),
                        value: Box::new(value),
                    },
                    at_span,
                )
            }
            E::AssignPattern { pattern, value } => {
                let pattern_span = ast_span(pattern.span);
                let pattern = self.pattern(*pattern, None)?;
                (
                    ExprKind::Assign {
                        op: AssignOp::Assign,
                        target: Box::new(Target {
                            kind: TargetKind::Pattern {
                                p: Box::new(pattern),
                            },
                            span: pattern_span,
                        }),
                        value: self.boxed(*value)?,
                    },
                    node_span,
                )
            }
            E::Sequence(values) => (
                ExprKind::Seq {
                    exprs: self.exprs(values)?,
                },
                node_span,
            ),
            E::Yield { arg, delegate, pos } => (
                ExprKind::Yield {
                    e: match arg {
                        Some(arg) => Some(self.boxed(*arg)?),
                        None => None,
                    },
                    delegate,
                },
                at(pos),
            ),
            E::Await { arg, pos } => (
                ExprKind::Await {
                    e: self.boxed(*arg)?,
                },
                at(pos),
            ),
            E::PrivateIn { name, obj, pos } => (
                ExprKind::PrivateIn {
                    name: self.resolve_private(name)?,
                    obj: self.boxed(*obj)?,
                },
                at(pos),
            ),
        };
        Ok(expr(kind, span))
    }

    fn object(&mut self, props: Vec<ast::ObjProp>) -> Result<Vec<ObjProp>, Error> {
        let mut out = Vec::with_capacity(props.len());
        for prop in props {
            out.push(match prop {
                ast::ObjProp::Init {
                    key,
                    value,
                    key_pos: _,
                } => {
                    let value = match &key {
                        ast::PropKey::Name(name) => self.named_value_string(value, name.clone())?,
                        _ => self.expr(value)?,
                    };
                    ObjProp::Init {
                        key: Box::new(self.prop_key(key)?),
                        value: Box::new(value),
                        shorthand: false,
                    }
                }
                ast::ObjProp::Shorthand { key, value } => {
                    let at = ast_span(value.span);
                    let decl = self.resolve(value);
                    ObjProp::Init {
                        key: Box::new(PropKey::Name {
                            name: self.string(key),
                        }),
                        value: Box::new(expr(ExprKind::Decl { decl }, at)),
                        shorthand: true,
                    }
                }
                ast::ObjProp::Method { key, function } => {
                    let role = method_role(function.role);
                    let key = self.prop_key(key)?;
                    ObjProp::Method {
                        key: Box::new(key),
                        fn_: self.function(*function)?,
                        role,
                    }
                }
                ast::ObjProp::Spread { value, pos: _ } => ObjProp::Spread {
                    e: self.boxed(value)?,
                },
                ast::ObjProp::Proto { value, pos: _ } => ObjProp::Proto {
                    value: self.boxed(value)?,
                },
            });
        }
        Ok(out)
    }

    /// The reference an assignment writes: an identifier, member or super
    /// member, through parentheses.
    fn assignment_target(&mut self, target: ast::Expr) -> Result<Target, Error> {
        use ast::ExprKind as E;
        let node_span = ast_span(target.span);
        let at = |position: u32| span(position, node_span.end);
        let (kind, span) = match target.kind {
            E::Ident(reference) => {
                let decl = self.resolve(reference);
                self.mark_assigned(decl);
                (TargetKind::Decl { decl }, node_span)
            }
            E::Member { obj, prop, pos, .. } => (
                TargetKind::Member {
                    obj: self.boxed(*obj)?,
                    prop: Box::new(self.prop_key(prop)?),
                    layout: None,
                    field: None,
                },
                at(pos),
            ),
            E::SuperMember { prop, pos } => {
                let this = self.implicit(DeclKind::This, pos);
                let home = self.implicit(DeclKind::HomeObject, pos);
                (
                    TargetKind::SuperMember {
                        prop: Box::new(self.prop_key(prop)?),
                        this,
                        home,
                        super_at: node_span.start,
                    },
                    at(pos),
                )
            }
            E::Paren(inner) => return self.assignment_target(*inner),
            _ => {
                return Err(Error::internal("an assignment target is not a reference"));
            }
        };
        Ok(Target { kind, span })
    }

    fn target(&mut self, target: ast::Expr) -> Result<Target, Error> {
        self.assignment_target(target)
    }

    // ---- patterns ----

    /// A pattern; `declaring` for a declaration's bindings.
    fn pattern(
        &mut self,
        pattern: ast::Pattern,
        declaring: Option<DeclaringKind>,
    ) -> Result<Pattern, Error> {
        let at = ast_span(pattern.span);
        let kind = match pattern.kind {
            ast::PatternKind::Binding(reference) => {
                let decl = if declaring.is_some() {
                    self.resolve_declaration(reference)
                } else {
                    let decl = self.resolve(reference);
                    self.mark_assigned(decl);
                    decl
                };
                PatternKind::Decl { decl }
            }
            ast::PatternKind::Target(target) => PatternKind::Target {
                t: Box::new(self.assignment_target(*target)?),
            },
            ast::PatternKind::Object {
                props,
                rest,
                rest_pos,
            } => {
                let mut hir_props = Vec::with_capacity(props.len());
                for prop in props {
                    let key_at = prop.key_pos;
                    let inferred = leaf_name(&prop.value);
                    let key = self.prop_key(prop.key)?;
                    let value = self.pattern(prop.value, declaring)?;
                    let default = match prop.default {
                        Some(default) => Some(Box::new(self.named_value(default, inferred)?)),
                        None => None,
                    };
                    hir_props.push(PatternProp {
                        key: Box::new(key),
                        value: Box::new(value),
                        default,
                        key_at,
                    });
                }
                PatternKind::Object {
                    props: hir_props,
                    rest: match rest {
                        Some(rest) => Some(Box::new(self.pattern(*rest, declaring)?)),
                        None => None,
                    },
                    rest_at: rest_pos,
                }
            }
            ast::PatternKind::Array { items, rest } => {
                let mut hir_items = Vec::with_capacity(items.len());
                for item in items {
                    hir_items.push(match item {
                        Some(item) => {
                            let inferred = leaf_name(&item.pattern);
                            let p = self.pattern(item.pattern, declaring)?;
                            let default = match item.default {
                                Some(default) => {
                                    Some(Box::new(self.named_value(default, inferred)?))
                                }
                                None => None,
                            };
                            PatternItem::Item {
                                p: Box::new(p),
                                default,
                            }
                        }
                        None => PatternItem::Hole,
                    });
                }
                PatternKind::Array {
                    items: hir_items,
                    rest: match rest {
                        Some(rest) => Some(Box::new(self.pattern(*rest, declaring)?)),
                        None => None,
                    },
                }
            }
        };
        Ok(Pattern {
            kind,
            ty: TypeRef(0),
            span: at,
        })
    }

    // ---- modules ----

    fn module(&mut self, record: ast::ModuleRecord, top: FunctionRef) -> Result<Module, Error> {
        let mut requests = Vec::with_capacity(record.requests.len());
        for request in record.requests {
            let has_attributes = request.attributes.is_some();
            let attributes = request
                .attributes
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| ImportAttribute {
                    key: self.string(key),
                    value: self.string(value),
                })
                .collect();
            requests.push(ModuleRequest {
                specifier: self.string(request.specifier),
                has_attributes,
                attributes,
            });
        }
        let imports = record
            .imports
            .into_iter()
            .map(|import| Import {
                request: import.request,
                name: match import.name {
                    ast::ImportName::Name(name) => Some(self.string(name)),
                    ast::ImportName::Namespace => None,
                },
                decl: DeclRef(import.decl.0),
            })
            .collect();
        let root = &self.ast_functions[0];
        let (var_scope, body_scope) = (root.var_scope, root.body_scope);
        let mut exports = Vec::with_capacity(record.local_exports.len());
        for export in record.local_exports {
            let decl = self
                .lookup_in_scope(body_scope, export.local)
                .or_else(|| self.lookup_in_scope(var_scope, export.local))
                .ok_or_else(|| Error::internal("a module export lost its binding"))?;
            exports.push(schema::Export {
                name: self.string(export.export),
                decl: DeclRef(decl.0),
            });
        }
        let reexports = record
            .indirect_exports
            .into_iter()
            .map(|export| Reexport {
                request: export.request,
                name: match export.import {
                    ast::ImportName::Name(name) => Some(self.string(name)),
                    ast::ImportName::Namespace => None,
                },
                as_: self.string(export.export),
            })
            .collect();
        Ok(Module {
            file: FileRef(0),
            top,
            requests,
            imports,
            exports,
            reexports,
            star_exports: record.star_exports,
            dynamic: false,
            uses_import_meta: record.uses_import_meta,
            has_top_level_await: record.has_top_level_await,
        })
    }
}

fn pos(span: impl Into<ast::Span>) -> u32 {
    span.into().start
}

fn pos_end(span: impl Into<ast::Span>) -> u32 {
    span.into().end
}

fn span_of(value: &ast::Expr) -> Span {
    ast_span(value.span)
}

/// Functions with their own `arguments` object.
fn has_arguments(kind: ast::FunctionKind) -> bool {
    matches!(
        kind,
        ast::FunctionKind::Normal
            | ast::FunctionKind::Method
            | ast::FunctionKind::ClassConstructor { .. }
    )
}

fn method_role(role: ast::MethodRole) -> MethodRole {
    match role {
        ast::MethodRole::Method => MethodRole::Method,
        ast::MethodRole::Get => MethodRole::Get,
        ast::MethodRole::Set => MethodRole::Set,
    }
}

fn binary_op(op: ast::BinaryOp) -> BinOp {
    use ast::BinaryOp as B;
    match op {
        B::Add => BinOp::Add,
        B::Sub => BinOp::Sub,
        B::Mul => BinOp::Mul,
        B::Div => BinOp::Div,
        B::Rem => BinOp::Rem,
        B::Exp => BinOp::Exp,
        B::Shl => BinOp::Shl,
        B::Sar => BinOp::Sar,
        B::Shr => BinOp::Shr,
        B::BitAnd => BinOp::BitAnd,
        B::BitOr => BinOp::BitOr,
        B::BitXor => BinOp::BitXor,
        B::Eq => BinOp::Eq,
        B::Ne => BinOp::Ne,
        B::StrictEq => BinOp::StrictEq,
        B::StrictNe => BinOp::StrictNe,
        B::Lt => BinOp::Lt,
        B::Le => BinOp::Le,
        B::Gt => BinOp::Gt,
        B::Ge => BinOp::Ge,
        B::In => BinOp::In,
        B::InstanceOf => BinOp::InstanceOf,
    }
}

fn logical_op(op: ast::LogicalOp) -> LogicalOp {
    match op {
        ast::LogicalOp::And => LogicalOp::And,
        ast::LogicalOp::Or => LogicalOp::Or,
        ast::LogicalOp::Nullish => LogicalOp::Nullish,
    }
}

fn assign_op(op: ast::AssignOp) -> AssignOp {
    match op {
        ast::AssignOp::Assign => AssignOp::Assign,
        ast::AssignOp::Logical(ast::LogicalOp::And) => AssignOp::And,
        ast::AssignOp::Logical(ast::LogicalOp::Or) => AssignOp::Or,
        ast::AssignOp::Logical(ast::LogicalOp::Nullish) => AssignOp::Nullish,
        ast::AssignOp::Binary(op) => match binary_op(op) {
            BinOp::Add => AssignOp::Add,
            BinOp::Sub => AssignOp::Sub,
            BinOp::Mul => AssignOp::Mul,
            BinOp::Div => AssignOp::Div,
            BinOp::Rem => AssignOp::Rem,
            BinOp::Exp => AssignOp::Exp,
            BinOp::Shl => AssignOp::Shl,
            BinOp::Sar => AssignOp::Sar,
            BinOp::Shr => AssignOp::Shr,
            BinOp::BitAnd => AssignOp::BitAnd,
            BinOp::BitOr => AssignOp::BitOr,
            _ => AssignOp::BitXor,
        },
    }
}

/// An anonymous function, arrow or class definition, through parentheses.
fn is_anonymous_definition(value: &ast::Expr) -> bool {
    match &value.kind {
        ast::ExprKind::Function(function) => function.name.is_none(),
        ast::ExprKind::Class(class) => class.name.is_none(),
        ast::ExprKind::Paren(inner) => is_anonymous_definition(inner),
        _ => false,
    }
}

/// The name an identifier parameter binds, for its default's naming.
fn binding_name(pattern: &ast::Pattern) -> Option<NameId> {
    match &pattern.kind {
        ast::PatternKind::Binding(reference) => Some(reference.name),
        _ => None,
    }
}

/// The name a pattern leaf assigns, for its default's naming.
fn leaf_name(pattern: &ast::Pattern) -> Option<NameId> {
    match &pattern.kind {
        ast::PatternKind::Binding(reference) => Some(reference.name),
        ast::PatternKind::Target(target) => match &target.kind {
            ast::ExprKind::Ident(reference) => Some(reference.name),
            _ => None,
        },
        _ => None,
    }
}

fn pattern_decls(pattern: &Pattern, out: &mut Vec<DeclRef>) {
    match &pattern.kind {
        PatternKind::Decl { decl } => out.push(*decl),
        PatternKind::Target { .. } => {}
        PatternKind::Object { props, rest, .. } => {
            for prop in props {
                pattern_decls(&prop.value, out);
            }
            if let Some(rest) = rest {
                pattern_decls(rest, out);
            }
        }
        PatternKind::Array { items, rest } => {
            for item in items {
                if let PatternItem::Item { p, .. } = item {
                    pattern_decls(p, out);
                }
            }
            if let Some(rest) = rest {
                pattern_decls(rest, out);
            }
        }
    }
}
