//! Tokens, lexical goals, lookahead probes and identifier diagnostics.

use super::{FunctionKind, InMode, Parser};
use crate::engine::api::error::{Error, ErrorKind, NativeErrorMessage};
use crate::engine::ast::lexer::{
    Identifier, Keyword, LexContext, LexError, LexErrorKind, LexicalGoal, NumberKind,
    NumberLiteral, NumericRadix, Punctuator, Span, TemplatePart, TemplatePartKind, Token,
    TokenKind, quickjs_simple_lookahead_is_of,
};
use crate::engine::ast::names::NameId;
use crate::engine::ast::stack_guard::ParserStackFrame;
use crate::engine::value::bigint::JsBigInt;
use crate::engine::value::{JsString, JsStringError};
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ForHeadDelimiter {
    Parenthesis,
    Bracket,
    Brace,
    Template,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ForIterationKind {
    In,
    Of,
}

#[derive(Clone, Copy)]
pub(super) enum IdentifierContext {
    Reference,
    Variable,
    FunctionName,
    Argument,
}

/// A parsed numeric literal.
pub(super) enum Number {
    Number(f64),
    BigInt(JsBigInt),
}

impl<'source> Parser<'source> {
    pub(super) fn expect_punctuator(&mut self, punctuator: Punctuator) -> Result<(), Error> {
        if self.consume_punctuator(punctuator)? {
            Ok(())
        } else {
            Err(self.syntax_here(format!("expecting '{}'", punctuator.as_str())))
        }
    }

    pub(super) fn consume_punctuator(&mut self, punctuator: Punctuator) -> Result<bool, Error> {
        if self.is_punctuator(punctuator) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(super) fn is_punctuator(&self, punctuator: Punctuator) -> bool {
        matches!(self.current().kind, TokenKind::Punctuator(current) if current == punctuator)
    }

    pub(super) fn is_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(current) if current == keyword)
    }

    /// Decoded identifier text; a private identifier without its `#`.
    pub(super) fn identifier_text(&self, identifier: &Identifier<'source>) -> Cow<'source, str> {
        let raw = identifier.raw.strip_prefix('#').unwrap_or(identifier.raw);
        if !identifier.has_escape {
            return Cow::Borrowed(raw);
        }
        Cow::Owned(self.lexer.decode_identifier_text(identifier.raw))
    }

    pub(super) fn intern_identifier(&mut self, identifier: &Identifier<'source>) -> NameId {
        let raw = identifier.raw.strip_prefix('#').unwrap_or(identifier.raw);
        if !identifier.has_escape {
            return self.names.intern(raw);
        }
        let decoded = self.lexer.decode_identifier_text(identifier.raw);
        self.names.intern(&decoded)
    }

    /// The `#name` spelling of a private identifier, its binding name.
    pub(super) fn intern_private_identifier(&mut self, identifier: &Identifier<'source>) -> NameId {
        if !identifier.has_escape {
            return self.names.intern(identifier.raw);
        }
        let mut binding = String::with_capacity(identifier.raw.len().saturating_add(1));
        binding.push('#');
        binding.push_str(&self.lexer.decode_identifier_text(identifier.raw));
        self.names.intern(&binding)
    }

    pub(super) fn name_string(&mut self, name: NameId) -> Result<JsString, Error> {
        Ok(self.names.js_string(name)?)
    }

    pub(super) fn identical_name(&self, identifier: &Identifier<'source>, expected: &str) -> bool {
        self.identifier_text(identifier) == expected
    }

    pub(super) fn is_unescaped_name(
        &self,
        identifier: &Identifier<'source>,
        expected: &str,
    ) -> bool {
        !identifier.has_escape && identifier.raw == expected
    }

    pub(super) fn is_contextual_keyword(&self, expected: &str) -> bool {
        matches!(
            &self.current().kind,
            TokenKind::Identifier(identifier) if self.is_unescaped_name(identifier, expected)
        )
    }

    pub(super) fn decode_string_literal(&self, span: Span) -> Result<JsString, Error> {
        let body = span.start.byte_offset + 1..span.end.byte_offset - 1;
        if !self.lexer.source().as_bytes()[body.clone()].contains(&b'\\') {
            return self
                .lexer
                .source_range_to_js_string(body)?
                .ok_or_else(|| Error::internal("invalid string literal source range"));
        }
        let value = self
            .lexer
            .decode_string_literal(span.start)
            .map_err(lex_error)?;
        Ok(JsString::try_from_utf16(value.utf16)?)
    }

    pub(super) fn decode_template_raw_value(
        &self,
        part: &TemplatePart<'source>,
        span: Span,
    ) -> Result<JsString, Error> {
        let value = self
            .lexer
            .decode_template_raw_value(span.start, template_part_is_initial(part.kind))
            .map_err(lex_error)?;
        Ok(JsString::try_from_utf16(value.utf16)?)
    }

    pub(super) fn decode_template_cooked(
        &self,
        part: &TemplatePart<'source>,
        span: Span,
    ) -> Result<Option<JsString>, Error> {
        if part.invalid_escape {
            return Ok(None);
        }
        let value = self
            .lexer
            .decode_template_cooked_value(span.start, template_part_is_initial(part.kind))
            .map_err(lex_error)?;
        Ok(Some(JsString::try_from_utf16(value.utf16)?))
    }

    pub(super) fn is_for_of_keyword(&self) -> bool {
        matches!(
            &self.current().kind,
            TokenKind::Identifier(identifier) if self.is_unescaped_name(identifier, "of")
        )
    }

    pub(super) fn next_token_is_for_of_keyword(&self) -> bool {
        self.lexer
            .source()
            .get(self.current().span.end.byte_offset..)
            .is_some_and(quickjs_simple_lookahead_is_of)
    }

    /// The first `in` or `of` at the head's outer depth, without committing.
    pub(super) fn for_iteration_kind_ahead(&self) -> Option<ForIterationKind> {
        if let Some((start, context, hint)) = self.lookahead.borrow().iteration_hint
            && start == self.current().span.start.byte_offset
            && context == self.lexer.context()
        {
            return hint;
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.start);
        let mut delimiters = Vec::new();
        let mut goal = LexicalGoal::Div;
        let mut regexp_allowed = true;
        loop {
            let requested = goal;
            goal = LexicalGoal::Div;
            let Ok(mut token) = self.probe_token(&mut lexer, requested) else {
                return None;
            };
            if requested == LexicalGoal::Div
                && regexp_allowed
                && matches!(
                    token.kind,
                    TokenKind::Punctuator(Punctuator::Divide | Punctuator::DivideAssign)
                )
            {
                lexer.seek(token.span.start);
                let Ok(regexp) = self.probe_token(&mut lexer, LexicalGoal::RegExp) else {
                    return None;
                };
                token = regexp;
            }
            if delimiters.is_empty() {
                match &token.kind {
                    TokenKind::Keyword(Keyword::In) => return Some(ForIterationKind::In),
                    TokenKind::Identifier(identifier)
                        if self.is_unescaped_name(identifier, "of") =>
                    {
                        return Some(ForIterationKind::Of);
                    }
                    TokenKind::Punctuator(Punctuator::RightParen) | TokenKind::Eof => return None,
                    _ => {}
                }
            }
            match &token.kind {
                TokenKind::Punctuator(Punctuator::LeftParen) => {
                    delimiters.push(ForHeadDelimiter::Parenthesis);
                }
                TokenKind::Punctuator(Punctuator::LeftBracket) => {
                    delimiters.push(ForHeadDelimiter::Bracket);
                }
                TokenKind::Punctuator(Punctuator::LeftBrace) => {
                    delimiters.push(ForHeadDelimiter::Brace);
                }
                TokenKind::Punctuator(Punctuator::RightParen) => {
                    if delimiters.pop() != Some(ForHeadDelimiter::Parenthesis) {
                        return None;
                    }
                }
                TokenKind::Punctuator(Punctuator::RightBracket) => {
                    if delimiters.pop() != Some(ForHeadDelimiter::Bracket) {
                        return None;
                    }
                }
                TokenKind::Punctuator(Punctuator::RightBrace) => {
                    if delimiters.last() == Some(&ForHeadDelimiter::Template) {
                        goal = LexicalGoal::TemplateContinuation;
                        regexp_allowed = true;
                        continue;
                    }
                    if delimiters.pop() != Some(ForHeadDelimiter::Brace) {
                        return None;
                    }
                }
                TokenKind::Template(part) => match part.kind {
                    TemplatePartKind::Head => delimiters.push(ForHeadDelimiter::Template),
                    TemplatePartKind::Middle => {
                        if delimiters.last() != Some(&ForHeadDelimiter::Template) {
                            return None;
                        }
                    }
                    TemplatePartKind::Tail => {
                        if delimiters.pop() != Some(ForHeadDelimiter::Template) {
                            return None;
                        }
                    }
                    TemplatePartKind::NoSubstitution => {}
                },
                TokenKind::Eof => return None,
                _ => {}
            }
            regexp_allowed = for_head_regexp_allowed_after(&token.kind);
        }
    }

    /// QuickJS `js_parse_skip_parens_token` for classic `for`: any `;` at the
    /// outer head depth selects the classic grammar.
    pub(super) fn for_head_has_top_level_semicolon(&self) -> bool {
        if !self.is_punctuator(Punctuator::LeftParen) {
            return false;
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.start);
        let mut delimiters: Vec<ForHeadDelimiter> = Vec::new();
        let mut goal = LexicalGoal::Div;
        let mut regexp_allowed = true;
        let mut has_semicolon = false;
        let mut inner_start = None;
        let mut iteration_hint = None;
        let has_semicolon = (|| {
            loop {
                let requested = goal;
                goal = LexicalGoal::Div;
                let Ok(mut token) = self.probe_token(&mut lexer, requested) else {
                    return has_semicolon;
                };
                if requested == LexicalGoal::Div
                    && regexp_allowed
                    && matches!(
                        token.kind,
                        TokenKind::Punctuator(Punctuator::Divide | Punctuator::DivideAssign)
                    )
                {
                    lexer.seek(token.span.start);
                    let Ok(regexp) = self.probe_token(&mut lexer, LexicalGoal::RegExp) else {
                        return has_semicolon;
                    };
                    token = regexp;
                }
                if delimiters.len() == 1 {
                    inner_start.get_or_insert(token.span.start.byte_offset);
                    if iteration_hint.is_none() {
                        iteration_hint = match &token.kind {
                            TokenKind::Keyword(Keyword::In) => Some(ForIterationKind::In),
                            TokenKind::Identifier(identifier)
                                if self.is_unescaped_name(identifier, "of") =>
                            {
                                Some(ForIterationKind::Of)
                            }
                            _ => None,
                        };
                    }
                }
                match &token.kind {
                    TokenKind::Punctuator(Punctuator::LeftParen) => {
                        if delimiters.len() >= 255 {
                            return has_semicolon;
                        }
                        delimiters.push(ForHeadDelimiter::Parenthesis);
                    }
                    TokenKind::Punctuator(Punctuator::LeftBracket) => {
                        if delimiters.len() >= 255 {
                            return has_semicolon;
                        }
                        delimiters.push(ForHeadDelimiter::Bracket);
                    }
                    TokenKind::Punctuator(Punctuator::LeftBrace) => {
                        if delimiters.len() >= 255 {
                            return has_semicolon;
                        }
                        delimiters.push(ForHeadDelimiter::Brace);
                    }
                    TokenKind::Punctuator(Punctuator::RightParen) => {
                        if delimiters.pop() != Some(ForHeadDelimiter::Parenthesis) {
                            return has_semicolon;
                        }
                        if delimiters.is_empty() {
                            return has_semicolon;
                        }
                    }
                    TokenKind::Punctuator(Punctuator::RightBracket) => {
                        if delimiters.pop() != Some(ForHeadDelimiter::Bracket) {
                            return has_semicolon;
                        }
                    }
                    TokenKind::Punctuator(Punctuator::RightBrace) => {
                        if delimiters.last() == Some(&ForHeadDelimiter::Template) {
                            goal = LexicalGoal::TemplateContinuation;
                            regexp_allowed = true;
                            continue;
                        }
                        if delimiters.pop() != Some(ForHeadDelimiter::Brace) {
                            return has_semicolon;
                        }
                    }
                    TokenKind::Punctuator(Punctuator::Semicolon) if delimiters.len() == 1 => {
                        has_semicolon = true;
                    }
                    TokenKind::Template(part) => match part.kind {
                        TemplatePartKind::Head => {
                            if delimiters.len() >= 255 {
                                return has_semicolon;
                            }
                            delimiters.push(ForHeadDelimiter::Template);
                        }
                        TemplatePartKind::Middle => {
                            if delimiters.last() != Some(&ForHeadDelimiter::Template) {
                                return has_semicolon;
                            }
                        }
                        TemplatePartKind::Tail => {
                            if delimiters.pop() != Some(ForHeadDelimiter::Template) {
                                return has_semicolon;
                            }
                        }
                        TemplatePartKind::NoSubstitution => {}
                    },
                    TokenKind::Eof => return has_semicolon,
                    _ => {}
                }
                regexp_allowed = for_head_regexp_allowed_after(&token.kind);
            }
        })();
        if let Some(start) = inner_start {
            self.lookahead.borrow_mut().iteration_hint =
                iteration_hint.map(|hint| (start, self.lexer.context(), Some(hint)));
        }
        has_semicolon
    }

    /// QuickJS `is_let`: sloppy `let` starts a declaration before `[`, `{`,
    /// an identifier, `let`, `yield` or `await`.
    pub(super) fn lexical_declaration_ahead(
        &self,
        allow_line_terminated_other: bool,
    ) -> Result<bool, Error> {
        if matches!(
            self.current().kind,
            TokenKind::Keyword(Keyword::Let | Keyword::Const)
        ) {
            return Ok(true);
        }
        let TokenKind::Identifier(identifier) = &self.current().kind else {
            return Ok(false);
        };
        if !self.is_unescaped_name(identifier, "let") {
            return Ok(false);
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.start);
        self.probe_token(&mut lexer, LexicalGoal::Div)
            .map_err(lex_error)?;
        let next = self
            .probe_token(&mut lexer, LexicalGoal::Div)
            .map_err(lex_error)?;
        let other_declaration_start = matches!(
            &next.kind,
            TokenKind::Punctuator(Punctuator::LeftBrace)
                | TokenKind::Identifier(Identifier {
                    escaped_reserved_word: false,
                    ..
                })
                | TokenKind::Keyword(Keyword::Let | Keyword::Yield | Keyword::Await)
        );
        Ok(
            matches!(&next.kind, TokenKind::Punctuator(Punctuator::LeftBracket))
                || (other_declaration_start
                    && (!next.line_terminator_before || allow_line_terminated_other)),
        )
    }

    /// QuickJS `is_label`: a non-reserved identifier followed by `:`.
    pub(super) fn label_ahead(&self) -> Option<String> {
        let TokenKind::Identifier(identifier) = &self.current().kind else {
            return None;
        };
        if identifier.escaped_reserved_word {
            return None;
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.end);
        let Ok(next) = self.probe_token(&mut lexer, LexicalGoal::Div) else {
            return None;
        };
        matches!(next.kind, TokenKind::Punctuator(Punctuator::Colon))
            .then(|| self.identifier_text(identifier).into_owned())
    }

    pub(super) fn restricted_function_declaration_ahead(
        &self,
        annex_b_function_allowed: bool,
    ) -> Result<bool, Error> {
        let generator = self.is_keyword(Keyword::Function);
        let async_function = self.async_function_ahead();
        if (!generator || !annex_b_function_allowed) && !async_function {
            return Ok(false);
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.end);
        let next = self
            .probe_token(&mut lexer, LexicalGoal::Div)
            .map_err(lex_error)?;
        if generator {
            Ok(matches!(
                next.kind,
                TokenKind::Punctuator(Punctuator::Multiply)
            ))
        } else {
            Ok(async_function)
        }
    }

    /// `async function` without a line break between.
    pub(super) fn async_function_ahead(&self) -> bool {
        let TokenKind::Identifier(identifier) = &self.current().kind else {
            return false;
        };
        if !self.is_unescaped_name(identifier, "async") {
            return false;
        }
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.end);
        let Ok(function) = self.probe_token(&mut lexer, LexicalGoal::Div) else {
            return false;
        };
        !function.line_terminator_before
            && matches!(function.kind, TokenKind::Keyword(Keyword::Function))
    }

    pub(super) fn token_after_current(&self) -> Result<Token<'source>, Error> {
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.end);
        self.probe_token(&mut lexer, LexicalGoal::Div)
            .map_err(lex_error)
    }

    pub(super) fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    /// Whether `new.target` is allowed: inside a non-arrow function, through
    /// any arrows.
    pub(super) fn current_new_target_allowed(&self) -> bool {
        for state in self.stack.iter().rev() {
            match state.kind {
                FunctionKind::Arrow => {}
                FunctionKind::Script | FunctionKind::Module | FunctionKind::Eval => return false,
                _ => return true,
            }
        }
        false
    }

    pub(super) fn current(&self) -> &Token<'source> {
        &self.token
    }

    pub(super) fn advance(&mut self) -> Result<(), Error> {
        self.advance_with_goal(LexicalGoal::Div)
    }

    /// Advance to the first token of an expression: a leading slash starts a
    /// regular expression.
    pub(super) fn advance_expression_start(&mut self) -> Result<(), Error> {
        self.advance()?;
        if matches!(
            self.current().kind,
            TokenKind::Punctuator(Punctuator::Divide | Punctuator::DivideAssign)
        ) {
            self.relex_current_with_goal(LexicalGoal::RegExp)?;
        }
        Ok(())
    }

    #[inline]
    pub(super) fn advance_with_goal(&mut self, goal: LexicalGoal) -> Result<(), Error> {
        if !self.at_eof() {
            // QuickJS samples its C stack in next_token().
            if self.stack_guard.check_physical().is_err() {
                return Err(self.syntax_here("stack overflow"));
            }
            let previous_end = self.token.span.end;
            self.scan_next_token(goal)?;
            self.previous_end = Some(previous_end.byte_offset);
            self.previous_end_location = Some(previous_end);
        }
        Ok(())
    }

    #[inline]
    fn scan_next_token(&mut self, goal: LexicalGoal) -> Result<(), Error> {
        let start = self.lexer.current_position().byte_offset;
        let context = self.lexer.context();
        if !self.take_lookahead(start, goal, context) {
            self.lexer
                .next_token_into(goal, &mut self.token)
                .map_err(lex_error)?;
        }
        self.token_context = (context, goal);
        Ok(())
    }

    /// Rescan the current token under `goal`, keeping its line-break bit.
    pub(super) fn relex_current_with_goal(&mut self, goal: LexicalGoal) -> Result<(), Error> {
        if self.token_context == (self.lexer.context(), goal) {
            return Ok(());
        }
        let position = self.current().span.start;
        let line_terminator_before = self.current().line_terminator_before;
        self.lookahead_invalidate_from(position.byte_offset);
        self.lexer.seek(position);
        self.scan_next_token(goal)?;
        self.token.line_terminator_before = line_terminator_before;
        Ok(())
    }

    pub(super) fn relex_current_with_strict(&mut self, strict: bool) -> Result<(), Error> {
        let mut context = self.lexer.context();
        context.strict = strict;
        self.relex_current_with_context(context)
    }

    /// Rescan the current token and the rest under one function context.
    pub(super) fn relex_current_with_context(&mut self, context: LexContext) -> Result<(), Error> {
        self.lexer.set_context(context);
        if self.token_context == (context, LexicalGoal::Div) {
            return Ok(());
        }
        let position = self.current().span.start;
        let line_terminator_before = self.current().line_terminator_before;
        self.lookahead_invalidate_from(position.byte_offset);
        self.lexer.seek(position);
        self.scan_next_token(LexicalGoal::Div)?;
        self.token.line_terminator_before = line_terminator_before;
        Ok(())
    }

    /// Classify the tokens after the current one under `context`, keeping
    /// the current token as scanned.
    pub(super) fn set_future_lex_context(&mut self, context: LexContext) {
        let position = self.current().span.end;
        self.lookahead_invalidate_from(position.byte_offset);
        self.lexer.seek(position);
        self.lexer.set_context(context);
    }

    pub(super) fn directive_prologue_has_use_strict(
        &self,
        inherited_strict: bool,
    ) -> Result<bool, Error> {
        if !matches!(self.current().kind, TokenKind::String(_)) {
            return Ok(false);
        }
        let position = self.current().span.start;
        let mut lexer = self.lexer.clone();
        lexer.seek(position);
        let mut context = lexer.context();
        context.strict = inherited_strict;
        lexer.set_context(context);
        let mut token = self
            .probe_token(&mut lexer, LexicalGoal::Div)
            .map_err(lex_error)?;
        let mut found_strict = false;
        loop {
            let candidate = match &token.kind {
                TokenKind::String(literal) => {
                    !literal.has_escape && literal.raw[1..literal.raw.len() - 1] == *"use strict"
                }
                _ => return Ok(found_strict),
            };
            let next = self
                .probe_token(&mut lexer, LexicalGoal::Div)
                .map_err(lex_error)?;
            let consumed = match &next.kind {
                TokenKind::Punctuator(Punctuator::Semicolon) => 2,
                TokenKind::Punctuator(Punctuator::RightBrace) | TokenKind::Eof => 1,
                _ if next.line_terminator_before && quickjs_directive_asi_token(&next.kind) => 1,
                _ => return Ok(found_strict),
            };
            if candidate {
                found_strict = true;
            }
            token = if consumed == 1 {
                next
            } else {
                self.probe_token(&mut lexer, LexicalGoal::Div)
                    .map_err(lex_error)?
            };
            if candidate {
                let mut context = lexer.context();
                context.strict = true;
                lexer.set_context(context);
            }
        }
    }

    pub(super) fn consume_statement_terminator(&mut self) -> Result<(), Error> {
        if self.consume_punctuator(Punctuator::Semicolon)?
            || self.at_eof()
            || self.is_punctuator(Punctuator::RightBrace)
            || self.current().line_terminator_before
        {
            Ok(())
        } else {
            Err(self.syntax_here("expecting ';'"))
        }
    }

    pub(super) fn with_in_mode<T>(
        &mut self,
        mode: InMode,
        parse: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let previous = std::mem::replace(&mut self.in_mode, mode);
        let result = parse(self);
        self.in_mode = previous;
        result
    }

    /// Charge one recursive production at the current token.
    pub(super) fn parse_recursion<T>(
        &mut self,
        frame: ParserStackFrame,
        parse: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let weight = self.enter_recursion(frame)?;
        let result = parse(self);
        if result.is_ok() {
            self.leave_recursion(weight);
        }
        result
    }

    pub(super) fn enter_recursion(&mut self, frame: ParserStackFrame) -> Result<u64, Error> {
        self.stack_guard
            .enter(frame)
            .map_err(|_| self.syntax_here("stack overflow"))
    }

    pub(super) fn leave_recursion(&mut self, weight: u64) {
        self.stack_guard.leave(weight);
    }

    // ---- identifier diagnostics ----

    pub(super) fn validate_identifier(
        &self,
        identifier: &Identifier<'source>,
        span: Span,
        strict: bool,
        context: IdentifierContext,
    ) -> Result<(), Error> {
        self.validate_identifier_reservation(identifier, span, strict, context)?;
        if strict
            && !matches!(context, IdentifierContext::Reference)
            && matches!(
                self.identifier_text(identifier).as_ref(),
                "eval" | "arguments"
            )
        {
            let message = match context {
                IdentifierContext::Variable => "invalid variable name in strict mode",
                IdentifierContext::FunctionName => "invalid function name in strict code",
                IdentifierContext::Argument => "invalid argument name in strict code",
                IdentifierContext::Reference => unreachable!("reference context was excluded"),
            };
            return Err(Error::syntax(message, span));
        }
        Ok(())
    }

    pub(super) fn validate_identifier_reservation(
        &self,
        identifier: &Identifier<'source>,
        span: Span,
        strict: bool,
        context: IdentifierContext,
    ) -> Result<(), Error> {
        if identifier.escaped_reserved_word {
            let text = self.identifier_text(identifier);
            return Err(syntax_atom_error(
                "'",
                &text,
                "' is a reserved identifier",
                span,
            )?);
        }
        if strict
            && identifier
                .keyword_hint
                .is_some_and(strict_reserved_identifier)
        {
            let message = match context {
                IdentifierContext::Reference => {
                    let text = self.identifier_text(identifier);
                    return Err(syntax_atom_error(
                        "'",
                        &text,
                        "' is a reserved identifier",
                        span,
                    )?);
                }
                IdentifierContext::Variable => "invalid variable name in strict mode",
                IdentifierContext::FunctionName => "invalid function name in strict code",
                IdentifierContext::Argument => "invalid argument name in strict code",
            };
            return Err(Error::syntax(message, span));
        }
        Ok(())
    }

    /// `"eval"` or `"arguments"` in strict code: invalid binding names.
    pub(super) fn is_strict_restricted_name(&self, name: NameId) -> bool {
        self.strict() && matches!(self.names.name(name), "eval" | "arguments")
    }
}

pub(super) fn template_part_is_initial(kind: TemplatePartKind) -> bool {
    matches!(
        kind,
        TemplatePartKind::NoSubstitution | TemplatePartKind::Head
    )
}

#[cold]
#[inline(never)]
pub(super) fn lex_error(error: LexError) -> Error {
    if error.kind == LexErrorKind::StringTooLong {
        Error::new(ErrorKind::JsInternal, error.message)
    } else {
        Error::syntax(error.message, error.span)
    }
}

pub(super) fn syntax_atom_error(
    prefix: &str,
    atom: &str,
    suffix: &str,
    span: Span,
) -> Result<Error, JsStringError> {
    Ok(syntax_atom_error_without_span(prefix, atom, suffix)?.with_span(span))
}

pub(crate) fn syntax_atom_error_without_span(
    prefix: &str,
    atom: &str,
    suffix: &str,
) -> Result<Error, JsStringError> {
    let atom = JsString::try_from_utf8(atom)?;
    let mut message = NativeErrorMessage::new();
    message.push_utf8(prefix);
    atom.push_atom_get_str_to(&mut message);
    message.push_utf8(suffix);
    Ok(Error::from_native_message(ErrorKind::Syntax, message))
}

pub(super) const fn strict_reserved_identifier(keyword: Keyword) -> bool {
    matches!(
        keyword,
        Keyword::Implements
            | Keyword::Interface
            | Keyword::Let
            | Keyword::Package
            | Keyword::Private
            | Keyword::Protected
            | Keyword::Public
            | Keyword::Static
            | Keyword::Yield
    )
}

/// QuickJS `js_parse_directives`' ASI token set.
pub(super) fn quickjs_directive_asi_token(kind: &TokenKind<'_>) -> bool {
    matches!(
        kind,
        TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::Template(_)
            | TokenKind::Identifier(_)
            | TokenKind::RegExp(_)
            | TokenKind::Punctuator(Punctuator::Decrement | Punctuator::Increment)
            | TokenKind::Keyword(
                Keyword::Null
                    | Keyword::False
                    | Keyword::True
                    | Keyword::If
                    | Keyword::Return
                    | Keyword::Var
                    | Keyword::This
                    | Keyword::Delete
                    | Keyword::Typeof
                    | Keyword::New
                    | Keyword::Do
                    | Keyword::While
                    | Keyword::For
                    | Keyword::Switch
                    | Keyword::Throw
                    | Keyword::Try
                    | Keyword::Function
                    | Keyword::Debugger
                    | Keyword::With
                    | Keyword::Class
                    | Keyword::Const
                    | Keyword::Enum
                    | Keyword::Export
                    | Keyword::Import
                    | Keyword::Super
                    | Keyword::Interface
                    | Keyword::Let
                    | Keyword::Package
                    | Keyword::Private
                    | Keyword::Protected
                    | Keyword::Public
                    | Keyword::Static
            )
    )
}

/// QuickJS `is_regexp_allowed`, for the non-committing scanners.
pub(super) fn for_head_regexp_allowed_after(kind: &TokenKind<'_>) -> bool {
    if matches!(
        kind,
        TokenKind::Identifier(identifier)
            if !identifier.has_escape && matches!(identifier.raw, "of" | "yield")
    ) {
        return true;
    }
    !matches!(
        kind,
        TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::RegExp(_)
            | TokenKind::Identifier(_)
            | TokenKind::Keyword(Keyword::Null | Keyword::False | Keyword::True | Keyword::This)
            | TokenKind::Punctuator(
                Punctuator::RightParen
                    | Punctuator::RightBracket
                    | Punctuator::RightBrace
                    | Punctuator::Increment
                    | Punctuator::Decrement
            )
    )
}

// ---- numbers ----

pub(super) fn parse_number(number: &NumberLiteral<'_>) -> Result<Number, String> {
    let integer = match number.kind {
        NumberKind::Integer(radix) => Some(radix_digits(number.raw, radix)),
        NumberKind::LegacyOctal => Some((number.raw, 8)),
        _ => None,
    };
    if let Some((digits, radix)) = integer
        && let Some(value) = small_integer(digits, radix)
    {
        return Ok(Number::Number(value as f64));
    }
    let raw = if number.raw.contains('_') {
        Cow::Owned(number.raw.replace('_', ""))
    } else {
        Cow::Borrowed(number.raw)
    };
    if let NumberKind::BigInt(radix) = number.kind {
        let literal = raw
            .strip_suffix('n')
            .ok_or_else(|| "BigInt literal is missing its suffix".to_owned())?;
        let (digits, base) = match radix {
            NumericRadix::Binary => (literal.get(2..).unwrap_or_default(), 2),
            NumericRadix::Octal => (literal.get(2..).unwrap_or_default(), 8),
            NumericRadix::Decimal => (literal, 10),
            NumericRadix::Hexadecimal => (literal.get(2..).unwrap_or_default(), 16),
        };
        return JsBigInt::parse_radix(digits, base)
            .map(Number::BigInt)
            .map_err(|error| error.to_string());
    }
    let value = match number.kind {
        NumberKind::Integer(radix) => {
            let (digits, base) = radix_digits(&raw, radix);
            parse_digits(digits, base)?
        }
        NumberKind::Float | NumberKind::LegacyDecimal => raw
            .parse::<f64>()
            .map_err(|_| format!("invalid numeric literal '{raw}'"))?,
        NumberKind::LegacyOctal => parse_digits(&raw, 8)?,
        NumberKind::BigInt(_) => unreachable!("handled above"),
    };
    Ok(Number::Number(value))
}

fn radix_digits(raw: &str, radix: NumericRadix) -> (&str, u32) {
    match radix {
        NumericRadix::Binary => (raw.get(2..).unwrap_or_default(), 2),
        NumericRadix::Octal => (raw.get(2..).unwrap_or_default(), 8),
        NumericRadix::Decimal => (raw, 10),
        NumericRadix::Hexadecimal => (raw.get(2..).unwrap_or_default(), 16),
    }
}

fn small_integer(digits: &str, radix: u32) -> Option<u64> {
    let mut value = 0u64;
    let mut any_digit = false;
    for byte in digits.bytes() {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            b'_' => continue,
            _ => return None,
        };
        if u32::from(digit) >= radix {
            return None;
        }
        any_digit = true;
        value = value
            .checked_mul(u64::from(radix))?
            .checked_add(u64::from(digit))?;
    }
    any_digit.then_some(value)
}

fn parse_digits(digits: &str, radix: u32) -> Result<f64, String> {
    if digits.is_empty() {
        return Err("numeric literal has no digits".to_owned());
    }
    let value = BigUint::parse_bytes(digits.as_bytes(), radix)
        .ok_or_else(|| format!("invalid base-{radix} numeric literal"))?;
    Ok(value.to_f64().unwrap_or(f64::INFINITY))
}

/// A numeric property key's canonical string.
pub(super) fn number_key(number: &Number) -> Result<JsString, Error> {
    use crate::engine::value::PrimitiveValue;
    let value = match number {
        Number::Number(value) => PrimitiveValue::number(*value),
        Number::BigInt(value) => PrimitiveValue::BigInt(value.clone()),
    };
    value.to_js_string()
}
