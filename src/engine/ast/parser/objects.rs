//! Object literals.

use super::functions::FunctionOptions;
use super::support::{IdentifierContext, number_key, parse_number};
use super::{Parser, pos};
use crate::engine::api::error::Error;
use crate::engine::ast::{Expr, ExprKind, MethodRole, ObjProp, PropKey, Ref};
use crate::engine::compiler::lexer::{
    NumberKind, Punctuator, TokenKind, quickjs_simple_lookahead_has_line_terminator,
};
use crate::engine::value::JsString;

impl<'source> Parser<'source> {
    pub(super) fn parse_object_literal(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        if !self.is_punctuator(Punctuator::LeftBrace) {
            return Err(self.syntax_here("expecting '{'"));
        }
        self.advance()?;
        let mut props = Vec::new();
        let mut has_proto = false;
        while !self.is_punctuator(Punctuator::RightBrace) {
            if self.is_punctuator(Punctuator::Ellipsis) {
                let spread = self.current().span;
                self.advance_expression_start()?;
                let value = self.parse_assignment_allow_in()?;
                props.push(ObjProp::Spread {
                    value,
                    pos: pos(spread),
                });
            } else if self.is_punctuator(Punctuator::Multiply) {
                let span = self.current().span;
                self.advance()?;
                let key = self.parse_method_property_name()?;
                let function = self.parse_method(
                    span,
                    FunctionOptions::method(MethodRole::Method, false, true),
                )?;
                props.push(ObjProp::Method {
                    key,
                    function: Box::new(function),
                });
            } else if self.is_punctuator(Punctuator::LeftBracket) {
                let span = self.current().span;
                self.advance_expression_start()?;
                let key = self.parse_assignment_allow_in()?;
                self.expect_punctuator(Punctuator::RightBracket)?;
                if self.is_punctuator(Punctuator::LeftParen) {
                    let function = self.parse_method(
                        span,
                        FunctionOptions::method(MethodRole::Method, false, false),
                    )?;
                    props.push(ObjProp::Method {
                        key: PropKey::Computed(Box::new(key)),
                        function: Box::new(function),
                    });
                } else {
                    if !self.is_punctuator(Punctuator::Colon) {
                        return Err(self.syntax_here("expecting ':'"));
                    }
                    self.advance_expression_start()?;
                    let value = self.parse_assignment_allow_in()?;
                    props.push(ObjProp::Init {
                        key: PropKey::Computed(Box::new(key)),
                        value,
                        key_pos: pos(span),
                    });
                }
            } else {
                let token = *self.current();
                let mut shorthand = None;
                let mut prefix = None;
                let key = match token.kind {
                    TokenKind::Identifier(identifier) => {
                        let name = self.intern_identifier(&identifier);
                        // An escaped reserved word is a key, never a shorthand.
                        if !identifier.escaped_reserved_word {
                            shorthand = Some(identifier);
                        }
                        if !identifier.has_escape
                            && matches!(identifier.raw, "get" | "set" | "async")
                        {
                            prefix = Some(identifier.raw);
                        }
                        self.advance()?;
                        self.name_string(name)?
                    }
                    TokenKind::Keyword(keyword) => {
                        self.advance()?;
                        JsString::from_static(keyword.as_str())
                    }
                    TokenKind::String(string) => {
                        if self.strict() && string.has_legacy_octal_escape {
                            return Err(Error::syntax(
                                "legacy octal escapes are forbidden in strict mode",
                                token.span,
                            ));
                        }
                        self.advance()?;
                        self.decode_string_literal(token.span)?
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
                        let value = parse_number(&number)
                            .map_err(|message| Error::syntax(message, token.span))?;
                        number_key(&value)?
                    }
                    _ => return Err(self.syntax_here("invalid property name")),
                };
                let next_starts_name = matches!(
                    self.current().kind,
                    TokenKind::Identifier(_)
                        | TokenKind::Keyword(_)
                        | TokenKind::String(_)
                        | TokenKind::Number(_)
                        | TokenKind::PrivateIdentifier(_)
                        | TokenKind::Punctuator(Punctuator::LeftBracket)
                );
                let async_line_break = prefix == Some("async")
                    && quickjs_simple_lookahead_has_line_terminator(
                        &self.lexer.source()[token.span.end.byte_offset..],
                    );
                let is_prefix = prefix.is_some_and(|prefix| {
                    (next_starts_name
                        || (prefix == "async" && self.is_punctuator(Punctuator::Multiply)))
                        && (prefix != "async" || !async_line_break)
                });
                if self.is_punctuator(Punctuator::LeftParen) {
                    let function = self.parse_method(
                        token.span,
                        FunctionOptions::method(MethodRole::Method, false, false),
                    )?;
                    props.push(ObjProp::Method {
                        key: PropKey::Name(key),
                        function: Box::new(function),
                    });
                } else if is_prefix {
                    let prefix = prefix.expect("a method prefix");
                    let async_generator =
                        prefix == "async" && self.consume_punctuator(Punctuator::Multiply)?;
                    let key = self.parse_method_property_name()?;
                    let options = match (prefix, async_generator) {
                        ("async", generator) => {
                            FunctionOptions::method(MethodRole::Method, true, generator)
                        }
                        ("get", false) => FunctionOptions::method(MethodRole::Get, false, false),
                        ("set", false) => FunctionOptions::method(MethodRole::Set, false, false),
                        _ => return Err(Error::internal("invalid object method prefix")),
                    };
                    let function = self.parse_method(token.span, options)?;
                    props.push(ObjProp::Method {
                        key,
                        function: Box::new(function),
                    });
                } else if self.is_punctuator(Punctuator::Colon) {
                    self.advance_expression_start()?;
                    let value = self.parse_assignment_allow_in()?;
                    if key == JsString::from_static("__proto__") {
                        if has_proto {
                            return Err(Error::syntax(
                                "duplicate __proto__ property name",
                                token.span,
                            ));
                        }
                        has_proto = true;
                        props.push(ObjProp::Proto {
                            value,
                            pos: pos(token.span),
                        });
                    } else {
                        props.push(ObjProp::Init {
                            key: PropKey::Name(key),
                            value,
                            key_pos: pos(token.span),
                        });
                    }
                } else if let Some(identifier) = shorthand {
                    self.validate_identifier(
                        &identifier,
                        token.span,
                        self.strict(),
                        IdentifierContext::Reference,
                    )?;
                    let name = self.intern_identifier(&identifier);
                    props.push(ObjProp::Shorthand {
                        key,
                        value: Ref {
                            name,
                            span: token.span.into(),
                            scope: self.current_scope(),
                        },
                    });
                } else {
                    return Err(self.syntax_here("expecting ':'"));
                }
            }
            if !self.is_punctuator(Punctuator::Comma) {
                break;
            }
            self.advance()?;
        }
        self.expect_punctuator(Punctuator::RightBrace)?;
        Ok(Expr {
            kind: ExprKind::Object(props),
            span: self.span_from(start).into(),
        })
    }

    /// The name after `*`, `async`, `get` or `set`.
    pub(super) fn parse_method_property_name(&mut self) -> Result<PropKey, Error> {
        let token = *self.current();
        let key = match token.kind {
            TokenKind::Identifier(identifier) => {
                self.advance()?;
                let name = self.intern_identifier(&identifier);
                self.name_string(name)?
            }
            TokenKind::Keyword(keyword) => {
                self.advance()?;
                JsString::from_static(keyword.as_str())
            }
            TokenKind::String(string) => {
                if self.strict() && string.has_legacy_octal_escape {
                    return Err(Error::syntax(
                        "legacy octal escapes are forbidden in strict mode",
                        token.span,
                    ));
                }
                self.advance()?;
                self.decode_string_literal(token.span)?
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
                number_key(&value)?
            }
            TokenKind::Punctuator(Punctuator::LeftBracket) => {
                self.advance_expression_start()?;
                let key = self.parse_assignment_allow_in()?;
                self.expect_punctuator(Punctuator::RightBracket)?;
                return Ok(PropKey::Computed(Box::new(key)));
            }
            _ => return Err(self.syntax_here("invalid property name")),
        };
        Ok(PropKey::Name(key))
    }
}
