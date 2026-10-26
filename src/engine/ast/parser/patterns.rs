//! Binding patterns, assignment patterns and their skip-scans.
//!
//! Patterns parse in source order, each leaf registered as it is reached,
//! with QuickJS's error priorities (`js_parse_destructuring_element`): a
//! nested pattern is recognized by skip-scanning to its closer, and an
//! object pattern's key is followed by an unchecked advance over the colon.

use super::Parser;
use super::lookahead::BindingScanMemo;
use super::support::{
    ForHeadDelimiter, IdentifierContext, for_head_regexp_allowed_after, number_key, parse_number,
};
use crate::engine::api::error::Error;
use crate::engine::ast::lexer::{
    Identifier, LexicalGoal, NumberKind, Punctuator, Span, TemplatePartKind, Token, TokenKind,
};
use crate::engine::ast::{
    DeclKind, Expr, ExprKind, Pattern, PatternItem, PatternKind, PatternProp, PropKey, Ref,
};
use crate::engine::value::JsString;

/// Where a binding pattern binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BindingSite {
    Declaration,
    Iteration,
    Catch,
    Parameter,
}

/// What a binding pattern declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Declaring {
    Var,
    Let,
    Const,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PatternShape {
    Array,
    Object,
}

#[derive(Clone, Copy)]
pub(super) struct BindingPatternScan<'source> {
    pub(super) following: Token<'source>,
    pub(super) has_object_rest: bool,
    pub(super) has_assignment: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ParenthesizedParameterScan {
    pub(super) has_assignment: bool,
}

enum PropertyName<'source> {
    Fixed {
        key: JsString,
        token: Token<'source>,
        shorthand: Option<Identifier<'source>>,
    },
    Computed(Expr),
}

impl PropertyName<'_> {
    fn into_key(self) -> PropKey {
        match self {
            Self::Fixed { key, .. } => PropKey::Name(key),
            Self::Computed(expr) => PropKey::Computed(Box::new(expr)),
        }
    }
}

impl<'source> Parser<'source> {
    // ---- declarations ----

    /// `let [a] = x` or `var {a} = x`: a pattern that must have an initializer.
    pub(super) fn parse_binding_declaration(
        &mut self,
        declaring: Declaring,
    ) -> Result<(Pattern, Expr, u32), Error> {
        let pattern_span = self.current().span;
        let opening = if self.is_punctuator(Punctuator::LeftBracket) {
            Punctuator::LeftBracket
        } else {
            Punctuator::LeftBrace
        };
        if !self
            .binding_pattern_following_token(opening)
            .is_some_and(|token| matches!(token.kind, TokenKind::Punctuator(Punctuator::Equal)))
        {
            return Err(Error::syntax("variable name expected", pattern_span));
        }
        let pattern = self.parse_binding_pattern(declaring, BindingSite::Declaration)?;
        let equal = self.current().span;
        if !self.consume_punctuator(Punctuator::Equal)? {
            if let TokenKind::Punctuator(punctuator) = self.current().kind {
                return Err(self.syntax_here(format!(
                    "unexpected token in expression: '{}'",
                    punctuator.as_str()
                )));
            }
            return Err(Error::syntax("variable name expected", pattern_span));
        }
        let init = self.parse_assignment_allow_in()?;
        Ok((pattern, init, super::pos(equal)))
    }

    /// A catch parameter pattern, which QuickJS lets carry a default.
    pub(super) fn parse_catch_binding_pattern(&mut self) -> Result<(Pattern, Option<Expr>), Error> {
        self.parse_nested_binding_element(Declaring::Let, BindingSite::Catch, false)
    }

    /// A parameter pattern and its default.
    pub(super) fn parse_parameter_pattern(&mut self) -> Result<(Pattern, Option<Expr>), Error> {
        let opening = if self.is_punctuator(Punctuator::LeftBracket) {
            Punctuator::LeftBracket
        } else {
            Punctuator::LeftBrace
        };
        let has_initializer = self
            .binding_pattern_following_token(opening)
            .is_some_and(|token| matches!(token.kind, TokenKind::Punctuator(Punctuator::Equal)));
        let pattern = self.parse_binding_pattern(Declaring::Var, BindingSite::Parameter)?;
        if !has_initializer {
            return Ok((pattern, None));
        }
        self.expect_punctuator(Punctuator::Equal)?;
        let default = self.parse_assignment_allow_in()?;
        Ok((pattern, Some(default)))
    }

    pub(super) fn parse_binding_pattern(
        &mut self,
        declaring: Declaring,
        site: BindingSite,
    ) -> Result<Pattern, Error> {
        if self.is_punctuator(Punctuator::LeftBracket) {
            self.parse_array_binding_pattern(declaring, site)
        } else {
            self.parse_object_binding_pattern(declaring, site)
        }
    }

    fn parse_nested_binding_element(
        &mut self,
        declaring: Declaring,
        site: BindingSite,
        is_rest: bool,
    ) -> Result<(Pattern, Option<Expr>), Error> {
        let pattern = self.parse_binding_pattern(declaring, site)?;
        if is_rest {
            if self.is_punctuator(Punctuator::Equal) {
                return Err(self.syntax_here("rest element cannot have a default value"));
            }
            return Ok((pattern, None));
        }
        let default = if self.consume_punctuator(Punctuator::Equal)? {
            Some(self.parse_assignment_allow_in()?)
        } else {
            None
        };
        Ok((pattern, default))
    }

    fn nested_pattern_follows(
        &self,
        opening: Punctuator,
        closing: Punctuator,
    ) -> Option<Token<'source>> {
        if !self.is_punctuator(opening) {
            return None;
        }
        self.binding_pattern_following_token(opening).filter(|token| {
            matches!(token.kind, TokenKind::Punctuator(p) if p == Punctuator::Comma || p == Punctuator::Equal || p == closing)
        })
    }

    fn parse_array_binding_pattern(
        &mut self,
        declaring: Declaring,
        site: BindingSite,
    ) -> Result<Pattern, Error> {
        let start = self.current().span;
        self.expect_punctuator(Punctuator::LeftBracket)?;
        let mut items = Vec::new();
        let mut rest = None;
        while !self.is_punctuator(Punctuator::RightBracket) {
            let is_rest = self.consume_punctuator(Punctuator::Ellipsis)?;
            let element_span = self.current().span;
            let nested = self
                .nested_pattern_follows(Punctuator::LeftBrace, Punctuator::RightBracket)
                .or_else(|| {
                    self.nested_pattern_follows(Punctuator::LeftBracket, Punctuator::RightBracket)
                });
            if let Some(following) = nested {
                if is_rest && matches!(following.kind, TokenKind::Punctuator(Punctuator::Equal)) {
                    return Err(Error::syntax(
                        "rest element cannot have a default value",
                        element_span,
                    ));
                }
                let (pattern, default) =
                    self.parse_nested_binding_element(declaring, site, is_rest)?;
                if is_rest {
                    rest = Some(Box::new(pattern));
                } else {
                    items.push(Some(PatternItem { pattern, default }));
                }
                if self.is_punctuator(Punctuator::RightBracket) {
                    break;
                }
                if is_rest {
                    return Err(self.syntax_here("rest element must be the last one"));
                }
                self.expect_punctuator(Punctuator::Comma)?;
                continue;
            }
            if !is_rest && self.consume_punctuator(Punctuator::Comma)? {
                items.push(None);
                continue;
            }
            let token = *self.current();
            let TokenKind::Identifier(identifier) = token.kind else {
                if is_rest
                    && matches!(
                        token.kind,
                        TokenKind::Punctuator(Punctuator::Comma | Punctuator::RightBracket)
                    )
                {
                    return Err(Error::syntax("missing binding pattern...", token.span));
                }
                return Err(Error::syntax("invalid destructuring target", token.span));
            };
            if identifier.escaped_reserved_word {
                return Err(Error::syntax("invalid destructuring target", token.span));
            }
            self.validate_identifier_reservation(
                &identifier,
                token.span,
                self.strict(),
                IdentifierContext::Variable,
            )?;
            let invalid_lexical_let =
                declaring != Declaring::Var && self.identical_name(&identifier, "let");
            let name = self.intern_identifier(&identifier);
            self.advance()?;
            if invalid_lexical_let {
                return Err(self.syntax_here("invalid lexical variable name"));
            }
            if self.is_strict_restricted_name(name) {
                return Err(Error::syntax("invalid destructuring target", token.span));
            }
            let binding =
                self.register_binding_leaf(name, token.span, token.span, declaring, site)?;
            let pattern = Pattern {
                kind: PatternKind::Binding(binding),
                span: token.span,
            };
            if is_rest {
                rest = Some(Box::new(pattern));
            } else {
                let default = if self.consume_punctuator(Punctuator::Equal)? {
                    Some(self.parse_assignment_allow_in()?)
                } else {
                    None
                };
                items.push(Some(PatternItem { pattern, default }));
            }
            if self.is_punctuator(Punctuator::RightBracket) {
                break;
            }
            if is_rest {
                return Err(self.syntax_here("rest element must be the last one"));
            }
            self.expect_punctuator(Punctuator::Comma)?;
        }
        self.expect_punctuator(Punctuator::RightBracket)?;
        Ok(Pattern {
            kind: PatternKind::Array { items, rest },
            span: self.span_from(start),
        })
    }

    /// Register one declared name of a binding pattern. `parameter_conflict`
    /// is where a duplicate parameter is reported.
    fn register_binding_leaf(
        &mut self,
        name: crate::engine::ast::names::NameId,
        span: Span,
        parameter_conflict: Span,
        declaring: Declaring,
        site: BindingSite,
    ) -> Result<Ref, Error> {
        let conflict = self.current().span;
        match declaring {
            Declaring::Let | Declaring::Const => {
                let kind = if site == BindingSite::Catch {
                    DeclKind::Let
                } else if declaring == Declaring::Const {
                    DeclKind::Const
                } else {
                    DeclKind::Let
                };
                self.register_lexical(name, span, conflict, kind, false)?;
            }
            Declaring::Var if site == BindingSite::Parameter => {
                self.register_pattern_parameter(name, span, parameter_conflict)?;
            }
            Declaring::Var => {
                self.register_var(name, span, conflict)?;
            }
        }
        Ok(Ref {
            name,
            span: span.into(),
            scope: self.current_scope(),
        })
    }

    fn parse_object_binding_pattern(
        &mut self,
        declaring: Declaring,
        site: BindingSite,
    ) -> Result<Pattern, Error> {
        let start = self.current().span;
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let mut props = Vec::new();
        let mut rest = None;
        let mut rest_pos = 0;
        while !self.is_punctuator(Punctuator::RightBrace) {
            if self.is_punctuator(Punctuator::Ellipsis) {
                rest_pos = super::pos(self.current().span);
                rest = Some(Box::new(self.parse_object_binding_rest(declaring, site)?));
                break;
            }
            let key_pos = super::pos(self.current().span);
            let property = self.parse_pattern_property_name()?;
            let shorthand = matches!(
                &property,
                PropertyName::Fixed {
                    shorthand: Some(_),
                    ..
                }
            );
            if !shorthand {
                // QuickJS advances over the expected colon without checking it.
                self.advance()?;
            }
            let nested = (!shorthand)
                .then(|| {
                    self.nested_pattern_follows(Punctuator::LeftBracket, Punctuator::RightBrace)
                        .or_else(|| {
                            self.nested_pattern_follows(
                                Punctuator::LeftBrace,
                                Punctuator::RightBrace,
                            )
                        })
                })
                .flatten();
            let (value, default) = if nested.is_some() {
                self.parse_nested_binding_element(declaring, site, false)?
            } else {
                self.parse_object_binding_leaf(&property, declaring, site)?
            };
            props.push(PatternProp {
                key: property.into_key(),
                value,
                default,
                key_pos,
            });
            if self.is_punctuator(Punctuator::RightBrace) {
                break;
            }
            self.expect_punctuator(Punctuator::Comma)?;
        }
        self.expect_punctuator(Punctuator::RightBrace)?;
        Ok(Pattern {
            kind: PatternKind::Object {
                props,
                rest,
                rest_pos,
            },
            span: self.span_from(start),
        })
    }

    fn parse_pattern_property_name(&mut self) -> Result<PropertyName<'source>, Error> {
        let token = *self.current();
        match token.kind {
            TokenKind::Identifier(identifier) => {
                let key = JsString::try_from_utf8(&self.identifier_text(&identifier))?;
                self.advance()?;
                let shorthand = (!identifier.escaped_reserved_word
                    && !self.is_punctuator(Punctuator::Colon))
                .then_some(identifier);
                Ok(PropertyName::Fixed {
                    key,
                    token,
                    shorthand,
                })
            }
            TokenKind::Keyword(keyword) => {
                self.advance()?;
                Ok(PropertyName::Fixed {
                    key: JsString::from_static(keyword.as_str()),
                    token,
                    shorthand: None,
                })
            }
            TokenKind::String(string) => {
                if self.strict() && string.has_legacy_octal_escape {
                    return Err(Error::syntax(
                        "legacy octal escapes are forbidden in strict mode",
                        token.span,
                    ));
                }
                self.advance()?;
                Ok(PropertyName::Fixed {
                    key: self.decode_string_literal(token.span)?,
                    token,
                    shorthand: None,
                })
            }
            TokenKind::Number(number) => {
                if self.strict()
                    && matches!(
                        number.kind,
                        NumberKind::LegacyOctal | NumberKind::LegacyDecimal
                    )
                {
                    return Err(Error::syntax(
                        "legacy leading-zero numeric literals are forbidden in strict mode",
                        token.span,
                    ));
                }
                self.advance()?;
                let value =
                    parse_number(&number).map_err(|message| Error::syntax(message, token.span))?;
                Ok(PropertyName::Fixed {
                    key: number_key(&value)?,
                    token,
                    shorthand: None,
                })
            }
            TokenKind::Punctuator(Punctuator::LeftBracket) => {
                self.advance_expression_start()?;
                let expr = self.parse_assignment_allow_in()?;
                self.expect_punctuator(Punctuator::RightBracket)?;
                Ok(PropertyName::Computed(expr))
            }
            _ => Err(Error::syntax("invalid property name", token.span)),
        }
    }

    fn parse_object_binding_leaf(
        &mut self,
        property: &PropertyName<'source>,
        declaring: Declaring,
        site: BindingSite,
    ) -> Result<(Pattern, Option<Expr>), Error> {
        let shorthand = match property {
            PropertyName::Fixed {
                token,
                shorthand: Some(identifier),
                ..
            } => Some((*token, *identifier)),
            _ => None,
        };
        let (token, identifier) = if let Some(binding) = shorthand {
            binding
        } else {
            let token = *self.current();
            let TokenKind::Identifier(identifier) = token.kind else {
                return Err(Error::syntax("invalid destructuring target", token.span));
            };
            self.advance()?;
            (token, identifier)
        };
        if identifier.escaped_reserved_word {
            return Err(Error::syntax("invalid destructuring target", token.span));
        }
        self.validate_identifier_reservation(
            &identifier,
            token.span,
            self.strict(),
            IdentifierContext::Variable,
        )?;
        if declaring != Declaring::Var && self.identical_name(&identifier, "let") {
            return Err(self.syntax_here("invalid lexical variable name"));
        }
        let name = self.intern_identifier(&identifier);
        if self.is_strict_restricted_name(name) {
            let span = if shorthand.is_some() {
                self.current().span
            } else {
                token.span
            };
            return Err(Error::syntax("invalid destructuring target", span));
        }
        let parameter_conflict = if shorthand.is_some() {
            self.current().span
        } else {
            token.span
        };
        let binding =
            self.register_binding_leaf(name, token.span, parameter_conflict, declaring, site)?;
        let default = if self.consume_punctuator(Punctuator::Equal)? {
            Some(self.parse_assignment_allow_in()?)
        } else {
            None
        };
        Ok((
            Pattern {
                kind: PatternKind::Binding(binding),
                span: token.span,
            },
            default,
        ))
    }

    fn parse_object_binding_rest(
        &mut self,
        declaring: Declaring,
        site: BindingSite,
    ) -> Result<Pattern, Error> {
        self.advance()?;
        let token = *self.current();
        let TokenKind::Identifier(identifier) = token.kind else {
            return Err(Error::syntax("invalid destructuring target", token.span));
        };
        if identifier.escaped_reserved_word {
            return Err(Error::syntax("invalid destructuring target", token.span));
        }
        let name = self.intern_identifier(&identifier);
        self.validate_identifier_reservation(
            &identifier,
            token.span,
            self.strict(),
            IdentifierContext::Variable,
        )?;
        if self.is_strict_restricted_name(name) {
            return Err(Error::syntax("invalid destructuring target", token.span));
        }
        self.advance()?;
        if !self.is_punctuator(Punctuator::RightBrace) {
            return Err(self.syntax_here("assignment rest property must be last"));
        }
        if declaring != Declaring::Var && self.names.name(name) == "let" {
            return Err(self.syntax_here("invalid lexical variable name"));
        }
        let binding = self.register_binding_leaf(name, token.span, token.span, declaring, site)?;
        Ok(Pattern {
            kind: PatternKind::Binding(binding),
            span: token.span,
        })
    }

    // ---- assignment patterns ----

    /// `[a, b]` or `{a, b}` as an assignment target, at its opener.
    pub(super) fn parse_assignment_pattern(&mut self) -> Result<Pattern, Error> {
        if self.is_punctuator(Punctuator::LeftBracket) {
            self.parse_array_assignment_pattern()
        } else {
            self.parse_object_assignment_pattern()
        }
    }

    fn parse_nested_assignment_element(
        &mut self,
        is_rest: bool,
    ) -> Result<(Pattern, Option<Expr>), Error> {
        let pattern = self.parse_assignment_pattern()?;
        if is_rest {
            if self.is_punctuator(Punctuator::Equal) {
                return Err(self.syntax_here("rest element cannot have a default value"));
            }
            return Ok((pattern, None));
        }
        let default = if self.consume_punctuator(Punctuator::Equal)? {
            Some(self.parse_assignment_allow_in()?)
        } else {
            None
        };
        Ok((pattern, default))
    }

    fn parse_array_assignment_pattern(&mut self) -> Result<Pattern, Error> {
        let start = self.current().span;
        self.expect_punctuator(Punctuator::LeftBracket)?;
        let mut items = Vec::new();
        let mut rest = None;
        while !self.is_punctuator(Punctuator::RightBracket) {
            let is_rest = self.consume_punctuator(Punctuator::Ellipsis)?;
            let element_span = self.current().span;
            if is_rest
                && matches!(
                    self.current().kind,
                    TokenKind::Punctuator(Punctuator::Comma | Punctuator::RightBracket)
                )
            {
                return Err(Error::syntax("missing binding pattern...", element_span));
            }
            let nested = self
                .nested_pattern_follows(Punctuator::LeftBracket, Punctuator::RightBracket)
                .or_else(|| {
                    self.nested_pattern_follows(Punctuator::LeftBrace, Punctuator::RightBracket)
                });
            if let Some(following) = nested {
                if is_rest && matches!(following.kind, TokenKind::Punctuator(Punctuator::Equal)) {
                    return Err(Error::syntax(
                        "rest element cannot have a default value",
                        element_span,
                    ));
                }
                let (pattern, default) = self.parse_nested_assignment_element(is_rest)?;
                if is_rest {
                    rest = Some(Box::new(pattern));
                } else {
                    items.push(Some(PatternItem { pattern, default }));
                }
            } else if !is_rest && self.consume_punctuator(Punctuator::Comma)? {
                items.push(None);
                continue;
            } else {
                let target = self.parse_destructuring_target()?;
                if is_rest {
                    rest = Some(Box::new(target));
                } else {
                    let default = if self.consume_punctuator(Punctuator::Equal)? {
                        Some(self.parse_assignment_allow_in()?)
                    } else {
                        None
                    };
                    items.push(Some(PatternItem {
                        pattern: target,
                        default,
                    }));
                }
            }
            if self.is_punctuator(Punctuator::RightBracket) {
                break;
            }
            if is_rest {
                return Err(self.syntax_here("rest element must be the last one"));
            }
            self.expect_punctuator(Punctuator::Comma)?;
        }
        self.expect_punctuator(Punctuator::RightBracket)?;
        Ok(Pattern {
            kind: PatternKind::Array { items, rest },
            span: self.span_from(start),
        })
    }

    /// A LeftHandSideExpression that is a reference.
    fn parse_destructuring_target(&mut self) -> Result<Pattern, Error> {
        let start = self.current().span;
        let expr = self.parse_left_hand_side_expression()?;
        if expr.contains_optional_chain() {
            return Err(self.syntax_here("invalid destructuring target"));
        }
        match expr.reference_kind() {
            Some(ReferenceKind::Identifier(name)) => {
                if self.is_strict_restricted_name(name) {
                    return Err(self.syntax_here("invalid lvalue in strict mode"));
                }
            }
            Some(ReferenceKind::Member) => {}
            None => return Err(self.syntax_here("invalid destructuring target")),
        }
        Ok(Pattern {
            kind: PatternKind::Target(Box::new(expr)),
            span: self.span_from(start),
        })
    }

    fn parse_object_assignment_pattern(&mut self) -> Result<Pattern, Error> {
        let start = self.current().span;
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let mut props = Vec::new();
        let mut rest = None;
        let mut rest_pos = 0;
        while !self.is_punctuator(Punctuator::RightBrace) {
            if self.is_punctuator(Punctuator::Ellipsis) {
                rest_pos = super::pos(self.current().span);
                self.advance()?;
                let target = self.parse_destructuring_target()?;
                if !self.is_punctuator(Punctuator::RightBrace) {
                    return Err(self.syntax_here("assignment rest property must be last"));
                }
                rest = Some(Box::new(target));
                break;
            }
            let key_pos = super::pos(self.current().span);
            let property = self.parse_pattern_property_name()?;
            let shorthand = match &property {
                PropertyName::Fixed {
                    token,
                    shorthand: Some(identifier),
                    ..
                } => Some((*token, *identifier)),
                _ => None,
            };
            if shorthand.is_none() {
                self.advance()?;
            }
            let nested = shorthand
                .is_none()
                .then(|| {
                    self.nested_pattern_follows(Punctuator::LeftBracket, Punctuator::RightBrace)
                        .or_else(|| {
                            self.nested_pattern_follows(
                                Punctuator::LeftBrace,
                                Punctuator::RightBrace,
                            )
                        })
                })
                .flatten();
            let (value, default) = if nested.is_some() {
                self.parse_nested_assignment_element(false)?
            } else {
                let target = if let Some((token, identifier)) = shorthand {
                    self.shorthand_assignment_target(token, identifier)?
                } else {
                    self.parse_destructuring_target()?
                };
                let default = if self.consume_punctuator(Punctuator::Equal)? {
                    Some(self.parse_assignment_allow_in()?)
                } else {
                    None
                };
                (target, default)
            };
            props.push(PatternProp {
                key: property.into_key(),
                value,
                default,
                key_pos,
            });
            if self.is_punctuator(Punctuator::RightBrace) {
                break;
            }
            self.expect_punctuator(Punctuator::Comma)?;
        }
        self.expect_punctuator(Punctuator::RightBrace)?;
        Ok(Pattern {
            kind: PatternKind::Object {
                props,
                rest,
                rest_pos,
            },
            span: self.span_from(start),
        })
    }

    fn shorthand_assignment_target(
        &mut self,
        token: Token<'source>,
        identifier: Identifier<'source>,
    ) -> Result<Pattern, Error> {
        self.validate_identifier(
            &identifier,
            token.span,
            self.strict(),
            IdentifierContext::Reference,
        )?;
        let name = self.intern_identifier(&identifier);
        if self.is_strict_restricted_name(name) {
            return Err(self.syntax_here("invalid destructuring target"));
        }
        let reference = Ref {
            name,
            span: token.span.into(),
            scope: self.current_scope(),
        };
        Ok(Pattern {
            kind: PatternKind::Target(Box::new(Expr {
                kind: ExprKind::Ident(reference),
                span: token.span.into(),
            })),
            span: token.span,
        })
    }

    // ---- skip-scans ----

    pub(super) fn array_assignment_pattern_ahead(&self) -> bool {
        self.is_punctuator(Punctuator::LeftBracket)
            && self
                .binding_pattern_following_token(Punctuator::LeftBracket)
                .is_some_and(|token| matches!(token.kind, TokenKind::Punctuator(Punctuator::Equal)))
    }

    pub(super) fn object_assignment_pattern_ahead(&self) -> bool {
        self.is_punctuator(Punctuator::LeftBrace)
            && self
                .binding_pattern_following_token(Punctuator::LeftBrace)
                .is_some_and(|token| matches!(token.kind, TokenKind::Punctuator(Punctuator::Equal)))
    }

    /// A for-in/of head that is an assignment pattern.
    pub(super) fn for_assignment_pattern_ahead(&self, of: bool) -> bool {
        let opening = if self.is_punctuator(Punctuator::LeftBracket) {
            Punctuator::LeftBracket
        } else if self.is_punctuator(Punctuator::LeftBrace) {
            Punctuator::LeftBrace
        } else {
            return false;
        };
        self.binding_pattern_following_token(opening).is_some_and(|token| {
            if of {
                matches!(&token.kind, TokenKind::Identifier(identifier) if identifier.raw == "of" && !identifier.has_escape)
            } else {
                matches!(token.kind, TokenKind::Keyword(crate::engine::ast::lexer::Keyword::In))
            }
        })
    }

    /// Whether a parenthesized parameter list contains `=`, which gives the
    /// function a separate parameter environment.
    pub(super) fn parenthesized_parameter_scan(&self) -> Option<ParenthesizedParameterScan> {
        let start = self.current().span.start.byte_offset;
        let context = self.lexer.context();
        if let Some((offset, lex_context, result)) = self.lookahead.borrow().parameter_scan
            && (offset, lex_context) == (start, context)
        {
            return result;
        }
        let mut assignment_seen = false;
        let scan = self.binding_pattern_scan_recording(Punctuator::LeftParen, &mut assignment_seen);
        let has_assignment = scan
            .as_ref()
            .map(|scan| scan.has_assignment)
            .or_else(|| assignment_seen.then_some(true));
        let result =
            has_assignment.map(|has_assignment| ParenthesizedParameterScan { has_assignment });
        self.lookahead.borrow_mut().parameter_scan = Some((start, context, result));
        result
    }

    pub(super) fn binding_pattern_following_token(
        &self,
        opening: Punctuator,
    ) -> Option<Token<'source>> {
        let mut seen = false;
        self.binding_pattern_scan_recording(opening, &mut seen)
            .map(|scan| scan.following)
    }

    fn binding_pattern_scan_recording(
        &self,
        opening: Punctuator,
        assignment_seen: &mut bool,
    ) -> Option<BindingPatternScan<'source>> {
        if !self.is_punctuator(opening) {
            return None;
        }
        let start = self.current().span.start.byte_offset;
        let context = self.lexer.context();
        if let Some(memo) = self.lookahead.borrow().binding_scan
            && (memo.start, memo.context, memo.opening) == (start, context, opening)
        {
            *assignment_seen |= memo.assignment_seen;
            return memo.scan;
        }
        let mut seen = false;
        let scan = self.scan_binding_pattern(opening, &mut seen);
        self.lookahead.borrow_mut().binding_scan = Some(BindingScanMemo {
            start,
            context,
            opening,
            scan,
            assignment_seen: seen,
        });
        *assignment_seen |= seen;
        scan
    }

    fn scan_binding_pattern(
        &self,
        opening: Punctuator,
        assignment_seen: &mut bool,
    ) -> Option<BindingPatternScan<'source>> {
        let root = match opening {
            Punctuator::LeftParen => ForHeadDelimiter::Parenthesis,
            Punctuator::LeftBracket => ForHeadDelimiter::Bracket,
            Punctuator::LeftBrace => ForHeadDelimiter::Brace,
            _ => return None,
        };
        let mut lexer = self.lexer.clone();
        lexer.seek(self.current().span.start);
        let mut delimiters: Vec<ForHeadDelimiter> = Vec::new();
        let mut goal = LexicalGoal::Div;
        let mut regexp_allowed = true;
        let mut has_object_rest = false;
        let mut has_assignment = false;
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
            if root == ForHeadDelimiter::Brace
                && delimiters.as_slice() == [ForHeadDelimiter::Brace]
                && matches!(token.kind, TokenKind::Punctuator(Punctuator::Ellipsis))
            {
                has_object_rest = true;
            }
            if matches!(token.kind, TokenKind::Punctuator(Punctuator::Equal)) {
                has_assignment = true;
                *assignment_seen = true;
            }
            let closes = |delimiters: &Vec<ForHeadDelimiter>, kind: ForHeadDelimiter| {
                root == kind && delimiters.is_empty()
            };
            match &token.kind {
                TokenKind::Punctuator(Punctuator::LeftParen) => {
                    if delimiters.len() >= 255 {
                        return None;
                    }
                    delimiters.push(ForHeadDelimiter::Parenthesis);
                }
                TokenKind::Punctuator(Punctuator::LeftBracket) => {
                    if delimiters.len() >= 255 {
                        return None;
                    }
                    delimiters.push(ForHeadDelimiter::Bracket);
                }
                TokenKind::Punctuator(Punctuator::LeftBrace) => {
                    if delimiters.len() >= 255 {
                        return None;
                    }
                    delimiters.push(ForHeadDelimiter::Brace);
                }
                TokenKind::Punctuator(Punctuator::RightParen) => {
                    if delimiters.pop() != Some(ForHeadDelimiter::Parenthesis) {
                        return None;
                    }
                    if closes(&delimiters, ForHeadDelimiter::Parenthesis) {
                        return Some(BindingPatternScan {
                            following: self.probe_token(&mut lexer, LexicalGoal::Div).ok()?,
                            has_object_rest,
                            has_assignment,
                        });
                    }
                }
                TokenKind::Punctuator(Punctuator::RightBracket) => {
                    if delimiters.pop() != Some(ForHeadDelimiter::Bracket) {
                        return None;
                    }
                    if closes(&delimiters, ForHeadDelimiter::Bracket) {
                        return Some(BindingPatternScan {
                            following: self.probe_token(&mut lexer, LexicalGoal::Div).ok()?,
                            has_object_rest,
                            has_assignment,
                        });
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
                    if closes(&delimiters, ForHeadDelimiter::Brace) {
                        return Some(BindingPatternScan {
                            following: self.probe_token(&mut lexer, LexicalGoal::Div).ok()?,
                            has_object_rest,
                            has_assignment,
                        });
                    }
                }
                TokenKind::Template(part) => match part.kind {
                    TemplatePartKind::Head => {
                        if delimiters.len() >= 255 {
                            return None;
                        }
                        delimiters.push(ForHeadDelimiter::Template);
                    }
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
}

/// What kind of reference an expression is, through parentheses.
pub(super) enum ReferenceKind {
    Identifier(crate::engine::ast::names::NameId),
    Member,
}

impl Expr {
    /// The reference an expression denotes as an assignment target:
    /// parentheses keep a reference a reference.
    pub(super) fn reference_kind(&self) -> Option<ReferenceKind> {
        match &self.kind {
            ExprKind::Ident(reference) => Some(ReferenceKind::Identifier(reference.name)),
            ExprKind::Member { .. } | ExprKind::SuperMember { .. } => Some(ReferenceKind::Member),
            ExprKind::Paren(inner) => inner.reference_kind(),
            _ => None,
        }
    }

    /// Whether the expression is (through parentheses) an optional chain,
    /// which is never an assignment target.
    pub(super) fn contains_optional_chain(&self) -> bool {
        match &self.kind {
            ExprKind::OptionalChain(_) => true,
            ExprKind::Paren(inner) => inner.contains_optional_chain(),
            _ => false,
        }
    }
}
