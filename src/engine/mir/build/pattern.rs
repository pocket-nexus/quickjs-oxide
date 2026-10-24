//! Destructuring, as QuickJS lowers it: each leaf prepares its target
//! before the iterator steps or the property is read, a default replaces
//! only `undefined`, and an array pattern's iterator closes when the
//! pattern ends or throws.

use super::{Access, Builder, Control, Result, internal};
use crate::engine::hir::schema::{
    DeclKind, DeclRef, Expr, Pattern, PatternItem, PatternKind, PatternProp, PropKey, Target,
    TargetKind,
};
use crate::engine::mir::ir::{BlockId, Imm, Phi, Terminator, Value, VarKind};
use crate::engine::mir::op::Op;

/// How a pattern's leaves take their values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    /// A declaration's bindings: `var` ones are assigned (through a
    /// reference prepared before the value is read, for a global), the
    /// others initialized. `catch` for a catch parameter's pattern.
    Declare { var: bool, catch: bool },
    /// An assignment pattern's targets.
    Assign,
}

/// An assignment target with its base and key evaluated.
enum Prepared {
    Decl(DeclRef),
    Field {
        obj: Value,
        key: u32,
        at: u32,
    },
    Elem {
        obj: Value,
        key: Value,
        at: u32,
    },
    /// A global `var` binding's environment, prepared before the read.
    Global {
        env: Value,
        name: u32,
    },
    /// A binding initialized or stored directly.
    Binding(DeclRef),
    /// A private or `super` member.
    Place(super::expr::Place),
}

impl Builder<'_, '_> {
    /// Destructures `value` with `pattern`.
    pub(super) fn destructure(
        &mut self,
        pattern: &Pattern,
        value: Value,
        mode: Mode,
    ) -> Result<()> {
        match &pattern.kind {
            PatternKind::Decl { decl } => {
                let prepared = self.prepare_binding(*decl, mode)?;
                self.store_prepared(prepared, value, mode)
            }
            PatternKind::Target { t } => {
                let prepared = self.prepare_target(t)?;
                self.store_prepared(prepared, value, mode)
            }
            PatternKind::Array { items, rest } => {
                self.array_pattern(items, rest.as_deref(), value, mode)
            }
            PatternKind::Object {
                props,
                rest,
                rest_at,
            } => self.object_pattern(
                props,
                rest.as_deref(),
                *rest_at,
                pattern.span.start,
                value,
                mode,
            ),
        }
    }

    /// QuickJS's control-inverted layout of `pattern = init`: the pattern's
    /// code first, reached from the initializer after it. Returns the
    /// initializer's value.
    pub(super) fn destructure_after(
        &mut self,
        pattern: &Pattern,
        init: &Expr,
        mode: Mode,
    ) -> Result<Value> {
        let fragment = self.block();
        let init_block = self.block();
        let done = self.block();
        self.goto(init_block);
        let value = self.f.new_value(Default::default());
        self.start(fragment);
        self.destructure(pattern, value, mode)?;
        self.goto(done);
        self.start(init_block);
        let init_value = self.expr(init)?;
        let init_end = self.cur;
        self.goto(fragment);
        self.bind_entry(fragment, value, init_end, init_value);
        self.start(done);
        Ok(init_value)
    }

    /// Gives `block`, whose only predecessor is `pred`, the value `value`
    /// on entry: a one-operand phi.
    fn bind_entry(&mut self, block: BlockId, value: Value, pred: Option<BlockId>, from: Value) {
        let b = self.f.block_mut(block);
        b.preds = pred.into_iter().collect();
        b.phis.push(Phi {
            result: value,
            args: if b.preds.is_empty() {
                Vec::new()
            } else {
                vec![from]
            },
        });
    }

    /// A nested pattern's element: with a default, QuickJS tests the value,
    /// runs the pattern, and places the default after it with a jump back.
    pub(super) fn nested_element(
        &mut self,
        pattern: &Pattern,
        value: Value,
        default: Option<&Expr>,
        mode: Mode,
    ) -> Result<()> {
        let Some(default) = default else {
            return self.destructure(pattern, value, mode);
        };
        let undefined = self.undefined();
        let is_undefined = self.op(Op::StrictEq, &[value, undefined], Imm::None);
        let initializer = self.block();
        let through = self.block();
        let fragment = self.block();
        let done = self.block();
        self.end(Terminator::Branch {
            cond: is_undefined,
            then: initializer,
            else_: through,
        });
        self.start(through);
        let through_end = self.cur;
        self.goto(fragment);
        let element = self.f.new_value(Default::default());
        self.start(fragment);
        self.destructure(pattern, element, mode)?;
        self.goto(done);
        self.start(initializer);
        let defaulted = self.expr(default)?;
        let initializer_end = self.cur;
        self.goto(fragment);
        let mut preds = Vec::new();
        let mut args = Vec::new();
        for (pred, arg) in [(through_end, value), (initializer_end, defaulted)] {
            if let Some(pred) = pred {
                preds.push(pred);
                args.push(arg);
            }
        }
        let b = self.f.block_mut(fragment);
        b.preds = preds;
        b.phis.push(Phi {
            result: element,
            args,
        });
        self.start(done);
        Ok(())
    }

    /// `value === undefined ? default : value`.
    pub(super) fn with_default(&mut self, value: Value, default: Option<&Expr>) -> Result<Value> {
        let Some(default) = default else {
            return Ok(value);
        };
        let undefined = self.undefined();
        let is_undefined = self.op(Op::StrictEq, &[value, undefined], Imm::None);
        let use_default = self.block();
        let has_value = self.block();
        self.end(Terminator::Branch {
            cond: is_undefined,
            then: use_default,
            else_: has_value,
        });
        self.start(use_default);
        let defaulted = self.expr(default)?;
        let from_default = self.branch_end();
        self.start(has_value);
        let from_value = self.branch_end();
        Ok(self.join(vec![(from_default, defaulted), (from_value, value)]))
    }

    // ---- leaves ----

    /// A binding leaf's target: a global `var` binding's reference is
    /// prepared now.
    fn prepare_binding(&mut self, decl: DeclRef, mode: Mode) -> Result<Prepared> {
        if let Mode::Declare { var: true, .. } = mode
            && let Access::Global { name } = self.access(decl)?
        {
            let env = self.op(Op::GlobalRef, &[], Imm::Key(name.0));
            return Ok(Prepared::Global { env, name: name.0 });
        }
        Ok(Prepared::Binding(decl))
    }

    /// An assignment target's base and key.
    fn prepare_target(&mut self, target: &Target) -> Result<Prepared> {
        let at = target.span.start;
        Ok(match &target.kind {
            TargetKind::Decl { decl } => Prepared::Decl(*decl),
            TargetKind::Member { obj, prop, .. } => {
                let obj = self.expr(obj)?;
                match &**prop {
                    PropKey::Name { name } => Prepared::Field {
                        obj,
                        key: name.0,
                        at,
                    },
                    PropKey::Computed { e } => {
                        let key = self.expr(e)?;
                        Prepared::Elem { obj, key, at }
                    }
                    PropKey::Private { name } => Prepared::Place(super::expr::Place::Private {
                        obj,
                        decl: *name,
                        at,
                    }),
                }
            }
            TargetKind::Pattern { .. } => return internal("a pattern as a leaf target"),
            TargetKind::SuperMember {
                prop,
                this,
                home,
                super_at,
            } => Prepared::Place(self.super_place(prop, *this, *home, at, *super_at)?),
        })
    }

    /// The position a prepared target's read or step takes.
    fn target_at(target: &Target) -> u32 {
        target.span.start
    }

    fn store_prepared(&mut self, prepared: Prepared, value: Value, mode: Mode) -> Result<()> {
        match prepared {
            Prepared::Decl(decl) => self.put_decl(decl, value),
            Prepared::Field { obj, key, at } => {
                self.mark(Some(at));
                self.set_place(super::expr::Place::Field { obj, key, at }, value)
            }
            Prepared::Elem { obj, key, at } => {
                self.mark(Some(at));
                self.set_place(super::expr::Place::Elem { obj, key, at }, value)
            }
            Prepared::Place(place) => self.set_place(place, value),
            Prepared::Global { env, name } => {
                self.effect(Op::PutRef, &[env, value], Imm::Key(name));
                Ok(())
            }
            Prepared::Binding(decl) => match mode {
                Mode::Declare { var: true, .. } => self.put_decl(decl, value),
                _ => self.init_binding(decl, value),
            },
        }
    }

    // ---- arrays ----

    fn array_pattern(
        &mut self,
        items: &[PatternItem],
        rest: Option<&Pattern>,
        value: Value,
        mode: Mode,
    ) -> Result<()> {
        let record = self.f.new_value(Default::default());
        self.record(record);
        self.define_with(Op::IterStart, &[value], Imm::Int(0), &[record]);
        let opened = self.site;
        let close = self.block();
        let handler = self.open_handler(close);
        self.rebase();
        self.controls.push(Control::Iterator { record });
        for item in items {
            match item {
                PatternItem::Hole => {
                    self.op2(Op::IterNext, &[record], Imm::None);
                }
                PatternItem::Item { p, default } => match &p.kind {
                    PatternKind::Decl { decl } => {
                        let prepared = self.prepare_binding(*decl, mode)?;
                        let (element, _) = self.op2(Op::IterNext, &[record], Imm::None);
                        let element = self.with_default(element, default.as_deref())?;
                        self.store_prepared(prepared, element, mode)?;
                    }
                    PatternKind::Target { t } => {
                        let prepared = self.prepare_target(t)?;
                        let at = Self::target_at(t);
                        let (element, _) = self.op2_at(Op::IterNext, &[record], Imm::None, at);
                        let element = self.with_default(element, default.as_deref())?;
                        self.store_prepared(prepared, element, mode)?;
                    }
                    _ => {
                        let (element, _) = self.op2(Op::IterNext, &[record], Imm::None);
                        self.nested_element(p, element, default.as_deref(), mode)?;
                    }
                },
            }
        }
        if let Some(rest) = rest {
            match &rest.kind {
                PatternKind::Decl { decl } => {
                    let prepared = self.prepare_binding(*decl, mode)?;
                    let array = self.array_rest(record, None);
                    self.store_prepared(prepared, array, mode)?;
                }
                PatternKind::Target { t } => {
                    let prepared = self.prepare_target(t)?;
                    let array = self.array_rest(record, Some(Self::target_at(t)));
                    self.store_prepared(prepared, array, mode)?;
                }
                _ => {
                    let array = self.array_rest(record, None);
                    self.destructure(rest, array, mode)?;
                }
            }
        }
        self.effect(Op::IterClose, &[record], Imm::None);
        self.controls.pop();
        self.close_handler(handler);
        self.rebase();
        self.close_stub(close, record, opened);
        Ok(())
    }

    /// The handler that closes an iterator a throw leaves, and rethrows:
    /// placed after the function's code.
    pub(super) fn close_stub(
        &mut self,
        close: BlockId,
        record: Value,
        at: crate::engine::mir::ir::Pos,
    ) {
        let thrown = self.f.new_value(Default::default());
        self.f.block_mut(close).catch = Some(thrown);
        self.stub(close, at, |b| {
            b.effect(Op::IterCloseThrow, &[record, thrown], Imm::None);
            b.end(Terminator::Throw(thrown));
        });
    }

    /// QuickJS's spread into an array of what remains of `record`.
    fn array_rest(&mut self, record: Value, at: Option<u32>) -> Value {
        let array = self.op(Op::NewArray, &[], Imm::None);
        let index = self.new_var(VarKind::Local, None);
        let zero = self.int(0);
        self.set_var(index, zero);
        let step = self.block();
        let exit = self.block();
        self.loop_depth += 1;
        let body = self.block();
        self.enter(step);
        let (element, done) = match at {
            Some(at) => self.op2_at(Op::IterNext, &[record], Imm::None, at),
            None => self.op2(Op::IterNext, &[record], Imm::None),
        };
        self.end(Terminator::Branch {
            cond: done,
            then: exit,
            else_: body,
        });
        self.start(body);
        let position = self.get_var(index);
        self.effect(Op::DefineElem, &[array, position, element], Imm::None);
        let next = self.op(Op::Inc, &[position], Imm::None);
        self.set_var(index, next);
        self.goto(step);
        self.loop_depth -= 1;
        self.start(exit);
        array
    }

    // ---- objects ----

    fn object_pattern(
        &mut self,
        props: &[PatternProp],
        rest: Option<&Pattern>,
        rest_at: u32,
        at: u32,
        value: Value,
        mode: Mode,
    ) -> Result<()> {
        let source = match mode {
            Mode::Declare { catch: true, .. } => self.op(Op::ToObject, &[value], Imm::None),
            _ => self.op_at(Op::ToObject, &[value], Imm::None, at),
        };
        let excluded = if rest.is_some() {
            let slots = props
                .iter()
                .filter(|prop| matches!(*prop.key, PropKey::Name { .. }))
                .count() as u32;
            Some(self.op(Op::NewObject, &[], Imm::Flags(slots)))
        } else {
            None
        };
        for prop in props {
            let key_at = prop.key_at;
            let mut key = match &*prop.key {
                PropKey::Name { name } => Err(name.0),
                PropKey::Computed { e } => Ok(self.expr(e)?),
                PropKey::Private { .. } => return internal("a private key in a pattern"),
            };
            let mut canonical = false;
            if let Some(excluded) = excluded {
                let null = match key {
                    Err(name) => {
                        let null = self.op(Op::Null, &[], Imm::None);
                        self.effect_at(Op::DefineField, &[excluded, null], Imm::Key(name), key_at);
                        null
                    }
                    Ok(raw) => {
                        let converted = self.op_at(Op::ToPropertyKey, &[raw], Imm::None, key_at);
                        key = Ok(converted);
                        canonical = true;
                        let null = self.op(Op::Null, &[], Imm::None);
                        self.effect(Op::DefineElem, &[excluded, converted, null], Imm::None);
                        null
                    }
                };
                let _ = null;
            }
            match &prop.value.kind {
                PatternKind::Object { .. } | PatternKind::Array { .. } => {
                    let element = match key {
                        Err(name) => self.op_at(Op::GetField, &[source], Imm::Key(name), key_at),
                        Ok(key) => self.op_at(Op::GetElem, &[source, key], Imm::None, key_at),
                    };
                    self.nested_element(&prop.value, element, prop.default.as_deref(), mode)?;
                }
                PatternKind::Decl { decl } => {
                    if let Ok(raw) = key
                        && !canonical
                    {
                        key = Ok(self.op(Op::ToPropertyKey, &[raw], Imm::None));
                    }
                    let prepared = self.prepare_binding(*decl, mode)?;
                    let element = match key {
                        Err(name) => self.op_at(Op::GetField, &[source], Imm::Key(name), key_at),
                        Ok(key) => self.op_at(Op::GetElem, &[source, key], Imm::None, key_at),
                    };
                    let element = self.with_default(element, prop.default.as_deref())?;
                    self.store_prepared(prepared, element, mode)?;
                }
                PatternKind::Target { t } => {
                    if let Ok(raw) = key
                        && !canonical
                    {
                        key = Ok(self.op_at(Op::ToPropertyKey, &[raw], Imm::None, key_at));
                    }
                    let prepared = self.prepare_target(t)?;
                    let target_at = Self::target_at(t);
                    let element = match key {
                        Err(name) => self.op_at(Op::GetField, &[source], Imm::Key(name), target_at),
                        Ok(key) => self.op_at(Op::GetElem, &[source, key], Imm::None, target_at),
                    };
                    let element = self.with_default(element, prop.default.as_deref())?;
                    self.store_prepared(prepared, element, mode)?;
                }
            }
        }
        if let (Some(rest), Some(excluded)) = (rest, excluded) {
            match &rest.kind {
                PatternKind::Decl { decl } => {
                    let prepared = self.prepare_binding(*decl, mode)?;
                    let object = self.op(Op::NewObject, &[], Imm::Flags(0));
                    self.effect_at(
                        Op::CopyDataExcluded,
                        &[object, source, excluded],
                        Imm::None,
                        rest_at,
                    );
                    self.store_prepared(prepared, object, mode)?;
                }
                PatternKind::Target { t } => {
                    let prepared = self.prepare_target(t)?;
                    let object = self.op(Op::NewObject, &[], Imm::Flags(0));
                    self.effect_at(
                        Op::CopyDataExcluded,
                        &[object, source, excluded],
                        Imm::None,
                        Self::target_at(t),
                    );
                    self.store_prepared(prepared, object, mode)?;
                }
                _ => return internal("an object rest that is a pattern"),
            }
        }
        Ok(())
    }

    /// The bindings a pattern declares, in order.
    pub(super) fn pattern_decls(pattern: &Pattern, out: &mut Vec<DeclRef>) {
        match &pattern.kind {
            PatternKind::Decl { decl } => out.push(*decl),
            PatternKind::Target { .. } => {}
            PatternKind::Object { props, rest, .. } => {
                for prop in props {
                    Self::pattern_decls(&prop.value, out);
                }
                if let Some(rest) = rest {
                    Self::pattern_decls(rest, out);
                }
            }
            PatternKind::Array { items, rest } => {
                for item in items {
                    if let PatternItem::Item { p, .. } = item {
                        Self::pattern_decls(p, out);
                    }
                }
                if let Some(rest) = rest {
                    Self::pattern_decls(rest, out);
                }
            }
        }
    }

    /// Whether `decl` is declared with `var`.
    pub(super) fn is_var(&self, decl: DeclRef) -> bool {
        self.decl(decl).kind == DeclKind::Var
    }
}
