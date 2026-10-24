//! Statements and declarations.

use super::patterns::{BindingSite, Declaring, ReferenceKind};
use super::support::{ForIterationKind, IdentifierContext};
use super::{BreakControl, BreakKind, FunctionKind, InMode, Parser, StatementPosition, pos};
use crate::engine::api::error::Error;
use crate::engine::ast::{
    Catch, DeclKind, ForInStmt, ForInit, ForOfStmt, ForStmt, ForTarget, LabelId, Pattern,
    PatternKind, Ref, ScopeId, ScopeKind, Stmt, StmtKind, SwitchCase, SwitchStmt, TryStmt,
    VarDeclarator, VarKind,
};
use crate::engine::compiler::lexer::{Keyword, Punctuator, Span, TokenKind};
use crate::engine::compiler::stack_guard::ParserStackFrame;

impl<'source> Parser<'source> {
    pub(super) fn parse_script_body(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut body = Vec::new();
        while !self.at_eof() {
            body.push(self.parse_statement_or_decl(StatementPosition::ProgramBody)?);
        }
        Ok(body)
    }

    /// QuickJS's `js_parse_statement_or_decl`: every statement list and
    /// single-statement body goes through it.
    pub(super) fn parse_statement_or_decl(
        &mut self,
        position: StatementPosition,
    ) -> Result<Stmt, Error> {
        let start = self.current().span;
        if self.consume_punctuator(Punctuator::Semicolon)? {
            return Ok(Stmt {
                kind: StmtKind::Empty,
                span: start.into(),
            });
        }
        if self.lexical_declaration_ahead(position.allows_other_declaration())? {
            if !position.allows_other_declaration() {
                return Err(self
                    .syntax_here("lexical declarations can't appear in single-statement context"));
            }
            return self.parse_lexical_statement();
        }
        let annex_b_function_allowed = matches!(
            position,
            StatementPosition::AnnexBIfArm | StatementPosition::AnnexBLabelBody
        );
        if !position.allows_other_declaration()
            && self.restricted_function_declaration_ahead(annex_b_function_allowed)?
        {
            return Err(
                self.syntax_here("function declarations can't appear in single-statement context")
            );
        }
        if let Some(label) = self.label_ahead() {
            return self.parse_labeled_statement(label, position);
        }
        let kind = match self.current().kind {
            TokenKind::Punctuator(Punctuator::LeftBrace) => {
                let (body, scope) = self.parse_block()?;
                StmtKind::Block { body, scope }
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if_statement()?,
            TokenKind::Keyword(Keyword::While) => self.parse_while_statement(None)?,
            TokenKind::Keyword(Keyword::Do) => self.parse_do_while_statement(None)?,
            TokenKind::Keyword(Keyword::For) => self.parse_for_statement(None)?,
            TokenKind::Keyword(Keyword::Switch) => self.parse_switch_statement()?,
            TokenKind::Keyword(Keyword::Try) => self.parse_try_statement()?,
            TokenKind::Keyword(Keyword::With) => self.parse_with_statement()?,
            TokenKind::Keyword(Keyword::Break) => self.parse_loop_jump_statement(false)?,
            TokenKind::Keyword(Keyword::Continue) => self.parse_loop_jump_statement(true)?,
            TokenKind::Keyword(Keyword::Function) => {
                self.parse_hoistable_function_declaration(position)?
            }
            TokenKind::Identifier(_) if self.async_function_ahead() => {
                self.parse_hoistable_function_declaration(position)?
            }
            TokenKind::Keyword(Keyword::Class) => {
                if !position.allows_other_declaration() {
                    return Err(self.syntax_here(
                        "class declarations can't appear in single-statement context",
                    ));
                }
                self.parse_class_declaration()?
            }
            TokenKind::Keyword(Keyword::Var) => {
                self.advance()?;
                let declarators = self.parse_var_declarations()?;
                self.consume_statement_terminator()?;
                StmtKind::Var(VarKind::Var, declarators)
            }
            TokenKind::Keyword(Keyword::Return) => {
                if matches!(
                    self.state().kind,
                    FunctionKind::Script | FunctionKind::Module | FunctionKind::Eval
                ) {
                    return Err(self.syntax_here("return not in a function"));
                }
                self.parse_return_statement()?
            }
            TokenKind::Keyword(Keyword::Throw) => {
                self.advance()?;
                if self.current().line_terminator_before {
                    return Err(Error::syntax(
                        "line terminator not allowed after throw",
                        self.current().span,
                    ));
                }
                let value = self.parse_expression()?;
                self.consume_statement_terminator()?;
                StmtKind::Throw(value)
            }
            TokenKind::Keyword(Keyword::Debugger) => {
                self.advance()?;
                self.consume_statement_terminator()?;
                StmtKind::Debugger
            }
            TokenKind::Keyword(keyword @ (Keyword::Enum | Keyword::Export | Keyword::Extends)) => {
                return Err(self.syntax_here(format!("unsupported keyword: {}", keyword.as_str())));
            }
            _ => {
                let value = self.parse_expression()?;
                self.consume_statement_terminator()?;
                StmtKind::Expr(value)
            }
        };
        Ok(Stmt {
            kind,
            span: self.span_from(start).into(),
        })
    }

    fn parse_hoistable_function_declaration(
        &mut self,
        position: StatementPosition,
    ) -> Result<StmtKind, Error> {
        let kind = self.state().kind;
        if position == StatementPosition::ProgramBody {
            match kind {
                FunctionKind::Script => {
                    let header = self.parse_function_header(true)?;
                    let function = self.parse_function_tail(header, false)?;
                    let name = function.name.expect("a declaration has a name");
                    let decl = self.register_program_function(name.name, name.span);
                    return Ok(StmtKind::Function {
                        function: Box::new(function),
                        decl,
                        annex_b: None,
                    });
                }
                FunctionKind::Module => return self.parse_module_function_declaration(None),
                FunctionKind::Eval => {
                    let header = self.parse_function_header(true)?;
                    let function = self.parse_function_tail(header, false)?;
                    let name = function.name.expect("a declaration has a name");
                    let conflict = self.current().span;
                    let decl = self.register_eval_function(name.name, name.span, conflict)?;
                    return Ok(StmtKind::Function {
                        function: Box::new(function),
                        decl,
                        annex_b: None,
                    });
                }
                _ => {}
            }
        }
        if position == StatementPosition::FunctionBody
            && !matches!(
                kind,
                FunctionKind::Script | FunctionKind::Module | FunctionKind::Eval
            )
        {
            let header = self.parse_function_header(true)?;
            let function = self.parse_function_tail(header, false)?;
            let name = function.name.expect("a declaration has a name");
            let conflict = self.current().span;
            let decl = self.register_var(name.name, name.span, conflict)?;
            return Ok(StmtKind::Function {
                function: Box::new(function),
                decl,
                annex_b: None,
            });
        }
        if !matches!(
            position,
            StatementPosition::NestedList
                | StatementPosition::AnnexBIfArm
                | StatementPosition::AnnexBLabelBody
        ) {
            return Err(
                self.syntax_here("function declarations can't appear in single-statement context")
            );
        }
        let header = self.parse_function_header(true)?;
        let (identifier, span) = header.name.expect("a declaration has a name");
        let name = self.intern_identifier(&identifier);
        let program_body =
            kind == FunctionKind::Script && self.current_scope() == self.state().body_scope;
        if program_body {
            self.check_program_annex_b_function(name, self.current().span)?;
            let function = self.parse_function_tail(header, false)?;
            let decl = self.ensure_annex_b_var(name, span);
            return Ok(StmtKind::Function {
                function: Box::new(function),
                decl,
                annex_b: None,
            });
        }
        let non_ordinary = header.is_async || header.is_generator;
        let eligible = !non_ordinary && self.scoped_function_is_annex_b_eligible(name);
        let conflict = self.current().span;
        let decl = self.register_scoped_function(name, span, conflict, non_ordinary)?;
        let function = self.parse_function_tail(header, false)?;
        let annex_b = eligible.then(|| self.ensure_annex_b_var(name, span));
        Ok(StmtKind::Function {
            function: Box::new(function),
            decl,
            annex_b,
        })
    }

    fn parse_labeled_statement(
        &mut self,
        label: String,
        position: StatementPosition,
    ) -> Result<Stmt, Error> {
        let start = self.current().span;
        if self
            .state()
            .breaks
            .iter()
            .any(|control| control.label.as_deref() == Some(label.as_str()))
        {
            return Err(self.syntax_here("duplicate label name"));
        }
        self.advance()?;
        self.expect_punctuator(Punctuator::Colon)?;
        let weight = self.enter_recursion(ParserStackFrame::Label)?;
        let result = self.parse_labeled_body(label, position);
        if result.is_ok() {
            self.leave_recursion(weight);
        }
        let kind = result?;
        Ok(Stmt {
            kind,
            span: self.span_from(start).into(),
        })
    }

    fn parse_labeled_body(
        &mut self,
        label: String,
        position: StatementPosition,
    ) -> Result<StmtKind, Error> {
        // A label directly on a loop becomes the loop's own.
        match self.current().kind {
            TokenKind::Keyword(Keyword::While) => return self.parse_while_statement(Some(label)),
            TokenKind::Keyword(Keyword::Do) => return self.parse_do_while_statement(Some(label)),
            TokenKind::Keyword(Keyword::For) => return self.parse_for_statement(Some(label)),
            _ => {}
        }
        let id = self.push_break(BreakKind::Regular, Some(label));
        let body_position = if !self.strict() && position.allows_labelled_annex_b() {
            StatementPosition::AnnexBLabelBody
        } else {
            StatementPosition::Single
        };
        let body = self.parse_statement_or_decl(body_position)?;
        self.state_mut().breaks.pop();
        Ok(StmtKind::Labeled {
            body: Box::new(body),
            label: id,
        })
    }

    fn push_break(&mut self, kind: BreakKind, label: Option<String>) -> LabelId {
        let id = self.new_label();
        self.state_mut()
            .breaks
            .push(BreakControl { kind, label, id });
        id
    }

    /// `{ ... }`: an empty block has no scope.
    pub(super) fn parse_block(&mut self) -> Result<(Vec<Stmt>, Option<ScopeId>), Error> {
        self.advance()?;
        if self.is_punctuator(Punctuator::RightBrace) {
            self.advance()?;
            return Ok((Vec::new(), None));
        }
        self.parse_recursion(ParserStackFrame::Block, |parser| {
            let scope = parser.push_scope(ScopeKind::Block);
            let mut body = Vec::new();
            while !parser.is_punctuator(Punctuator::RightBrace) {
                body.push(parser.parse_statement_or_decl(StatementPosition::NestedList)?);
            }
            parser.advance()?;
            parser.pop_scope(scope)?;
            Ok((body, Some(scope)))
        })
    }

    fn parse_with_statement(&mut self) -> Result<StmtKind, Error> {
        let with_span = self.current().span;
        if self.strict() {
            return Err(Error::syntax("invalid keyword: with", with_span));
        }
        self.advance()?;
        self.expect_punctuator(Punctuator::LeftParen)?;
        let object = self.parse_expression()?;
        self.expect_punctuator(Punctuator::RightParen)?;
        let scope = self.push_scope(ScopeKind::With);
        self.has_with = true;
        let weight = self.enter_recursion(ParserStackFrame::StatementHead)?;
        let body = self.parse_statement_or_decl(StatementPosition::Single)?;
        self.leave_recursion(weight);
        self.pop_scope(scope)?;
        Ok(StmtKind::With {
            object,
            body: Box::new(body),
            scope,
        })
    }

    fn parse_if_statement(&mut self) -> Result<StmtKind, Error> {
        self.advance()?;
        let scope = self.push_scope(ScopeKind::If);
        self.expect_punctuator(Punctuator::LeftParen)?;
        let test = self.parse_expression()?;
        self.expect_punctuator(Punctuator::RightParen)?;
        let branch = if self.strict() {
            StatementPosition::Single
        } else {
            StatementPosition::AnnexBIfArm
        };
        let weight = self.enter_recursion(ParserStackFrame::StatementHead)?;
        let then = self.parse_statement_or_decl(branch)?;
        let otherwise = if self.is_keyword(Keyword::Else) {
            self.advance()?;
            Some(Box::new(self.parse_statement_or_decl(branch)?))
        } else {
            None
        };
        self.leave_recursion(weight);
        self.pop_scope(scope)?;
        Ok(StmtKind::If {
            test,
            then: Box::new(then),
            otherwise,
            scope,
        })
    }

    fn parse_switch_statement(&mut self) -> Result<StmtKind, Error> {
        self.advance()?;
        self.expect_punctuator(Punctuator::LeftParen)?;
        let discriminant = self.parse_expression()?;
        self.expect_punctuator(Punctuator::RightParen)?;
        let scope = self.push_scope(ScopeKind::Switch);
        let label = self.push_break(BreakKind::Switch, None);
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let mut cases: Vec<SwitchCase> = Vec::new();
        let mut has_default = false;
        let mut head_weight = None;
        while !self.is_punctuator(Punctuator::RightBrace) {
            match self.current().kind {
                TokenKind::Keyword(Keyword::Case) => {
                    self.advance()?;
                    let test = self.parse_expression()?;
                    self.expect_punctuator(Punctuator::Colon)?;
                    cases.push(SwitchCase {
                        test: Some(test),
                        body: Vec::new(),
                    });
                }
                TokenKind::Keyword(Keyword::Default) => {
                    self.advance()?;
                    self.expect_punctuator(Punctuator::Colon)?;
                    if has_default {
                        return Err(self.syntax_here("duplicate default"));
                    }
                    has_default = true;
                    cases.push(SwitchCase {
                        test: None,
                        body: Vec::new(),
                    });
                }
                _ => {
                    if cases.is_empty() {
                        return Err(self.syntax_here("invalid switch statement"));
                    }
                    if head_weight.is_none() {
                        head_weight = Some(self.enter_recursion(ParserStackFrame::StatementHead)?);
                    }
                    let statement = self.parse_statement_or_decl(StatementPosition::NestedList)?;
                    cases.last_mut().expect("a case").body.push(statement);
                }
            }
        }
        self.advance()?;
        if let Some(weight) = head_weight {
            self.leave_recursion(weight);
        }
        self.state_mut().breaks.pop();
        self.pop_scope(scope)?;
        Ok(StmtKind::Switch(Box::new(SwitchStmt {
            discriminant,
            cases,
            scope,
            label,
        })))
    }

    fn parse_try_statement(&mut self) -> Result<StmtKind, Error> {
        self.advance()?;
        if !self.is_punctuator(Punctuator::LeftBrace) {
            return Err(self.syntax_here("expecting '{'"));
        }
        let (block, block_scope) = self.parse_block()?;
        let catch = if self.is_keyword(Keyword::Catch) {
            self.advance()?;
            let scope = self.push_scope(ScopeKind::Catch);
            let (param, param_default) = if self.is_punctuator(Punctuator::LeftBrace) {
                (None, None)
            } else {
                self.expect_punctuator(Punctuator::LeftParen)?;
                let parameter = if self.is_punctuator(Punctuator::LeftBrace)
                    || self.is_punctuator(Punctuator::LeftBracket)
                {
                    let (pattern, default) = self.parse_catch_binding_pattern()?;
                    (Some(pattern), default)
                } else {
                    let token = *self.current();
                    let TokenKind::Identifier(identifier) = token.kind else {
                        return Err(self.syntax_here("identifier expected"));
                    };
                    if identifier.escaped_reserved_word {
                        return Err(self.syntax_here("identifier expected"));
                    }
                    self.validate_identifier_reservation(
                        &identifier,
                        token.span,
                        self.strict(),
                        IdentifierContext::Variable,
                    )?;
                    let name = self.intern_identifier(&identifier);
                    let invalid = self.is_strict_restricted_name(name);
                    self.advance()?;
                    if invalid {
                        return Err(Error::syntax(
                            "invalid variable name in strict mode",
                            self.current().span,
                        ));
                    }
                    let conflict = self.current().span;
                    self.register_lexical(name, token.span, conflict, DeclKind::CatchParam, false)?;
                    (
                        Some(Pattern {
                            kind: PatternKind::Binding(Ref {
                                name,
                                span: token.span.into(),
                                scope,
                            }),
                            span: token.span,
                        }),
                        None,
                    )
                };
                self.expect_punctuator(Punctuator::RightParen)?;
                parameter
            };
            self.expect_punctuator(Punctuator::LeftBrace)?;
            let body_scope = (!self.is_punctuator(Punctuator::RightBrace))
                .then(|| self.push_scope(ScopeKind::Block));
            let mut body = Vec::new();
            while !self.is_punctuator(Punctuator::RightBrace) {
                if self.at_eof() {
                    return Err(self.syntax_here("unterminated catch block"));
                }
                body.push(self.parse_statement_or_decl(StatementPosition::NestedList)?);
            }
            self.advance()?;
            if let Some(body_scope) = body_scope {
                self.pop_scope(body_scope)?;
            }
            self.pop_scope(scope)?;
            Some(Catch {
                param,
                param_default,
                scope,
                body,
                body_scope,
            })
        } else if self.is_keyword(Keyword::Finally) {
            None
        } else {
            return Err(self.syntax_here("expecting catch or finally"));
        };
        let finally = if self.is_keyword(Keyword::Finally) {
            self.advance()?;
            if !self.is_punctuator(Punctuator::LeftBrace) {
                return Err(self.syntax_here("expecting '{'"));
            }
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(StmtKind::Try(Box::new(TryStmt {
            block,
            block_scope,
            catch,
            finally,
        })))
    }

    fn parse_loop_jump_statement(&mut self, is_continue: bool) -> Result<StmtKind, Error> {
        self.advance()?;
        let label = if self.current().line_terminator_before {
            None
        } else if let TokenKind::Identifier(identifier) = &self.current().kind
            && !identifier.escaped_reserved_word
        {
            Some(self.identifier_text(identifier).into_owned())
        } else {
            None
        };
        let target = self
            .state()
            .breaks
            .iter()
            .rev()
            .find(|control| match label.as_deref() {
                Some(label) if is_continue => {
                    control.kind == BreakKind::Loop && control.label.as_deref() == Some(label)
                }
                Some(label) => control.label.as_deref() == Some(label),
                None if is_continue => control.kind == BreakKind::Loop,
                None => matches!(control.kind, BreakKind::Loop | BreakKind::Switch),
            });
        let Some(target) = target.map(|control| control.id) else {
            return Err(self.syntax_here(if label.is_some() {
                "break/continue label not found"
            } else if is_continue {
                "continue must be inside loop"
            } else {
                "break must be inside loop or switch"
            }));
        };
        if label.is_some() {
            self.advance()?;
        }
        self.consume_statement_terminator()?;
        Ok(if is_continue {
            StmtKind::Continue(target)
        } else {
            StmtKind::Break(target)
        })
    }

    fn parse_while_statement(&mut self, label: Option<String>) -> Result<StmtKind, Error> {
        let id = self.push_break(BreakKind::Loop, label);
        self.advance()?;
        self.expect_punctuator(Punctuator::LeftParen)?;
        let test = self.parse_expression()?;
        self.expect_punctuator(Punctuator::RightParen)?;
        let weight = self.enter_recursion(ParserStackFrame::StatementHead)?;
        let body = self.parse_statement_or_decl(StatementPosition::Single)?;
        self.leave_recursion(weight);
        self.state_mut().breaks.pop();
        Ok(StmtKind::While {
            test,
            body: Box::new(body),
            label: id,
        })
    }

    fn parse_do_while_statement(&mut self, label: Option<String>) -> Result<StmtKind, Error> {
        let id = self.push_break(BreakKind::Loop, label);
        self.advance()?;
        let weight = self.enter_recursion(ParserStackFrame::StatementHead)?;
        let body = self.parse_statement_or_decl(StatementPosition::Single)?;
        if !self.is_keyword(Keyword::While) {
            // QuickJS formats the expected token through `%c`.
            return Err(self.syntax_here("expecting '\u{fffd}"));
        }
        self.advance()?;
        self.expect_punctuator(Punctuator::LeftParen)?;
        let test = self.parse_expression()?;
        self.expect_punctuator(Punctuator::RightParen)?;
        // The semicolon is optional even before a same-line token.
        self.consume_punctuator(Punctuator::Semicolon)?;
        self.state_mut().breaks.pop();
        self.leave_recursion(weight);
        Ok(StmtKind::DoWhile {
            body: Box::new(body),
            test,
            label: id,
        })
    }

    fn parse_for_statement(&mut self, label: Option<String>) -> Result<StmtKind, Error> {
        self.advance()?;
        let is_await = self.is_keyword(Keyword::Await);
        if is_await {
            if !self.state().is_async {
                return Err(self.syntax_here("for await is only valid in asynchronous functions"));
            }
            self.advance()?;
        }
        let classic = !is_await && self.for_head_has_top_level_semicolon();
        self.expect_punctuator(Punctuator::LeftParen)?;
        let scope = self.push_scope(ScopeKind::For);
        if !classic {
            return self.parse_for_in_of_statement(label, scope, is_await);
        }
        let init = if self.is_punctuator(Punctuator::Semicolon) {
            None
        } else if self.lexical_declaration_ahead(true)? {
            let (kind, declarators) =
                self.with_in_mode(InMode::Disallow, Self::parse_lexical_declarations)?;
            Some(ForInit::Var(kind, declarators))
        } else if self.is_keyword(Keyword::Var) {
            self.advance()?;
            let declarators = self.with_in_mode(InMode::Disallow, Self::parse_var_declarations)?;
            Some(ForInit::Var(VarKind::Var, declarators))
        } else {
            Some(ForInit::Expr(self.parse_expression_no_in()?))
        };
        self.expect_punctuator(Punctuator::Semicolon)?;
        let id = self.push_break(BreakKind::Loop, label);
        let test = if self.is_punctuator(Punctuator::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_punctuator(Punctuator::Semicolon)?;
        let update = if self.is_punctuator(Punctuator::RightParen) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_punctuator(Punctuator::RightParen)?;
        let weight = self.enter_recursion(ParserStackFrame::StatementHead)?;
        let body = self.parse_statement_or_decl(StatementPosition::Single)?;
        self.leave_recursion(weight);
        self.state_mut().breaks.pop();
        self.pop_scope(scope)?;
        Ok(StmtKind::For(Box::new(ForStmt {
            init,
            test,
            update,
            body,
            scope,
            label: id,
        })))
    }

    fn parse_for_in_of_statement(
        &mut self,
        label: Option<String>,
        scope: ScopeId,
        is_await: bool,
    ) -> Result<StmtKind, Error> {
        let probed = self.for_iteration_kind_ahead();
        let hint = probed.unwrap_or(ForIterationKind::In);
        if is_await && probed == Some(ForIterationKind::In) {
            return Err(self.syntax_here("'for await' loop should be used with 'of'"));
        }
        let (target, simple_var, destructuring) = self.parse_for_target(hint, is_await)?;
        let init = if self.consume_punctuator(Punctuator::Equal)? {
            Some(match hint {
                ForIterationKind::In => {
                    self.with_in_mode(InMode::Disallow, Self::parse_assignment)?
                }
                ForIterationKind::Of => self.parse_assignment_allow_in()?,
            })
        } else {
            None
        };
        let kind = if probed.is_none() {
            return Err(self.syntax_here("expected 'of' or 'in' in for control expression"));
        } else if self.is_for_of_keyword() {
            ForIterationKind::Of
        } else if self.is_keyword(Keyword::In) {
            ForIterationKind::In
        } else {
            return Err(self.syntax_here("expected 'of' or 'in' in for control expression"));
        };
        if kind != hint {
            return Err(Error::internal("for-in/of delimiter probe drifted"));
        }
        if is_await && kind != ForIterationKind::Of {
            return Err(self.syntax_here("'for await' loop should be used with 'of'"));
        }
        if init.is_some()
            && (kind == ForIterationKind::Of || !simple_var || self.strict() || destructuring)
        {
            return Err(self.syntax_here(format!(
                "a declaration in the head of a for-{} loop can't have an initializer",
                if kind == ForIterationKind::Of {
                    "of"
                } else {
                    "in"
                }
            )));
        }
        self.advance_expression_start()?;
        let source = if kind == ForIterationKind::Of {
            self.parse_assignment_allow_in()?
        } else {
            self.parse_expression()?
        };
        self.expect_punctuator(Punctuator::RightParen)?;
        let id = self.push_break(BreakKind::Loop, label);
        let weight = self.enter_recursion(ParserStackFrame::StatementHead)?;
        let body = self.parse_statement_or_decl(StatementPosition::Single)?;
        self.leave_recursion(weight);
        self.state_mut().breaks.pop();
        self.pop_scope(scope)?;
        Ok(match kind {
            ForIterationKind::In => StmtKind::ForIn(Box::new(ForInStmt {
                target,
                init,
                object: source,
                body,
                scope,
                label: id,
            })),
            ForIterationKind::Of => StmtKind::ForOf(Box::new(ForOfStmt {
                target,
                iterable: source,
                body,
                scope,
                label: id,
                is_await,
            })),
        })
    }

    /// The target of a for-in/of head, whether it is a plain `var name`,
    /// and whether it destructures.
    fn parse_for_target(
        &mut self,
        kind: ForIterationKind,
        is_await: bool,
    ) -> Result<(ForTarget, bool, bool), Error> {
        if self.for_assignment_pattern_ahead(kind == ForIterationKind::Of) {
            let pattern = self.parse_assignment_pattern()?;
            return Ok((ForTarget::Pattern(pattern), false, true));
        }
        if self.lexical_declaration_ahead(true)? {
            let is_const = self.is_keyword(Keyword::Const);
            let declaring = if is_const {
                Declaring::Const
            } else {
                Declaring::Let
            };
            let var_kind = if is_const {
                VarKind::Const
            } else {
                VarKind::Let
            };
            self.advance()?;
            if self.is_punctuator(Punctuator::LeftBracket)
                || self.is_punctuator(Punctuator::LeftBrace)
            {
                let pattern = self.parse_binding_pattern(declaring, BindingSite::Iteration)?;
                return Ok((ForTarget::Declare(var_kind, pattern), false, true));
            }
            let token = *self.current();
            let TokenKind::Identifier(identifier) = token.kind else {
                return Err(self.syntax_here("variable name expected"));
            };
            self.validate_identifier_reservation(
                &identifier,
                token.span,
                self.strict(),
                IdentifierContext::Variable,
            )?;
            let name = self.intern_identifier(&identifier);
            self.advance()?;
            if self.is_strict_restricted_name(name) {
                return Err(Error::syntax(
                    "invalid variable name in strict mode",
                    self.current().span,
                ));
            }
            if self.names.name(name) == "let" {
                return Err(Error::syntax(
                    "invalid lexical variable name",
                    self.current().span,
                ));
            }
            let conflict = self.current().span;
            let decl_kind = if is_const {
                DeclKind::Const
            } else {
                DeclKind::Let
            };
            self.register_lexical(name, token.span, conflict, decl_kind, false)?;
            let pattern = binding(name, token.span, self.current_scope());
            return Ok((ForTarget::Declare(var_kind, pattern), false, false));
        }
        if self.is_keyword(Keyword::Var) {
            self.advance()?;
            if self.is_punctuator(Punctuator::LeftBracket)
                || self.is_punctuator(Punctuator::LeftBrace)
            {
                let pattern = self.parse_binding_pattern(Declaring::Var, BindingSite::Iteration)?;
                return Ok((ForTarget::Declare(VarKind::Var, pattern), false, true));
            }
            let token = *self.current();
            let TokenKind::Identifier(identifier) = token.kind else {
                return Err(self.syntax_here("variable name expected"));
            };
            self.validate_identifier_reservation(
                &identifier,
                token.span,
                self.strict(),
                IdentifierContext::Variable,
            )?;
            let name = self.intern_identifier(&identifier);
            self.advance()?;
            if self.is_strict_restricted_name(name) {
                return Err(Error::syntax(
                    "invalid variable name in strict mode",
                    self.current().span,
                ));
            }
            let conflict = self.current().span;
            self.register_var(name, token.span, conflict)?;
            let pattern = binding(name, token.span, self.current_scope());
            return Ok((ForTarget::Declare(VarKind::Var, pattern), true, false));
        }
        if let TokenKind::Identifier(identifier) = &self.current().kind
            && self.is_unescaped_name(identifier, "async")
            && !is_await
            && self.next_token_is_for_of_keyword()
        {
            return Err(Error::syntax(
                "'for of' expression cannot start with 'async'",
                self.current().span,
            ));
        }
        let target = self.parse_left_hand_side_expression()?;
        if target.contains_optional_chain() {
            return Err(self.syntax_here("invalid for in/of left hand-side"));
        }
        match target.reference_kind() {
            Some(ReferenceKind::Identifier(name)) => {
                if self.is_strict_restricted_name(name) {
                    return Err(self.syntax_here("invalid lvalue in strict mode"));
                }
            }
            Some(ReferenceKind::Member) => {}
            None => return Err(self.syntax_here("invalid for in/of left hand-side")),
        }
        Ok((ForTarget::Assign(target), false, false))
    }

    pub(super) fn parse_lexical_statement(&mut self) -> Result<Stmt, Error> {
        let start = self.current().span;
        let (kind, declarators) =
            self.with_in_mode(InMode::Allow, Self::parse_lexical_declarations)?;
        self.consume_statement_terminator()?;
        Ok(Stmt {
            kind: StmtKind::Var(kind, declarators),
            span: self.span_from(start).into(),
        })
    }

    pub(super) fn parse_lexical_declarations(
        &mut self,
    ) -> Result<(VarKind, Vec<VarDeclarator>), Error> {
        let is_const = self.is_keyword(Keyword::Const);
        let (kind, declaring, decl_kind) = if is_const {
            (VarKind::Const, Declaring::Const, DeclKind::Const)
        } else {
            (VarKind::Let, Declaring::Let, DeclKind::Let)
        };
        self.advance()?;
        let mut declarators = Vec::new();
        loop {
            if self.is_punctuator(Punctuator::LeftBracket)
                || self.is_punctuator(Punctuator::LeftBrace)
            {
                let (target, init, at) = self.parse_binding_declaration(declaring)?;
                declarators.push(VarDeclarator {
                    target,
                    init: Some(init),
                    pos: at,
                });
            } else {
                let token = *self.current();
                let TokenKind::Identifier(identifier) = token.kind else {
                    return Err(self.syntax_here("variable name expected"));
                };
                self.validate_identifier_reservation(
                    &identifier,
                    token.span,
                    self.strict(),
                    IdentifierContext::Variable,
                )?;
                if self.identical_name(&identifier, "let") {
                    return Err(Error::syntax(
                        "'let' is not a valid lexical identifier",
                        token.span,
                    ));
                }
                let name = self.intern_identifier(&identifier);
                self.advance()?;
                if self.is_strict_restricted_name(name) {
                    return Err(Error::syntax(
                        "invalid variable name in strict mode",
                        self.current().span,
                    ));
                }
                let conflict = self.current().span;
                self.register_lexical(name, token.span, conflict, decl_kind, false)?;
                let target = binding(name, token.span, self.current_scope());
                let equal = self.current().span;
                let (init, at) = if self.consume_punctuator(Punctuator::Equal)? {
                    (Some(self.parse_assignment()?), pos(equal))
                } else {
                    if is_const {
                        return Err(Error::syntax(
                            "missing initializer for const variable",
                            self.current().span,
                        ));
                    }
                    (None, pos(token.span))
                };
                declarators.push(VarDeclarator {
                    target,
                    init,
                    pos: at,
                });
            }
            if !self.consume_punctuator(Punctuator::Comma)? {
                break;
            }
        }
        Ok((kind, declarators))
    }

    pub(super) fn parse_var_declarations(&mut self) -> Result<Vec<VarDeclarator>, Error> {
        let mut declarators = Vec::new();
        loop {
            if self.is_punctuator(Punctuator::LeftBracket)
                || self.is_punctuator(Punctuator::LeftBrace)
            {
                let (target, init, at) = self.parse_binding_declaration(Declaring::Var)?;
                declarators.push(VarDeclarator {
                    target,
                    init: Some(init),
                    pos: at,
                });
            } else {
                let token = *self.current();
                let TokenKind::Identifier(identifier) = token.kind else {
                    return Err(self.syntax_here("variable name expected"));
                };
                self.validate_identifier_reservation(
                    &identifier,
                    token.span,
                    self.strict(),
                    IdentifierContext::Variable,
                )?;
                let name = self.intern_identifier(&identifier);
                self.advance()?;
                if self.is_strict_restricted_name(name) {
                    return Err(Error::syntax(
                        "invalid variable name in strict mode",
                        self.current().span,
                    ));
                }
                let conflict = self.current().span;
                self.register_var(name, token.span, conflict)?;
                let target = binding(name, token.span, self.current_scope());
                let equal = self.current().span;
                let (init, at) = if self.consume_punctuator(Punctuator::Equal)? {
                    (Some(self.parse_assignment()?), pos(equal))
                } else {
                    (None, pos(token.span))
                };
                declarators.push(VarDeclarator {
                    target,
                    init,
                    pos: at,
                });
            }
            if !self.consume_punctuator(Punctuator::Comma)? {
                break;
            }
        }
        Ok(declarators)
    }

    fn parse_return_statement(&mut self) -> Result<StmtKind, Error> {
        if self.state().is_static_block() {
            return Err(self.syntax_here("return in a static initializer block"));
        }
        self.advance()?;
        let has_value = !(self.current().line_terminator_before
            || self.at_eof()
            || self.is_punctuator(Punctuator::Semicolon)
            || self.is_punctuator(Punctuator::RightBrace));
        let value = if has_value {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.consume_statement_terminator()?;
        Ok(StmtKind::Return(value))
    }
}

pub(super) fn binding(
    name: crate::engine::compiler::names::NameId,
    span: Span,
    scope: ScopeId,
) -> Pattern {
    Pattern {
        kind: PatternKind::Binding(Ref {
            name,
            span: span.into(),
            scope,
        }),
        span,
    }
}
