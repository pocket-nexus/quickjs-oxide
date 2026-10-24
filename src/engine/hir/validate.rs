//! The HIR validator: the boundary at which the middle end trusts the front
//! end. A failure is a front-end bug, reported with the offending table
//! index, never a user diagnostic.
//!
//! Reference closure: every table reference is in range, every function is
//! defined by exactly one node (or is the entry), every declaration's owner
//! lists it, captures run only from inner functions, and every `break` and
//! `continue` names an enclosing statement of its function. Semantic
//! consistency: a function's captures are the outer declarations its
//! subtree uses, `const` is never `assigned`, `yield`, `await`, `return` and
//! `super()` appear only where the language allows them. M1's type fields:
//! every one holds its default.

use super::schema::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValidateError(pub(crate) String);

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid HIR: {}", self.0)
    }
}

type Result<T> = std::result::Result<T, ValidateError>;

fn fail<T>(message: String) -> Result<T> {
    Err(ValidateError(message))
}

/// Checks `program`.
///
/// # Errors
/// Returns the first broken rule.
pub(crate) fn validate(program: &Program) -> Result<()> {
    if !program.types.is_empty() || !program.layouts.is_empty() || !program.descriptors.is_empty() {
        return fail("M1 HIR has type, layout or descriptor entries".into());
    }
    let mut validator = Validator {
        program,
        defined: vec![0; program.functions.len()],
        uses: vec![BTreeSet::new(); program.functions.len()],
        function: FunctionRef(0),
        labels: Vec::new(),
    };
    let top = match &*program.entry {
        Entry::Script { top } | Entry::Eval { top } => *top,
        Entry::Module { module } => program
            .modules
            .get(module.index())
            .map(|module| module.top)
            .ok_or_else(|| ValidateError(format!("entry names missing module #{}", module.0)))?,
    };
    validator.function_ref(top)?;
    validator.defined[top.index()] += 1;
    for (index, decl) in program.decls.iter().enumerate() {
        validator.string(decl.name)?;
        validator.function_ref(decl.owner)?;
        if !program.functions[decl.owner.index()]
            .decls
            .contains(&DeclRef(index as u32))
        {
            return fail(format!(
                "decl #{index} is not listed by its owner fn #{}",
                decl.owner.0
            ));
        }
        if decl.ty.0 != 0 {
            return fail(format!("decl #{index} has a type"));
        }
        if decl.kind == DeclKind::Const && decl.assigned {
            return fail(format!("const decl #{index} is assigned"));
        }
        for &function in &decl.captured_by {
            validator.function_ref(function)?;
            if !validator.is_inner(function, decl.owner) {
                return fail(format!(
                    "decl #{index} is captured by fn #{}, not inside its owner",
                    function.0
                ));
            }
        }
    }
    for (index, function) in program.functions.iter().enumerate() {
        validator.function = FunctionRef(index as u32);
        validator.labels.clear();
        if function.sig.is_some() || function.effects.is_some() {
            return fail(format!("fn #{index} has a signature or effects"));
        }
        for &inner in &function.inner {
            validator.function_ref(inner)?;
        }
        for &decl in function.decls.iter().chain(&function.captures) {
            validator.decl(decl)?;
        }
        for param in &function.params {
            validator.pattern(&param.pattern)?;
            if let Some(default) = &param.default {
                validator.expr(default)?;
            }
        }
        if let Some(rest) = &function.rest {
            validator.pattern(rest)?;
        }
        validator.block(&function.body)?;
    }
    for (index, &count) in validator.defined.iter().enumerate() {
        if count != 1 {
            return fail(format!("fn #{index} is defined {count} times"));
        }
    }
    // A function's captures are the outer declarations its own body and its
    // inner functions use.
    for (index, function) in program.functions.iter().enumerate() {
        let mut expected = validator.uses[index].clone();
        let mut pending = function.inner.clone();
        while let Some(inner) = pending.pop() {
            expected.extend(validator.uses[inner.index()].iter().copied());
            pending.extend(program.functions[inner.index()].inner.iter().copied());
        }
        let expected: BTreeSet<DeclRef> = expected
            .into_iter()
            .filter(|decl| {
                let decl = &program.decls[decl.index()];
                decl.owner.index() != index
                    && decl.kind != DeclKind::Global
                    && !validator.is_inner(decl.owner, FunctionRef(index as u32))
            })
            .collect();
        let actual: BTreeSet<DeclRef> = function.captures.iter().copied().collect();
        if expected != actual {
            return fail(format!(
                "fn #{index} captures {:?} but uses {:?}",
                actual.iter().map(|d| d.0).collect::<Vec<_>>(),
                expected.iter().map(|d| d.0).collect::<Vec<_>>()
            ));
        }
    }
    Ok(())
}

struct Validator<'a> {
    program: &'a Program,
    defined: Vec<u32>,
    /// Declarations each function's own body uses.
    uses: Vec<BTreeSet<DeclRef>>,
    function: FunctionRef,
    /// The labels of the statements enclosing the current one, and whether
    /// each is a loop.
    labels: Vec<(u32, bool)>,
}

impl Validator<'_> {
    fn current(&self) -> &Function {
        &self.program.functions[self.function.index()]
    }

    fn string(&self, string: StringRef) -> Result<()> {
        if string.index() >= self.program.strings.len() {
            return fail(format!("string #{} is out of range", string.0));
        }
        Ok(())
    }

    fn function_ref(&self, function: FunctionRef) -> Result<()> {
        if function.index() >= self.program.functions.len() {
            return fail(format!("fn #{} is out of range", function.0));
        }
        Ok(())
    }

    fn define(&mut self, function: FunctionRef) -> Result<()> {
        self.function_ref(function)?;
        self.defined[function.index()] += 1;
        if !self.current().inner.contains(&function) {
            return fail(format!(
                "fn #{} defines fn #{} without listing it as inner",
                self.function.0, function.0
            ));
        }
        Ok(())
    }

    fn decl(&self, decl: DeclRef) -> Result<()> {
        if decl.index() >= self.program.decls.len() {
            return fail(format!("decl #{} is out of range", decl.0));
        }
        Ok(())
    }

    fn use_decl(&mut self, decl: DeclRef) -> Result<()> {
        self.decl(decl)?;
        self.uses[self.function.index()].insert(decl);
        Ok(())
    }

    /// Whether `inner` is nested (at any depth) in `outer`.
    fn is_inner(&self, inner: FunctionRef, outer: FunctionRef) -> bool {
        let mut pending = self.program.functions[outer.index()].inner.clone();
        while let Some(function) = pending.pop() {
            if function == inner {
                return true;
            }
            pending.extend(
                self.program.functions[function.index()]
                    .inner
                    .iter()
                    .copied(),
            );
        }
        false
    }

    fn block(&mut self, block: &Block) -> Result<()> {
        for &decl in &block.decls {
            self.decl(decl)?;
        }
        for statement in &block.stmts {
            self.stmt(statement)?;
        }
        Ok(())
    }

    fn labeled<T>(
        &mut self,
        label: u32,
        is_loop: bool,
        f: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.labels.push((label, is_loop));
        let result = f(self);
        self.labels.pop();
        result
    }

    fn stmt(&mut self, statement: &Stmt) -> Result<()> {
        match &statement.kind {
            StmtKind::Expr { e } | StmtKind::Throw { e } => self.expr(e),
            StmtKind::Declare { decl, init } => {
                self.use_decl(*decl)?;
                init.as_deref().map_or(Ok(()), |init| self.expr(init))
            }
            StmtKind::Destructure { pattern, init, .. } => {
                self.pattern(pattern)?;
                self.expr(init)
            }
            StmtKind::Block { body } => self.block(body),
            StmtKind::If { test, then, else_ } => {
                self.expr(test)?;
                self.stmt(then)?;
                else_.as_deref().map_or(Ok(()), |s| self.stmt(s))
            }
            StmtKind::While { test, body, label } => {
                self.expr(test)?;
                self.labeled(*label, true, |v| v.stmt(body))
            }
            StmtKind::DoWhile { body, test, label } => {
                self.labeled(*label, true, |v| v.stmt(body))?;
                self.expr(test)
            }
            StmtKind::For {
                init,
                test,
                update,
                body,
                label,
                per_iteration,
            } => {
                for &decl in per_iteration {
                    self.decl(decl)?;
                    if !matches!(
                        self.program.decls[decl.index()].kind,
                        DeclKind::Let | DeclKind::Const
                    ) {
                        return fail(format!("for per-iteration decl #{} is not lexical", decl.0));
                    }
                }
                if let Some(init) = init {
                    self.stmt(init)?;
                }
                self.labeled(*label, true, |v| {
                    if let Some(test) = test {
                        v.expr(test)?;
                    }
                    if let Some(update) = update {
                        v.expr(update)?;
                    }
                    v.stmt(body)
                })
            }
            StmtKind::ForIn {
                target,
                init,
                obj,
                body,
                label,
            } => {
                self.for_target(target)?;
                if let Some(init) = init {
                    self.expr(init)?;
                }
                self.expr(obj)?;
                self.labeled(*label, true, |v| v.stmt(body))
            }
            StmtKind::ForOf {
                target,
                iter,
                body,
                label,
                await_,
                array_fast,
            } => {
                if *array_fast {
                    return fail("M1 HIR has a fast for-of".into());
                }
                if *await_ && !self.current().async_ {
                    return fail(format!(
                        "for await outside an async fn #{}",
                        self.function.0
                    ));
                }
                self.for_target(target)?;
                self.expr(iter)?;
                self.labeled(*label, true, |v| v.stmt(body))
            }
            StmtKind::Switch {
                disc,
                cases,
                label,
                exhaustive,
            } => {
                if *exhaustive {
                    return fail("M1 HIR has an exhaustive switch".into());
                }
                self.expr(disc)?;
                self.labeled(*label, false, |v| {
                    for case in cases {
                        if let Some(test) = &case.test {
                            v.expr(test)?;
                        }
                        for statement in &case.body {
                            v.stmt(statement)?;
                        }
                    }
                    Ok(())
                })
            }
            StmtKind::Break { label } => {
                if !self.labels.iter().any(|(l, _)| l == label) {
                    return fail(format!("break to label {label} outside it"));
                }
                Ok(())
            }
            StmtKind::Continue { label } => {
                if !self
                    .labels
                    .iter()
                    .any(|(l, is_loop)| l == label && *is_loop)
                {
                    return fail(format!("continue to label {label}, not an enclosing loop"));
                }
                Ok(())
            }
            StmtKind::Return { e } => {
                if matches!(
                    self.current().kind,
                    FunctionKind::Script
                        | FunctionKind::Module
                        | FunctionKind::Eval
                        | FunctionKind::StaticBlock
                ) {
                    return fail(format!("return in fn #{}", self.function.0));
                }
                e.as_deref().map_or(Ok(()), |e| self.expr(e))
            }
            StmtKind::Try {
                body,
                catch_param,
                catch_body,
                finally,
                catch_default,
            } => {
                self.block(body)?;
                if let Some(param) = catch_param {
                    self.pattern(param)?;
                }
                if let Some(default) = catch_default {
                    self.expr(default)?;
                }
                if let Some(catch_body) = catch_body {
                    self.block(catch_body)?;
                }
                finally.as_ref().map_or(Ok(()), |f| self.block(f))
            }
            StmtKind::Labeled { label, body } => self.labeled(*label, false, |v| v.stmt(body)),
            StmtKind::FnDecl { decl, fn_, annex_b } => {
                self.use_decl(*decl)?;
                if let Some(annex_b) = annex_b {
                    self.use_decl(*annex_b)?;
                }
                self.define(*fn_)
            }
            StmtKind::ClassDecl { decl, cls } => {
                self.use_decl(*decl)?;
                self.class(cls)
            }
            StmtKind::Debugger | StmtKind::Empty => Ok(()),
            StmtKind::DefineField { key, value } => {
                if let FieldKey::Computed { decl } = &**key {
                    self.use_decl(*decl)?;
                }
                value.as_deref().map_or(Ok(()), |v| self.expr(v))
            }
            StmtKind::DefinePrivate { decl, value } => {
                self.use_decl(*decl)?;
                value.as_deref().map_or(Ok(()), |v| self.expr(v))
            }
            StmtKind::StaticBlock { fn_ } => self.define(*fn_),
        }
    }

    fn for_target(&mut self, target: &ForTarget) -> Result<()> {
        match target {
            ForTarget::Assign { t } => self.target(t),
            ForTarget::Declare { pattern } => self.pattern(pattern),
        }
    }

    fn class(&mut self, class: &Class) -> Result<()> {
        if class.layout.is_some() {
            return fail("M1 HIR has a class layout".into());
        }
        if let Some(name) = class.name {
            self.decl(name)?;
        }
        if let Some(extends) = &class.extends {
            self.expr(extends)?;
        }
        self.define(class.ctor)?;
        for member in &class.members {
            match member {
                ClassMember::Method { key, fn_, .. } => {
                    self.prop_key(key)?;
                    self.define(*fn_)?;
                }
                ClassMember::PrivateMethod { decl, fn_, .. } => {
                    self.use_decl(*decl)?;
                    self.define(*fn_)?;
                }
                ClassMember::FieldKey { key, decl, .. } => {
                    self.expr(key)?;
                    self.decl(*decl)?;
                }
                ClassMember::PrivateField { decl, .. } => self.decl(*decl)?,
            }
        }
        for init in [class.instance_init, class.static_init]
            .into_iter()
            .flatten()
        {
            self.define(init)?;
        }
        Ok(())
    }

    fn prop_key(&mut self, key: &PropKey) -> Result<()> {
        match key {
            PropKey::Name { name } => self.string(*name),
            PropKey::Computed { e } => self.expr(e),
            PropKey::Private { name } => self.use_decl(*name),
        }
    }

    fn target(&mut self, target: &Target) -> Result<()> {
        match &target.kind {
            TargetKind::Decl { decl } => self.use_decl(*decl),
            TargetKind::Member {
                obj,
                prop,
                layout,
                field,
            } => {
                if layout.is_some() || field.is_some() {
                    return fail("M1 HIR has a member layout".into());
                }
                self.expr(obj)?;
                self.prop_key(prop)
            }
            TargetKind::Pattern { p } => self.pattern(p),
            TargetKind::SuperMember { prop, this, home } => {
                self.use_decl(*this)?;
                self.use_decl(*home)?;
                self.prop_key(prop)
            }
        }
    }

    fn pattern(&mut self, pattern: &Pattern) -> Result<()> {
        if pattern.ty.0 != 0 {
            return fail("M1 HIR has a typed pattern".into());
        }
        match &pattern.kind {
            PatternKind::Decl { decl } => self.use_decl(*decl),
            PatternKind::Target { t } => self.target(t),
            PatternKind::Object { props, rest } => {
                for prop in props {
                    self.prop_key(&prop.key)?;
                    self.pattern(&prop.value)?;
                    if let Some(default) = &prop.default {
                        self.expr(default)?;
                    }
                }
                rest.as_deref().map_or(Ok(()), |rest| self.pattern(rest))
            }
            PatternKind::Array { items, rest } => {
                for item in items {
                    if let PatternItem::Item { p, default } = item {
                        self.pattern(p)?;
                        if let Some(default) = default {
                            self.expr(default)?;
                        }
                    }
                }
                rest.as_deref().map_or(Ok(()), |rest| self.pattern(rest))
            }
        }
    }

    fn args(&mut self, args: &[Arg]) -> Result<()> {
        for arg in args {
            match arg {
                Arg::Arg { e } | Arg::Spread { e } => self.expr(e)?,
            }
        }
        Ok(())
    }

    fn expr(&mut self, value: &Expr) -> Result<()> {
        if value.ty.0 != 0 {
            return fail("M1 HIR has a typed expression".into());
        }
        match &value.kind {
            ExprKind::Const { value } => {
                if value.index() >= self.program.consts.len() {
                    return fail(format!("const #{} is out of range", value.0));
                }
                Ok(())
            }
            ExprKind::Str { value } => self.string(*value),
            ExprKind::Bool { .. } | ExprKind::Null | ExprKind::Undefined | ExprKind::ImportMeta => {
                Ok(())
            }
            ExprKind::Template { quasis, exprs } => {
                for &quasi in quasis {
                    self.string(quasi)?;
                }
                exprs.iter().try_for_each(|e| self.expr(e))
            }
            ExprKind::TaggedTemplate { tag, exprs, .. } => {
                self.expr(tag)?;
                exprs.iter().try_for_each(|e| self.expr(e))
            }
            ExprKind::Array { items } => {
                for item in items {
                    match item {
                        ArrayItem::Item { e } | ArrayItem::Spread { e } => self.expr(e)?,
                        ArrayItem::Hole => {}
                    }
                }
                Ok(())
            }
            ExprKind::Object { props } => {
                for prop in props {
                    match prop {
                        ObjProp::Init { key, value, .. } => {
                            self.prop_key(key)?;
                            self.expr(value)?;
                        }
                        ObjProp::Method { key, fn_, .. } => {
                            self.prop_key(key)?;
                            self.define(*fn_)?;
                        }
                        ObjProp::Spread { e } => self.expr(e)?,
                        ObjProp::Proto { value } => self.expr(value)?,
                    }
                }
                Ok(())
            }
            ExprKind::Fn { fn_ } => self.define(*fn_),
            ExprKind::Class { cls } => self.class(cls),
            ExprKind::Decl { decl } | ExprKind::This { decl } | ExprKind::NewTarget { decl } => {
                self.use_decl(*decl)
            }
            ExprKind::SuperMember { prop, this, home } => {
                self.use_decl(*this)?;
                self.use_decl(*home)?;
                self.prop_key(prop)
            }
            ExprKind::Member {
                obj,
                prop,
                layout,
                field,
                ..
            } => {
                if layout.is_some() || field.is_some() {
                    return fail("M1 HIR has a member layout".into());
                }
                self.expr(obj)?;
                self.prop_key(prop)
            }
            ExprKind::Unary { e, .. } | ExprKind::Await { e } | ExprKind::OptionalChain { e } => {
                if matches!(&value.kind, ExprKind::Await { .. })
                    && !self.current().async_
                    && self.current().kind != FunctionKind::Module
                {
                    return fail(format!("await outside an async fn #{}", self.function.0));
                }
                self.expr(e)
            }
            ExprKind::Update { target, .. } => self.target(target),
            ExprKind::Binary { l, r, numeric, .. } => {
                if numeric.is_some() {
                    return fail("M1 HIR has a numeric binary".into());
                }
                self.expr(l)?;
                self.expr(r)
            }
            ExprKind::Logical { l, r, .. } => {
                self.expr(l)?;
                self.expr(r)
            }
            ExprKind::Cond { test, then, else_ } => {
                self.expr(test)?;
                self.expr(then)?;
                self.expr(else_)
            }
            ExprKind::Assign { target, value, .. } => {
                self.target(target)?;
                self.expr(value)
            }
            ExprKind::Seq { exprs } => exprs.iter().try_for_each(|e| self.expr(e)),
            ExprKind::Call {
                callee,
                args,
                direct,
                intrinsic,
                ..
            } => {
                if direct.is_some() || intrinsic.is_some() {
                    return fail("M1 HIR has a direct or intrinsic call".into());
                }
                self.expr(callee)?;
                self.args(args)
            }
            ExprKind::New {
                callee,
                args,
                direct,
            } => {
                if direct.is_some() {
                    return fail("M1 HIR has a direct construct".into());
                }
                self.expr(callee)?;
                self.args(args)
            }
            ExprKind::SuperCall {
                args,
                this,
                new_target,
                func,
            } => {
                for decl in [*this, *new_target, *func] {
                    self.use_decl(decl)?;
                }
                let owner = self.program.decls[func.index()].owner;
                if self.program.functions[owner.index()].kind != FunctionKind::DerivedConstructor {
                    return fail(format!(
                        "super() outside a derived constructor in fn #{}",
                        self.function.0
                    ));
                }
                self.args(args)
            }
            ExprKind::Import { spec, options } => {
                self.expr(spec)?;
                options.as_deref().map_or(Ok(()), |o| self.expr(o))
            }
            ExprKind::Yield { e, .. } => {
                if !self.current().generator {
                    return fail(format!("yield outside a generator fn #{}", self.function.0));
                }
                e.as_deref().map_or(Ok(()), |e| self.expr(e))
            }
            ExprKind::Decode { .. } | ExprKind::Scratch { .. } => {
                fail("M1 HIR has a decode or scratch node".into())
            }
            ExprKind::PrivateIn { name, obj } => {
                self.use_decl(*name)?;
                self.expr(obj)
            }
            ExprKind::Named { name, e } => {
                self.string(*name)?;
                self.expr(e)
            }
        }
    }
}
