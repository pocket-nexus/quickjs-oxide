//! Statements, in QuickJS's layout: each construct's blocks are created in
//! the order the old front end emits their code.

use super::entry::lexical;
use super::pattern::Mode;
use super::{Access, Builder, Control, Result, internal};
use crate::engine::hir::schema::{
    self as hir, Block, DeclKind, DeclRef, ForTarget, PatternKind, Stmt, StmtKind, TargetKind,
};
use crate::engine::mir::ir::{BlockId, Imm, Terminator, Value, VarKind};
use crate::engine::mir::op::Op;

/// How a `break` or `continue` leaves the statements it crosses.
enum Exit {
    Break,
    Continue,
}

impl Builder<'_, '_> {
    pub(super) fn statements(&mut self, stmts: &[Stmt]) -> Result<()> {
        for stmt in stmts {
            self.statement(stmt)?;
        }
        Ok(())
    }

    /// Sets the completion value of script and eval code to `undefined`, as
    /// QuickJS does at the start of a control statement.
    fn reset_completion(&mut self) {
        if let Some(completion) = self.completion {
            let undefined = self.undefined();
            self.set_var(completion, undefined);
        }
    }

    pub(super) fn statement(&mut self, stmt: &Stmt) -> Result<()> {
        let at = stmt.span.start;
        match &stmt.kind {
            StmtKind::Expr { e } => {
                self.seed(at);
                let value = self.expr(e)?;
                if let Some(completion) = self.completion {
                    self.set_var(completion, value);
                }
            }
            StmtKind::Declare { decl, init } => {
                let value = match init {
                    Some(init) => self.expr(init)?,
                    None => self.undefined(),
                };
                let d = self.decl(*decl);
                if d.kind == DeclKind::Var {
                    self.mark(Some(at));
                    self.put_decl(*decl, value)?;
                } else {
                    self.mark(Some(at));
                    self.init_binding(*decl, value)?;
                }
            }
            StmtKind::Destructure {
                pattern,
                init,
                declaring,
            } => {
                let mode = match declaring {
                    Some(hir::DeclaringKind::Var) => Mode::Declare {
                        var: true,
                        catch: false,
                    },
                    Some(_) => Mode::Declare {
                        var: false,
                        catch: false,
                    },
                    None => Mode::Assign,
                };
                self.destructure_after(pattern, init, mode)?;
            }
            StmtKind::Block { body } => {
                if let [single] = body.stmts.as_slice()
                    && !body.decls.is_empty()
                    && matches!(
                        single.kind,
                        StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::ForOf { .. }
                    )
                {
                    // The head scope of a for statement.
                    self.enter_scope(&body.decls, &body.stmts)?;
                    self.loop_statement(single, true)?;
                    self.leave_scope();
                } else {
                    self.block_statement(body)?;
                }
            }
            StmtKind::If { test, then, else_ } => {
                self.reset_completion();
                let cond = self.expr(test)?;
                let then_block = self.block();
                let else_block = self.block();
                self.end(Terminator::Branch {
                    cond,
                    then: then_block,
                    else_: else_block,
                });
                self.start(then_block);
                self.statement(then)?;
                match else_ {
                    Some(else_) => {
                        let end = self.block();
                        self.goto(end);
                        self.start(else_block);
                        self.statement(else_)?;
                        self.enter(end);
                    }
                    None => self.enter(else_block),
                }
            }
            StmtKind::While { .. }
            | StmtKind::DoWhile { .. }
            | StmtKind::For { .. }
            | StmtKind::ForIn { .. }
            | StmtKind::ForOf { .. } => self.loop_statement(stmt, false)?,
            StmtKind::Switch {
                disc, cases, label, ..
            } => self.switch_statement(disc, cases, *label)?,
            StmtKind::Break { label } => self.jump(*label, Exit::Break)?,
            StmtKind::Continue { label } => self.jump(*label, Exit::Continue)?,
            StmtKind::Return { e } => {
                let value = match e {
                    Some(e) => {
                        let value = self.expr(e)?;
                        self.remark_last_call(at);
                        Some(value)
                    }
                    None => None,
                };
                self.return_completion(value, at, value.is_some())?;
            }
            StmtKind::Throw { e } => {
                let value = self.expr(e)?;
                self.end_at(Terminator::Throw(value), at);
            }
            StmtKind::Try {
                body,
                catch_param,
                catch_body,
                finally,
                catch_default,
            } => {
                self.try_statement(
                    body,
                    catch_param.as_deref(),
                    catch_default.as_deref(),
                    catch_body.as_ref(),
                    finally.as_ref(),
                )?;
            }
            StmtKind::Labeled { label, body } => {
                let break_to = self.block();
                self.controls.push(Control::Breakable {
                    label: *label,
                    break_to,
                    continue_to: None,
                    iterator: None,
                    scope_depth: self.scopes.len(),
                });
                self.statement(body)?;
                self.controls.pop();
                self.enter(break_to);
            }
            StmtKind::FnDecl { decl, annex_b, .. } => {
                // The binding was made where its scope starts; Annex B also
                // sets the function's var when the declaration is evaluated.
                if let Some(var) = annex_b {
                    let value = self.read_decl(*decl, None)?;
                    self.put_decl(*var, value)?;
                }
            }
            StmtKind::ClassDecl { decl, cls } => {
                let class = self.class(cls, None)?;
                self.mark(Some(self.decl(*decl).span.start));
                self.init_binding(*decl, class)?;
            }
            StmtKind::Debugger | StmtKind::Empty => {}
            StmtKind::DefineField { key, value } => self.define_field(key, value.as_deref())?,
            StmtKind::DefinePrivate { decl, value } => {
                self.define_private(*decl, value.as_deref())?;
            }
            StmtKind::StaticBlock { fn_ } => {
                let block = self.closure(*fn_);
                self.effect(Op::CallStaticBlock, &[block], Imm::None);
            }
        }
        Ok(())
    }

    // ---- scopes ----

    fn block_statement(&mut self, body: &Block) -> Result<()> {
        self.enter_scope(&body.decls, &body.stmts)?;
        self.statements(&body.stmts)?;
        self.leave_scope();
        Ok(())
    }

    /// Enters a lexical scope: QuickJS resets each lexical binding to the
    /// TDZ (newest first) and makes the scope's function declarations.
    pub(super) fn enter_scope(&mut self, decls: &[DeclRef], stmts: &[Stmt]) -> Result<()> {
        let functions: Vec<(DeclRef, hir::FunctionRef)> = stmts
            .iter()
            .filter_map(|stmt| match stmt.kind {
                StmtKind::FnDecl { decl, fn_, .. } => Some((decl, fn_)),
                _ => None,
            })
            .collect();
        let mut close = Vec::new();
        for &decl in decls.iter().rev() {
            let Some(&var) = self.vars.get(&decl) else {
                continue;
            };
            let d = self.decl(decl);
            if !lexical(d.kind) && d.kind != DeclKind::Function && d.kind != DeclKind::Catch {
                continue;
            }
            let captured = self.captured(decl);
            if let Some(&(_, function)) = functions.iter().rev().find(|(f, _)| *f == decl) {
                let closure = self.closure(function);
                self.init_binding(decl, closure)?;
            } else if d.kind == DeclKind::Catch {
                // Bound where the catch starts.
            } else if captured {
                let cell = self.get_var(var);
                self.effect(Op::ClearCell, &[cell], Imm::None);
            } else if d.tdz {
                let empty = self.op(Op::Empty, &[], Imm::None);
                self.set_var(var, empty);
            }
            if captured {
                close.push(decl);
            }
        }
        self.checked.clear();
        self.scopes.push(close);
        Ok(())
    }

    /// Leaves the innermost scope on the normal path.
    pub(super) fn leave_scope(&mut self) {
        let close = self.scopes.pop().unwrap_or_default();
        self.close_cells(&close);
    }

    /// Detaches the captured bindings `decls` from the closures made so far.
    pub(super) fn close_cells(&mut self, decls: &[DeclRef]) {
        for &decl in decls {
            let vars = [
                self.vars.get(&decl).copied(),
                self.setter_vars.get(&decl).copied(),
            ];
            for var in vars.into_iter().flatten() {
                let cell = self.get_var(var);
                let fresh = self.op(Op::CloseCell, &[cell], Imm::None);
                self.set_var(var, fresh);
            }
        }
    }

    /// Closes the scopes above `depth` for a jump out of them; the scopes
    /// stay entered for the code after the jump.
    fn close_scopes_to(&mut self, depth: usize) {
        let scopes: Vec<Vec<DeclRef>> = self.scopes[depth.min(self.scopes.len())..]
            .iter()
            .rev()
            .cloned()
            .collect();
        for close in scopes {
            self.close_cells(&close);
        }
    }

    /// Makes the function declarations of a function body.
    pub(super) fn hoist_functions(&mut self, stmts: &[Stmt]) -> Result<()> {
        for stmt in stmts {
            if let StmtKind::FnDecl { decl, fn_, .. } = stmt.kind {
                let closure = self.closure(fn_);
                self.init_binding(decl, closure)?;
            }
        }
        Ok(())
    }

    // ---- bindings ----

    /// Initializes `decl` (a declaration's binding becoming live) with
    /// `value`.
    pub(super) fn init_binding(&mut self, decl: DeclRef, value: Value) -> Result<()> {
        match self.access(decl)? {
            Access::Var { var, cell: false } => self.set_var(var, value),
            Access::Var { var, cell: true } => {
                let cell = self.get_var(var);
                self.effect(Op::StoreCell, &[cell, value], Imm::None);
            }
            Access::Captured { index } => {
                self.effect(Op::StoreCaptured, &[value], Imm::Capture(index));
            }
            Access::Global { name } => {
                let flags = self.global_flags();
                self.effect(Op::DefineGlobal, &[value], Imm::KeyFlags(name.0, flags));
            }
        }
        Ok(())
    }

    pub(super) fn global_flags(&self) -> u32 {
        if self.hf.kind == hir::FunctionKind::Eval {
            u32::from(oxide_core::bytecode::opcode::global::EVAL)
        } else {
            0
        }
    }

    /// Stores `value` into `decl` by assignment (`var x = v`, `x = v`):
    /// a TDZ binding is checked first, a constant throws.
    pub(super) fn put_decl(&mut self, decl: DeclRef, value: Value) -> Result<()> {
        let d = self.decl(decl);
        if d.kind == DeclKind::Const || d.kind == DeclKind::Import {
            let key = d.name.0;
            if matches!(self.access(decl)?, Access::Global { .. }) {
                // The global lexical binding throws itself.
            } else {
                // QuickJS throws before any TDZ check.
                self.end(Terminator::ThrowError {
                    kind: crate::engine::mir::ir::ThrowKind::ConstAssign,
                    key,
                });
                return Ok(());
            }
        }
        if d.kind == DeclKind::FunctionName {
            // A named function expression's own name is read-only: a sloppy
            // write is ignored, a strict one throws.
            if self.hf.strict || self.cx.function(d.owner).strict {
                self.end(Terminator::ThrowError {
                    kind: crate::engine::mir::ir::ThrowKind::ConstAssign,
                    key: d.name.0,
                });
            }
            return Ok(());
        }
        let check = d.tdz;
        match self.access(decl)? {
            Access::Var { var, cell: false } => {
                if check {
                    let current = self.get_var(var);
                    self.check_init(decl, current);
                }
                self.set_var(var, value);
            }
            Access::Var { var, cell: true } => {
                let cell = self.get_var(var);
                if check {
                    let current = self.op(Op::LoadCell, &[cell], Imm::None);
                    self.check_init(decl, current);
                }
                self.effect(Op::StoreCell, &[cell, value], Imm::None);
            }
            Access::Captured { index } => {
                if check {
                    let current = self.op(Op::LoadCaptured, &[], Imm::Capture(index));
                    self.check_init(decl, current);
                }
                self.effect(Op::StoreCaptured, &[value], Imm::Capture(index));
            }
            Access::Global { name } => {
                self.effect(Op::SetGlobal, &[value], Imm::Key(name.0));
            }
        }
        Ok(())
    }

    /// Throws `ReferenceError` when `value`, the current value of `decl`, is
    /// the TDZ marker; once per block.
    pub(super) fn check_init(&mut self, decl: DeclRef, value: Value) {
        if self.checked.insert(decl) {
            let key = self.decl_key(decl);
            self.effect(Op::CheckInit, &[value], Imm::Key(key));
        }
    }

    /// Reads `decl`; `own` is the read's own position.
    pub(super) fn read_decl(&mut self, decl: DeclRef, own: Option<u32>) -> Result<Value> {
        let d = self.decl(decl);
        // A derived constructor's `this` waits for `super()`.
        let check = d.tdz
            || (d.kind == DeclKind::This
                && self.cx.function(d.owner).kind == hir::FunctionKind::DerivedConstructor);
        let value = match self.access(decl)? {
            Access::Var { var, cell: false } => {
                let value = self.push_get_var(var, own);
                if check {
                    self.check_init(decl, value);
                }
                value
            }
            Access::Var { var, cell: true } => {
                let cell = self.push_get_var(var, own);
                let value = self.op(Op::LoadCell, &[cell], Imm::None);
                if check {
                    self.check_init(decl, value);
                }
                value
            }
            Access::Captured { index } => {
                let value = match own {
                    Some(at) => self.op_at(Op::LoadCaptured, &[], Imm::Capture(index), at),
                    None => self.op(Op::LoadCaptured, &[], Imm::Capture(index)),
                };
                if check {
                    self.check_init(decl, value);
                }
                value
            }
            Access::Global { name } => match own {
                Some(at) => self.op_at(Op::GetGlobal, &[], Imm::Key(name.0), at),
                None => self.op(Op::GetGlobal, &[], Imm::Key(name.0)),
            },
        };
        Ok(value)
    }

    fn push_get_var(&mut self, var: u32, own: Option<u32>) -> Value {
        match own {
            Some(at) => {
                let value = self.op_at(Op::GetVar, &[], Imm::Var(var), at);
                if self.f.vars[var as usize].kind == VarKind::This {
                    self.this_values.insert(value);
                }
                value
            }
            None => self.get_var(var),
        }
    }

    // ---- loops ----

    /// A loop; `scoped` when it is the only statement of its head scope.
    fn loop_statement(&mut self, stmt: &Stmt, scoped: bool) -> Result<()> {
        match &stmt.kind {
            StmtKind::While { test, body, label } => {
                self.reset_completion();
                let test_block = self.block();
                let exit = self.block();
                self.enter(test_block);
                self.loop_depth += 1;
                let body_block = self.block();
                let cond = self.expr(test)?;
                self.end(Terminator::Branch {
                    cond,
                    then: body_block,
                    else_: exit,
                });
                self.start(body_block);
                self.loop_body(*label, exit, Some(test_block), None, body)?;
                self.goto(test_block);
                self.loop_depth -= 1;
                self.start(exit);
            }
            StmtKind::DoWhile { body, test, label } => {
                self.loop_depth += 1;
                let body_block = self.block();
                let test_block = self.block();
                let exit = self.block();
                self.enter(body_block);
                self.reset_completion();
                self.loop_body(*label, exit, Some(test_block), None, body)?;
                self.enter(test_block);
                let cond = self.expr(test)?;
                self.end(Terminator::Branch {
                    cond,
                    then: body_block,
                    else_: exit,
                });
                self.loop_depth -= 1;
                self.start(exit);
            }
            StmtKind::For {
                init,
                test,
                update,
                body,
                label,
                per_iteration,
            } => {
                self.reset_completion();
                if let Some(init) = init {
                    match &init.kind {
                        StmtKind::Expr { e } => {
                            self.expr(e)?;
                        }
                        _ => self.statement(init)?,
                    }
                    let close: Vec<DeclRef> = per_iteration
                        .iter()
                        .rev()
                        .copied()
                        .filter(|&decl| self.captured(decl) && self.vars.contains_key(&decl))
                        .collect();
                    self.close_cells(&close);
                }
                let _ = scoped;
                let exit = self.block();
                self.loop_depth += 1;
                let test_block = test.as_ref().map(|_| self.block());
                if let Some(test_block) = test_block {
                    self.enter(test_block);
                }
                let body_block = self.block();
                if let Some(test) = test {
                    let cond = self.expr(test)?;
                    self.end(Terminator::Branch {
                        cond,
                        then: body_block,
                        else_: exit,
                    });
                }
                let update_block = update.as_ref().map(|_| self.block());
                let continue_to = update_block.or(test_block).unwrap_or(body_block);
                self.enter(body_block);
                self.loop_body(*label, exit, Some(continue_to), None, body)?;
                // The head bindings' cells close on the normal path only; a
                // `continue` skips this, as in QuickJS.
                let close: Vec<DeclRef> = per_iteration
                    .iter()
                    .rev()
                    .copied()
                    .filter(|&decl| self.captured(decl) && self.vars.contains_key(&decl))
                    .collect();
                self.close_cells(&close);
                if let Some(update_block) = update_block {
                    self.enter(update_block);
                    self.expr(update.as_deref().expect("an update"))?;
                }
                self.goto(test_block.unwrap_or(body_block));
                self.loop_depth -= 1;
                self.start(exit);
            }
            StmtKind::ForIn {
                target,
                init,
                obj,
                body,
                label,
            } => self.for_in(target, init.as_deref(), obj, body, *label, scoped)?,
            StmtKind::ForOf {
                target,
                iter,
                body,
                label,
                await_,
                ..
            } => {
                self.for_of(target, iter, body, *label, scoped, *await_)?;
            }
            _ => return internal("not a loop"),
        }
        Ok(())
    }

    fn loop_body(
        &mut self,
        label: u32,
        break_to: BlockId,
        continue_to: Option<BlockId>,
        iterator: Option<Value>,
        body: &Stmt,
    ) -> Result<()> {
        self.loop_body_at(
            label,
            break_to,
            continue_to,
            iterator,
            body,
            self.scopes.len(),
        )
    }

    fn loop_body_at(
        &mut self,
        label: u32,
        break_to: BlockId,
        continue_to: Option<BlockId>,
        iterator: Option<Value>,
        body: &Stmt,
        scope_depth: usize,
    ) -> Result<()> {
        self.controls.push(Control::Breakable {
            label,
            break_to,
            continue_to,
            iterator,
            scope_depth,
        });
        let result = self.statement(body);
        self.controls.pop();
        result
    }

    /// The head scopes' captured bindings, closed after the head's
    /// expression and after each iteration.
    fn head_closures(&mut self, scoped: bool) {
        if scoped && let Some(close) = self.scopes.last().cloned() {
            self.close_cells(&close);
        }
    }

    fn for_in(
        &mut self,
        target: &ForTarget,
        init: Option<&hir::Expr>,
        obj: &hir::Expr,
        body: &Stmt,
        label: u32,
        scoped: bool,
    ) -> Result<()> {
        self.reset_completion();
        let outer_depth = self.scopes.len() - usize::from(scoped);
        let expr_block = self.block();
        let assign = self.block();
        self.goto(expr_block);
        self.loop_depth += 1;
        let body_block = self.block();
        let next = self.block();
        let exit = self.block();
        // The assignment fragment comes first, as in QuickJS; its value is
        // the key `ForInNext` defines on the edge into it.
        let key = self.f.new_value(Default::default());
        self.start(assign);
        self.for_target(target, key)?;
        self.goto(body_block);
        self.loop_depth -= 1;
        self.start(expr_block);
        if let Some(init) = init {
            let value = self.expr(init)?;
            let ForTarget::Declare { pattern } = target else {
                return internal("a for-in initializer without a declaration");
            };
            let PatternKind::Decl { decl } = pattern.kind else {
                return internal("a for-in initializer with a pattern");
            };
            self.put_decl(decl, value)?;
        }
        let object = self.expr(obj)?;
        self.head_closures(scoped);
        let iter = self.op(Op::ForInStart, &[object], Imm::None);
        self.goto(next);
        self.loop_depth += 1;
        self.start(body_block);
        self.loop_body_at(label, exit, Some(next), None, body, outer_depth)?;
        self.head_closures(scoped);
        self.enter(next);
        self.end(Terminator::ForInNext {
            iter,
            key,
            then: assign,
            else_: exit,
        });
        self.loop_depth -= 1;
        self.start(exit);
        Ok(())
    }

    fn for_of(
        &mut self,
        target: &ForTarget,
        iterable: &hir::Expr,
        body: &Stmt,
        label: u32,
        scoped: bool,
        asynchronous: bool,
    ) -> Result<()> {
        self.reset_completion();
        let outer = self.handler;
        let outer_depth = self.scopes.len() - usize::from(scoped);
        let expr_block = self.block();
        let exit = self.block();
        // The iterator's region: a throw inside closes it and rethrows.
        let close = self.block();
        let handler = self.open_handler(close);
        self.loop_depth += 1;
        let assign = self.block();
        let body_block = self.block();
        let next = self.block();
        self.loop_depth -= 1;
        self.close_handler(handler);
        self.goto(expr_block);
        // The record and the value `IterNext` produces are defined after the
        // fragment in emission order but dominate it.
        let record = self.f.new_value(Default::default());
        self.record(record);
        let value = self.f.new_value(Default::default());
        let done = self.f.new_value(Default::default());
        // The assignment fragment first, as QuickJS lays it out.
        self.handler = Some(handler);
        self.loop_depth += 1;
        self.start(assign);
        self.controls.push(Control::Fragment);
        self.for_target(target, value)?;
        self.controls.pop();
        self.goto(body_block);
        self.loop_depth -= 1;
        self.handler = outer;
        self.start(expr_block);
        let iterable = self.expr(iterable)?;
        self.head_closures(scoped);
        if asynchronous {
            self.define_with(Op::IterStartAsync, &[iterable], Imm::None, &[record]);
        } else {
            self.define_with(Op::IterStart, &[iterable], Imm::Int(0), &[record]);
        }
        let opened = self.site;
        self.goto(next);
        self.handler = Some(handler);
        self.loop_depth += 1;
        self.start(body_block);
        self.loop_body_at(label, exit, Some(next), Some(record), body, outer_depth)?;
        self.head_closures(scoped);
        self.enter(next);
        if asynchronous {
            let raw = self.op(Op::IterNextRaw, &[record], Imm::None);
            let settled = self.await_value(raw, None);
            self.define_with(
                Op::IterResult,
                &[settled, record],
                Imm::None,
                &[value, done],
            );
        } else {
            self.define_with(Op::IterNext, &[record], Imm::None, &[value, done]);
        }
        self.end(Terminator::Branch {
            cond: done,
            then: exit,
            else_: assign,
        });
        self.loop_depth -= 1;
        self.handler = outer;
        self.start(exit);
        self.effect(Op::IterClose, &[record], Imm::None);
        self.close_stub(close, record, opened);
        Ok(())
    }

    /// Emits `op` defining the given, already allocated values.
    pub(super) fn define_with(&mut self, op: Op, args: &[Value], imm: Imm, results: &[Value]) {
        let pos = self.mark(None);
        let Some(cur) = self.cur else {
            return;
        };
        let mut slots = [None, None];
        for (slot, &value) in slots.iter_mut().zip(results) {
            *slot = Some(value);
        }
        let insns = &mut self.f.blocks[cur.index()].insns;
        self.last = Some((cur, insns.len()));
        insns.push(crate::engine::mir::ir::Insn {
            op,
            args: crate::engine::mir::ir::Args::from_slice(args),
            results: slots,
            imm,
            pos,
        });
    }

    /// Assigns the value of an iteration to a for-in/of head.
    fn for_target(&mut self, target: &ForTarget, value: Value) -> Result<()> {
        match target {
            ForTarget::Declare { pattern } => match pattern.kind {
                PatternKind::Decl { decl } => {
                    self.mark(Some(pattern.span.start));
                    if self.decl(decl).kind == DeclKind::Var {
                        self.put_decl(decl, value)
                    } else {
                        self.init_binding(decl, value)
                    }
                }
                _ => {
                    let mut decls = Vec::new();
                    Self::pattern_decls(pattern, &mut decls);
                    let var = decls.first().is_some_and(|&decl| self.is_var(decl));
                    self.destructure(pattern, value, Mode::Declare { var, catch: false })
                }
            },
            ForTarget::Assign { t } => match &t.kind {
                TargetKind::Decl { decl } => self.put_decl(*decl, value),
                TargetKind::Member { .. } => self.assign_member_value(t, value),
                TargetKind::Pattern { p } => self.destructure(p, value, Mode::Assign),
                TargetKind::SuperMember { prop, this, home } => {
                    let place = self.super_place(prop, *this, *home, t.span.start)?;
                    self.set_place(place, value)
                }
            },
        }
    }

    // ---- switch ----

    fn switch_statement(
        &mut self,
        disc: &hir::Expr,
        cases: &[hir::SwitchCase],
        label: u32,
    ) -> Result<()> {
        self.reset_completion();
        let value = self.expr(disc)?;
        let exit = self.block();
        self.controls.push(Control::Breakable {
            label,
            break_to: exit,
            continue_to: None,
            iterator: None,
            scope_depth: self.scopes.len(),
        });
        // The block whose failed test goes to the next test, the default or
        // the exit; and the default's body.
        let mut pending: Option<BlockId> = None;
        let mut default_body: Option<BlockId> = None;
        let mut index = 0;
        while index < cases.len() {
            let case = &cases[index];
            match &case.test {
                Some(_) => {
                    let fallthrough = if pending.is_some() {
                        self.cur.take()
                    } else {
                        None
                    };
                    let test_block = self.block();
                    match pending.take() {
                        Some(pending) => self.set_else(pending, test_block),
                        // The first test, entered from the discriminant.
                        None => self.goto(test_block),
                    }
                    self.start(test_block);
                    let body_block = self.block();
                    // Consecutive tests of an empty case share a body.
                    loop {
                        let test = cases[index].test.as_deref().expect("a test");
                        let tested = self.expr(test)?;
                        let matched = self.op(Op::StrictEq, &[value, tested], Imm::None);
                        let chained = cases[index].body.is_empty()
                            && cases.get(index + 1).is_some_and(|next| next.test.is_some());
                        if chained {
                            let next_test = self.block();
                            self.end(Terminator::Branch {
                                cond: matched,
                                then: body_block,
                                else_: next_test,
                            });
                            self.start(next_test);
                            index += 1;
                        } else {
                            let test_end = self.cur;
                            self.end(Terminator::Branch {
                                cond: matched,
                                then: body_block,
                                else_: body_block,
                            });
                            pending = test_end;
                            break;
                        }
                    }
                    if let Some(fallthrough) = fallthrough {
                        self.cur = Some(fallthrough);
                        self.goto(body_block);
                    }
                    self.start(body_block);
                }
                None => {
                    if pending.is_none() {
                        // The tests before failed to the default.
                        pending = self.cur.take();
                        if let Some(p) = pending {
                            self.f.block_mut(p).term = Terminator::Jump(p);
                        }
                    }
                    let body = self.block();
                    self.enter(body);
                    default_body = Some(body);
                }
            }
            self.statements(&cases[index].body)?;
            index += 1;
        }
        let no_match = default_body.unwrap_or(exit);
        if let Some(pending) = pending {
            self.set_else(pending, no_match);
        }
        self.controls.pop();
        self.enter(exit);
        Ok(())
    }

    /// Points the failure edge of `block` (a branch, or a placeholder
    /// jump) at `target`.
    fn set_else(&mut self, block: BlockId, target: BlockId) {
        match &mut self.f.block_mut(block).term {
            Terminator::Branch { else_, .. } => *else_ = target,
            Terminator::Jump(to) => *to = target,
            _ => {}
        }
    }

    // ---- jumps ----

    fn jump(&mut self, label: u32, exit: Exit) -> Result<()> {
        let Some(target) = self.controls.iter().rposition(
            |control| matches!(control, Control::Breakable { label: l, .. } if *l == label),
        ) else {
            return internal(format!("no statement has label {label}"));
        };
        let saved = self.handler;
        for index in (target + 1..self.controls.len()).rev() {
            let control = self.controls[index].clone();
            let depth = self.control_scope_depth(index);
            self.close_scopes_to(depth);
            self.unwind(&control, false);
        }
        let Control::Breakable {
            break_to,
            continue_to,
            scope_depth,
            ..
        } = self.controls[target].clone()
        else {
            unreachable!("found a breakable control");
        };
        self.close_scopes_to(scope_depth);
        let to = match exit {
            Exit::Break => break_to,
            Exit::Continue => match continue_to {
                Some(to) => to,
                None => return internal("continue to a statement that is not a loop"),
            },
        };
        self.goto(to);
        self.handler = saved;
        Ok(())
    }

    /// The scope depth a control was entered at.
    fn control_scope_depth(&self, index: usize) -> usize {
        // Controls other than breakable statements do not track scopes:
        // they close nothing of their own.
        for control in self.controls[..=index].iter().rev() {
            if let Control::Breakable { scope_depth, .. } = control {
                return *scope_depth;
            }
        }
        0
    }

    /// The cleanup of leaving `control` abruptly: `for_return` for a
    /// `return`, which closes iterators the same way but keeps the value.
    fn unwind(&mut self, control: &Control, for_return: bool) {
        match control {
            Control::Finally { finally } => {
                let finally = *finally;
                // Leaving the try region first.
                if let Some(h) = self.handler {
                    self.handler = self.f.handlers[h.index()].parent;
                }
                let cont = self.block();
                self.end(Terminator::Gosub { finally, cont });
                self.start(cont);
            }
            Control::FinallyBody => {}
            Control::Breakable {
                iterator: Some(record),
                ..
            }
            | Control::Iterator { record } => {
                let record = *record;
                self.effect(Op::IterClose, &[record], Imm::None);
                if let Some(h) = self.handler {
                    self.handler = self.f.handlers[h.index()].parent;
                }
                self.rebase();
            }
            Control::Fragment => {
                if for_return && let Some(h) = self.handler {
                    self.handler = self.f.handlers[h.index()].parent;
                    self.rebase();
                }
            }
            Control::Breakable { .. } => {}
        }
    }

    /// QuickJS's return path for `value` (`undefined` when `None`): an
    /// async generator awaits it, the open iterators close and the
    /// `finally` bodies run, innermost first, then the function returns.
    pub(super) fn return_completion(
        &mut self,
        value: Option<Value>,
        at: u32,
        awaited: bool,
    ) -> Result<()> {
        let mut value = match value {
            Some(value) => value,
            None => self.undefined(),
        };
        if awaited && self.async_generator() {
            value = self.await_value(value, Some(at));
        }
        let saved = self.handler;
        let controls = self.controls.clone();
        for control in controls.iter().rev() {
            match control {
                Control::Breakable { iterator: None, .. } | Control::FinallyBody => {}
                Control::Breakable {
                    iterator: Some(record),
                    ..
                }
                | Control::Iterator { record }
                    if self.async_generator() =>
                {
                    // The value survives the awaited close in a variable.
                    let var = self.new_var(VarKind::Local, None);
                    self.set_var(var, value);
                    self.async_iterator_return(*record)?;
                    if let Some(h) = self.handler {
                        self.handler = self.f.handlers[h.index()].parent;
                    }
                    self.rebase();
                    value = self.get_var(var);
                }
                _ => {
                    // The value survives the cleanup in a variable when a
                    // finally body may run.
                    if matches!(control, Control::Finally { .. }) {
                        let var = self.new_var(VarKind::Local, None);
                        self.set_var(var, value);
                        self.unwind(control, true);
                        value = self.get_var(var);
                    } else {
                        self.unwind(control, true);
                    }
                }
            }
        }
        if self.hf.kind == hir::FunctionKind::DerivedConstructor {
            self.derived_return(value, Some(at))?;
        } else {
            self.end_at(Terminator::Return(value), at);
        }
        self.handler = saved;
        Ok(())
    }

    // ---- try ----

    fn try_statement(
        &mut self,
        body: &Block,
        catch_param: Option<&hir::Pattern>,
        catch_default: Option<&hir::Expr>,
        catch_body: Option<&Block>,
        finally: Option<&Block>,
    ) -> Result<()> {
        self.reset_completion();
        let outer = self.handler;
        let end = self.block();
        let finally_block = finally.map(|_| {
            let block = self.block();
            let addr = self.f.new_value(Default::default());
            self.f.block_mut(block).ret_addr = Some(addr);
            block
        });
        // The try body.
        let catch_block = self.block();
        let handler = self.open_handler(catch_block);
        let try_block = self.block();
        self.enter(try_block);
        if let Some(finally) = finally_block {
            self.controls.push(Control::Finally { finally });
        }
        self.block_statement(body)?;
        if finally_block.is_some() {
            self.controls.pop();
        }
        self.close_handler(handler);
        self.leave_try(finally_block, end);
        // The catch.
        self.start(catch_block);
        let thrown = self.f.new_value(Default::default());
        self.f.block_mut(catch_block).catch = Some(thrown);
        match catch_body {
            Some(catch_body) => {
                // The catch parameter's bindings start as `undefined`; the
                // body's own bindings enter their scope after them.
                let mut bound = Vec::new();
                if let Some(pattern) = catch_param {
                    Self::pattern_decls(pattern, &mut bound);
                }
                let (params, own): (Vec<DeclRef>, Vec<DeclRef>) =
                    catch_body.decls.iter().partition(|&&decl| {
                        self.decl(decl).kind == DeclKind::Catch || bound.contains(&decl)
                    });
                let mut close_params = Vec::new();
                for &decl in params.iter().rev() {
                    if self.vars.contains_key(&decl) {
                        let undefined = self.undefined();
                        self.init_binding(decl, undefined)?;
                        if self.captured(decl) {
                            close_params.push(decl);
                        }
                    }
                }
                match catch_param {
                    Some(hir::Pattern {
                        kind: PatternKind::Decl { decl },
                        span,
                        ..
                    }) => {
                        self.mark(Some(span.start));
                        self.init_binding(*decl, thrown)?;
                    }
                    Some(pattern) => self.nested_element(
                        pattern,
                        thrown,
                        catch_default,
                        Mode::Declare {
                            var: false,
                            catch: true,
                        },
                    )?,
                    None => {}
                }
                self.scopes.push(close_params);
                let rethrow = self.block();
                let inner = self.open_handler(rethrow);
                let catch_entry = self.block();
                self.enter(catch_entry);
                if let Some(finally) = finally_block {
                    self.controls.push(Control::Finally { finally });
                }
                self.enter_scope(&own, &catch_body.stmts)?;
                self.statements(&catch_body.stmts)?;
                self.leave_scope();
                if finally_block.is_some() {
                    self.controls.pop();
                }
                self.leave_scope();
                self.close_handler(inner);
                self.leave_try(finally_block, end);
                // A throw from the catch body runs the finally and rethrows.
                self.start(rethrow);
                let again = self.f.new_value(Default::default());
                self.f.block_mut(rethrow).catch = Some(again);
                self.run_finally(finally_block);
                self.end(Terminator::Throw(again));
            }
            None => {
                self.run_finally(finally_block);
                self.end(Terminator::Throw(thrown));
            }
        }
        // The finally subroutine.
        if let (Some(finally), Some(block_body)) = (finally_block, finally) {
            self.start(finally);
            let addr = self.f.block(finally).ret_addr.expect("a return address");
            self.controls.push(Control::FinallyBody);
            let saved = self.completion.map(|completion| {
                let value = self.get_var(completion);
                let saved = self.new_var(VarKind::Local, None);
                self.set_var(saved, value);
                let undefined = self.undefined();
                self.set_var(completion, undefined);
                saved
            });
            self.block_statement(block_body)?;
            if let (Some(saved), Some(completion)) = (saved, self.completion) {
                let value = self.get_var(saved);
                self.set_var(completion, value);
            }
            self.controls.pop();
            self.end(Terminator::Ret(addr));
        }
        self.handler = outer;
        self.start(end);
        Ok(())
    }

    /// The normal exit of a try or catch body: through the finally to `end`.
    fn leave_try(&mut self, finally: Option<BlockId>, end: BlockId) {
        self.run_finally(finally);
        self.goto(end);
    }

    fn run_finally(&mut self, finally: Option<BlockId>) {
        if let Some(finally) = finally
            && self.reachable()
        {
            let cont = self.block();
            self.end(Terminator::Gosub { finally, cont });
            self.start(cont);
        }
    }
}
