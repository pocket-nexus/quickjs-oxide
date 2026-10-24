//! Module bodies: imports, exports and module-level function declarations.

use super::support::IdentifierContext;
use super::{ModuleExport, Parser, StatementPosition};
use crate::engine::api::error::{Error, ErrorKind, NativeErrorMessage};
use crate::engine::ast::{
    ImportName, IndirectExport, LocalExport, ModuleImport, ModuleRecord, ModuleRequest, Stmt,
    StmtKind, VarKind,
};
use crate::engine::code::module::MODULE_DEFAULT_BINDING_NAME;
use crate::engine::compiler::lexer::{Keyword, Punctuator, Span, TokenKind};
use crate::engine::compiler::names::NameId;
use crate::engine::value::JsString;

impl<'source> Parser<'source> {
    pub(super) fn parse_module_body(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut body = Vec::new();
        while !self.at_eof() {
            if self.is_keyword(Keyword::Export) {
                if let Some(statement) = self.parse_module_export()? {
                    body.push(statement);
                }
            } else if self.is_keyword(Keyword::Import) && self.static_import_ahead()? {
                self.parse_module_import()?;
            } else {
                body.push(self.parse_statement_or_decl(StatementPosition::ProgramBody)?);
            }
        }
        Ok(body)
    }

    fn module_mut(&mut self) -> Result<&mut ModuleRecord, Error> {
        self.module
            .as_mut()
            .ok_or_else(|| Error::internal("module parser has no module record"))
    }

    fn static_import_ahead(&self) -> Result<bool, Error> {
        let next = self.token_after_current()?;
        Ok(!matches!(
            next.kind,
            TokenKind::Punctuator(Punctuator::LeftParen | Punctuator::Dot)
        ))
    }

    fn with_module_export<T>(
        &mut self,
        export: ModuleExport,
        parse: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let state = self.state();
        self.module_export = Some((state.id, state.current_scope, export));
        let result = parse(self);
        self.module_export = None;
        result
    }

    fn export_name_of(&mut self, name: NameId) -> Result<JsString, Error> {
        self.name_string(name)
    }

    fn add_local_export(
        &mut self,
        local: NameId,
        export: JsString,
        span: Span,
    ) -> Result<(), Error> {
        self.check_export_unique(&export, span)?;
        self.module_mut()?.local_exports.push(LocalExport {
            local,
            export,
            span,
        });
        Ok(())
    }

    fn add_indirect_export(&mut self, export: IndirectExport) -> Result<(), Error> {
        self.check_export_unique(&export.export, export.span)?;
        self.module_mut()?.indirect_exports.push(export);
        Ok(())
    }

    fn check_export_unique(&mut self, export: &JsString, span: Span) -> Result<(), Error> {
        let module = self.module_mut()?;
        if module
            .local_exports
            .iter()
            .any(|entry| entry.export == *export)
            || module
                .indirect_exports
                .iter()
                .any(|entry| entry.export == *export)
        {
            let mut message = NativeErrorMessage::new();
            message.push_utf8("duplicate exported name '");
            export.push_atom_get_str_to(&mut message);
            message.push_utf8("'");
            return Err(Error::from_native_message(ErrorKind::Syntax, message).with_span(span));
        }
        Ok(())
    }

    /// Export every name a `var`, `let`, `const` or class declaration binds.
    fn export_declared_names(
        &mut self,
        statement: &Stmt,
        export: ModuleExport,
    ) -> Result<(), Error> {
        match &statement.kind {
            StmtKind::Var(_, declarators) => {
                for declarator in declarators {
                    let mut names = Vec::new();
                    collect_bound_names(&declarator.target, &mut names);
                    for (name, span) in names {
                        let export_name = self.export_name_of(name)?;
                        self.add_local_export(name, export_name, span)?;
                    }
                }
            }
            StmtKind::Class { decl, .. } => {
                let decl = &self.decls[decl.index()];
                let (name, span) = (decl.name, decl.span);
                let export_name = match export {
                    ModuleExport::Named => self.export_name_of(name)?,
                    ModuleExport::Default => JsString::from_static("default"),
                };
                self.add_local_export(name, export_name, span)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn parse_module_export(&mut self) -> Result<Option<Stmt>, Error> {
        let export_span = self.current().span;
        self.advance()?;
        let start = self.current().span;
        let kind = match self.current().kind {
            TokenKind::Punctuator(Punctuator::LeftBrace) => {
                self.parse_export_clause()?;
                return Ok(None);
            }
            TokenKind::Punctuator(Punctuator::Multiply) => {
                self.parse_star_export()?;
                return Ok(None);
            }
            TokenKind::Keyword(Keyword::Default) => return self.parse_default_export(export_span),
            TokenKind::Keyword(Keyword::Function) => {
                self.parse_module_function_declaration(Some(ModuleExport::Named))?
            }
            TokenKind::Identifier(_) if self.async_function_ahead() => {
                self.parse_module_function_declaration(Some(ModuleExport::Named))?
            }
            TokenKind::Keyword(Keyword::Var) => {
                self.advance()?;
                let declarators =
                    self.with_module_export(ModuleExport::Named, Self::parse_var_declarations)?;
                self.consume_statement_terminator()?;
                let statement = Stmt {
                    kind: StmtKind::Var(VarKind::Var, declarators),
                    span: self.span_from(start).into(),
                };
                self.export_declared_names(&statement, ModuleExport::Named)?;
                return Ok(Some(statement));
            }
            TokenKind::Keyword(Keyword::Let | Keyword::Const) => {
                let statement =
                    self.with_module_export(ModuleExport::Named, Self::parse_lexical_statement)?;
                self.export_declared_names(&statement, ModuleExport::Named)?;
                return Ok(Some(statement));
            }
            TokenKind::Keyword(Keyword::Class) => {
                let kind =
                    self.with_module_export(ModuleExport::Named, Self::parse_class_declaration)?;
                let statement = Stmt {
                    kind,
                    span: self.span_from(start).into(),
                };
                self.export_declared_names(&statement, ModuleExport::Named)?;
                return Ok(Some(statement));
            }
            _ => return Err(self.syntax_here("invalid export syntax")),
        };
        Ok(Some(Stmt {
            kind,
            span: self.span_from(start).into(),
        }))
    }

    fn parse_default_export(&mut self, export_span: Span) -> Result<Option<Stmt>, Error> {
        let default_span = self.current().span;
        self.advance()?;
        let start = self.current().span;
        if self.is_keyword(Keyword::Class) {
            let kind =
                self.with_module_export(ModuleExport::Default, Self::parse_class_declaration)?;
            let statement = Stmt {
                kind,
                span: self.span_from(start).into(),
            };
            self.export_declared_names(&statement, ModuleExport::Default)?;
            return Ok(Some(statement));
        }
        if self.is_keyword(Keyword::Function)
            || (matches!(self.current().kind, TokenKind::Identifier(_))
                && self.async_function_ahead())
        {
            let kind = self.parse_module_function_declaration(Some(ModuleExport::Default))?;
            return Ok(Some(Stmt {
                kind,
                span: self.span_from(start).into(),
            }));
        }
        let value = self.parse_assignment_allow_in()?;
        let default_name = self.names.intern(MODULE_DEFAULT_BINDING_NAME);
        let decl = match self.decl_in_scope(self.state().body_scope, default_name) {
            Some(decl) => decl,
            None => self.register_lexical(
                default_name,
                default_span,
                default_span,
                crate::engine::ast::DeclKind::Hidden { is_const: false },
                false,
            )?,
        };
        self.add_local_export(default_name, JsString::from_static("default"), export_span)?;
        self.consume_statement_terminator()?;
        Ok(Some(Stmt {
            kind: StmtKind::ExportDefault { value, decl },
            span: self.span_from(start).into(),
        }))
    }

    /// `[export [default]] [async] function [*] name`.
    pub(super) fn parse_module_function_declaration(
        &mut self,
        export: Option<ModuleExport>,
    ) -> Result<StmtKind, Error> {
        let header = self.parse_function_header(export != Some(ModuleExport::Default))?;
        let redefinition = self.current().span;
        let named = header.name.is_some();
        let (name, span) = match &header.name {
            Some((identifier, span)) => (self.intern_identifier(identifier), *span),
            None => (self.names.intern(MODULE_DEFAULT_BINDING_NAME), header.span),
        };
        if named {
            self.check_module_function(name, redefinition)?;
        }
        let function = self.parse_function_tail(header, false)?;
        let decl = self.register_module_function(name, span);
        if let Some(export) = export {
            let export_name = match export {
                ModuleExport::Named => self.name_string(name)?,
                ModuleExport::Default => JsString::from_static("default"),
            };
            self.add_local_export(name, export_name, span)?;
        }
        Ok(StmtKind::Function {
            function: Box::new(function),
            decl,
            annex_b: None,
        })
    }

    fn parse_module_import(&mut self) -> Result<(), Error> {
        self.advance()?;
        if matches!(self.current().kind, TokenKind::String(_)) {
            self.parse_module_request()?;
            return self.consume_statement_terminator();
        }
        let mut imports = Vec::new();
        let secondary = if matches!(self.current().kind, TokenKind::Identifier(_)) {
            let (name, span) = self.module_binding_identifier()?;
            let decl = self.register_import(name, span)?;
            imports.push((decl, ImportName::Name(JsString::from_static("default"))));
            self.consume_punctuator(Punctuator::Comma)?
        } else {
            true
        };
        if secondary && self.is_punctuator(Punctuator::Multiply) {
            self.advance()?;
            if !self.is_contextual_keyword("as") {
                return Err(self.syntax_here("expecting 'as'"));
            }
            self.advance()?;
            let (name, span) = self.module_binding_identifier()?;
            let decl = self.register_import(name, span)?;
            imports.push((decl, ImportName::Namespace));
        } else if secondary && self.is_punctuator(Punctuator::LeftBrace) {
            self.advance()?;
            while !self.is_punctuator(Punctuator::RightBrace) {
                let imported = *self.current();
                let import_name = self.module_export_name()?;
                let (local, local_span) = if self.is_contextual_keyword("as") {
                    self.advance()?;
                    self.module_binding_identifier()?
                } else {
                    let TokenKind::Identifier(identifier) = imported.kind else {
                        return Err(Error::syntax(
                            "imported keyword requires an 'as' binding",
                            imported.span,
                        ));
                    };
                    self.validate_identifier_reservation(
                        &identifier,
                        imported.span,
                        true,
                        IdentifierContext::Variable,
                    )?;
                    (self.intern_identifier(&identifier), imported.span)
                };
                let decl = self.register_import(local, local_span)?;
                imports.push((decl, ImportName::Name(import_name)));
                if !self.consume_punctuator(Punctuator::Comma)? {
                    break;
                }
            }
            self.expect_punctuator(Punctuator::RightBrace)?;
        } else if secondary {
            return Err(self.syntax_here("default, namespace, or named imports expected"));
        }
        let request = self.parse_from_clause()?;
        let module = self.module_mut()?;
        for (decl, name) in imports {
            module.imports.push(ModuleImport {
                request,
                name,
                decl,
            });
        }
        self.consume_statement_terminator()
    }

    fn parse_from_clause(&mut self) -> Result<u32, Error> {
        if !self.is_contextual_keyword("from") {
            return Err(self.syntax_here("from clause expected"));
        }
        self.advance()?;
        self.parse_module_request()
    }

    fn parse_module_request(&mut self) -> Result<u32, Error> {
        let token = *self.current();
        let TokenKind::String(_) = token.kind else {
            return Err(self.syntax_here("string expected"));
        };
        let specifier = self.decode_string_literal(token.span)?;
        self.advance()?;
        let attributes = self.parse_import_attributes()?;
        let has_brace = attributes.is_some();
        let module = self.module_mut()?;
        let index = module.requests.len() as u32;
        module.requests.push(ModuleRequest {
            specifier,
            attributes,
        });
        if has_brace {
            self.expect_punctuator(Punctuator::RightBrace)?;
        }
        Ok(index)
    }

    fn parse_import_attributes(&mut self) -> Result<Option<Vec<(JsString, JsString)>>, Error> {
        if !self.is_contextual_keyword("with") && !self.is_keyword(Keyword::With) {
            return Ok(None);
        }
        self.advance()?;
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let mut attributes: Vec<(JsString, JsString)> = Vec::new();
        while !self.is_punctuator(Punctuator::RightBrace) {
            let key_token = *self.current();
            let key = match key_token.kind {
                TokenKind::String(_) => self.decode_string_literal(key_token.span)?,
                TokenKind::Identifier(identifier) => {
                    JsString::try_from_utf8(&self.identifier_text(&identifier))?
                }
                TokenKind::Keyword(keyword) => JsString::try_from_utf8(keyword.as_str())?,
                _ => return Err(self.syntax_here("identifier expected")),
            };
            self.advance()?;
            self.expect_punctuator(Punctuator::Colon)?;
            let value_token = *self.current();
            let TokenKind::String(_) = value_token.kind else {
                return Err(Error::syntax("string expected", key_token.span));
            };
            if attributes.iter().any(|(existing, _)| *existing == key) {
                return Err(self.syntax_here("duplicate with key"));
            }
            attributes.push((key, self.decode_string_literal(value_token.span)?));
            self.advance()?;
            if !self.consume_punctuator(Punctuator::Comma)? {
                break;
            }
        }
        Ok(Some(attributes))
    }

    fn module_binding_identifier(&mut self) -> Result<(NameId, Span), Error> {
        let token = *self.current();
        let TokenKind::Identifier(identifier) = token.kind else {
            return Err(self.syntax_here("identifier expected"));
        };
        self.validate_identifier_reservation(
            &identifier,
            token.span,
            true,
            IdentifierContext::Variable,
        )?;
        self.advance()?;
        Ok((self.intern_identifier(&identifier), token.span))
    }

    fn parse_export_clause(&mut self) -> Result<(), Error> {
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let mut entries = Vec::new();
        while !self.is_punctuator(Punctuator::RightBrace) {
            let (local, span) = self.module_identifier_name()?;
            let export = if self.is_contextual_keyword("as") {
                self.advance()?;
                self.module_export_name()?
            } else {
                self.name_string(local)?
            };
            entries.push((local, export, span));
            if !self.consume_punctuator(Punctuator::Comma)? {
                break;
            }
        }
        self.expect_punctuator(Punctuator::RightBrace)?;
        if self.is_contextual_keyword("from") {
            let request = self.parse_from_clause()?;
            for (import, export, span) in entries {
                let import = self.name_string(import)?;
                self.add_indirect_export(IndirectExport {
                    export,
                    request,
                    import: ImportName::Name(import),
                    span,
                })?;
            }
        } else {
            for (local, export, span) in entries {
                self.add_local_export(local, export, span)?;
            }
        }
        self.consume_statement_terminator()
    }

    fn parse_star_export(&mut self) -> Result<(), Error> {
        self.expect_punctuator(Punctuator::Multiply)?;
        if self.is_contextual_keyword("as") {
            self.advance()?;
            // QuickJS accepts an IdentifierName here, not a string.
            let (export, span) = self.module_identifier_name()?;
            let request = self.parse_from_clause()?;
            let export = self.name_string(export)?;
            self.add_indirect_export(IndirectExport {
                export,
                request,
                import: ImportName::Namespace,
                span,
            })?;
        } else {
            let request = self.parse_from_clause()?;
            self.module_mut()?.star_exports.push(request);
        }
        self.consume_statement_terminator()
    }

    fn module_identifier_name(&mut self) -> Result<(NameId, Span), Error> {
        let token = *self.current();
        let name = match token.kind {
            TokenKind::Identifier(identifier) => self.intern_identifier(&identifier),
            TokenKind::Keyword(keyword) => self.names.intern(keyword.as_str()),
            _ => return Err(self.syntax_here("identifier expected")),
        };
        self.advance()?;
        Ok((name, token.span))
    }

    fn module_export_name(&mut self) -> Result<JsString, Error> {
        let token = *self.current();
        let name = match token.kind {
            TokenKind::String(_) => {
                let name = self.decode_string_literal(token.span)?;
                String::from_utf16(&name.utf16_units().collect::<Vec<_>>())
                    .map_err(|_| Error::syntax("contains unpaired surrogate", token.span))?;
                name
            }
            TokenKind::Identifier(identifier) => {
                JsString::try_from_utf8(&self.identifier_text(&identifier))?
            }
            TokenKind::Keyword(keyword) => JsString::try_from_utf8(keyword.as_str())?,
            _ => return Err(self.syntax_here("identifier expected")),
        };
        self.advance()?;
        Ok(name)
    }

    /// Every local export must name a module binding.
    pub(super) fn resolve_module_exports(&self) -> Result<(), Error> {
        let Some(module) = &self.module else {
            return Ok(());
        };
        let root = &self.functions[0];
        for export in &module.local_exports {
            let found = [root.var_scope, root.body_scope].iter().any(|scope| {
                self.scopes[scope.index()]
                    .decls
                    .iter()
                    .any(|decl| self.decls[decl.index()].name == export.local)
            });
            if !found {
                return Err(Error::syntax(
                    format!(
                        "exported variable '{}' does not exist",
                        self.names.name(export.local)
                    ),
                    export.span,
                ));
            }
        }
        Ok(())
    }
}

fn collect_bound_names(pattern: &crate::engine::ast::Pattern, out: &mut Vec<(NameId, Span)>) {
    use crate::engine::ast::PatternKind;
    match &pattern.kind {
        PatternKind::Binding(reference) => out.push((reference.name, pattern.span)),
        PatternKind::Target(_) => {}
        PatternKind::Object { props, rest, .. } => {
            for prop in props {
                collect_bound_names(&prop.value, out);
            }
            if let Some(rest) = rest {
                collect_bound_names(rest, out);
            }
        }
        PatternKind::Array { items, rest } => {
            for item in items.iter().flatten() {
                collect_bound_names(&item.pattern, out);
            }
            if let Some(rest) = rest {
                collect_bound_names(rest, out);
            }
        }
    }
}
