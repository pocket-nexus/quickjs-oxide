//! Expressions, as QuickJS evaluates them.

use super::{Access, Builder, Result, internal, unsupported};
use crate::engine::hir::schema::{
    Arg, ArrayItem, AssignOp, BinOp, Const, DeclKind, DeclRef, Expr, ExprKind, LogicalOp,
    MethodRole, ObjProp, PropKey, Target, TargetKind, UnaryOp, UpdateOp,
};
use crate::engine::mir::ir::{BlockId, CmpOp, Constant, Imm, SuspendKind, Terminator, Value};
use crate::engine::mir::op::Op;
use oxide_core::bytecode::opcode::method;

/// A property reference whose base (and key) are evaluated.
#[derive(Clone, Copy)]
pub(super) enum Place {
    Field {
        obj: Value,
        key: u32,
        at: u32,
    },
    /// The key as evaluated: QuickJS converts it when it is used.
    Elem {
        obj: Value,
        key: Value,
        at: u32,
    },
    Private {
        obj: Value,
        decl: DeclRef,
        at: u32,
    },
    /// `super[key]` with receiver `this` and the home object's prototype.
    Super {
        this: Value,
        base: Value,
        key: Value,
        at: u32,
    },
}

/// An optional chain's short circuit: its block, ended, and its value.
pub(super) type Short = (Option<BlockId>, Value);

/// The fixed and spread arguments of a call.
pub(super) enum Args {
    Fixed(Vec<Value>),
    /// An array of the arguments.
    Spread(Value),
}

impl Builder<'_, '_> {
    pub(super) fn expr(&mut self, e: &Expr) -> Result<Value> {
        let at = e.span.start;
        Ok(match &e.kind {
            ExprKind::Const { value } => self.constant_expr(value.0)?,
            ExprKind::Str { value } => self.string(value.0),
            ExprKind::Bool { value: true } => self.op(Op::True, &[], Imm::None),
            ExprKind::Bool { value: false } => self.op(Op::False, &[], Imm::None),
            ExprKind::Null => self.op(Op::Null, &[], Imm::None),
            ExprKind::Undefined => self.undefined(),
            ExprKind::Template { quasis, exprs } => self.template(quasis, exprs)?,
            ExprKind::TaggedTemplate {
                tag,
                cooked,
                raw,
                exprs,
                ..
            } => self.tagged_template(tag, cooked, raw, exprs, at)?,
            ExprKind::Array { items } => self.array(items)?,
            ExprKind::Object { props } => self.object(props)?,
            ExprKind::Fn { fn_ } => self.closure(*fn_),
            ExprKind::Class { cls } => self.class(cls, None)?,
            ExprKind::Decl { decl } => self.read_decl(*decl, Some(at))?,
            ExprKind::This { decl } => self.read_this(*decl)?,
            ExprKind::NewTarget { decl } => self.read_decl(*decl, None)?,
            ExprKind::ImportMeta { decl } => self.read_decl(*decl, Some(at))?,
            ExprKind::SuperMember {
                prop,
                this,
                home,
                super_at,
            } => {
                let place = self.super_place(prop, *this, *home, at, *super_at)?;
                self.get_place(place)?
            }
            ExprKind::Member { .. } => {
                let (_, value) = self.member(e, &mut Vec::new())?;
                value
            }
            ExprKind::Unary { op, e: operand } => self.unary(*op, operand, at)?,
            ExprKind::Update { op, prefix, target } => self.update(*op, *prefix, target, at)?,
            ExprKind::Binary { op, l, r, .. } => {
                let l = self.expr(l)?;
                let r = self.expr(r)?;
                self.binary(*op, l, r, at)
            }
            ExprKind::Logical { op, l, r } => self.logical(*op, l, r)?,
            ExprKind::Cond { test, then, else_ } => {
                let cond = self.expr(test)?;
                let then_block = self.block();
                let else_block = self.block();
                self.end(Terminator::Branch {
                    cond,
                    then: then_block,
                    else_: else_block,
                });
                self.start(then_block);
                let a = self.expr(then)?;
                let from_then = self.branch_end();
                self.start(else_block);
                let b = self.expr(else_)?;
                let from_else = self.branch_end();
                self.join(vec![(from_then, a), (from_else, b)])
            }
            ExprKind::Assign { op, target, value } => self.assign(*op, target, value, at)?,
            ExprKind::Seq { exprs } => {
                let mut last = None;
                for e in exprs {
                    last = Some(self.expr(e)?);
                }
                match last {
                    Some(value) => value,
                    None => return internal("an empty sequence"),
                }
            }
            ExprKind::Call { .. } | ExprKind::New { .. } => self.call_expr(e, &mut Vec::new())?,
            ExprKind::SuperCall {
                args,
                this,
                new_target,
                func,
                super_at,
            } => self.super_call(args, *this, *new_target, *func, at, *super_at)?,
            ExprKind::Import { spec, options } => {
                let spec = self.expr(spec)?;
                let options = match options {
                    Some(options) => self.expr(options)?,
                    None => self.undefined(),
                };
                self.op_at(Op::ImportCall, &[spec, options], Imm::None, at)
            }
            ExprKind::Yield { e: value, delegate } => {
                let value = match value {
                    Some(value) => self.expr(value)?,
                    None => self.undefined(),
                };
                if *delegate {
                    self.yield_star(value, at)?
                } else {
                    self.yield_value(value, at)?
                }
            }
            ExprKind::Await { e: value } => {
                let value = self.expr(value)?;
                self.await_value(value, None)
            }
            ExprKind::Decode { .. } | ExprKind::Scratch { .. } => {
                return unsupported("typed expressions");
            }
            ExprKind::OptionalChain { e: inner } => {
                let mut shorts = Vec::new();
                let value = self.chain_expr(inner, &mut shorts)?;
                self.end_chain(value, shorts)
            }
            ExprKind::PrivateIn { name, obj } => {
                let obj = self.expr(obj)?;
                self.private_in(*name, obj, at)?
            }
            ExprKind::Named { name, e: inner } => self.named(name.0, inner)?,
        })
    }

    fn constant_expr(&mut self, index: u32) -> Result<Value> {
        match &self.cx.hir.consts[index as usize] {
            Const::Number { value } => Ok(self.number(*value)),
            Const::Bigint { digits } => {
                let constant = self.constant(Constant::BigInt(digits.0));
                Ok(self.op(Op::Const, &[], Imm::Const(constant)))
            }
            Const::Regexp { source, .. } => {
                let Some(program) = self.cx.regexps.get(&index) else {
                    return internal("a regular expression without its program");
                };
                let flags = program.flags().canonical_string();
                let flags = self.cx.intern(
                    crate::engine::value::JsString::try_from_utf8(&flags)
                        .map_err(|error| super::BuildError::Internal(error.to_string()))?,
                );
                let constant = self.constant(Constant::RegExp {
                    source: source.0,
                    flags,
                });
                Ok(self.op(Op::NewRegExp, &[], Imm::Const(constant)))
            }
        }
    }

    fn read_this(&mut self, decl: DeclRef) -> Result<Value> {
        self.read_decl(decl, None)
    }

    /// NamedEvaluation: an anonymous function takes the name in its unit, an
    /// anonymous class through `SetFunctionName`.
    fn named(&mut self, name: u32, inner: &Expr) -> Result<Value> {
        match &inner.kind {
            ExprKind::Fn { fn_ } => {
                let function = self.cx.function(*fn_);
                if function.name.is_none() {
                    self.cx.inferred[fn_.index()] = Some(name);
                }
                Ok(self.closure(*fn_))
            }
            ExprKind::Class { cls } => self.class(cls, Some(name)),
            _ => self.expr(inner),
        }
    }

    // ---- operators ----

    pub(super) fn binary(&mut self, op: BinOp, l: Value, r: Value, at: u32) -> Value {
        let op = match op {
            BinOp::Add => Op::Add,
            BinOp::Sub => Op::Sub,
            BinOp::Mul => Op::Mul,
            BinOp::Div => Op::Div,
            BinOp::Rem => Op::Mod,
            BinOp::Exp => Op::Pow,
            BinOp::Shl => Op::Shl,
            BinOp::Sar => Op::Sar,
            BinOp::Shr => Op::Shr,
            BinOp::BitAnd => Op::BitAnd,
            BinOp::BitOr => Op::BitOr,
            BinOp::BitXor => Op::BitXor,
            BinOp::Eq => Op::Eq,
            BinOp::Ne => Op::Neq,
            BinOp::StrictEq => Op::StrictEq,
            BinOp::StrictNe => Op::StrictNeq,
            BinOp::Lt => Op::Lt,
            BinOp::Le => Op::Lte,
            BinOp::Gt => Op::Gt,
            BinOp::Ge => Op::Gte,
            BinOp::In => Op::In,
            BinOp::InstanceOf => Op::InstanceOf,
        };
        self.op_at(op, &[l, r], Imm::None, at)
    }

    fn unary(&mut self, op: UnaryOp, operand: &Expr, at: u32) -> Result<Value> {
        Ok(match op {
            UnaryOp::Neg => {
                let value = self.expr(operand)?;
                self.op_at(Op::Neg, &[value], Imm::None, at)
            }
            UnaryOp::Plus => {
                let value = self.expr(operand)?;
                self.op_at(Op::Plus, &[value], Imm::None, at)
            }
            UnaryOp::BitNot => {
                let value = self.expr(operand)?;
                self.op_at(Op::BitNot, &[value], Imm::None, at)
            }
            UnaryOp::Not => {
                let value = self.expr(operand)?;
                self.op(Op::Not, &[value], Imm::None)
            }
            UnaryOp::Void => {
                self.expr(operand)?;
                self.undefined()
            }
            UnaryOp::TypeOf => {
                let value = match operand.kind {
                    // A name that may be undeclared reads as `undefined`.
                    ExprKind::Decl { decl } if self.is_unresolved_global(decl)? => {
                        let name = self.decl(decl).name.0;
                        self.op_at(
                            Op::GetGlobalOrUndefined,
                            &[],
                            Imm::Key(name),
                            operand.span.start,
                        )
                    }
                    _ => self.expr(operand)?,
                };
                self.op(Op::TypeOf, &[value], Imm::None)
            }
            UnaryOp::Delete => self.delete(operand)?,
        })
    }

    /// Whether reading `decl` goes to the global object by name.
    fn is_unresolved_global(&mut self, decl: DeclRef) -> Result<bool> {
        Ok(matches!(self.access(decl)?, Access::Global { .. }))
    }

    fn delete(&mut self, operand: &Expr) -> Result<Value> {
        match &operand.kind {
            ExprKind::Member {
                obj,
                prop,
                optional: false,
                ..
            } => {
                let at = operand.span.start;
                let obj = self.expr(obj)?;
                let key = match &**prop {
                    PropKey::Name { name } => {
                        let index = self.string_const(name.0);
                        self.op_at(Op::Const, &[], Imm::Const(index), at)
                    }
                    PropKey::Computed { e } => {
                        let key = self.expr(e)?;
                        self.mark(Some(at));
                        key
                    }
                    PropKey::Private { .. } => return internal("delete of a private name"),
                };
                Ok(self.op(Op::DeleteElem, &[obj, key], Imm::None))
            }
            ExprKind::OptionalChain { e: inner }
                if matches!(inner.kind, ExprKind::Member { .. }) =>
            {
                // The short circuits make `delete` true.
                let ExprKind::Member {
                    obj,
                    prop,
                    optional,
                    ..
                } = &inner.kind
                else {
                    unreachable!("a member");
                };
                let at = inner.span.start;
                let mut shorts = Vec::new();
                let saved = std::mem::replace(&mut self.chain_delete, true);
                let obj = self.chain_object(obj, *optional, &mut shorts);
                self.chain_delete = saved;
                let obj = obj?;
                let key = match &**prop {
                    PropKey::Name { name } => {
                        let index = self.string_const(name.0);
                        self.op_at(Op::Const, &[], Imm::Const(index), at)
                    }
                    PropKey::Computed { e } => {
                        let key = self.expr(e)?;
                        self.mark(Some(at));
                        key
                    }
                    PropKey::Private { .. } => return unsupported("private names"),
                };
                let deleted = self.op(Op::DeleteElem, &[obj, key], Imm::None);
                let mut incoming = vec![(self.branch_end(), deleted)];
                incoming.extend(shorts);
                Ok(self.join(incoming))
            }
            ExprKind::Decl { decl } => match self.access(*decl)? {
                Access::Global { name } => {
                    Ok(self.op_at(Op::DeleteGlobal, &[], Imm::Key(name.0), operand.span.start))
                }
                // A declared binding is not deletable.
                _ => {
                    let _ = self.read_decl(*decl, Some(operand.span.start))?;
                    Ok(self.op(Op::False, &[], Imm::None))
                }
            },
            ExprKind::SuperMember {
                prop,
                this,
                home,
                super_at,
            } => {
                // QuickJS evaluates the reference, then throws.
                let at = operand.span.start;
                self.super_place(prop, *this, *home, at, *super_at)?;
                let message = self.static_key("unsupported reference to 'super'");
                self.end_at(
                    Terminator::ThrowError {
                        kind: crate::engine::mir::ir::ThrowKind::Reference,
                        key: message,
                    },
                    at,
                );
                Ok(self.undefined())
            }
            _ => {
                self.expr(operand)?;
                Ok(self.op(Op::True, &[], Imm::None))
            }
        }
    }

    fn logical(&mut self, op: LogicalOp, l: &Expr, r: &Expr) -> Result<Value> {
        let left = self.expr(l)?;
        let rhs = self.block();
        let short = self.block();
        let cond = match op {
            LogicalOp::And | LogicalOp::Or => left,
            LogicalOp::Nullish => self.op(Op::IsUndefinedOrNull, &[left], Imm::None),
        };
        let (then, else_) = match op {
            LogicalOp::And => (rhs, short),
            LogicalOp::Or => (short, rhs),
            LogicalOp::Nullish => (rhs, short),
        };
        self.end(Terminator::Branch { cond, then, else_ });
        self.start(rhs);
        let right = self.expr(r)?;
        let from_rhs = self.branch_end();
        self.start(short);
        let from_short = self.branch_end();
        Ok(self.join(vec![(from_rhs, right), (from_short, left)]))
    }

    // ---- members ----

    /// A property's base and key, for a read, call or write.
    fn place(&mut self, obj: Value, prop: &PropKey, at: u32) -> Result<Place> {
        Ok(match prop {
            PropKey::Name { name } => Place::Field {
                obj,
                key: name.0,
                at,
            },
            PropKey::Computed { e } => {
                let key = self.expr(e)?;
                Place::Elem { obj, key, at }
            }
            PropKey::Private { name } => Place::Private {
                obj,
                decl: *name,
                at,
            },
        })
    }

    /// `super[key]`'s place: QuickJS reads `this` and the home object
    /// before a computed key.
    pub(super) fn super_place(
        &mut self,
        prop: &PropKey,
        this: DeclRef,
        home: DeclRef,
        at: u32,
        super_at: u32,
    ) -> Result<Place> {
        let this = self.read_decl(this, Some(super_at))?;
        let home = self.read_decl(home, Some(super_at))?;
        let base = self.op(Op::GetSuper, &[home], Imm::None);
        let key = match prop {
            PropKey::Name { name } => self.string(name.0),
            PropKey::Computed { e } => self.expr(e)?,
            PropKey::Private { .. } => return internal("a private name after super"),
        };
        Ok(Place::Super {
            this,
            base,
            key,
            at,
        })
    }

    pub(super) fn get_place(&mut self, place: Place) -> Result<Value> {
        Ok(match place {
            Place::Field { obj, key, at } => self.op_at(Op::GetField, &[obj], Imm::Key(key), at),
            Place::Elem { obj, key, at } => self.op_at(Op::GetElem, &[obj, key], Imm::None, at),
            Place::Private { obj, decl, at } => self.get_private(obj, decl, at)?,
            Place::Super {
                this,
                base,
                key,
                at,
            } => self.op_at(Op::GetSuperValue, &[this, base, key], Imm::Int(0), at),
        })
    }

    pub(super) fn set_place(&mut self, place: Place, value: Value) -> Result<()> {
        match place {
            Place::Field { obj, key, .. } => {
                if self.this_values.contains(&obj) {
                    self.this_keys.insert(key);
                }
                self.effect(Op::SetField, &[obj, value], Imm::Key(key));
            }
            Place::Elem { obj, key, .. } => self.effect(Op::SetElem, &[obj, key, value], Imm::None),
            Place::Private { obj, decl, at } => self.put_private(obj, decl, value, at)?,
            Place::Super {
                this, base, key, ..
            } => {
                self.effect(Op::PutSuperValue, &[this, base, key, value], Imm::None);
            }
        }
        Ok(())
    }

    /// A member expression, maybe inside an optional chain: its object and
    /// value.
    fn member(&mut self, e: &Expr, shorts: &mut Vec<Short>) -> Result<(Value, Value)> {
        let ExprKind::Member {
            obj,
            prop,
            optional,
            ..
        } = &e.kind
        else {
            return internal("not a member");
        };
        let obj = self.chain_object(obj, *optional, shorts)?;
        let place = self.place(obj, prop, e.span.start)?;
        Ok((obj, self.get_place(place)?))
    }

    /// The object of a member in a chain, tested when `optional`.
    fn chain_object(
        &mut self,
        obj: &Expr,
        optional: bool,
        shorts: &mut Vec<Short>,
    ) -> Result<Value> {
        let obj = self.chain_expr(obj, shorts)?;
        if optional {
            self.nullish_test(obj, shorts);
        }
        Ok(obj)
    }

    /// QuickJS's optional-chain test: a nullish value short-circuits the
    /// chain to `undefined`.
    fn nullish_test(&mut self, value: Value, shorts: &mut Vec<Short>) {
        let nullish = self.op(Op::IsUndefinedOrNull, &[value], Imm::None);
        let short = self.block();
        let cont = self.block();
        self.end(Terminator::Branch {
            cond: nullish,
            then: short,
            else_: cont,
        });
        // The short circuit's value comes here, as QuickJS emits it: in a
        // `delete`, `true`.
        self.start(short);
        let fallback = if self.chain_delete {
            self.op(Op::True, &[], Imm::None)
        } else {
            self.undefined()
        };
        shorts.push((self.branch_end(), fallback));
        self.start(cont);
    }

    /// An expression inside an optional chain: members and calls continue
    /// the chain.
    fn chain_expr(&mut self, e: &Expr, shorts: &mut Vec<Short>) -> Result<Value> {
        match &e.kind {
            ExprKind::Member { .. } => Ok(self.member(e, shorts)?.1),
            ExprKind::Call { .. } => self.call_expr(e, shorts),
            _ => self.expr(e),
        }
    }

    /// Ends a chain: its value, or `undefined` from each short circuit.
    fn end_chain(&mut self, value: Value, shorts: Vec<Short>) -> Value {
        let mut incoming = vec![(self.branch_end(), value)];
        incoming.extend(shorts);
        self.join(incoming)
    }

    // ---- calls ----

    fn call_expr(&mut self, e: &Expr, shorts: &mut Vec<Short>) -> Result<Value> {
        let at = e.span.start;
        match &e.kind {
            ExprKind::New { callee, args, .. } => {
                let callee = self.expr(callee)?;
                let args = self.arguments(args)?;
                Ok(match args {
                    Args::Fixed(args) => {
                        let mut operands = vec![callee, callee];
                        operands.extend(args);
                        self.op_at(Op::New, &operands, Imm::None, at)
                    }
                    Args::Spread(array) => {
                        self.op_at(Op::NewSpread, &[callee, callee, array], Imm::None, at)
                    }
                })
            }
            ExprKind::Call {
                callee,
                args,
                optional,
                direct_eval,
                ..
            } => {
                // The callee, and its receiver when it is a member.
                let (receiver, function) = match &callee.kind {
                    ExprKind::Member { .. } => {
                        let (obj, function) = self.member(callee, shorts)?;
                        (Some(obj), function)
                    }
                    ExprKind::SuperMember {
                        prop,
                        this,
                        home,
                        super_at,
                    } => {
                        let Place::Super {
                            this,
                            base,
                            key,
                            at,
                        } = self.super_place(prop, *this, *home, callee.span.start, *super_at)?
                        else {
                            unreachable!("a super place");
                        };
                        let function =
                            self.op_at(Op::GetSuperValue, &[this, base, key], Imm::Int(1), at);
                        (Some(this), function)
                    }
                    ExprKind::OptionalChain { e: inner }
                        if matches!(inner.kind, ExprKind::Member { .. }) =>
                    {
                        // `(a?.b)()` keeps the receiver; a short circuit
                        // calls `undefined` with an `undefined` receiver.
                        let mut inner_shorts = Vec::new();
                        let (obj, function) = self.member(inner, &mut inner_shorts)?;
                        let mut objs = vec![(self.cur, obj)];
                        let mut functions = vec![(self.branch_end(), function)];
                        objs.extend(inner_shorts.iter().copied());
                        functions.extend(inner_shorts);
                        let function = self.join(functions);
                        let obj = self.second_phi(objs);
                        (Some(obj), function)
                    }
                    _ => (None, self.chain_expr(callee, shorts)?),
                };
                if *optional {
                    self.nullish_test(function, shorts);
                }
                let args = self.arguments(args)?;
                Ok(match (receiver, args) {
                    (Some(receiver), Args::Fixed(args)) => {
                        let mut operands = vec![receiver, function];
                        operands.extend(args);
                        self.op_at(Op::CallMethod, &operands, Imm::None, at)
                    }
                    (None, Args::Fixed(args)) => {
                        let mut operands = vec![function];
                        operands.extend(args);
                        let op = if *direct_eval && !*optional {
                            Op::CallEval
                        } else {
                            Op::Call
                        };
                        self.op_at(op, &operands, Imm::None, at)
                    }
                    (receiver, Args::Spread(array)) => {
                        let receiver = match receiver {
                            Some(receiver) => receiver,
                            None => self.undefined(),
                        };
                        self.op_at(Op::CallSpread, &[function, receiver, array], Imm::None, at)
                    }
                })
            }
            _ => internal("not a call"),
        }
    }

    /// The second phi of a join just made by [`Self::join`] over the same
    /// predecessors.
    fn second_phi(&mut self, incoming: Vec<(Option<BlockId>, Value)>) -> Value {
        let incoming: Vec<Value> = incoming
            .into_iter()
            .filter_map(|(block, value)| block.map(|_| value))
            .collect();
        if incoming.len() == 1 {
            return incoming[0];
        }
        let cur = self.cur.expect("the join");
        let result = self.f.new_value(Default::default());
        self.f
            .block_mut(cur)
            .phis
            .push(crate::engine::mir::ir::Phi {
                result,
                args: incoming,
            });
        result
    }

    /// Evaluates call arguments: fixed ones as values; from the first
    /// spread on, QuickJS builds an array.
    pub(super) fn arguments(&mut self, args: &[Arg]) -> Result<Args> {
        let first_spread = args
            .iter()
            .position(|arg| matches!(arg, Arg::Spread { .. }));
        let Some(first_spread) = first_spread else {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                let Arg::Arg { e } = arg else {
                    unreachable!("no spread")
                };
                values.push(self.expr(e)?);
            }
            return Ok(Args::Fixed(values));
        };
        let mut prefix = Vec::with_capacity(first_spread);
        for arg in &args[..first_spread] {
            let Arg::Arg { e } = arg else {
                unreachable!("before the spread")
            };
            prefix.push(self.expr(e)?);
        }
        let array = self.op(Op::NewArray, &prefix, Imm::None);
        let mut index = self.int(first_spread as i32);
        for arg in &args[first_spread..] {
            match arg {
                Arg::Spread { e } => {
                    let source = self.expr(e)?;
                    index = self.op(Op::AppendSpread, &[array, index, source], Imm::None);
                }
                Arg::Arg { e } => {
                    let value = self.expr(e)?;
                    self.effect(Op::DefineElem, &[array, index, value], Imm::None);
                    index = self.op(Op::Inc, &[index], Imm::None);
                }
            }
        }
        Ok(Args::Spread(array))
    }

    // ---- literals ----

    fn template(
        &mut self,
        quasis: &[crate::engine::hir::schema::StringRef],
        exprs: &[Expr],
    ) -> Result<Value> {
        let first = self.string(quasis[0].0);
        if exprs.is_empty() {
            return Ok(first);
        }
        let concat = self.static_key("concat");
        let function = self.op(Op::GetField, &[first], Imm::Key(concat));
        let mut args = Vec::new();
        for (index, e) in exprs.iter().enumerate() {
            args.push(self.expr(e)?);
            if let Some(quasi) = quasis.get(index + 1)
                && !self.cx.strings[quasi.index()].is_empty()
            {
                args.push(self.string(quasi.0));
            }
        }
        let mut operands = vec![first, function];
        operands.extend(args);
        Ok(self.op(Op::CallMethod, &operands, Imm::None))
    }

    fn tagged_template(
        &mut self,
        tag: &Expr,
        cooked: &[Option<crate::engine::hir::schema::StringRef>],
        raw: &[crate::engine::hir::schema::StringRef],
        exprs: &[Expr],
        at: u32,
    ) -> Result<Value> {
        let (receiver, function) = match &tag.kind {
            ExprKind::Member { .. } => {
                let (obj, function) = self.member(tag, &mut Vec::new())?;
                (Some(obj), function)
            }
            _ => (None, self.expr(tag)?),
        };
        let constant = self.constant(Constant::Template {
            cooked: cooked.iter().map(|part| part.map(|part| part.0)).collect(),
            raw: raw.iter().map(|part| part.0).collect(),
        });
        let site = self.op(Op::Const, &[], Imm::Const(constant));
        let mut args = vec![site];
        for e in exprs {
            args.push(self.expr(e)?);
        }
        Ok(match receiver {
            Some(receiver) => {
                let mut operands = vec![receiver, function];
                operands.extend(args);
                self.op_at(Op::CallMethod, &operands, Imm::None, at)
            }
            None => {
                let mut operands = vec![function];
                operands.extend(args);
                self.op_at(Op::Call, &operands, Imm::None, at)
            }
        })
    }

    fn array(&mut self, items: &[ArrayItem]) -> Result<Value> {
        const DENSE_PREFIX_LIMIT: usize = 32;
        let mut index = 0usize;
        let mut prefix = Vec::new();
        while index < items.len() && index < DENSE_PREFIX_LIMIT {
            let ArrayItem::Item { e } = &items[index] else {
                break;
            };
            prefix.push(self.expr(e)?);
            index += 1;
        }
        let array = self.op(Op::NewArray, &prefix, Imm::None);
        let mut need_length = false;
        while index < items.len() {
            match &items[index] {
                ArrayItem::Spread { .. } => break,
                ArrayItem::Hole => need_length = true,
                ArrayItem::Item { e } => {
                    let value = self.expr(e)?;
                    let key = self.cx.intern(
                        crate::engine::value::JsString::try_from_utf8(&index.to_string())
                            .map_err(|error| super::BuildError::Internal(error.to_string()))?,
                    );
                    self.effect(Op::DefineField, &[array, value], Imm::Key(key));
                    need_length = false;
                }
            }
            index += 1;
        }
        if index == items.len() {
            if need_length {
                let length = self.static_key("length");
                let value = self.int(index as i32);
                self.effect(Op::SetField, &[array, value], Imm::Key(length));
            }
            return Ok(array);
        }
        let mut position = self.int(index as i32);
        while index < items.len() {
            match &items[index] {
                ArrayItem::Spread { e } => {
                    let source = self.expr(e)?;
                    position = self.op(Op::AppendSpread, &[array, position, source], Imm::None);
                    need_length = false;
                }
                ArrayItem::Hole => {
                    need_length = true;
                    position = self.op(Op::Inc, &[position], Imm::None);
                }
                ArrayItem::Item { e } => {
                    let value = self.expr(e)?;
                    self.effect(Op::DefineElem, &[array, position, value], Imm::None);
                    need_length = false;
                    position = self.op(Op::Inc, &[position], Imm::None);
                }
            }
            index += 1;
        }
        if need_length {
            let length = self.static_key("length");
            self.effect(Op::SetField, &[array, position], Imm::Key(length));
        }
        Ok(array)
    }

    fn object(&mut self, props: &[ObjProp]) -> Result<Value> {
        let slots = props
            .iter()
            .filter(|prop| {
                matches!(prop, ObjProp::Init { key, .. } if matches!(**key, PropKey::Name { .. }))
            })
            .count() as u32;
        let object = self.op(Op::NewObject, &[], Imm::Flags(slots));
        for prop in props {
            match prop {
                ObjProp::Spread { e } => {
                    let source = self.expr(e)?;
                    self.effect_at(Op::CopyData, &[object, source], Imm::None, e.span.start);
                }
                ObjProp::Proto { value } => {
                    let value = self.expr(value)?;
                    self.effect(Op::SetProto, &[object, value], Imm::None);
                }
                ObjProp::Init { key, value, .. } => match &**key {
                    PropKey::Name { name } => {
                        let value = self.expr(value)?;
                        self.effect(Op::DefineField, &[object, value], Imm::Key(name.0));
                    }
                    PropKey::Computed { e } => {
                        let key = self.expr(e)?;
                        let key = self.op(Op::ToPropertyKey, &[key], Imm::None);
                        let anonymous = self.anonymous_function(value);
                        let value = self.expr(value)?;
                        if anonymous {
                            self.effect(Op::SetFunctionName, &[value, key], Imm::None);
                        }
                        self.effect(Op::DefineElem, &[object, key, value], Imm::None);
                    }
                    PropKey::Private { .. } => return internal("a private key in a literal"),
                },
                ObjProp::Method { key, fn_, role } => {
                    let flags = method::ENUMERABLE
                        | match role {
                            MethodRole::Method => 0,
                            MethodRole::Get => method::GETTER,
                            MethodRole::Set => method::SETTER,
                        };
                    match &**key {
                        PropKey::Name { name } => {
                            if *role == MethodRole::Method {
                                self.cx.inferred[fn_.index()] = Some(name.0);
                            }
                            let closure = self.closure(*fn_);
                            self.effect(
                                Op::DefineMethod,
                                &[object, closure],
                                Imm::KeyFlags(name.0, flags),
                            );
                        }
                        PropKey::Computed { e } => {
                            let key = self.expr(e)?;
                            let key = self.op(Op::ToPropertyKey, &[key], Imm::None);
                            let closure = self.closure(*fn_);
                            self.effect(
                                Op::DefineMethodComputed,
                                &[object, key, closure],
                                Imm::Flags(flags),
                            );
                        }
                        PropKey::Private { .. } => return internal("a private key in a literal"),
                    }
                }
            }
        }
        Ok(object)
    }

    /// Whether `e` is an anonymous function or class definition, which a
    /// computed key names at run time.
    fn anonymous_function(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Fn { fn_ } => self.cx.function(*fn_).name.is_none(),
            ExprKind::Class { cls } => cls.name.is_none(),
            _ => false,
        }
    }

    // ---- assignment ----

    fn assign(&mut self, op: AssignOp, target: &Target, value: &Expr, at: u32) -> Result<Value> {
        let target_at = target.span.start;
        match op {
            AssignOp::Assign => match &target.kind {
                TargetKind::Decl { decl } => {
                    // QuickJS gives `=` no position: an unmarked right side
                    // takes the target's, and the store the last one.
                    self.seed(target_at);
                    let value = self.expr(value)?;
                    self.put_decl(*decl, value)?;
                    Ok(value)
                }
                TargetKind::Member { obj, prop, .. } => {
                    let obj = self.expr(obj)?;
                    let place = self.place(obj, prop, target_at)?;
                    self.seed(target_at);
                    let value = self.expr(value)?;
                    self.set_place(place, value)?;
                    Ok(value)
                }
                TargetKind::Pattern { p } => {
                    self.destructure_after(p, value, super::pattern::Mode::Assign)
                }
                TargetKind::SuperMember {
                    prop,
                    this,
                    home,
                    super_at,
                } => {
                    let place = self.super_place(prop, *this, *home, target_at, *super_at)?;
                    self.seed(target_at);
                    let value = self.expr(value)?;
                    self.set_place(place, value)?;
                    Ok(value)
                }
            },
            AssignOp::And | AssignOp::Or | AssignOp::Nullish => {
                self.logical_assign(op, target, value)
            }
            _ => {
                let binary = compound_op(op);
                match &target.kind {
                    TargetKind::Decl { decl } => {
                        let old = self.read_decl(*decl, Some(target_at))?;
                        let rhs = self.expr(value)?;
                        let new = self.binary(binary, old, rhs, at);
                        self.put_decl(*decl, new)?;
                        Ok(new)
                    }
                    TargetKind::Member { obj, prop, .. } => {
                        let obj = self.expr(obj)?;
                        let place = self.place(obj, prop, target_at)?;
                        let place = self.convert_key(place);
                        let old = self.get_place(place)?;
                        let rhs = self.expr(value)?;
                        let new = self.binary(binary, old, rhs, at);
                        self.set_place(place, new)?;
                        Ok(new)
                    }
                    TargetKind::SuperMember {
                        prop,
                        this,
                        home,
                        super_at,
                    } => {
                        let place = self.super_place(prop, *this, *home, target_at, *super_at)?;
                        let place = self.convert_key(place);
                        let old = self.get_place(place)?;
                        let rhs = self.expr(value)?;
                        let new = self.binary(binary, old, rhs, at);
                        self.set_place(place, new)?;
                        Ok(new)
                    }
                    TargetKind::Pattern { .. } => internal("compound assignment to a pattern"),
                }
            }
        }
    }

    /// The key of a computed place converted once, as QuickJS's
    /// `get_array_el3` does for an update: a nullish base throws first.
    fn convert_key(&mut self, place: Place) -> Place {
        match place {
            Place::Elem { obj, key, at } => {
                let key = self.op(Op::ToPropertyKeyOf, &[key, obj], Imm::None);
                Place::Elem { obj, key, at }
            }
            Place::Super {
                this,
                base,
                key,
                at,
            } => {
                let key = self.op(Op::ToPropertyKey, &[key], Imm::None);
                Place::Super {
                    this,
                    base,
                    key,
                    at,
                }
            }
            place => place,
        }
    }

    fn logical_assign(&mut self, op: AssignOp, target: &Target, value: &Expr) -> Result<Value> {
        let target_at = target.span.start;
        let (old, place) = match &target.kind {
            TargetKind::Decl { decl } => (self.read_decl(*decl, Some(target_at))?, None),
            TargetKind::Member { obj, prop, .. } => {
                let obj = self.expr(obj)?;
                let place = self.place(obj, prop, target_at)?;
                let place = self.convert_key(place);
                (self.get_place(place)?, Some(place))
            }
            TargetKind::SuperMember {
                prop,
                this,
                home,
                super_at,
            } => {
                let place = self.super_place(prop, *this, *home, target_at, *super_at)?;
                let place = self.convert_key(place);
                (self.get_place(place)?, Some(place))
            }
            TargetKind::Pattern { .. } => return internal("logical assignment to a pattern"),
        };
        let cond = match op {
            AssignOp::Nullish => self.op(Op::IsUndefinedOrNull, &[old], Imm::None),
            _ => old,
        };
        let write = self.block();
        let short = self.block();
        let (then, else_) = match op {
            AssignOp::Or => (short, write),
            _ => (write, short),
        };
        self.end(Terminator::Branch { cond, then, else_ });
        self.start(write);
        let new = self.expr(value)?;
        match (&target.kind, place) {
            (TargetKind::Decl { decl }, _) => self.put_decl(*decl, new)?,
            (_, Some(place)) => self.set_place(place, new)?,
            _ => unreachable!("a place"),
        }
        let from_write = self.branch_end();
        self.start(short);
        let from_short = self.branch_end();
        Ok(self.join(vec![(from_write, new), (from_short, old)]))
    }

    /// Writes an iteration's value to a member target (for-in/of heads).
    pub(super) fn assign_member_value(&mut self, target: &Target, value: Value) -> Result<()> {
        let TargetKind::Member { obj, prop, .. } = &target.kind else {
            return internal("not a member target");
        };
        let obj = self.expr(obj)?;
        let place = self.place(obj, prop, target.span.start)?;
        self.mark(Some(place_at(place)));
        self.set_place(place, value)?;
        Ok(())
    }

    fn update(&mut self, op: UpdateOp, prefix: bool, target: &Target, at: u32) -> Result<Value> {
        let step = match op {
            UpdateOp::Inc => Op::Inc,
            UpdateOp::Dec => Op::Dec,
        };
        match &target.kind {
            TargetKind::Decl { decl } => {
                let old = self.read_decl(*decl, Some(target.span.start))?;
                let (result, new) = if prefix {
                    let new = self.op_at(step, &[old], Imm::None, at);
                    (new, new)
                } else {
                    let numeric = self.op_at(Op::ToNumeric, &[old], Imm::None, at);
                    let new = self.op(step, &[numeric], Imm::None);
                    (numeric, new)
                };
                self.put_decl(*decl, new)?;
                Ok(result)
            }
            TargetKind::Member { obj, prop, .. } => {
                let obj = self.expr(obj)?;
                let place = self.place(obj, prop, target.span.start)?;
                let place = self.convert_key(place);
                let old = self.get_place(place)?;
                let (result, new) = if prefix {
                    let new = self.op_at(step, &[old], Imm::None, at);
                    (new, new)
                } else {
                    let numeric = self.op_at(Op::ToNumeric, &[old], Imm::None, at);
                    let new = self.op(step, &[numeric], Imm::None);
                    (numeric, new)
                };
                self.set_place(place, new)?;
                Ok(result)
            }
            TargetKind::SuperMember {
                prop,
                this,
                home,
                super_at,
            } => {
                let place = self.super_place(prop, *this, *home, target.span.start, *super_at)?;
                let place = self.convert_key(place);
                let old = self.get_place(place)?;
                let (result, new) = if prefix {
                    let new = self.op_at(step, &[old], Imm::None, at);
                    (new, new)
                } else {
                    let numeric = self.op_at(Op::ToNumeric, &[old], Imm::None, at);
                    let new = self.op(step, &[numeric], Imm::None);
                    (numeric, new)
                };
                self.set_place(place, new)?;
                Ok(result)
            }
            TargetKind::Pattern { .. } => internal("update of a pattern"),
        }
    }

    // ---- suspension ----

    pub(super) fn await_value(&mut self, value: Value, at: Option<u32>) -> Value {
        let resume = self.block();
        let sent = self.f.new_value(Default::default());
        let term = Terminator::Suspend {
            kind: SuspendKind::Await,
            value: Some(value),
            sent: Some(sent),
            resume_kind: None,
            resume,
        };
        match at {
            Some(at) => self.end_at(term, at),
            None => self.end(term),
        }
        self.start(resume);
        sent
    }
}

#[allow(dead_code)]
fn place_at(place: Place) -> u32 {
    match place {
        Place::Field { at, .. }
        | Place::Elem { at, .. }
        | Place::Private { at, .. }
        | Place::Super { at, .. } => at,
    }
}

fn compound_op(op: AssignOp) -> BinOp {
    match op {
        AssignOp::Add => BinOp::Add,
        AssignOp::Sub => BinOp::Sub,
        AssignOp::Mul => BinOp::Mul,
        AssignOp::Div => BinOp::Div,
        AssignOp::Rem => BinOp::Rem,
        AssignOp::Exp => BinOp::Exp,
        AssignOp::Shl => BinOp::Shl,
        AssignOp::Sar => BinOp::Sar,
        AssignOp::Shr => BinOp::Shr,
        AssignOp::BitAnd => BinOp::BitAnd,
        AssignOp::BitOr => BinOp::BitOr,
        AssignOp::BitXor => BinOp::BitXor,
        _ => BinOp::Add,
    }
}

impl DeclKind {
    pub(crate) fn is_lexical_binding(self) -> bool {
        matches!(self, DeclKind::Let | DeclKind::Const | DeclKind::Class)
    }
}

#[allow(dead_code)]
fn cmp(op: BinOp) -> Option<CmpOp> {
    Some(match op {
        BinOp::Lt => CmpOp::Lt,
        BinOp::Le => CmpOp::Lte,
        BinOp::Gt => CmpOp::Gt,
        BinOp::Ge => CmpOp::Gte,
        BinOp::Eq => CmpOp::Eq,
        BinOp::Ne => CmpOp::Neq,
        BinOp::StrictEq => CmpOp::StrictEq,
        BinOp::StrictNe => CmpOp::StrictNeq,
        _ => return None,
    })
}
