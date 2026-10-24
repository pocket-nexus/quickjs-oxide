//! Classes: QuickJS's `js_parse_class`.
//!
//! A class keeps its fields' initializers in two hidden functions, one for
//! the instance fields and one for the static elements (static fields and
//! static blocks), as QuickJS does. Each is created at its first element and
//! parsed piecewise: the parser suspends it between fields.

use super::functions::FunctionOptions;
use super::support::{IdentifierContext, number_key, parse_number};
use super::{FunctionState, ModuleExport, Parser, pos};
use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::ast::{
    Class, ClassMember, DeclKind, Expr, ExprKind, FieldInit, FieldKey, Function, FunctionKind,
    Ident, MethodRole, PropKey, ScopeKind, Stmt, StmtKind,
};
use crate::engine::code::module::MODULE_DEFAULT_BINDING_NAME;
use crate::engine::compiler::lexer::{
    Keyword, NumberKind, Punctuator, Span, TokenKind, quickjs_simple_lookahead_has_line_terminator,
};
use crate::engine::compiler::stack_guard::ParserStackFrame;
use crate::engine::value::JsString;

/// A suspended aggregate field initializer.
struct Aggregate {
    state: FunctionState,
    body: Vec<Stmt>,
    span: Span,
}

#[derive(Default)]
struct Elements {
    instance: Option<Aggregate>,
    statics: Option<Aggregate>,
    instance_computed: usize,
    static_computed: usize,
    constructor: Option<Function>,
    members: Vec<ClassMember>,
}

enum ClassKey {
    Fixed(JsString),
    Computed(Expr, Span),
    Private(crate::engine::compiler::names::NameId, Span),
}

impl<'source> Parser<'source> {
    pub(super) fn parse_class_declaration(&mut self) -> Result<StmtKind, Error> {
        let (class, decl) = self.parse_class(false)?;
        Ok(StmtKind::Class {
            class: Box::new(class),
            decl: decl.expect("a class declaration has an outer binding"),
        })
    }

    pub(super) fn parse_class_expression(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let (class, _) = self.parse_class(true)?;
        Ok(Expr {
            kind: ExprKind::Class(Box::new(class)),
            span: self.span_from(start),
        })
    }

    pub(super) fn current_module_export(&self) -> Option<ModuleExport> {
        let state = self.state();
        self.module_export
            .filter(|(function, scope, _)| *function == state.id && *scope == state.current_scope)
            .map(|(_, _, export)| export)
    }

    fn parse_class(
        &mut self,
        expression: bool,
    ) -> Result<(Class, Option<crate::engine::ast::DeclId>), Error> {
        let class_token = *self.current();
        let outer_strict = self.strict();
        // QuickJS parses the class in strict mode.
        self.state_mut().strict = true;
        self.relex_current_with_strict(true)?;
        self.advance()?;
        let name = if let TokenKind::Identifier(identifier) = self.current().kind {
            let span = self.current().span;
            // Only reserved-word status is checked: `eval` and `arguments`
            // are accepted class names.
            self.validate_identifier_reservation(
                &identifier,
                span,
                true,
                IdentifierContext::Variable,
            )?;
            self.advance()?;
            Some(Ident {
                name: self.intern_identifier(&identifier),
                span,
            })
        } else {
            None
        };
        let default_declaration =
            !expression && self.current_module_export() == Some(ModuleExport::Default);
        if !expression && name.is_none() && !default_declaration {
            return Err(self.syntax_here("class statement requires a name"));
        }
        let outer = if expression {
            None
        } else if let Some(name) = name {
            Some((name.name, name.span))
        } else {
            Some((
                self.names.intern(MODULE_DEFAULT_BINDING_NAME),
                class_token.span,
            ))
        };
        // QuickJS defines the outer binding after the class body, so a
        // redeclaration is reported after any error inside the class, at the
        // token that follows it.
        let mut outer_decl = None;
        let mut deferred = None;
        if let Some((outer_name, outer_span)) = outer {
            let conflict = self.current().span;
            match self.register_lexical(outer_name, outer_span, conflict, DeclKind::Class, false) {
                Ok(decl) => outer_decl = Some(decl),
                Err(error) if error.kind() == ErrorKind::Syntax => deferred = Some(error),
                Err(error) => return Err(error),
            }
        }
        let scope = self.push_scope(ScopeKind::Block);
        let name_binding = match name {
            Some(name) => {
                let conflict = self.current().span;
                Some(self.register_lexical(
                    name.name,
                    name.span,
                    conflict,
                    DeclKind::ClassName,
                    false,
                )?)
            }
            None => None,
        };
        let heritage = if self.is_keyword(Keyword::Extends) {
            self.advance_expression_start()?;
            Some(Box::new(self.parse_left_hand_side_expression()?))
        } else {
            None
        };
        let derived = heritage.is_some();
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let private_scope = self.push_scope(ScopeKind::ClassPrivate);
        let mut elements = Elements::default();
        while !self.is_punctuator(Punctuator::RightBrace) {
            if self.at_eof() {
                return Err(self.syntax_here("unterminated class body"));
            }
            if self.consume_punctuator(Punctuator::Semicolon)? {
                continue;
            }
            self.parse_class_element(&mut elements, derived)?;
        }
        let closing = self.current().span;
        self.advance()?;
        let source = (pos(class_token.span), closing.end.byte_offset as u32);
        let mut constructor = match elements.constructor.take() {
            Some(constructor) => constructor,
            None => self.synthesize_constructor(class_token.span, derived),
        };
        constructor.name = name;
        constructor.span = class_token.span;
        constructor.source = source;
        let instance_init = elements
            .instance
            .take()
            .map(|aggregate| self.finish_aggregate(aggregate));
        let static_init = elements
            .statics
            .take()
            .map(|aggregate| self.finish_aggregate(aggregate));
        self.pop_scope(private_scope)?;
        self.pop_scope(scope)?;
        self.state_mut().strict = outer_strict;
        self.relex_current_with_strict(outer_strict)?;
        if let Some(error) = deferred {
            return Err(error.with_span(self.current().span));
        }
        Ok((
            Class {
                name,
                name_binding,
                heritage,
                scope,
                private_scope,
                constructor: Box::new(constructor),
                members: elements.members,
                instance_init: instance_init.map(Box::new),
                static_init: static_init.map(Box::new),
                span: self.span_from(class_token.span),
            },
            outer_decl,
        ))
    }

    fn synthesize_constructor(&mut self, span: Span, derived: bool) -> Function {
        let definition = self.current_scope();
        let id = self.begin_function(
            FunctionKind::ClassConstructor { derived },
            true,
            Some(definition),
        );
        self.end_function();
        Function {
            id,
            kind: FunctionKind::ClassConstructor { derived },
            role: MethodRole::Method,
            is_async: false,
            is_generator: false,
            name: None,
            self_binding: None,
            params: Vec::new(),
            rest: None,
            simple_params: true,
            length: 0,
            body: Vec::new(),
            concise: false,
            strict: true,
            span,
            source: (pos(span), pos(span)),
        }
    }

    fn parse_class_element(&mut self, elements: &mut Elements, derived: bool) -> Result<(), Error> {
        let element_span = self.current().span;
        let mut is_static = false;
        if self.is_keyword(Keyword::Static) {
            let next = self.token_after_current()?;
            if !matches!(
                next.kind,
                TokenKind::Punctuator(
                    Punctuator::LeftParen
                        | Punctuator::Semicolon
                        | Punctuator::Equal
                        | Punctuator::RightBrace
                )
            ) {
                is_static = true;
                self.advance()?;
                if self.is_punctuator(Punctuator::LeftBrace) {
                    return self.parse_static_block(elements, element_span);
                }
            }
        }
        let function_span = self.current().span;
        let generator = self.consume_punctuator(Punctuator::Multiply)?;
        let asynchronous = !generator && self.class_async_method_ahead()?;
        let async_generator = if asynchronous {
            self.advance()?;
            self.consume_punctuator(Punctuator::Multiply)?
        } else {
            false
        };
        let mut role = MethodRole::Method;
        if !generator
            && !asynchronous
            && let TokenKind::Identifier(identifier) = &self.current().kind
            && !identifier.has_escape
            && matches!(identifier.raw, "get" | "set")
        {
            let getter = identifier.raw == "get";
            let next = self.token_after_current()?;
            if !next.line_terminator_before && class_property_name_starts(&next.kind) {
                role = if getter {
                    MethodRole::Get
                } else {
                    MethodRole::Set
                };
                self.advance()?;
            }
        }
        let key = self.parse_class_property_key()?;
        let is_generator = generator || async_generator;
        if !self.is_punctuator(Punctuator::LeftParen) {
            if generator || asynchronous || role != MethodRole::Method {
                return Err(self.syntax_here("invalid property name"));
            }
            if is_static
                && matches!(&key, ClassKey::Fixed(name) if *name == JsString::from_static("prototype"))
            {
                return Err(self.syntax_here("invalid method name"));
            }
            return match key {
                ClassKey::Private(name, span) => {
                    self.parse_private_field(elements, is_static, name, span)
                }
                key => self.parse_public_field(elements, is_static, key, function_span),
            };
        }
        let options = FunctionOptions::method(role, asynchronous, is_generator);
        if let ClassKey::Private(name, span) = key {
            if self.names.name(name) == "#constructor" {
                return Err(self.syntax_here("invalid method name"));
            }
            let decl = if role == MethodRole::Method {
                let function = self.parse_method(function_span, options)?;
                let decl =
                    self.register_private(name, span, DeclKind::PrivateMethod { is_static })?;
                self.ensure_aggregate(elements, is_static, span);
                elements.members.push(ClassMember::PrivateMethod {
                    decl,
                    function: Box::new(function),
                    is_static,
                });
                return Ok(());
            } else {
                self.register_private_accessor(name, span, is_static, role == MethodRole::Get)?
            };
            let function = self.parse_method(function_span, options)?;
            self.ensure_aggregate(elements, is_static, span);
            elements.members.push(ClassMember::PrivateMethod {
                decl,
                function: Box::new(function),
                is_static,
            });
            return Ok(());
        }
        let fixed = match &key {
            ClassKey::Fixed(name) => Some(name.clone()),
            _ => None,
        };
        let is_constructor = fixed
            .as_ref()
            .is_some_and(|name| *name == JsString::from_static("constructor"));
        if !is_static && is_constructor && (role != MethodRole::Method || generator || asynchronous)
        {
            return Err(self.syntax_here("invalid method name"));
        }
        if is_static
            && fixed
                .as_ref()
                .is_some_and(|name| *name == JsString::from_static("prototype"))
        {
            return Err(self.syntax_here("invalid method name"));
        }
        if !is_static && is_constructor {
            if elements.constructor.is_some() {
                return Err(Error::syntax(
                    "property constructor appears more than once",
                    function_span,
                ));
            }
            let options = FunctionOptions {
                kind: FunctionKind::ClassConstructor { derived },
                role: MethodRole::Method,
                is_async: false,
                is_generator: false,
                self_binding: false,
                reject_duplicate_parameters: true,
            };
            elements.constructor = Some(self.parse_method(function_span, options)?);
            return Ok(());
        }
        let function = self.parse_method(function_span, options)?;
        let key = match key {
            ClassKey::Fixed(name) => PropKey::Name(name),
            ClassKey::Computed(expr, _) => PropKey::Computed(Box::new(expr)),
            ClassKey::Private(..) => unreachable!("private methods returned above"),
        };
        elements.members.push(ClassMember::Method {
            key,
            function: Box::new(function),
            is_static,
        });
        Ok(())
    }

    fn parse_class_property_key(&mut self) -> Result<ClassKey, Error> {
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
                if string.has_legacy_octal_escape {
                    return Err(Error::syntax(
                        "legacy octal escapes are forbidden in strict mode",
                        token.span,
                    ));
                }
                self.advance()?;
                self.decode_string_literal(token.span)?
            }
            TokenKind::Number(number) => {
                if matches!(
                    number.kind,
                    NumberKind::LegacyOctal | NumberKind::LegacyDecimal
                ) {
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
                let expr = self.parse_assignment_allow_in()?;
                self.expect_punctuator(Punctuator::RightBracket)?;
                return Ok(ClassKey::Computed(expr, token.span));
            }
            TokenKind::PrivateIdentifier(identifier) => {
                let is_constructor = self.identical_name(&identifier, "constructor");
                self.advance()?;
                // QuickJS checks `#constructor` after advancing.
                if is_constructor {
                    return Err(self.syntax_here("invalid method name"));
                }
                let name = self.intern_private_identifier(&identifier);
                return Ok(ClassKey::Private(name, token.span));
            }
            _ => return Err(self.syntax_here("invalid property name")),
        };
        Ok(ClassKey::Fixed(key))
    }

    fn class_async_method_ahead(&self) -> Result<bool, Error> {
        let TokenKind::Identifier(identifier) = &self.current().kind else {
            return Ok(false);
        };
        if !self.is_unescaped_name(identifier, "async") {
            return Ok(false);
        }
        let next = self.token_after_current()?;
        let line_break = quickjs_simple_lookahead_has_line_terminator(
            &self.lexer.source()[self.current().span.end.byte_offset..],
        );
        Ok(!line_break
            && !matches!(
                next.kind,
                TokenKind::Punctuator(
                    Punctuator::Colon
                        | Punctuator::Comma
                        | Punctuator::RightBrace
                        | Punctuator::LeftParen
                        | Punctuator::Equal
                )
            ))
    }

    // ---- fields ----

    fn ensure_aggregate(&mut self, elements: &mut Elements, is_static: bool, span: Span) {
        let slot = if is_static {
            &elements.statics
        } else {
            &elements.instance
        };
        if slot.is_some() {
            return;
        }
        let definition = self.current_scope();
        self.begin_function(
            FunctionKind::ClassFieldInit { is_static },
            true,
            Some(definition),
        );
        let mut state = self.stack.pop().expect("the aggregate was just begun");
        state.in_function_body = true;
        let aggregate = Aggregate {
            state,
            body: Vec::new(),
            span,
        };
        if is_static {
            elements.statics = Some(aggregate);
        } else {
            elements.instance = Some(aggregate);
        }
    }

    fn aggregate(elements: &mut Elements, is_static: bool) -> &mut Aggregate {
        if is_static {
            elements.statics.as_mut().expect("the aggregate exists")
        } else {
            elements.instance.as_mut().expect("the aggregate exists")
        }
    }

    /// Parse `[= value]` in the aggregate initializer.
    fn parse_field_initializer(
        &mut self,
        elements: &mut Elements,
        is_static: bool,
    ) -> Result<Option<Expr>, Error> {
        let aggregate = Self::aggregate(elements, is_static);
        let placeholder = placeholder_state(&aggregate.state);
        let state = std::mem::replace(&mut aggregate.state, placeholder);
        let parent_context = self.lexer.context();
        self.stack.push(state);
        let mut field_context = parent_context;
        field_context.strict = true;
        field_context.generator = false;
        field_context.async_function = false;
        self.set_future_lex_context(field_context);
        let value = if self.consume_punctuator(Punctuator::Equal)? {
            Some(self.parse_assignment())
        } else {
            None
        };
        let state = self.stack.pop().expect("the aggregate was pushed");
        Self::aggregate(elements, is_static).state = state;
        // QuickJS restores the context at the terminator without rescanning it.
        self.set_future_lex_context(parent_context);
        value.transpose()
    }

    fn parse_public_field(
        &mut self,
        elements: &mut Elements,
        is_static: bool,
        key: ClassKey,
        span: Span,
    ) -> Result<(), Error> {
        if matches!(&key, ClassKey::Fixed(name)
            if *name == JsString::from_static("constructor") || *name == JsString::from_static("prototype"))
        {
            return Err(self.syntax_here("invalid field name"));
        }
        self.ensure_aggregate(elements, is_static, span);
        let field_key = match key {
            ClassKey::Fixed(name) => FieldKey::Name(name),
            ClassKey::Computed(expr, key_span) => {
                let count = if is_static {
                    &mut elements.static_computed
                } else {
                    &mut elements.instance_computed
                };
                let hidden = if is_static {
                    format!("<static_computed_field>{count}")
                } else {
                    format!("<computed_field>{count}")
                };
                *count += 1;
                let hidden = self.names.intern(&hidden);
                let conflict = self.current().span;
                let decl = self.register_lexical(
                    hidden,
                    span,
                    conflict,
                    DeclKind::Hidden { is_const: true },
                    false,
                )?;
                elements.members.push(ClassMember::FieldKey {
                    key: Box::new(expr),
                    decl,
                    is_static,
                    pos: pos(key_span),
                });
                FieldKey::Computed(decl)
            }
            ClassKey::Private(..) => unreachable!("private fields are parsed separately"),
        };
        let value = self.parse_field_initializer(elements, is_static)?;
        let field = Stmt {
            kind: StmtKind::FieldInit(Box::new(FieldInit::Public {
                key: field_key,
                value,
                span,
            })),
            span,
        };
        Self::aggregate(elements, is_static).body.push(field);
        self.consume_statement_terminator()
    }

    fn parse_private_field(
        &mut self,
        elements: &mut Elements,
        is_static: bool,
        name: crate::engine::compiler::names::NameId,
        span: Span,
    ) -> Result<(), Error> {
        if self.names.name(name) == "#constructor" {
            return Err(self.syntax_here("invalid method name"));
        }
        let decl = self.register_private(name, span, DeclKind::PrivateField { is_static })?;
        elements
            .members
            .push(ClassMember::PrivateField { decl, is_static });
        self.ensure_aggregate(elements, is_static, span);
        let value = self.parse_field_initializer(elements, is_static)?;
        let field = Stmt {
            kind: StmtKind::FieldInit(Box::new(FieldInit::Private { decl, value, span })),
            span,
        };
        Self::aggregate(elements, is_static).body.push(field);
        self.consume_statement_terminator()
    }

    fn parse_static_block(&mut self, elements: &mut Elements, span: Span) -> Result<(), Error> {
        self.ensure_aggregate(elements, true, span);
        let aggregate = Self::aggregate(elements, true);
        let placeholder = placeholder_state(&aggregate.state);
        let state = std::mem::replace(&mut aggregate.state, placeholder);
        self.stack.push(state);
        let result = self.parse_static_block_body(span);
        let state = self.stack.pop().expect("the aggregate was pushed");
        Self::aggregate(elements, true).state = state;
        let function = result?;
        Self::aggregate(elements, true).body.push(Stmt {
            kind: StmtKind::FieldInit(Box::new(FieldInit::StaticBlock(Box::new(function)))),
            span,
        });
        Ok(())
    }

    fn parse_static_block_body(&mut self, span: Span) -> Result<Function, Error> {
        let definition = self.current_scope();
        let id = self.begin_function(FunctionKind::StaticBlock, true, Some(definition));
        self.state_mut().in_function_body = true;
        // An unescaped `await` is a keyword in a static block, which stays a
        // synchronous function.
        let parent_context = self.lexer.context();
        let mut block_context = parent_context;
        block_context.async_function = true;
        self.set_future_lex_context(block_context);
        self.expect_punctuator(Punctuator::LeftBrace)?;
        let body =
            self.parse_recursion(ParserStackFrame::FunctionBody, Self::parse_function_body)?;
        let closing = self.current().span;
        self.relex_current_with_context(parent_context)?;
        self.expect_punctuator(Punctuator::RightBrace)?;
        self.end_function();
        Ok(Function {
            id,
            kind: FunctionKind::StaticBlock,
            role: MethodRole::Method,
            is_async: false,
            is_generator: false,
            name: None,
            self_binding: None,
            params: Vec::new(),
            rest: None,
            simple_params: true,
            length: 0,
            body,
            concise: false,
            strict: true,
            span,
            source: (pos(span), closing.end.byte_offset as u32),
        })
    }

    fn finish_aggregate(&mut self, aggregate: Aggregate) -> Function {
        let Aggregate { state, body, span } = aggregate;
        let kind = state.kind;
        let id = state.id;
        self.stack.push(state);
        self.end_function();
        let name = match kind {
            FunctionKind::ClassFieldInit { is_static: true } => "<class_static_init>",
            _ => "<class_fields_init>",
        };
        Function {
            id,
            kind,
            role: MethodRole::Method,
            is_async: false,
            is_generator: false,
            name: Some(Ident {
                name: self.names.intern(name),
                span,
            }),
            self_binding: None,
            params: Vec::new(),
            rest: None,
            simple_params: true,
            length: 0,
            body,
            concise: false,
            strict: true,
            span,
            source: (pos(span), pos(span)),
        }
    }
}

fn class_property_name_starts(kind: &TokenKind<'_>) -> bool {
    matches!(
        kind,
        TokenKind::Identifier(_)
            | TokenKind::Keyword(_)
            | TokenKind::String(_)
            | TokenKind::Number(_)
            | TokenKind::PrivateIdentifier(_)
            | TokenKind::Punctuator(Punctuator::LeftBracket)
    )
}

/// A stand-in while the real state is on the parser's stack.
fn placeholder_state(state: &FunctionState) -> FunctionState {
    FunctionState {
        id: state.id,
        kind: state.kind,
        is_async: false,
        is_generator: false,
        strict: true,
        current_scope: state.current_scope,
        var_scope: state.var_scope,
        body_scope: state.body_scope,
        parameter_scope: None,
        in_function_body: true,
        super_call_allowed: false,
        super_allowed: true,
        arguments_forbidden: true,
        breaks: Vec::new(),
        parameter_names: Vec::new(),
        pattern_parameter_initialization: false,
        simple_params: true,
        global_declarations: Vec::new(),
    }
}
