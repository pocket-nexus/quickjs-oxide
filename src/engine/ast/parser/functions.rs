//! Function definitions, methods, arrows, formal parameters and bodies.

use super::support::{ForHeadDelimiter, IdentifierContext, for_head_regexp_allowed_after};
use super::{FunctionKind, Parser, StatementPosition, pos};
use crate::engine::api::error::Error;
use crate::engine::ast::{
    DeclKind, Expr, ExprKind, Function, Ident, MethodRole, Param, Pattern, PatternKind, Ref,
    ScopeKind, Stmt, StmtKind, Storage,
};
use crate::engine::compiler::lexer::{
    Identifier, Keyword, LexContext, Lexer, LexicalGoal, Punctuator, Span, TemplatePartKind,
    TokenKind,
};
use crate::engine::compiler::stack_guard::ParserStackFrame;

/// A parenthesized arrow probe that declined at its first token, kept until
/// the AssignmentExpression it began has been parsed.
#[derive(Clone)]
pub(super) struct DeclinedArrow<'source> {
    lexer: Lexer<'source>,
    first: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArrowHead {
    Identifier,
    Parenthesized,
}

/// What kind of function a definition starts.
#[derive(Clone, Copy, Debug)]
pub(super) struct FunctionOptions {
    pub(super) kind: FunctionKind,
    pub(super) role: MethodRole,
    pub(super) is_async: bool,
    pub(super) is_generator: bool,
    /// A named function expression binds its own name inside.
    pub(super) self_binding: bool,
    /// UniqueFormalParameters: methods reject duplicates even when sloppy.
    pub(super) reject_duplicate_parameters: bool,
}

impl FunctionOptions {
    pub(super) const fn method(role: MethodRole, is_async: bool, is_generator: bool) -> Self {
        Self {
            kind: FunctionKind::Method,
            role,
            is_async,
            is_generator,
            self_binding: false,
            reject_duplicate_parameters: matches!(role, MethodRole::Method)
                || is_async
                || is_generator,
        }
    }
}

/// The `function` keyword and name of a definition.
pub(super) struct FunctionHeader<'source> {
    pub(super) span: Span,
    pub(super) name: Option<(Identifier<'source>, Span)>,
    pub(super) is_async: bool,
    pub(super) is_generator: bool,
    pub(super) parent_context: LexContext,
}

impl<'source> Parser<'source> {
    pub(super) fn parse_function_expression(&mut self) -> Result<Expr, Error> {
        let header = self.parse_function_header(false)?;
        let span = header.span;
        let function = self.parse_function_tail(header, true)?;
        Ok(Expr {
            kind: ExprKind::Function(Box::new(function)),
            span: self.span_from(span),
        })
    }

    /// `[async] function [*] [name]`.
    pub(super) fn parse_function_header(
        &mut self,
        require_name: bool,
    ) -> Result<FunctionHeader<'source>, Error> {
        let parent_context = self.lexer.context();
        let span = self.current().span;
        let is_async = self.async_function_ahead();
        if is_async {
            self.advance()?;
        }
        self.advance()?;
        let is_generator = self.consume_punctuator(Punctuator::Multiply)?;
        let name = match self.current().kind {
            TokenKind::Identifier(identifier) => {
                let span = self.current().span;
                self.validate_identifier(
                    &identifier,
                    span,
                    false,
                    IdentifierContext::FunctionName,
                )?;
                self.advance()?;
                Some((identifier, span))
            }
            // QuickJS admits a contextual `yield` or `await` keyword token as
            // a sloppy function expression's name.
            TokenKind::Keyword(Keyword::Yield) if !require_name && !parent_context.strict => {
                let span = self.current().span;
                self.advance()?;
                Some((
                    Identifier {
                        raw: "yield",
                        has_escape: false,
                        keyword_hint: Some(Keyword::Yield),
                        escaped_reserved_word: false,
                    },
                    span,
                ))
            }
            TokenKind::Keyword(Keyword::Await) if !require_name && !parent_context.module => {
                let span = self.current().span;
                self.advance()?;
                Some((
                    Identifier {
                        raw: "await",
                        has_escape: false,
                        keyword_hint: Some(Keyword::Await),
                        escaped_reserved_word: false,
                    },
                    span,
                ))
            }
            _ => None,
        };
        if require_name && name.is_none() {
            return Err(self.syntax_here("function name expected"));
        }
        Ok(FunctionHeader {
            span,
            name,
            is_async,
            is_generator,
            parent_context,
        })
    }

    /// The parameters and body after a header; `expression` for a function
    /// expression, which binds its name inside.
    pub(super) fn parse_function_tail(
        &mut self,
        header: FunctionHeader<'source>,
        expression: bool,
    ) -> Result<Function, Error> {
        if expression
            && header.is_generator
            && let Some((identifier, span)) = &header.name
            && self.identical_name(identifier, "yield")
            && (!header.parent_context.generator
                || header.parent_context.strict
                || identifier.has_escape
                || identifier.keyword_hint != Some(Keyword::Yield))
        {
            return Err(crate::engine::api::error::Error::syntax(
                "'yield' is a reserved identifier",
                *span,
            ));
        }
        if expression
            && header.is_async
            && let Some((identifier, span)) = &header.name
            && self.identical_name(identifier, "await")
            && (!header.parent_context.async_function
                || header.parent_context.module
                || identifier.has_escape
                || identifier.keyword_hint != Some(Keyword::Await))
        {
            return Err(Error::syntax("'await' is a reserved identifier", *span));
        }
        let options = FunctionOptions {
            kind: FunctionKind::Normal,
            role: MethodRole::Method,
            is_async: header.is_async,
            is_generator: header.is_generator,
            self_binding: expression,
            reject_duplicate_parameters: false,
        };
        self.parse_function_with(header.span, header.name, header.parent_context, options)
    }

    /// A method, accessor or class constructor after its key.
    pub(super) fn parse_method(
        &mut self,
        span: Span,
        options: FunctionOptions,
    ) -> Result<Function, Error> {
        if (options.is_async || options.is_generator) && !self.is_punctuator(Punctuator::LeftParen)
        {
            return Err(self.syntax_here("invalid property name"));
        }
        let context = self.lexer.context();
        self.parse_function_with(span, None, context, options)
    }

    fn parse_function_with(
        &mut self,
        span: Span,
        name: Option<(Identifier<'source>, Span)>,
        parent_context: LexContext,
        options: FunctionOptions,
    ) -> Result<Function, Error> {
        let parent_strict = self.strict();
        let definition_scope = self.current_scope();
        let name_ident = name.as_ref().map(|(identifier, span)| Ident {
            name: self.intern_identifier(identifier),
            span: *span,
        });
        let id = self.begin_function(options.kind, parent_strict, Some(definition_scope));
        {
            let state = self.state_mut();
            state.is_async = options.is_async;
            state.is_generator = options.is_generator;
        }
        // A named function expression's own name, outside its parameters.
        let self_binding = match (options.self_binding, name_ident) {
            (true, Some(ident)) => {
                let scope = self.new_scope(None, ScopeKind::FunctionName, id);
                Some(self.add_decl(
                    scope,
                    scope,
                    ident.name,
                    DeclKind::FunctionName,
                    Storage::Local,
                    ident.span,
                ))
            }
            _ => None,
        };
        let mut child_context = parent_context;
        child_context.strict = parent_strict;
        child_context.generator = options.is_generator;
        child_context.async_function = options.is_async;
        self.relex_current_with_context(child_context)?;
        if self
            .parenthesized_parameter_scan()
            .is_some_and(|scan| scan.has_assignment)
        {
            self.activate_parameter_environment();
        }
        self.expect_punctuator(Punctuator::LeftParen)?;
        let accessor_arity = match options.role {
            MethodRole::Get => Some(0),
            MethodRole::Set => Some(1),
            MethodRole::Method => None,
        };
        let (params, rest, parameter_tokens, list_end) =
            self.parse_formal_parameters(options.role == MethodRole::Set)?;
        if accessor_arity.is_some_and(|arity| params.len() + usize::from(rest.is_some()) != arity) {
            return Err(Error::syntax(
                "invalid number of arguments for getter or setter",
                list_end,
            ));
        }
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let has_use_strict = self.directive_prologue_has_use_strict(parent_strict)?;
        let strict = parent_strict || has_use_strict;
        child_context.strict = strict;
        self.relex_current_with_context(child_context)?;
        self.check_function_parameters(
            has_use_strict,
            strict,
            name.as_ref().map(|(i, _)| i),
            &parameter_tokens,
            options.reject_duplicate_parameters,
        )?;
        self.state_mut().strict = strict;
        self.finish_parameters();
        self.state_mut().in_function_body = true;
        let body =
            self.parse_recursion(ParserStackFrame::FunctionBody, Self::parse_function_body)?;
        let closing = self.current().span;
        self.relex_current_with_context(parent_context)?;
        self.expect_punctuator(Punctuator::RightBrace)?;
        let state = self.end_function();
        Ok(Function {
            id,
            kind: options.kind,
            role: options.role,
            is_async: options.is_async,
            is_generator: options.is_generator,
            name: name_ident,
            self_binding,
            length: parameter_length(&params, rest.is_some()),
            params,
            rest,
            simple_params: state.simple_params,
            body,
            concise: false,
            strict,
            span,
            source: (pos(span), closing.end.byte_offset as u32),
        })
    }

    /// QuickJS's parentless parameter scope, for a list with a default.
    fn activate_parameter_environment(&mut self) {
        let function = self.state().id;
        let scope = self.new_scope(None, ScopeKind::Parameter, function);
        let state = self.state_mut();
        state.parameter_scope = Some(scope);
        state.current_scope = scope;
    }

    /// Pattern parameters evaluate in the function root.
    fn activate_pattern_parameter_initialization(&mut self) {
        let state = self.state_mut();
        if state.pattern_parameter_initialization {
            return;
        }
        state.pattern_parameter_initialization = true;
        if state.parameter_scope.is_none() {
            state.current_scope = state.var_scope;
        }
    }

    /// Return to the body scope; pattern parameters of a separate parameter
    /// environment get their body copies.
    fn finish_parameters(&mut self) {
        let (parameter_scope, var_scope, body_scope) = {
            let state = self.state();
            (state.parameter_scope, state.var_scope, state.body_scope)
        };
        if let Some(parameter_scope) = parameter_scope {
            let copies: Vec<_> = self.scopes[parameter_scope.index()]
                .decls
                .iter()
                .map(|decl| (self.decls[decl.index()].name, self.decls[decl.index()].span))
                .collect();
            for (name, span) in copies {
                if self.decl_in_scope(var_scope, name).is_none() {
                    self.add_decl(
                        var_scope,
                        var_scope,
                        name,
                        DeclKind::Var,
                        Storage::Local,
                        span,
                    );
                }
            }
        }
        self.state_mut().current_scope = body_scope;
    }

    /// The formal parameter list after `(`, through `)`.
    #[allow(clippy::type_complexity)]
    fn parse_formal_parameters(
        &mut self,
        setter: bool,
    ) -> Result<
        (
            Vec<Param>,
            Option<Pattern>,
            Vec<(Identifier<'source>, Span)>,
            Span,
        ),
        Error,
    > {
        let mut params = Vec::new();
        let mut rest = None;
        let mut tokens = Vec::new();
        let mut end = self.current().span;
        if self.is_punctuator(Punctuator::RightParen) {
            self.advance()?;
            return Ok((params, rest, tokens, end));
        }
        loop {
            let is_rest = self.consume_punctuator(Punctuator::Ellipsis)?;
            if is_rest && setter {
                return Err(Error::syntax(
                    "invalid number of arguments for getter or setter",
                    self.current().span,
                ));
            }
            if self.is_punctuator(Punctuator::LeftBracket)
                || self.is_punctuator(Punctuator::LeftBrace)
            {
                self.activate_pattern_parameter_initialization();
                self.state_mut().simple_params = false;
                let (pattern, default) = self.parse_parameter_pattern()?;
                if is_rest {
                    if default.is_some() {
                        return Err(Error::syntax(
                            "rest element cannot have a default value",
                            pattern.span,
                        ));
                    }
                    rest = Some(pattern);
                    if !self.is_punctuator(Punctuator::RightParen) {
                        return Err(self.syntax_here("expecting ')'"));
                    }
                    end = self.current().span;
                    self.advance()?;
                    break;
                }
                params.push(Param { pattern, default });
                if !self.consume_punctuator(Punctuator::Comma)? {
                    if !self.is_punctuator(Punctuator::RightParen) {
                        return Err(Error::syntax("expecting ','", self.current().span));
                    }
                    end = self.current().span;
                    self.advance()?;
                    break;
                }
                if self.is_punctuator(Punctuator::RightParen) {
                    end = self.current().span;
                    self.advance()?;
                    break;
                }
                continue;
            }
            let token = *self.current();
            let TokenKind::Identifier(identifier) = token.kind else {
                return Err(self.syntax_here("missing formal parameter"));
            };
            self.validate_identifier(&identifier, token.span, false, IdentifierContext::Argument)?;
            tokens.push((identifier, token.span));
            let name = self.intern_identifier(&identifier);
            self.advance()?;
            let pattern = self.identifier_parameter(name, token.span)?;
            if is_rest {
                self.state_mut().simple_params = false;
                rest = Some(pattern);
                if !self.is_punctuator(Punctuator::RightParen) {
                    return Err(self.syntax_here("expecting ')'"));
                }
                end = self.current().span;
                self.advance()?;
                break;
            }
            let default = if self.consume_punctuator(Punctuator::Equal)? {
                self.state_mut().simple_params = false;
                Some(self.parse_assignment_allow_in()?)
            } else {
                None
            };
            params.push(Param { pattern, default });
            if !self.consume_punctuator(Punctuator::Comma)? {
                if !self.is_punctuator(Punctuator::RightParen) {
                    return Err(Error::syntax("expecting ','", self.current().span));
                }
                end = self.current().span;
                self.advance()?;
                break;
            }
            if self.is_punctuator(Punctuator::RightParen) {
                end = self.current().span;
                self.advance()?;
                break;
            }
        }
        Ok((params, rest, tokens, end))
    }

    fn identifier_parameter(
        &mut self,
        name: crate::engine::compiler::names::NameId,
        span: Span,
    ) -> Result<Pattern, Error> {
        self.register_identifier_parameter(name, span)?;
        Ok(Pattern {
            kind: PatternKind::Binding(Ref {
                name,
                span,
                scope: self.current_scope(),
            }),
            span,
        })
    }

    /// The checks QuickJS makes once the body's strictness is known, at the
    /// body's first token.
    fn check_function_parameters(
        &self,
        has_use_strict: bool,
        strict: bool,
        name: Option<&Identifier<'source>>,
        parameter_tokens: &[(Identifier<'source>, Span)],
        reject_duplicates: bool,
    ) -> Result<(), Error> {
        let simple = self.state().simple_params;
        if has_use_strict && !simple {
            return Err(Error::syntax(
                "\"use strict\" not allowed in function with default or destructuring parameter",
                self.current().span,
            ));
        }
        let here = self.current().span;
        if strict {
            if let Some(identifier) = name {
                self.validate_identifier(identifier, here, true, IdentifierContext::FunctionName)?;
            }
            for (identifier, _) in parameter_tokens {
                self.validate_identifier(identifier, here, true, IdentifierContext::Argument)?;
            }
        }
        if reject_duplicates || strict || !simple {
            let names = &self.state().parameter_names;
            for (index, name) in names.iter().enumerate() {
                if names[..index].contains(name) {
                    return Err(Error::syntax(
                        "duplicate argument names not allowed in this context",
                        here,
                    ));
                }
            }
        }
        Ok(())
    }

    /// The statements of a function body, up to its `}`.
    pub(super) fn parse_function_body(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut body = Vec::new();
        while !self.is_punctuator(Punctuator::RightBrace) {
            if self.at_eof() {
                return Err(self.syntax_here("unterminated function body"));
            }
            body.push(self.parse_statement_or_decl(StatementPosition::FunctionBody)?);
        }
        Ok(body)
    }

    // ---- arrows ----

    pub(super) fn parse_arrow_function(&mut self, head: ArrowHead) -> Result<Expr, Error> {
        let span = self.current().span;
        let context = self.lexer.context();
        self.parse_arrow_with(head, span, context, false)
    }

    pub(super) fn parse_async_arrow_function(&mut self, head: ArrowHead) -> Result<Expr, Error> {
        let span = self.current().span;
        let context = self.lexer.context();
        // QuickJS commits the token after `async` in the parent.
        self.advance()?;
        self.parse_arrow_with(head, span, context, true)
    }

    fn parse_arrow_with(
        &mut self,
        head: ArrowHead,
        span: Span,
        parent_context: LexContext,
        is_async: bool,
    ) -> Result<Expr, Error> {
        let parent_strict = self.strict();
        let parent_generator = self.state().is_generator;
        let parent_async = self.state().is_async;
        let parent_static_block = self.state().is_static_block();
        let definition_scope = self.current_scope();
        let id = self.begin_function(FunctionKind::Arrow, parent_strict, Some(definition_scope));
        self.state_mut().is_async = is_async;
        let mut parameter_context = parent_context;
        parameter_context.strict = parent_strict;
        // Arrow parameters classify `yield`/`await` from the arrow and its
        // immediate parent.
        parameter_context.generator = parent_generator;
        parameter_context.async_function = is_async || parent_async || parent_static_block;
        self.set_future_lex_context(parameter_context);

        let mut params = Vec::new();
        let mut rest = None;
        let mut tokens = Vec::new();
        match head {
            ArrowHead::Identifier => {
                let token = *self.current();
                let TokenKind::Identifier(identifier) = token.kind else {
                    return Err(Error::internal(
                        "identifier arrow lookahead lost its parameter token",
                    ));
                };
                self.validate_identifier(
                    &identifier,
                    token.span,
                    false,
                    IdentifierContext::Argument,
                )?;
                let name = self.intern_identifier(&identifier);
                let pattern = self.identifier_parameter(name, token.span)?;
                params.push(Param {
                    pattern,
                    default: None,
                });
                tokens.push((identifier, token.span));
                self.advance()?;
            }
            ArrowHead::Parenthesized => {
                if self
                    .parenthesized_parameter_scan()
                    .is_some_and(|scan| scan.has_assignment)
                {
                    self.activate_parameter_environment();
                }
                self.expect_punctuator(Punctuator::LeftParen)?;
                if !self.consume_punctuator(Punctuator::RightParen)? {
                    loop {
                        let is_rest = self.consume_punctuator(Punctuator::Ellipsis)?;
                        if self.is_punctuator(Punctuator::LeftBracket)
                            || self.is_punctuator(Punctuator::LeftBrace)
                        {
                            self.activate_pattern_parameter_initialization();
                            self.state_mut().simple_params = false;
                            let (pattern, default) = self.parse_parameter_pattern()?;
                            if is_rest {
                                if default.is_some() {
                                    return Err(Error::syntax(
                                        "rest element cannot have a default value",
                                        pattern.span,
                                    ));
                                }
                                rest = Some(pattern);
                                if !self.is_punctuator(Punctuator::RightParen) {
                                    return Err(self.syntax_here("expecting ')'"));
                                }
                                self.advance()?;
                                break;
                            }
                            params.push(Param { pattern, default });
                            if self.consume_punctuator(Punctuator::RightParen)? {
                                break;
                            }
                            self.expect_punctuator(Punctuator::Comma)?;
                            if self.consume_punctuator(Punctuator::RightParen)? {
                                break;
                            }
                            continue;
                        }
                        let token = *self.current();
                        let TokenKind::Identifier(identifier) = token.kind else {
                            return Err(self.syntax_here("missing formal parameter"));
                        };
                        self.validate_identifier(
                            &identifier,
                            token.span,
                            false,
                            IdentifierContext::Argument,
                        )?;
                        let name = self.intern_identifier(&identifier);
                        tokens.push((identifier, token.span));
                        self.advance()?;
                        let pattern = self.identifier_parameter(name, token.span)?;
                        if is_rest {
                            self.state_mut().simple_params = false;
                            rest = Some(pattern);
                            if !self.is_punctuator(Punctuator::RightParen) {
                                return Err(self.syntax_here("expecting ')'"));
                            }
                            self.advance()?;
                            break;
                        }
                        let default = if self.consume_punctuator(Punctuator::Equal)? {
                            self.state_mut().simple_params = false;
                            Some(self.parse_assignment_allow_in()?)
                        } else {
                            None
                        };
                        params.push(Param { pattern, default });
                        if self.consume_punctuator(Punctuator::RightParen)? {
                            break;
                        }
                        self.expect_punctuator(Punctuator::Comma)?;
                        if self.consume_punctuator(Punctuator::RightParen)? {
                            break;
                        }
                    }
                }
            }
        }
        if !self.is_punctuator(Punctuator::Arrow) || self.current().line_terminator_before {
            return Err(self.syntax_here("expecting '=>'"));
        }
        self.advance_expression_start()?;
        let mut child_context = parent_context;
        child_context.strict = parent_strict;
        child_context.generator = false;
        child_context.async_function = is_async;
        self.relex_current_with_context(child_context)?;
        let block = self.is_punctuator(Punctuator::LeftBrace);
        if block {
            self.advance()?;
        }
        let has_use_strict = block && self.directive_prologue_has_use_strict(parent_strict)?;
        let strict = parent_strict || has_use_strict;
        if block {
            child_context.strict = strict;
            self.relex_current_with_context(child_context)?;
        }
        self.check_function_parameters(has_use_strict, strict, None, &tokens, true)?;
        self.state_mut().strict = strict;
        self.finish_parameters();
        self.state_mut().in_function_body = true;
        let (body, end) = self.parse_recursion(ParserStackFrame::Arrow, |parser| {
            if block {
                let body = parser
                    .parse_recursion(ParserStackFrame::ArrowBlockBody, Self::parse_function_body)?;
                let closing = parser.current().span;
                parser.relex_current_with_context(parent_context)?;
                parser.expect_punctuator(Punctuator::RightBrace)?;
                Ok((body, closing.end.byte_offset))
            } else {
                let value = parser.parse_assignment()?;
                let end = parser
                    .previous_end
                    .unwrap_or(parser.current().span.start.byte_offset);
                parser.relex_current_with_context(parent_context)?;
                let span = value.span;
                Ok((
                    vec![Stmt {
                        kind: StmtKind::Return(Some(value)),
                        span,
                    }],
                    end,
                ))
            }
        })?;
        let state = self.end_function();
        let function = Function {
            id,
            kind: FunctionKind::Arrow,
            role: MethodRole::Method,
            is_async,
            is_generator: false,
            name: None,
            self_binding: None,
            length: parameter_length(&params, rest.is_some()),
            params,
            rest,
            simple_params: state.simple_params,
            body,
            concise: !block,
            strict,
            span,
            source: (pos(span), end as u32),
        };
        Ok(Expr {
            kind: ExprKind::Function(Box::new(function)),
            span: self.span_from(span),
        })
    }

    /// Whether the current token starts an arrow function.
    pub(super) fn arrow_head_ahead(&self) -> Option<ArrowHead> {
        match &self.current().kind {
            TokenKind::Identifier(_) => {
                if !self.arrow_may_follow_current() {
                    return None;
                }
                let next = self.token_after_current().ok()?;
                (!next.line_terminator_before
                    && matches!(next.kind, TokenKind::Punctuator(Punctuator::Arrow)))
                .then_some(ArrowHead::Identifier)
            }
            TokenKind::Punctuator(Punctuator::LeftParen)
                if self.parenthesized_arrow_ahead(self.current().span) =>
            {
                Some(ArrowHead::Parenthesized)
            }
            _ => None,
        }
    }

    fn arrow_may_follow_current(&self) -> bool {
        let bytes = self.lexer.source().as_bytes();
        let mut offset = self.current().span.end.byte_offset;
        while matches!(bytes.get(offset), Some(b' ' | b'\t' | 0x0b | 0x0c)) {
            offset += 1;
        }
        match bytes.get(offset) {
            Some(b'=') => bytes.get(offset + 1) == Some(&b'>'),
            Some(b'/' | b'<' | b'-' | 0x80..=0xff) => true,
            _ => false,
        }
    }

    pub(super) fn async_arrow_ahead(&self) -> Option<ArrowHead> {
        let TokenKind::Identifier(identifier) = &self.current().kind else {
            return None;
        };
        if !self.is_unescaped_name(identifier, "async") {
            return None;
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.end);
        let parameter = self.probe_token(&mut lexer, LexicalGoal::Div).ok()?;
        if parameter.line_terminator_before {
            return None;
        }
        match parameter.kind {
            TokenKind::Identifier(identifier) if !identifier.escaped_reserved_word => {
                let arrow = self.probe_token(&mut lexer, LexicalGoal::Div).ok()?;
                (!arrow.line_terminator_before
                    && matches!(arrow.kind, TokenKind::Punctuator(Punctuator::Arrow)))
                .then_some(ArrowHead::Identifier)
            }
            TokenKind::Punctuator(Punctuator::LeftParen) => self
                .parenthesized_arrow_ahead(parameter.span)
                .then_some(ArrowHead::Parenthesized),
            _ => None,
        }
    }

    fn parenthesized_arrow_ahead(&self, opening: Span) -> bool {
        if let Some(arrow) = self.cached_parenthesized_arrow(opening.start.byte_offset) {
            return arrow;
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(opening.start);
        let at_opening = lexer.clone();
        let Ok(first) = self.probe_token(&mut lexer, LexicalGoal::Div) else {
            return false;
        };
        if !matches!(first.kind, TokenKind::Punctuator(Punctuator::LeftParen)) {
            return false;
        }
        if let Some(first) = self.first_token_rejecting_arrow_parameters(&lexer) {
            self.lookahead.borrow_mut().declined_arrow = Some(DeclinedArrow {
                lexer: at_opening,
                first,
            });
            return false;
        }
        self.scan_parenthesized_arrow(lexer, opening.start.byte_offset, true)
    }

    fn first_token_rejecting_arrow_parameters(&self, lexer: &Lexer<'source>) -> Option<Span> {
        let mut lexer = lexer.clone();
        let token = self.probe_token(&mut lexer, LexicalGoal::Div).ok()?;
        let rejects = match token.kind {
            TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::Template(_)
            | TokenKind::RegExp(_) => true,
            TokenKind::Punctuator(punctuator) => !matches!(
                punctuator,
                Punctuator::LeftBracket
                    | Punctuator::LeftBrace
                    | Punctuator::Ellipsis
                    | Punctuator::RightParen
            ),
            TokenKind::Keyword(keyword) => matches!(
                keyword,
                Keyword::Null
                    | Keyword::False
                    | Keyword::True
                    | Keyword::This
                    | Keyword::Delete
                    | Keyword::Void
                    | Keyword::Typeof
                    | Keyword::New
                    | Keyword::Function
                    | Keyword::Class
                    | Keyword::Super
                    | Keyword::Import
            ),
            _ => false,
        };
        rejects.then_some(token.span)
    }

    /// The diagnostic QuickJS reports for a declined probe whose parentheses
    /// turn out to be followed by `=>`.
    pub(super) fn declined_arrow_error(&self, declined: DeclinedArrow<'source>) -> Option<Error> {
        let mut lexer = declined.lexer;
        let opening = lexer.current_position().byte_offset;
        self.probe_token(&mut lexer, LexicalGoal::Div).ok()?;
        self.scan_parenthesized_arrow(lexer, opening, false)
            .then(|| Error::syntax("missing formal parameter", declined.first))
    }

    fn scan_parenthesized_arrow(
        &self,
        mut lexer: Lexer<'source>,
        opening: usize,
        cache: bool,
    ) -> bool {
        let mut delimiters = vec![ForHeadDelimiter::Parenthesis];
        let mut parentheses = vec![opening];
        let mut closed_parenthesis = None;
        let mut goal = LexicalGoal::Div;
        let mut regexp_allowed = true;
        loop {
            let requested = goal;
            goal = LexicalGoal::Div;
            let Ok(mut token) = self.probe_token(&mut lexer, requested) else {
                return false;
            };
            if let Some(start) = closed_parenthesis.take()
                && cache
            {
                self.cache_parenthesized_arrow(
                    start,
                    !token.line_terminator_before
                        && matches!(token.kind, TokenKind::Punctuator(Punctuator::Arrow)),
                );
            }
            if requested == LexicalGoal::Div
                && regexp_allowed
                && matches!(
                    token.kind,
                    TokenKind::Punctuator(Punctuator::Divide | Punctuator::DivideAssign)
                )
            {
                lexer.seek(token.span.start);
                let Ok(regexp) = self.probe_token(&mut lexer, LexicalGoal::RegExp) else {
                    return false;
                };
                token = regexp;
            }
            match &token.kind {
                TokenKind::Punctuator(Punctuator::LeftParen) => {
                    if delimiters.len() >= 255 {
                        return false;
                    }
                    delimiters.push(ForHeadDelimiter::Parenthesis);
                    parentheses.push(token.span.start.byte_offset);
                }
                TokenKind::Punctuator(Punctuator::LeftBracket) => {
                    if delimiters.len() >= 255 {
                        return false;
                    }
                    delimiters.push(ForHeadDelimiter::Bracket);
                }
                TokenKind::Punctuator(Punctuator::LeftBrace) => {
                    if delimiters.len() >= 255 {
                        return false;
                    }
                    delimiters.push(ForHeadDelimiter::Brace);
                }
                TokenKind::Punctuator(Punctuator::RightParen) => {
                    if delimiters.pop() != Some(ForHeadDelimiter::Parenthesis) {
                        return false;
                    }
                    let start = parentheses.pop().expect("balanced parenthesis probe");
                    if delimiters.is_empty() {
                        let Ok(arrow) = self.probe_token(&mut lexer, LexicalGoal::Div) else {
                            return false;
                        };
                        let result = !arrow.line_terminator_before
                            && matches!(arrow.kind, TokenKind::Punctuator(Punctuator::Arrow));
                        if cache {
                            self.cache_parenthesized_arrow(start, result);
                        }
                        return result;
                    }
                    closed_parenthesis = Some(start);
                }
                TokenKind::Punctuator(Punctuator::RightBracket) => {
                    if delimiters.pop() != Some(ForHeadDelimiter::Bracket) {
                        return false;
                    }
                }
                TokenKind::Punctuator(Punctuator::RightBrace) => {
                    if delimiters.last() == Some(&ForHeadDelimiter::Template) {
                        goal = LexicalGoal::TemplateContinuation;
                        regexp_allowed = true;
                        continue;
                    }
                    if delimiters.pop() != Some(ForHeadDelimiter::Brace) {
                        return false;
                    }
                }
                TokenKind::Template(part) => match part.kind {
                    TemplatePartKind::Head => {
                        if delimiters.len() >= 255 {
                            return false;
                        }
                        delimiters.push(ForHeadDelimiter::Template);
                    }
                    TemplatePartKind::Middle => {
                        if delimiters.last() != Some(&ForHeadDelimiter::Template) {
                            return false;
                        }
                    }
                    TemplatePartKind::Tail => {
                        if delimiters.pop() != Some(ForHeadDelimiter::Template) {
                            return false;
                        }
                    }
                    TemplatePartKind::NoSubstitution => {}
                },
                TokenKind::Eof => return false,
                _ => {}
            }
            regexp_allowed = for_head_regexp_allowed_after(&token.kind);
        }
    }
}

/// `Function.length`: the formals before the first default or rest.
fn parameter_length(params: &[Param], _has_rest: bool) -> u32 {
    params
        .iter()
        .take_while(|param| param.default.is_none())
        .count() as u32
}
