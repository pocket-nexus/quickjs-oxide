//! Declaration registration with QuickJS's redeclaration early errors.
//!
//! Each rule is the stack-code parser's (`compiler::parser::declarations`),
//! with storage derived from the function kind instead of allocated: a
//! script's `var`s and top-level lexicals are global, a module's root
//! bindings are module bindings, sloppy (indirect) eval code declares
//! globals and strict eval code locals.

use super::{FunctionKind, Parser};
use crate::engine::api::error::Error;
use crate::engine::ast::{Decl, DeclId, DeclKind, ScopeId, ScopeKind, Storage};
use crate::engine::compiler::lexer::Span;
use crate::engine::compiler::names::NameId;

const LEXICAL_REDEFINITION: &str = "invalid redefinition of lexical identifier";

impl<'source> Parser<'source> {
    pub(super) fn add_decl(
        &mut self,
        scope: ScopeId,
        written_in: ScopeId,
        name: NameId,
        kind: DeclKind,
        storage: Storage,
        span: Span,
    ) -> DeclId {
        let id = DeclId(self.decls.len() as u32);
        self.decls.push(Decl {
            name,
            kind,
            storage,
            scope,
            written_in,
            span,
        });
        self.scopes[scope.index()].decls.push(id);
        id
    }

    fn var_storage(&self) -> Storage {
        match self.state().kind {
            FunctionKind::Script => Storage::Global,
            FunctionKind::Module => Storage::Module,
            FunctionKind::Eval if !self.strict() => Storage::Global,
            _ => Storage::Local,
        }
    }

    fn is_program_body(&self, scope: ScopeId, kind: FunctionKind) -> bool {
        let state = self.state();
        state.kind == kind && scope == state.body_scope
    }

    /// The module's root declarations of `name`: its var scope and body.
    fn module_root_decls(&self, name: NameId) -> impl Iterator<Item = &Decl> + '_ {
        let state = self.state();
        let (var, body) = (state.var_scope, state.body_scope);
        [var, body]
            .into_iter()
            .flat_map(move |scope| self.scopes[scope.index()].decls.iter())
            .map(|decl| &self.decls[decl.index()])
            .filter(move |decl| decl.name == name)
    }

    /// A `var` (or a sloppy-mode function declaration hoisted like one).
    pub(super) fn register_var(
        &mut self,
        name: NameId,
        declaration_span: Span,
        conflict_span: Span,
    ) -> Result<DeclId, Error> {
        let current = self.current_scope();
        let (kind, var_scope, body_scope) = {
            let state = self.state();
            (state.kind, state.var_scope, state.body_scope)
        };
        if let Some((scope, decl)) = self.decl_from_scope(current, name) {
            let found = &self.decls[decl.index()];
            if found.kind.is_lexical() {
                let catch_parameter = found.kind == DeclKind::CatchParam;
                let masked = kind == FunctionKind::Script
                    && scope == body_scope
                    && self.state().first_global_declaration_is_normal(name);
                let import_only = kind == FunctionKind::Module && found.kind == DeclKind::Import;
                if !catch_parameter && !masked && !import_only {
                    return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
                }
            }
        }
        if matches!(kind, FunctionKind::Script) || (kind == FunctionKind::Eval && !self.strict()) {
            self.state_mut().global_declarations.push((name, false));
        }
        if let Some(existing) = self.decl_in_scope(var_scope, name) {
            return Ok(existing);
        }
        let storage = self.var_storage();
        Ok(self.add_decl(
            var_scope,
            current,
            name,
            DeclKind::Var,
            storage,
            declaration_span,
        ))
    }

    /// A `let`, `const`, `class`, catch parameter or block-scoped function.
    pub(super) fn register_lexical(
        &mut self,
        name: NameId,
        declaration_span: Span,
        conflict_span: Span,
        kind: DeclKind,
        allow_body_parameter_shadow: bool,
    ) -> Result<DeclId, Error> {
        let scope = self.current_scope();
        let module_body = self.is_program_body(scope, FunctionKind::Module);
        let result = self.register_lexical_in_scope(
            name,
            declaration_span,
            conflict_span,
            kind,
            allow_body_parameter_shadow,
        );
        // QuickJS keeps module-body lexicals in its global variable table.
        match result {
            Err(error) if module_body && error.message() == LEXICAL_REDEFINITION => {
                Err(Error::syntax(
                    "invalid redefinition of global identifier",
                    error.span().unwrap_or(conflict_span),
                ))
            }
            result => result,
        }
    }

    fn register_lexical_in_scope(
        &mut self,
        name: NameId,
        declaration_span: Span,
        conflict_span: Span,
        kind: DeclKind,
        allow_body_parameter_shadow: bool,
    ) -> Result<DeclId, Error> {
        let scope = self.current_scope();
        let is_global = self.is_program_body(scope, FunctionKind::Script);
        let is_eval_body = self.is_program_body(scope, FunctionKind::Eval);
        if self.is_program_body(scope, FunctionKind::Module) {
            if self
                .module_root_decls(name)
                .any(|decl| decl.kind != DeclKind::Import)
            {
                return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
            }
            return Ok(self.add_decl(scope, scope, name, kind, Storage::Module, declaration_span));
        }
        let (var_scope, body_scope) = {
            let state = self.state();
            (state.var_scope, state.body_scope)
        };
        if is_eval_body
            && self
                .state()
                .global_declarations
                .iter()
                .any(|(n, lexical)| *n == name && !lexical)
        {
            return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
        }
        // A lexical declaration directly in a catch block may not shadow the
        // catch parameter.
        if let Some(parent) = self.scopes[scope.index()].parent
            && self.scopes[parent.index()].kind == ScopeKind::Catch
            && self
                .decl_in_scope(parent, name)
                .is_some_and(|decl| self.decls[decl.index()].kind == DeclKind::CatchParam)
        {
            return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
        }
        if let Some(existing) = self.decl_in_scope(scope, name) {
            let masked = is_global
                && self.state().first_global_declaration_is_normal(name)
                && self.decls[existing.index()].kind.is_lexical();
            if masked {
                self.state_mut().global_declarations.push((name, true));
                return Ok(existing);
            }
            return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
        }
        if let Some(found) = self.decl_in_scope(var_scope, name) {
            let found = &self.decls[found.index()];
            let within = self.scope_is_within(found.written_in, scope);
            let message = match (found.storage, found.kind) {
                (Storage::Argument, _) if scope == body_scope && allow_body_parameter_shadow => "",
                (Storage::Argument, _) if scope == body_scope => {
                    "invalid redefinition of parameter name"
                }
                (Storage::Argument, _) => "",
                (Storage::Local, DeclKind::Var) if within => "invalid redefinition of a variable",
                (Storage::Module, DeclKind::Var) if within => {
                    "invalid redefinition of module identifier"
                }
                (Storage::Global, DeclKind::Var) if within => {
                    "invalid redefinition of global identifier"
                }
                _ => "",
            };
            if !message.is_empty() {
                return Err(Error::syntax(message, conflict_span));
            }
        }
        if is_global {
            self.state_mut().global_declarations.push((name, true));
            return Ok(self.add_decl(scope, scope, name, kind, Storage::Global, declaration_span));
        }
        Ok(self.add_decl(scope, scope, name, kind, Storage::Local, declaration_span))
    }

    /// A function declaration at a script's top level: a global, without the
    /// ordinary conflict check.
    pub(super) fn register_program_function(&mut self, name: NameId, span: Span) -> DeclId {
        self.state_mut().global_declarations.push((name, false));
        let (var_scope, current) = (self.state().var_scope, self.current_scope());
        if let Some(existing) = self.decl_in_scope(var_scope, name) {
            return existing;
        }
        self.add_decl(
            var_scope,
            current,
            name,
            DeclKind::Var,
            Storage::Global,
            span,
        )
    }

    /// A function declaration at eval code's top level.
    pub(super) fn register_eval_function(
        &mut self,
        name: NameId,
        span: Span,
        conflict_span: Span,
    ) -> Result<DeclId, Error> {
        let body_scope = self.state().body_scope;
        if self
            .decl_in_scope(body_scope, name)
            .is_some_and(|decl| self.decls[decl.index()].kind.is_lexical())
        {
            return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
        }
        if self.strict() {
            return self.register_var(name, span, conflict_span);
        }
        Ok(self.register_program_function(name, span))
    }

    /// QuickJS rejects a module function whose name was declared in the
    /// same scope, before parsing the function.
    pub(super) fn check_module_function(
        &self,
        name: NameId,
        redefinition_span: Span,
    ) -> Result<(), Error> {
        let current = self.current_scope();
        if self
            .decl_from_scope(current, name)
            .is_some_and(|(_, decl)| self.decls[decl.index()].written_in == current)
        {
            return Err(Error::syntax(
                "invalid redefinition of global identifier in module code",
                redefinition_span,
            ));
        }
        Ok(())
    }

    /// A function declaration at a module's top level.
    pub(super) fn register_module_function(&mut self, name: NameId, span: Span) -> DeclId {
        let (var_scope, current) = (self.state().var_scope, self.current_scope());
        if let Some(existing) = self.decl_in_scope(var_scope, name) {
            return existing;
        }
        self.add_decl(
            var_scope,
            current,
            name,
            DeclKind::Var,
            Storage::Module,
            span,
        )
    }

    // ---- Annex B block functions ----

    /// Whether a block function also gets a function-root var (B.3.3).
    pub(super) fn scoped_function_is_annex_b_eligible(&self, name: NameId) -> bool {
        let state = self.state();
        if state.strict {
            return false;
        }
        if (matches!(state.kind, FunctionKind::Normal | FunctionKind::Method)
            && self.names.name(name) == "arguments")
            || (matches!(
                state.kind,
                FunctionKind::Normal | FunctionKind::Method | FunctionKind::Arrow
            ) && state.parameter_names.contains(&name))
        {
            return false;
        }
        let mut scope = state.current_scope;
        loop {
            if let Some(decl) = self.decl_in_scope(scope, name) {
                let decl = &self.decls[decl.index()];
                if decl.kind.is_lexical() && decl.kind != DeclKind::CatchParam {
                    let masked = state.kind == FunctionKind::Script
                        && scope == state.body_scope
                        && state.first_global_declaration_is_normal(name);
                    if !masked {
                        return false;
                    }
                }
            }
            let Some(parent) = self.scopes[scope.index()].parent else {
                break;
            };
            scope = parent;
        }
        true
    }

    /// The block binding of a function declared in a block.
    pub(super) fn register_scoped_function(
        &mut self,
        name: NameId,
        declaration_span: Span,
        conflict_span: Span,
        generator_or_async: bool,
    ) -> Result<DeclId, Error> {
        let scope = self.current_scope();
        if let Some(existing) = self.decl_in_scope(scope, name) {
            let duplicate_ordinary = matches!(
                self.decls[existing.index()].kind,
                DeclKind::ScopedFunction {
                    generator_or_async: false
                }
            ) && !generator_or_async;
            if self.strict() || !duplicate_ordinary {
                return Err(Error::syntax(LEXICAL_REDEFINITION, conflict_span));
            }
            return Ok(self.add_decl(
                scope,
                scope,
                name,
                DeclKind::ScopedFunction { generator_or_async },
                Storage::Local,
                declaration_span,
            ));
        }
        self.register_lexical(
            name,
            declaration_span,
            conflict_span,
            DeclKind::ScopedFunction { generator_or_async },
            true,
        )
    }

    /// The function-root var a sloppy block function also assigns.
    pub(super) fn ensure_annex_b_var(&mut self, name: NameId, span: Span) -> DeclId {
        let (kind, var_scope) = (self.state().kind, self.state().var_scope);
        let global =
            matches!(kind, FunctionKind::Script) || (kind == FunctionKind::Eval && !self.strict());
        if global {
            self.state_mut().global_declarations.push((name, false));
        }
        let storage = if global {
            Storage::Global
        } else {
            Storage::Local
        };
        if let Some(existing) = self.scopes[var_scope.index()]
            .decls
            .iter()
            .rev()
            .copied()
            .find(|decl| {
                let decl = &self.decls[decl.index()];
                decl.name == name && decl.kind == DeclKind::Var && decl.storage == storage
            })
        {
            return existing;
        }
        self.add_decl(var_scope, var_scope, name, DeclKind::Var, storage, span)
    }

    /// A labelled function at a script's top level, which QuickJS treats as
    /// an Annex B global.
    pub(super) fn check_program_annex_b_function(
        &self,
        name: NameId,
        conflict_span: Span,
    ) -> Result<(), Error> {
        let state = self.state();
        let conflicts = if let Some(decl) = self.decl_in_scope(state.var_scope, name) {
            self.decls[decl.index()].written_in == state.body_scope
        } else {
            self.decl_in_scope(state.body_scope, name).is_some()
        };
        if conflicts {
            return Err(Error::syntax(
                "invalid redefinition of global identifier",
                conflict_span,
            ));
        }
        Ok(())
    }

    // ---- parameters ----

    /// An identifier formal parameter.
    pub(super) fn register_identifier_parameter(
        &mut self,
        name: NameId,
        span: Span,
    ) -> Result<DeclId, Error> {
        let state = self.state();
        if state.parameter_scope.is_some() && state.parameter_names.contains(&name) {
            return Err(Error::syntax(
                "duplicate parameter names not allowed in this context",
                span,
            ));
        }
        let var_scope = state.var_scope;
        self.state_mut().parameter_names.push(name);
        let argument = self.add_decl(
            var_scope,
            var_scope,
            name,
            DeclKind::Param,
            Storage::Argument,
            span,
        );
        if let Some(parameter_scope) = self.state().parameter_scope {
            return Ok(self.add_decl(
                parameter_scope,
                parameter_scope,
                name,
                DeclKind::Param,
                Storage::Local,
                span,
            ));
        }
        Ok(argument)
    }

    /// A name bound by a parameter pattern.
    pub(super) fn register_pattern_parameter(
        &mut self,
        name: NameId,
        declaration_span: Span,
        conflict_span: Span,
    ) -> Result<DeclId, Error> {
        if self.state().parameter_names.contains(&name) {
            return Err(Error::syntax(
                "duplicate parameter names not allowed in this context",
                conflict_span,
            ));
        }
        self.state_mut().parameter_names.push(name);
        match self.state().parameter_scope {
            None => self.register_var(name, declaration_span, conflict_span),
            Some(parameter_scope) => Ok(self.add_decl(
                parameter_scope,
                parameter_scope,
                name,
                DeclKind::Param,
                Storage::Local,
                declaration_span,
            )),
        }
    }

    // ---- private names ----

    pub(super) fn register_private(
        &mut self,
        name: NameId,
        span: Span,
        kind: DeclKind,
    ) -> Result<DeclId, Error> {
        let scope = self.current_scope();
        if self.scopes[scope.index()].kind != ScopeKind::ClassPrivate {
            return Err(Error::internal(
                "private field declaration escaped the class-private scope",
            ));
        }
        if self.decl_in_scope(scope, name).is_some() {
            return Err(self.syntax_here("private class field is already defined"));
        }
        Ok(self.add_decl(scope, scope, name, kind, Storage::Local, span))
    }

    /// A private getter or setter, pairing with the other half.
    pub(super) fn register_private_accessor(
        &mut self,
        name: NameId,
        span: Span,
        is_static: bool,
        getter: bool,
    ) -> Result<DeclId, Error> {
        let scope = self.current_scope();
        let incoming = if getter {
            DeclKind::PrivateGetter { is_static }
        } else {
            DeclKind::PrivateSetter { is_static }
        };
        let Some(existing) = self.decl_in_scope(scope, name) else {
            return self.register_private(name, span, incoming);
        };
        let pairable = matches!(
            (self.decls[existing.index()].kind, incoming),
            (DeclKind::PrivateGetter { is_static: a }, DeclKind::PrivateSetter { is_static: b })
                | (DeclKind::PrivateSetter { is_static: a }, DeclKind::PrivateGetter { is_static: b })
                if a == b
        );
        if !pairable {
            return Err(self.syntax_here("private class field is already defined"));
        }
        self.decls[existing.index()].kind = DeclKind::PrivateGetterSetter { is_static };
        Ok(existing)
    }

    // ---- modules ----

    pub(super) fn register_import(&mut self, name: NameId, span: Span) -> Result<DeclId, Error> {
        if matches!(self.names.name(name), "eval" | "arguments") {
            return Err(self.syntax_here("invalid import binding"));
        }
        if self
            .module_root_decls(name)
            .any(|decl| decl.kind == DeclKind::Import)
        {
            return Err(Error::syntax(LEXICAL_REDEFINITION, span));
        }
        let body = self.state().body_scope;
        Ok(self.add_decl(body, body, name, DeclKind::Import, Storage::Module, span))
    }
}
