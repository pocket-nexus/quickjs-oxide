//! Expressions: assignment, operators, calls, members and primaries.

use super::patterns::ReferenceKind;
use super::support::{IdentifierContext, Number, parse_number};
use super::{FunctionKind, InMode, Parser, pos};
use crate::engine::api::error::{Error, ErrorKind, NativeErrorMessage};
use crate::engine::ast::{
    Arg, ArrayItem, AssignOp, BinaryOp, Expr, ExprKind, LogicalOp, PropKey, Ref, RegExpLiteral,
    UnaryOp,
};
use crate::engine::compiler::lexer::{
    Keyword, LexicalGoal, NumberKind, Punctuator, Span, TokenKind,
};
use crate::engine::compiler::stack_guard::ParserStackFrame;
use crate::engine::value::JsString;
use std::rc::Rc;

/// QuickJS's `PF_POW_ALLOWED`, `PF_POW_FORBIDDEN` and zero flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PowerMode {
    Allowed,
    Forbidden,
    None,
}

/// QuickJS's fixed-argument limit.
const MAX_CALL_ARGUMENTS: usize = 65_535;

fn expr(kind: ExprKind, span: Span) -> Expr {
    Expr { kind, span }
}

impl<'source> Parser<'source> {
    pub(super) fn parse_expression(&mut self) -> Result<Expr, Error> {
        self.with_in_mode(InMode::Allow, Self::parse_comma)
    }

    pub(super) fn parse_expression_no_in(&mut self) -> Result<Expr, Error> {
        self.with_in_mode(InMode::Disallow, Self::parse_comma)
    }

    pub(super) fn parse_assignment_allow_in(&mut self) -> Result<Expr, Error> {
        self.with_in_mode(InMode::Allow, Self::parse_assignment)
    }

    fn parse_comma(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let first = self.parse_assignment()?;
        if !self.is_punctuator(Punctuator::Comma) {
            return Ok(first);
        }
        let mut exprs = vec![first];
        while self.is_punctuator(Punctuator::Comma) {
            self.advance()?;
            exprs.push(self.parse_assignment()?);
        }
        Ok(expr(ExprKind::Sequence(exprs), self.span_from(start)))
    }

    pub(super) fn parse_assignment(&mut self) -> Result<Expr, Error> {
        if self.is_keyword(Keyword::Yield) {
            return self.parse_recursion(ParserStackFrame::Yield, Self::parse_yield_expression);
        }
        if let Some(head) = self.async_arrow_ahead() {
            return self.parse_async_arrow_function(head);
        }
        if let Some(head) = self.arrow_head_ahead() {
            return self.parse_arrow_function(head);
        }
        let Some(declined) = self.lookahead.get_mut().declined_arrow.take() else {
            return self.parse_assignment_tail();
        };
        let result = self.parse_assignment_tail();
        if (result.is_err() || self.is_punctuator(Punctuator::Arrow))
            && let Some(error) = self.declined_arrow_error(declined)
        {
            return Err(error);
        }
        result
    }

    fn parse_assignment_tail(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        if self.object_assignment_pattern_ahead() || self.array_assignment_pattern_ahead() {
            let pattern = self.parse_assignment_pattern()?;
            self.expect_punctuator(Punctuator::Equal)?;
            let value = self.parse_assignment_allow_in()?;
            return Ok(expr(
                ExprKind::AssignPattern {
                    pattern: Box::new(pattern),
                    value: Box::new(value),
                },
                self.span_from(start),
            ));
        }
        let target = self.parse_conditional()?;
        let op = match self.current().kind {
            TokenKind::Punctuator(p) => match p {
                Punctuator::Equal => AssignOp::Assign,
                Punctuator::PlusAssign => AssignOp::Binary(BinaryOp::Add),
                Punctuator::MinusAssign => AssignOp::Binary(BinaryOp::Sub),
                Punctuator::MultiplyAssign => AssignOp::Binary(BinaryOp::Mul),
                Punctuator::DivideAssign => AssignOp::Binary(BinaryOp::Div),
                Punctuator::RemainderAssign => AssignOp::Binary(BinaryOp::Rem),
                Punctuator::ExponentAssign => AssignOp::Binary(BinaryOp::Exp),
                Punctuator::ShiftLeftAssign => AssignOp::Binary(BinaryOp::Shl),
                Punctuator::ShiftRightAssign => AssignOp::Binary(BinaryOp::Sar),
                Punctuator::UnsignedShiftRightAssign => AssignOp::Binary(BinaryOp::Shr),
                Punctuator::BitAndAssign => AssignOp::Binary(BinaryOp::BitAnd),
                Punctuator::BitXorAssign => AssignOp::Binary(BinaryOp::BitXor),
                Punctuator::BitOrAssign => AssignOp::Binary(BinaryOp::BitOr),
                Punctuator::LogicalAndAssign => AssignOp::Logical(LogicalOp::And),
                Punctuator::LogicalOrAssign => AssignOp::Logical(LogicalOp::Or),
                Punctuator::NullishAssign => AssignOp::Logical(LogicalOp::Nullish),
                _ => return Ok(target),
            },
            _ => return Ok(target),
        };
        let operator = self.current().span;
        // QuickJS consumes the operator before rejecting a non-reference, so
        // these errors point at the first RHS token.
        self.advance()?;
        if target.contains_optional_chain() {
            return Err(self.syntax_here("invalid assignment left-hand side"));
        }
        match target.reference_kind() {
            Some(ReferenceKind::Identifier(name)) => {
                if self.is_strict_restricted_name(name) {
                    return Err(self.syntax_here("invalid lvalue in strict mode"));
                }
            }
            Some(ReferenceKind::Member) => {}
            None => return Err(self.syntax_here("invalid assignment left-hand side")),
        }
        let value = self.parse_recursion(ParserStackFrame::Assignment, Self::parse_assignment)?;
        Ok(expr(
            ExprKind::Assign {
                op,
                target: Box::new(target),
                value: Box::new(value),
                pos: pos(operator),
            },
            self.span_from(start),
        ))
    }

    fn parse_conditional(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let test = self.parse_coalesce()?;
        if !self.is_punctuator(Punctuator::Question) {
            return Ok(test);
        }
        self.advance()?;
        // The consequent is an ordinary AssignmentExpression even in NoIn.
        let then = self.parse_recursion(
            ParserStackFrame::Conditional,
            Self::parse_assignment_allow_in,
        )?;
        self.expect_punctuator(Punctuator::Colon)?;
        let otherwise =
            self.parse_recursion(ParserStackFrame::Conditional, Self::parse_assignment)?;
        Ok(expr(
            ExprKind::Conditional {
                test: Box::new(test),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            self.span_from(start),
        ))
    }

    /// A `??` chain: its right operands are below the `&&`/`||` level, so
    /// unparenthesized mixing stays a syntax error.
    fn parse_coalesce(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut left = self.parse_logical_or()?;
        while self.is_punctuator(Punctuator::NullishCoalesce) {
            self.advance()?;
            let right = self.parse_binary(1)?;
            left = expr(
                ExprKind::Logical {
                    op: LogicalOp::Nullish,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                self.span_from(start),
            );
        }
        Ok(left)
    }

    fn parse_logical_or(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut left = self.parse_logical_and()?;
        let mut composed = false;
        while self.is_punctuator(Punctuator::LogicalOr) {
            composed = true;
            self.advance()?;
            let right = self.parse_logical_and()?;
            left = expr(
                ExprKind::Logical {
                    op: LogicalOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                self.span_from(start),
            );
        }
        if composed && self.is_punctuator(Punctuator::NullishCoalesce) {
            return Err(self.syntax_here("cannot mix ?? with && or ||"));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut left = self.parse_binary(1)?;
        let mut composed = false;
        while self.is_punctuator(Punctuator::LogicalAnd) {
            composed = true;
            self.advance()?;
            let right = self.parse_binary(1)?;
            left = expr(
                ExprKind::Logical {
                    op: LogicalOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                self.span_from(start),
            );
        }
        if composed && self.is_punctuator(Punctuator::NullishCoalesce) {
            return Err(self.syntax_here("cannot mix ?? with && or ||"));
        }
        Ok(left)
    }

    fn parse_binary(&mut self, minimum: u8) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut left = match self.parse_private_in_head(minimum)? {
            Some(head) => head,
            None => self.parse_unary(PowerMode::Allowed)?,
        };
        while let Some((precedence, op)) = self.binary_operator() {
            if precedence < minimum {
                break;
            }
            let operator = self.current().span;
            self.advance()?;
            let right = self.parse_binary(precedence + 1)?;
            left = expr(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    pos: pos(operator),
                },
                self.span_from(start),
            );
        }
        Ok(left)
    }

    fn binary_operator(&self) -> Option<(u8, BinaryOp)> {
        use Punctuator as P;
        Some(match self.current().kind {
            TokenKind::Punctuator(operator) => match operator {
                P::BitOr => (1, BinaryOp::BitOr),
                P::BitXor => (2, BinaryOp::BitXor),
                P::BitAnd => (3, BinaryOp::BitAnd),
                P::EqualEqual => (4, BinaryOp::Eq),
                P::StrictEqual => (4, BinaryOp::StrictEq),
                P::NotEqual => (4, BinaryOp::Ne),
                P::StrictNotEqual => (4, BinaryOp::StrictNe),
                P::Less => (5, BinaryOp::Lt),
                P::LessEqual => (5, BinaryOp::Le),
                P::Greater => (5, BinaryOp::Gt),
                P::GreaterEqual => (5, BinaryOp::Ge),
                P::ShiftLeft => (6, BinaryOp::Shl),
                P::ShiftRight => (6, BinaryOp::Sar),
                P::UnsignedShiftRight => (6, BinaryOp::Shr),
                P::Plus => (7, BinaryOp::Add),
                P::Minus => (7, BinaryOp::Sub),
                P::Multiply => (8, BinaryOp::Mul),
                P::Divide => (8, BinaryOp::Div),
                P::Remainder => (8, BinaryOp::Rem),
                _ => return None,
            },
            TokenKind::Keyword(Keyword::Instanceof) => (5, BinaryOp::InstanceOf),
            TokenKind::Keyword(Keyword::In) if self.in_mode == InMode::Allow => (5, BinaryOp::In),
            _ => return None,
        })
    }

    /// `#name in ShiftExpression`, at a relational head.
    fn parse_private_in_head(&mut self, minimum: u8) -> Result<Option<Expr>, Error> {
        if minimum > 5 || self.in_mode == InMode::Disallow {
            return Ok(None);
        }
        let TokenKind::PrivateIdentifier(identifier) = self.current().kind else {
            return Ok(None);
        };
        let next = self.token_after_current()?;
        if !matches!(next.kind, TokenKind::Keyword(Keyword::In)) {
            return Ok(None);
        }
        let token = *self.current();
        let name = self.intern_private_identifier(&identifier);
        let scope = self.current_scope();
        self.advance()?;
        self.advance()?;
        let object = self.parse_binary(6)?;
        Ok(Some(expr(
            ExprKind::PrivateIn {
                name: Ref {
                    name,
                    span: token.span,
                    scope,
                },
                obj: Box::new(object),
                pos: pos(token.span),
            },
            self.span_from(token.span),
        )))
    }

    fn parse_unary_operand(&mut self) -> Result<Expr, Error> {
        self.parse_recursion(ParserStackFrame::Unary, |parser| {
            parser.parse_unary(PowerMode::Forbidden)
        })
    }

    fn parse_unary(&mut self, power_mode: PowerMode) -> Result<Expr, Error> {
        let start = self.current().span;
        if matches!(
            self.current().kind,
            TokenKind::Punctuator(Punctuator::Increment | Punctuator::Decrement)
        ) {
            let increment = self.is_punctuator(Punctuator::Increment);
            self.advance()?;
            // `++x ** 2` is valid: the operand gets no power flag.
            let target = self.parse_unary(PowerMode::None)?;
            self.check_update_target(&target)?;
            let update = expr(
                ExprKind::Update {
                    increment,
                    prefix: true,
                    target: Box::new(target),
                    pos: pos(start),
                },
                self.span_from(start),
            );
            return self.parse_power_suffix(update, start, power_mode);
        }
        if self.is_keyword(Keyword::Await) {
            let state = self.state();
            if !state.is_async {
                return Err(self.syntax_here("unexpected 'await' keyword"));
            }
            if !state.in_function_body {
                return Err(self.syntax_here("await in default expression"));
            }
            if state.kind == FunctionKind::Module
                && let Some(module) = self.module.as_mut()
            {
                module.has_top_level_await = true;
            }
            self.advance()?;
            let arg = self.parse_unary_operand()?;
            return Ok(expr(
                ExprKind::Await {
                    arg: Box::new(arg),
                    pos: pos(start),
                },
                self.span_from(start),
            ));
        }
        let op = match self.current().kind {
            TokenKind::Keyword(Keyword::Typeof) => Some(UnaryOp::TypeOf),
            TokenKind::Keyword(Keyword::Delete) => Some(UnaryOp::Delete),
            TokenKind::Keyword(Keyword::Void) => Some(UnaryOp::Void),
            TokenKind::Punctuator(Punctuator::Plus) => Some(UnaryOp::Plus),
            TokenKind::Punctuator(Punctuator::Minus) => Some(UnaryOp::Neg),
            TokenKind::Punctuator(Punctuator::BitNot) => Some(UnaryOp::BitNot),
            TokenKind::Punctuator(Punctuator::Not) => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            self.advance()?;
            let operand = self.parse_unary_operand()?;
            if op == UnaryOp::Delete {
                self.check_delete_operand(&operand)?;
            }
            let unary = expr(
                ExprKind::Unary {
                    op,
                    expr: Box::new(operand),
                    pos: pos(start),
                },
                self.span_from(start),
            );
            return self.parse_power_suffix(unary, start, power_mode);
        }
        let postfix = self.parse_postfix()?;
        self.parse_power_suffix(postfix, start, power_mode)
    }

    fn check_delete_operand(&self, operand: &Expr) -> Result<(), Error> {
        // An optional chain's private member is deleted as a value.
        let mut inner = operand;
        while let ExprKind::Paren(next) = &inner.kind {
            inner = next;
        }
        match &inner.kind {
            ExprKind::Member {
                prop: PropKey::Private(_),
                ..
            } => Err(self.syntax_here("cannot delete a private class field")),
            ExprKind::Ident(_) if self.strict() => {
                Err(self.syntax_here("cannot delete a direct reference in strict mode"))
            }
            _ => Ok(()),
        }
    }

    fn check_update_target(&self, target: &Expr) -> Result<(), Error> {
        if target.contains_optional_chain() {
            return Err(self.syntax_here("invalid increment/decrement operand"));
        }
        match target.reference_kind() {
            Some(ReferenceKind::Identifier(name)) => {
                if self.is_strict_restricted_name(name) {
                    return Err(self.syntax_here("invalid lvalue in strict mode"));
                }
                Ok(())
            }
            Some(ReferenceKind::Member) => Ok(()),
            None => Err(self.syntax_here("invalid increment/decrement operand")),
        }
    }

    fn parse_power_suffix(
        &mut self,
        left: Expr,
        start: Span,
        power_mode: PowerMode,
    ) -> Result<Expr, Error> {
        if !self.is_punctuator(Punctuator::Exponent) {
            return Ok(left);
        }
        match power_mode {
            PowerMode::None => return Ok(left),
            PowerMode::Forbidden => {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    "unparenthesized unary expression can't appear on the left-hand side of '**'",
                ));
            }
            PowerMode::Allowed => {}
        }
        let operator = self.current().span;
        self.advance()?;
        let right = self.parse_recursion(ParserStackFrame::Exponentiation, |parser| {
            parser.parse_unary(PowerMode::Allowed)
        })?;
        Ok(expr(
            ExprKind::Binary {
                op: BinaryOp::Exp,
                left: Box::new(left),
                right: Box::new(right),
                pos: pos(operator),
            },
            self.span_from(start),
        ))
    }

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let target = self.parse_left_hand_side_expression()?;
        if !self.current().line_terminator_before
            && matches!(
                self.current().kind,
                TokenKind::Punctuator(Punctuator::Increment | Punctuator::Decrement)
            )
        {
            let operator = self.current().span;
            let increment = self.is_punctuator(Punctuator::Increment);
            self.check_update_target(&target)?;
            self.advance()?;
            return Ok(expr(
                ExprKind::Update {
                    increment,
                    prefix: false,
                    target: Box::new(target),
                    pos: pos(operator),
                },
                self.span_from(start),
            ));
        }
        Ok(target)
    }

    /// A LeftHandSideExpression: a primary and its member, call and template
    /// suffixes, without a postfix update.
    pub(super) fn parse_left_hand_side_expression(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut current = self.parse_primary(true)?;
        let mut chain = false;
        loop {
            let optional = if self.is_punctuator(Punctuator::OptionalChain) {
                let optional_span = self.current().span;
                self.advance()?;
                chain = true;
                if !self.is_punctuator(Punctuator::LeftParen) {
                    current = self.parse_member_access(current, start, optional_span, true)?;
                    continue;
                }
                true
            } else {
                false
            };
            if !optional && let Some(member) = self.parse_member_suffix(&mut current, start)? {
                current = member;
                continue;
            }
            if optional || self.is_punctuator(Punctuator::LeftParen) {
                let call = self.current().span;
                let direct_eval = !optional && is_eval_reference(&current, &self.names);
                self.advance()?;
                let args = self.parse_call_arguments()?;
                current = expr(
                    ExprKind::Call {
                        callee: Box::new(current),
                        args,
                        optional,
                        direct_eval,
                        pos: pos(call),
                    },
                    self.span_from(start),
                );
                continue;
            }
            if chain && matches!(self.current().kind, TokenKind::Template(_)) {
                return Err(self.syntax_here("template literal cannot appear in an optional chain"));
            }
            if matches!(self.current().kind, TokenKind::Template(_)) {
                current = self.parse_tagged_template(current, start)?;
                continue;
            }
            break;
        }
        if chain {
            current = expr(
                ExprKind::OptionalChain(Box::new(current)),
                self.span_from(start),
            );
        }
        Ok(current)
    }

    /// A `.name`, `.#name` or `[expr]` suffix, or `None`.
    fn parse_member_suffix(
        &mut self,
        current: &mut Expr,
        start: Span,
    ) -> Result<Option<Expr>, Error> {
        if !self.is_punctuator(Punctuator::Dot) && !self.is_punctuator(Punctuator::LeftBracket) {
            return Ok(None);
        }
        let member_span = self.current().span;
        let computed = self.is_punctuator(Punctuator::LeftBracket);
        let object = std::mem::replace(current, expr(ExprKind::Null, member_span));
        if computed {
            return self
                .parse_member_access(object, start, member_span, false)
                .map(Some);
        }
        self.advance()?;
        self.parse_member_name(object, start, member_span, false)
            .map(Some)
    }

    /// The access after `?.` (at its name or `[`) or at a `[`.
    fn parse_member_access(
        &mut self,
        object: Expr,
        start: Span,
        member_span: Span,
        optional: bool,
    ) -> Result<Expr, Error> {
        if self.is_punctuator(Punctuator::LeftBracket) {
            self.advance_expression_start()?;
            let key =
                self.parse_recursion(ParserStackFrame::MemberAccess, Self::parse_expression)?;
            self.expect_punctuator(Punctuator::RightBracket)?;
            return Ok(expr(
                ExprKind::Member {
                    obj: Box::new(object),
                    prop: PropKey::Computed(Box::new(key)),
                    optional,
                    pos: pos(member_span),
                },
                self.span_from(start),
            ));
        }
        self.parse_member_name(object, start, member_span, optional)
    }

    fn parse_member_name(
        &mut self,
        object: Expr,
        start: Span,
        member_span: Span,
        optional: bool,
    ) -> Result<Expr, Error> {
        let token = *self.current();
        let prop = match token.kind {
            TokenKind::PrivateIdentifier(identifier) => {
                let name = self.intern_private_identifier(&identifier);
                PropKey::Private(Ref {
                    name,
                    span: token.span,
                    scope: self.current_scope(),
                })
            }
            TokenKind::Identifier(identifier) => {
                let name = self.intern_identifier(&identifier);
                PropKey::Name(self.name_string(name)?)
            }
            TokenKind::Keyword(keyword) => PropKey::Name(JsString::from_static(keyword.as_str())),
            _ => return Err(self.syntax_here("expecting field name")),
        };
        self.advance()?;
        Ok(expr(
            ExprKind::Member {
                obj: Box::new(object),
                prop,
                optional,
                pos: pos(member_span),
            },
            self.span_from(start),
        ))
    }

    /// The contents of an already-consumed call or construct `(`.
    pub(super) fn parse_call_arguments(&mut self) -> Result<Vec<Arg>, Error> {
        self.parse_recursion(
            ParserStackFrame::CallArguments,
            Self::parse_call_arguments_inner,
        )
    }

    fn parse_call_arguments_inner(&mut self) -> Result<Vec<Arg>, Error> {
        let mut args = Vec::new();
        let mut fixed = 0usize;
        let mut spread = false;
        while !self.is_punctuator(Punctuator::RightParen) {
            if !spread && fixed >= MAX_CALL_ARGUMENTS {
                return Err(self.syntax_here("Too many call arguments"));
            }
            if self.is_punctuator(Punctuator::Ellipsis) {
                spread = true;
                self.advance_expression_start()?;
                args.push(Arg::Spread(self.parse_assignment_allow_in()?));
            } else {
                args.push(Arg::Arg(self.parse_assignment_allow_in()?));
                if !spread {
                    fixed += 1;
                }
            }
            if self.is_punctuator(Punctuator::RightParen) {
                break;
            }
            self.expect_punctuator(Punctuator::Comma)?;
        }
        self.expect_punctuator(Punctuator::RightParen)?;
        Ok(args)
    }

    fn parse_new_expression(&mut self) -> Result<Expr, Error> {
        let new_span = self.current().span;
        self.advance()?;
        if self.consume_punctuator(Punctuator::Dot)? {
            let TokenKind::Identifier(identifier) = self.current().kind else {
                return Err(self.syntax_here("expecting target"));
            };
            if !self.is_unescaped_name(&identifier, "target") {
                return Err(self.syntax_here("expecting target"));
            }
            if !self.current_new_target_allowed() {
                return Err(Error::syntax(
                    "new.target only allowed within functions",
                    new_span,
                ));
            }
            self.advance()?;
            return Ok(expr(ExprKind::NewTarget, self.span_from(new_span)));
        }
        let weight = self.enter_recursion(ParserStackFrame::NewWithoutArguments)?;
        let result = self.parse_new_constructor_tail(new_span);
        if result.is_ok() {
            self.leave_recursion(weight);
        }
        result
    }

    fn parse_new_constructor_tail(&mut self, new_span: Span) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut callee = self.parse_primary(false)?;
        loop {
            if self.is_punctuator(Punctuator::OptionalChain) {
                return Err(self.syntax_here("new keyword cannot be used with an optional chain"));
            }
            if let Some(member) = self.parse_member_suffix(&mut callee, start)? {
                callee = member;
                continue;
            }
            if matches!(self.current().kind, TokenKind::Template(_)) {
                callee = self.parse_tagged_template(callee, start)?;
                continue;
            }
            break;
        }
        let construct = self.current().span;
        let args = if self.is_punctuator(Punctuator::LeftParen) {
            self.advance()?;
            self.parse_recursion(
                ParserStackFrame::ConstructArguments,
                Self::parse_call_arguments_inner,
            )?
        } else {
            Vec::new()
        };
        Ok(expr(
            ExprKind::New {
                callee: Box::new(callee),
                args,
                pos: pos(construct),
            },
            self.span_from(new_span),
        ))
    }

    fn parse_super(&mut self) -> Result<Expr, Error> {
        let super_span = self.current().span;
        self.advance()?;
        if self.is_punctuator(Punctuator::LeftParen) {
            if !self.state().super_call_allowed {
                return Err(
                    self.syntax_here("super() is only valid in a derived class constructor")
                );
            }
            let call = self.current().span;
            self.advance()?;
            let args = self.parse_call_arguments()?;
            return Ok(expr(
                ExprKind::SuperCall {
                    args,
                    pos: pos(call),
                },
                self.span_from(super_span),
            ));
        }
        if !matches!(
            self.current().kind,
            TokenKind::Punctuator(Punctuator::Dot | Punctuator::LeftBracket)
        ) {
            return Err(Error::syntax("invalid use of 'super'", self.current().span));
        }
        if !self.state().super_allowed {
            return Err(Error::syntax(
                "'super' is only valid in a method",
                self.current().span,
            ));
        }
        let member = self.current().span;
        let prop = if self.is_punctuator(Punctuator::Dot) {
            self.advance()?;
            let token = *self.current();
            let key = match token.kind {
                TokenKind::PrivateIdentifier(_) => {
                    return Err(Error::syntax(
                        "private class field forbidden after super",
                        token.span,
                    ));
                }
                TokenKind::Identifier(identifier) => {
                    let name = self.intern_identifier(&identifier);
                    self.name_string(name)?
                }
                TokenKind::Keyword(keyword) => JsString::from_static(keyword.as_str()),
                _ => return Err(self.syntax_here("expecting field name")),
            };
            self.advance()?;
            PropKey::Name(key)
        } else {
            self.advance_expression_start()?;
            let key = self.parse_expression()?;
            self.expect_punctuator(Punctuator::RightBracket)?;
            PropKey::Computed(Box::new(key))
        };
        Ok(expr(
            ExprKind::SuperMember {
                prop,
                pos: pos(member),
            },
            self.span_from(super_span),
        ))
    }

    fn parse_import_expression(&mut self, import_call_allowed: bool) -> Result<Expr, Error> {
        let import_span = self.current().span;
        self.advance()?;
        if self.is_punctuator(Punctuator::Dot) {
            self.advance()?;
            if !self.is_contextual_keyword("meta") {
                return Err(self.syntax_here("meta expected"));
            }
            let Some(module) = self.module.as_mut() else {
                return Err(self.syntax_here("import.meta only valid in module code"));
            };
            module.uses_import_meta = true;
            self.advance()?;
            return Ok(expr(ExprKind::ImportMeta, self.span_from(import_span)));
        }
        self.expect_punctuator(Punctuator::LeftParen)?;
        if !import_call_allowed {
            return Err(self.syntax_here("invalid use of 'import()'"));
        }
        let weight = self.enter_recursion(ParserStackFrame::DynamicImport)?;
        let result = (|| {
            let spec = self.parse_assignment_allow_in()?;
            let mut options = None;
            if self.is_punctuator(Punctuator::Comma) {
                self.advance()?;
                if !self.is_punctuator(Punctuator::RightParen) {
                    options = Some(Box::new(self.parse_assignment_allow_in()?));
                    if self.is_punctuator(Punctuator::Comma) {
                        self.advance()?;
                    }
                }
            }
            self.expect_punctuator(Punctuator::RightParen)?;
            Ok(expr(
                ExprKind::Import {
                    spec: Box::new(spec),
                    options,
                    pos: pos(import_span),
                },
                self.span_from(import_span),
            ))
        })();
        if result.is_ok() {
            self.leave_recursion(weight);
        }
        result
    }

    pub(super) fn parse_yield_expression(&mut self) -> Result<Expr, Error> {
        let yield_span = self.current().span;
        if !self.state().is_generator {
            return Err(self.syntax_here("unexpected 'yield' keyword"));
        }
        if !self.state().in_function_body {
            return Err(self.syntax_here("yield in default expression"));
        }
        self.advance()?;
        let line_terminator = self.current().line_terminator_before;
        let delegate = !line_terminator && self.is_punctuator(Punctuator::Multiply);
        let arg = if delegate {
            self.advance_expression_start()?;
            Some(Box::new(self.parse_assignment()?))
        } else if line_terminator
            || matches!(
                self.current().kind,
                TokenKind::Punctuator(
                    Punctuator::Semicolon
                        | Punctuator::RightParen
                        | Punctuator::RightBracket
                        | Punctuator::RightBrace
                        | Punctuator::Comma
                        | Punctuator::Colon
                )
            )
        {
            None
        } else {
            Some(Box::new(self.parse_assignment()?))
        };
        Ok(expr(
            ExprKind::Yield {
                arg,
                delegate,
                pos: pos(yield_span),
            },
            self.span_from(yield_span),
        ))
    }

    pub(super) fn parse_primary(&mut self, import_call_allowed: bool) -> Result<Expr, Error> {
        // A slash asked to begin an operand is a regular expression.
        if matches!(
            self.current().kind,
            TokenKind::Punctuator(Punctuator::Divide | Punctuator::DivideAssign)
        ) {
            self.relex_current_with_goal(LexicalGoal::RegExp)?;
        }
        let token = *self.current();
        let span = token.span;
        let kind = match token.kind {
            TokenKind::Keyword(Keyword::Null) => {
                self.advance()?;
                ExprKind::Null
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance()?;
                ExprKind::Bool(false)
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance()?;
                ExprKind::Bool(true)
            }
            TokenKind::Keyword(Keyword::This) => {
                self.advance()?;
                ExprKind::This
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
                        span,
                    ));
                }
                self.advance()?;
                match parse_number(&number).map_err(|message| Error::syntax(message, span))? {
                    Number::Number(value) => ExprKind::Number(value),
                    Number::BigInt(value) => ExprKind::BigInt(Box::new(value)),
                }
            }
            TokenKind::String(string) => {
                if self.strict() && string.has_legacy_octal_escape {
                    return Err(Error::syntax(
                        "legacy octal escapes are forbidden in strict mode",
                        span,
                    ));
                }
                self.advance()?;
                ExprKind::String(self.decode_string_literal(span)?)
            }
            TokenKind::Punctuator(Punctuator::LeftParen) => {
                self.advance()?;
                let inner =
                    self.parse_recursion(ParserStackFrame::Parenthesized, Self::parse_expression)?;
                self.expect_punctuator(Punctuator::RightParen)?;
                ExprKind::Paren(Box::new(inner))
            }
            TokenKind::Punctuator(Punctuator::LeftBrace) => {
                return self
                    .parse_recursion(ParserStackFrame::ObjectLiteral, Self::parse_object_literal);
            }
            TokenKind::Punctuator(Punctuator::LeftBracket) => {
                return self
                    .parse_recursion(ParserStackFrame::ArrayLiteral, Self::parse_array_literal);
            }
            TokenKind::Template(_) => return self.parse_template_literal(),
            TokenKind::Identifier(ref identifier)
                if self.is_unescaped_name(identifier, "async") && self.async_function_ahead() =>
            {
                return self.parse_function_expression();
            }
            TokenKind::Identifier(identifier) => {
                let name = self.intern_identifier(&identifier);
                if self.names.name(name) == "arguments" && self.state().arguments_forbidden {
                    return Err(Error::syntax(
                        "'arguments' identifier is not allowed in class field initializer",
                        span,
                    ));
                }
                self.validate_identifier(
                    &identifier,
                    span,
                    self.strict(),
                    IdentifierContext::Reference,
                )?;
                self.advance()?;
                ExprKind::Ident(Ref {
                    name,
                    span,
                    scope: self.current_scope(),
                })
            }
            TokenKind::Keyword(Keyword::Function) => return self.parse_function_expression(),
            TokenKind::Keyword(Keyword::Class) => return self.parse_class_expression(),
            TokenKind::Keyword(Keyword::New) => return self.parse_new_expression(),
            TokenKind::Keyword(Keyword::Super) => return self.parse_super(),
            TokenKind::Keyword(Keyword::Import) => {
                return self.parse_import_expression(import_call_allowed);
            }
            TokenKind::Keyword(Keyword::Yield) if self.state().is_generator => {
                return Err(self.syntax_here("unexpected token in expression: 'yield'"));
            }
            TokenKind::Keyword(keyword) => {
                return Err(self.syntax_here(format!(
                    "unexpected token in expression: '{}'",
                    keyword.as_str()
                )));
            }
            TokenKind::RegExp(literal) => {
                let start = span.start.byte_offset + 1;
                let end = start + literal.pattern.len();
                let pattern = self
                    .lexer
                    .source_range_to_js_string(start..end)?
                    .ok_or_else(|| Error::internal("regexp source range is invalid"))?;
                let flags = JsString::try_from_utf8(literal.flags)?;
                // QuickJS compiles the pattern before the next token.
                let program = oxide_regexp::compile(
                    &pattern.utf16_units().collect::<Vec<_>>(),
                    &flags.utf16_units().collect::<Vec<_>>(),
                )
                .map_err(|error| {
                    let kind = match error.kind() {
                        oxide_regexp::CompileErrorKind::Internal => ErrorKind::JsInternal,
                        oxide_regexp::CompileErrorKind::Syntax
                        | oxide_regexp::CompileErrorKind::TooManyCaptures
                        | oxide_regexp::CompileErrorKind::TooManyRegisters => ErrorKind::Syntax,
                    };
                    let message = oxide_regexp::javascript_compile_error_message(&error).to_owned();
                    Error::new(kind, message).with_span(span)
                })?;
                self.advance()?;
                ExprKind::RegExp(Box::new(RegExpLiteral {
                    pattern,
                    flags,
                    program: Rc::new(program),
                }))
            }
            TokenKind::PrivateIdentifier(identifier) => {
                let mut message = NativeErrorMessage::new();
                message.push_utf8("unexpected token in expression: '");
                message.push_utf8(identifier.raw);
                message.push_utf8("'");
                return Err(Error::from_native_message(ErrorKind::Syntax, message).with_span(span));
            }
            TokenKind::Punctuator(punctuator) => {
                return Err(self.syntax_here(format!(
                    "unexpected token in expression: '{}'",
                    punctuator.as_str()
                )));
            }
            TokenKind::RawAscii(0) | TokenKind::Eof => {
                return Err(self.syntax_here("unexpected token in expression: ''"));
            }
            TokenKind::RawAscii(byte) => {
                return Err(self.syntax_here(format!(
                    "unexpected token in expression: '{}'",
                    char::from(byte)
                )));
            }
        };
        Ok(expr(kind, self.span_from(span)))
    }

    fn parse_array_literal(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        if !self.is_punctuator(Punctuator::LeftBracket) {
            return Err(self.syntax_here("expecting '['"));
        }
        self.advance_expression_start()?;
        let mut items = Vec::new();
        while !self.is_punctuator(Punctuator::RightBracket) {
            if self.is_punctuator(Punctuator::Comma) {
                items.push(ArrayItem::Hole);
                self.advance_expression_start()?;
                continue;
            }
            if self.is_punctuator(Punctuator::Ellipsis) {
                let spread = self.current().span;
                self.advance_expression_start()?;
                items.push(ArrayItem::Spread(
                    self.parse_assignment_allow_in()?,
                    pos(spread),
                ));
            } else {
                items.push(ArrayItem::Item(self.parse_assignment_allow_in()?));
            }
            if self.is_punctuator(Punctuator::Comma) {
                // A comma before `]` is trailing, not a hole.
                self.advance_expression_start()?;
            } else if !self.is_punctuator(Punctuator::RightBracket) {
                self.expect_punctuator(Punctuator::RightBracket)?;
            }
        }
        self.expect_punctuator(Punctuator::RightBracket)?;
        Ok(expr(ExprKind::Array(items), self.span_from(start)))
    }

    fn parse_template_literal(&mut self) -> Result<Expr, Error> {
        let start = self.current().span;
        let mut quasis = Vec::new();
        let mut exprs = Vec::new();
        loop {
            let token = *self.current();
            let TokenKind::Template(part) = token.kind else {
                return Err(Error::internal(
                    "template parser lost its continuation token",
                ));
            };
            let Some(cooked) = self.decode_template_cooked(&part, token.span)? else {
                let initial = super::support::template_part_is_initial(part.kind);
                let invalid = self
                    .lexer
                    .template_escape_error(token.span.start, initial)
                    .map_err(super::support::lex_error)?
                    .map(|error| error.span);
                return Err(Error::syntax(
                    "malformed escape sequence in string literal",
                    invalid.unwrap_or(token.span),
                ));
            };
            quasis.push(cooked);
            use crate::engine::compiler::lexer::TemplatePartKind as Part;
            if matches!(part.kind, Part::NoSubstitution | Part::Tail) {
                self.advance()?;
                return Ok(expr(
                    ExprKind::Template { quasis, exprs },
                    self.span_from(start),
                ));
            }
            self.advance()?;
            exprs.push(self.parse_recursion(ParserStackFrame::Template, Self::parse_expression)?);
            self.expect_template_substitution_end()?;
        }
    }

    fn parse_tagged_template(&mut self, tag: Expr, start: Span) -> Result<Expr, Error> {
        let first = self.current().span;
        let mut cooked = Vec::new();
        let mut raw = Vec::new();
        let mut exprs = Vec::new();
        loop {
            let token = *self.current();
            let TokenKind::Template(part) = token.kind else {
                return Err(Error::internal(
                    "tagged template parser lost its continuation token",
                ));
            };
            cooked.push(self.decode_template_cooked(&part, token.span)?);
            raw.push(self.decode_template_raw_value(&part, token.span)?);
            use crate::engine::compiler::lexer::TemplatePartKind as Part;
            if matches!(part.kind, Part::NoSubstitution | Part::Tail) {
                self.advance()?;
                return Ok(expr(
                    ExprKind::TaggedTemplate {
                        tag: Box::new(tag),
                        cooked,
                        raw,
                        exprs,
                        pos: pos(first),
                    },
                    self.span_from(start),
                ));
            }
            self.advance()?;
            exprs.push(self.parse_recursion(ParserStackFrame::Template, Self::parse_expression)?);
            self.expect_template_substitution_end()?;
        }
    }

    fn expect_template_substitution_end(&mut self) -> Result<(), Error> {
        if !self.is_punctuator(Punctuator::RightBrace) {
            return Err(self.syntax_here("expected '}' after template expression"));
        }
        self.advance_with_goal(LexicalGoal::TemplateContinuation)
    }
}

/// A callee written `eval` (through parentheses): a direct eval.
fn is_eval_reference(callee: &Expr, names: &crate::engine::compiler::names::NameTable) -> bool {
    match &callee.kind {
        ExprKind::Ident(reference) => names.name(reference.name) == "eval",
        ExprKind::Paren(inner) => is_eval_reference(inner, names),
        _ => false,
    }
}
