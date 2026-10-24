//! A function's entry: its variables, QuickJS's prologue, the parameters,
//! and the body with its implicit return.

use super::{Builder, Result, internal, unsupported};
use crate::engine::hir::schema::{self as hir, DeclKind, DeclRef, FunctionKind, PatternKind};
use crate::engine::mir::ir::{Imm, SuspendKind, Terminator, VarKind};
use crate::engine::mir::op::Op;
use oxide_core::bytecode::flags;
use oxide_core::bytecode::opcode::global;

impl Builder<'_, '_> {
    pub(super) fn entry(&mut self) -> Result<()> {
        let hf = self.hf;
        self.unit_info();
        self.allocate_vars()?;
        match hf.kind {
            FunctionKind::Script | FunctionKind::Eval => self.program_entry(),
            FunctionKind::Module => unsupported("modules"),
            _ => self.function_entry(),
        }
    }

    /// The unit fields the HIR decides.
    fn unit_info(&mut self) {
        let hf = self.hf;
        let unit = &mut self.f.unit;
        let mut unit_flags = 0;
        if hf.strict {
            unit_flags |= flags::STRICT;
        }
        match hf.kind {
            FunctionKind::Normal if !hf.async_ && !hf.generator => {
                unit_flags |= flags::CONSTRUCTOR | flags::PROTOTYPE;
            }
            FunctionKind::Normal if hf.generator => unit_flags |= flags::PROTOTYPE,
            FunctionKind::Constructor => {
                unit_flags |= flags::CONSTRUCTOR | flags::CLASS_CONSTRUCTOR
            }
            FunctionKind::DerivedConstructor => {
                unit_flags |= flags::CONSTRUCTOR | flags::CLASS_CONSTRUCTOR | flags::DERIVED;
            }
            FunctionKind::Method | FunctionKind::Getter | FunctionKind::Setter if hf.generator => {
                unit_flags |= flags::PROTOTYPE;
            }
            FunctionKind::Arrow => unit_flags |= flags::ARROW,
            FunctionKind::Script | FunctionKind::Eval => unit_flags |= flags::SCRIPT,
            FunctionKind::Module => unit_flags |= flags::MODULE,
            _ => {}
        }
        if hf.kind == FunctionKind::Eval {
            unit_flags |= flags::EVAL;
        }
        if hf.generator {
            unit_flags |= flags::GENERATOR;
        }
        if hf.async_ {
            unit_flags |= flags::ASYNC;
        }
        unit.flags = unit_flags;
        unit.param_count = hf.params.len() as u16 + u16::from(hf.rest.is_some());
        unit.length = hf.length as u16;
        unit.name = hf.name.map(|name| name.0);
        unit.name_atom = hf.name.map(|name| name.0);
        if matches!(hf.kind, FunctionKind::Script | FunctionKind::Eval) {
            let eval = self.cx.static_string("<eval>");
            unit.name = Some(eval);
            unit.name_atom = Some(eval);
        }
        if !matches!(hf.kind, FunctionKind::Script | FunctionKind::Eval) {
            unit.source = Some((hf.source.start, hf.source.end));
        }
    }

    /// A variable for every binding this function owns that is not global.
    fn allocate_vars(&mut self) -> Result<()> {
        let hf = self.hf;
        for (index, &decl) in hf.captures.iter().enumerate() {
            self.captures.insert(decl, index as u32);
        }
        // The parameters' registers: a simple parameter's binding lives in
        // its argument's register; with a parameter environment the
        // argument's binding is the one the environment's copies to.
        let mut argument_decls = vec![None; hf.params.len()];
        for (index, param) in hf.params.iter().enumerate() {
            if let PatternKind::Decl { decl } = param.pattern.kind {
                let argument = hf
                    .param_copies
                    .iter()
                    .find(|copy| copy.from == decl && self.decl(copy.to).kind == DeclKind::Param)
                    .map_or(decl, |copy| copy.to);
                argument_decls[index] = Some(argument);
            }
        }
        let rest_decl = match hf.rest.as_deref().map(|rest| &rest.kind) {
            Some(PatternKind::Decl { decl }) => Some(
                hf.param_copies
                    .iter()
                    .find(|copy| copy.from == *decl && self.decl(copy.to).kind == DeclKind::Param)
                    .map_or(*decl, |copy| copy.to),
            ),
            _ => None,
        };
        for (index, argument) in argument_decls.iter().enumerate() {
            let var = self.new_var(VarKind::Param(index as u32), *argument);
            if let Some(decl) = argument {
                self.vars.insert(*decl, var);
            }
        }
        if hf.rest.is_some() {
            let var = self.new_var(VarKind::Param(hf.params.len() as u32), rest_decl);
            if let Some(decl) = rest_decl {
                self.vars.insert(decl, var);
            }
        }
        for &decl in &hf.decls {
            if self.vars.contains_key(&decl) || self.is_global(decl) {
                continue;
            }
            let d = self.decl(decl);
            if d.kind.is_private() || d.kind == DeclKind::Global {
                continue;
            }
            let kind = match d.kind {
                // The receiver's own register, unless a closure captures it
                // or it waits for `super()`.
                DeclKind::This
                    if !self.captured(decl) && hf.kind != FunctionKind::DerivedConstructor =>
                {
                    VarKind::This
                }
                _ => VarKind::Local,
            };
            let var = self.new_var(kind, Some(decl));
            self.vars.insert(decl, var);
        }
        Ok(())
    }

    /// Script and eval code: QuickJS's global declaration instantiation,
    /// then the body, returning the completion value.
    fn program_entry(&mut self) -> Result<()> {
        let hf = self.hf;
        let completion = self.new_var(VarKind::Local, None);
        self.completion = Some(completion);
        self.global_declarations()?;
        self.cells_and_tdz(&hf.decls.clone())?;
        self.hoist_functions(&hf.body.stmts)?;
        self.statements(&hf.body.stmts)?;
        if self.reachable() {
            let value = self.get_var(completion);
            self.end(Terminator::Return(value));
        }
        Ok(())
    }

    /// GlobalDeclarationInstantiation's checks (16.1.7 steps 3, 4, 10 and
    /// 11) in QuickJS's passes, then the declarations.
    fn global_declarations(&mut self) -> Result<()> {
        let hf = self.hf;
        let eval = if hf.kind == FunctionKind::Eval {
            global::EVAL
        } else {
            0
        };
        // (opcode, flags, key) in declaration order.
        let mut declarations = Vec::new();
        for &decl in &hf.decls {
            if !self.is_global(decl) {
                continue;
            }
            let d = self.decl(decl);
            let (op, flags) = match d.kind {
                DeclKind::Var => (Op::DeclareGlobal, eval),
                DeclKind::Function => (Op::DeclareGlobal, eval | global::FUNCTION),
                DeclKind::Let | DeclKind::Class => (Op::DeclareGlobalLet, eval),
                DeclKind::Const => (Op::DeclareGlobalConst, eval),
                _ => continue,
            };
            declarations.push((op, u32::from(flags), d.name.0));
        }
        type Select = fn(Op, u32) -> bool;
        let passes: [(Select, Op, u32); 4] = [
            (|op, _| op != Op::DeclareGlobal, Op::CheckGlobalLexical, 0),
            (
                |op, _| op == Op::DeclareGlobal,
                Op::CheckGlobalVar,
                u32::from(global::LEXICAL_CONFLICT),
            ),
            (
                |op, f| op == Op::DeclareGlobal && f & u32::from(global::FUNCTION) != 0,
                Op::CheckGlobalVar,
                u32::from(global::DEFINABLE),
            ),
            (
                |op, f| op == Op::DeclareGlobal && f & u32::from(global::FUNCTION) == 0,
                Op::CheckGlobalVar,
                u32::from(global::DEFINABLE),
            ),
        ];
        for (select, check, extra) in passes {
            for &(op, flags, key) in &declarations {
                if !select(op, flags) {
                    continue;
                }
                if check == Op::CheckGlobalLexical {
                    self.effect(check, &[], Imm::Key(key));
                } else {
                    self.effect(check, &[], Imm::KeyFlags(key, flags | extra));
                }
            }
        }
        for (op, flags, key) in declarations {
            let imm = if op == Op::DeclareGlobal {
                Imm::KeyFlags(key, flags)
            } else {
                Imm::Key(key)
            };
            self.effect(op, &[], imm);
        }
        Ok(())
    }

    /// A function's prologue and body.
    fn function_entry(&mut self) -> Result<()> {
        let hf = self.hf;
        if hf.async_ && !hf.generator {
            self.effect(Op::AsyncStart, &[], Imm::None);
        }
        // The implicit bindings QuickJS keeps in locals.
        let mut implicit = Vec::new();
        for &decl in &hf.decls {
            let d = self.decl(decl);
            match d.kind {
                DeclKind::FunctionName => implicit.push((decl, Op::Function)),
                DeclKind::HomeObject => implicit.push((decl, Op::HomeObject)),
                DeclKind::ActiveFunction => implicit.push((decl, Op::Function)),
                _ => {}
            }
        }
        for (decl, op) in implicit {
            let value = self.op(op, &[], Imm::None);
            self.init_decl_value(decl, value)?;
        }
        // Captured parameters and the parameters a mapped `arguments`
        // aliases hold cells in their own registers.
        for index in 0..self.f.vars.len() {
            let var = self.f.vars[index];
            if let (VarKind::Param(_), Some(decl)) = (var.kind, var.decl)
                && self.var_is_cell(decl)
            {
                let value = self.get_var(index as u32);
                let cell = self.op(Op::NewCell, &[value], Imm::None);
                self.set_var(index as u32, cell);
            }
        }
        let decls = hf.decls.clone();
        self.cells_and_tdz(&decls)?;
        self.this_and_new_target()?;
        if let Some(arguments) = hf.arguments_decl
            && self.vars.contains_key(&arguments)
            && hf.uses_arguments
        {
            let mapped = i32::from(self.mapped_arguments());
            let value = self.op(Op::NewArguments, &[], Imm::Int(mapped));
            self.init_decl_value(arguments, value)?;
            self.f.unit.flags |= flags::ARGUMENTS;
        }
        if matches!(
            hf.kind,
            FunctionKind::Constructor | FunctionKind::DerivedConstructor
        ) {
            let new_target = self.new_target_register();
            let value = self.get_var(new_target);
            self.effect(Op::CheckCtor, &[], Imm::None);
            let _ = value;
            return unsupported("class constructors");
        }
        self.parameters()?;
        if hf.generator {
            let resume = self.block();
            self.end(Terminator::Suspend {
                kind: SuspendKind::InitialYield,
                value: None,
                sent: None,
                resume_kind: None,
                resume,
            });
            self.start(resume);
        }
        self.hoist_functions(&hf.body.stmts)?;
        if hf.concise {
            let Some(hir::Stmt {
                kind: hir::StmtKind::Return { e: Some(e) },
                ..
            }) = hf.body.stmts.first()
            else {
                return internal("a concise arrow body is not one return");
            };
            let value = self.expr(e)?;
            self.end(Terminator::Return(value));
            return Ok(());
        }
        self.statements(&hf.body.stmts)?;
        if self.reachable() {
            self.end(Terminator::ReturnUndefined);
        }
        Ok(())
    }

    /// The register `new.target` arrives in, flagging the unit to receive it.
    pub(super) fn new_target_register(&mut self) -> u32 {
        if let Some(index) = self
            .f
            .vars
            .iter()
            .position(|var| var.kind == VarKind::NewTarget)
        {
            return index as u32;
        }
        self.f.unit.flags |= flags::NEW_TARGET;
        self.new_var(VarKind::NewTarget, None)
    }

    /// Loads `this` and `new.target` into the cells or locals that hold them.
    fn this_and_new_target(&mut self) -> Result<()> {
        let hf = self.hf;
        for &decl in &hf.decls {
            let d = self.decl(decl);
            match d.kind {
                DeclKind::This if hf.kind != FunctionKind::DerivedConstructor => {
                    if self.captured(decl) {
                        let this = self.new_var(VarKind::This, None);
                        let value = self.get_var(this);
                        self.init_decl_value(decl, value)?;
                    }
                }
                DeclKind::NewTarget => {
                    let register = self.new_target_register();
                    let value = self.get_var(register);
                    self.init_decl_value(decl, value)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Gives a binding of the function its first value: into its cell, or
    /// its variable.
    pub(super) fn init_decl_value(
        &mut self,
        decl: DeclRef,
        value: crate::engine::mir::ir::Value,
    ) -> Result<()> {
        let Some(&var) = self.vars.get(&decl) else {
            return internal(format!("decl #{} has no variable", decl.0));
        };
        if self.var_is_cell(decl) {
            let cell = self.get_var(var);
            self.effect(Op::StoreCell, &[cell, value], Imm::None);
        } else {
            self.set_var(var, value);
        }
        Ok(())
    }

    /// QuickJS's prologue for the bindings of `decls` that live in locals:
    /// a captured one gets its cell (in the TDZ when lexical), an
    /// uncaptured lexical one the TDZ marker.
    pub(super) fn cells_and_tdz(&mut self, decls: &[DeclRef]) -> Result<()> {
        for &decl in decls {
            let Some(&var) = self.vars.get(&decl) else {
                continue;
            };
            if matches!(self.f.vars[var as usize].kind, VarKind::Param(_)) {
                continue;
            }
            let d = self.decl(decl);
            let lexical = lexical(d.kind);
            if self.captured(decl) {
                let cell = if lexical {
                    self.op(Op::NewCellEmpty, &[], Imm::None)
                } else {
                    let value = self.get_var(var);
                    self.op(Op::NewCell, &[value], Imm::None)
                };
                self.set_var(var, cell);
            } else if lexical && d.tdz {
                let empty = self.op(Op::Empty, &[], Imm::None);
                self.set_var(var, empty);
            }
        }
        Ok(())
    }

    /// The formal parameters: defaults, destructuring and rest, in
    /// QuickJS's order.
    fn parameters(&mut self) -> Result<()> {
        let hf = self.hf;
        if hf.simple_params && hf.rest.is_none() {
            return Ok(());
        }
        let has_environment = !hf.param_copies.is_empty();
        for (index, param) in hf.params.iter().enumerate() {
            let argument = index as u32;
            match (&param.pattern.kind, &param.default) {
                (PatternKind::Decl { decl }, None) => {
                    if has_environment && !self.is_argument_binding(*decl, argument) {
                        let value = self.get_var(argument);
                        self.init_binding(*decl, value)?;
                    }
                }
                (PatternKind::Decl { decl }, Some(default)) => {
                    let value = self.get_var(argument);
                    let undefined = self.undefined();
                    let missing = self.block();
                    let present = self.block();
                    self.end(Terminator::BranchCmp {
                        op: crate::engine::mir::ir::CmpOp::StrictEq,
                        l: value,
                        r: undefined,
                        then: missing,
                        else_: present,
                    });
                    self.start(missing);
                    let defaulted = self.expr(default)?;
                    self.set_var(argument, defaulted);
                    let from_default = self.branch_end();
                    self.start(present);
                    let from_argument = self.branch_end();
                    let value = self.join(vec![(from_default, defaulted), (from_argument, value)]);
                    if !self.is_argument_binding(*decl, argument) {
                        self.init_binding(*decl, value)?;
                    }
                }
                _ => return unsupported("destructuring parameters"),
            }
        }
        if let Some(rest) = &hf.rest {
            let argument = hf.params.len() as u32;
            let value = self.op(Op::Rest, &[], Imm::Int(argument as i32));
            self.f.unit.flags |= flags::ARGUMENTS;
            match rest.kind {
                PatternKind::Decl { decl } => {
                    if self.is_argument_binding(decl, argument) {
                        self.set_var(argument, value);
                    } else {
                        self.set_var(argument, value);
                        self.init_binding(decl, value)?;
                    }
                }
                _ => return unsupported("rest patterns"),
            }
        }
        Ok(())
    }

    /// Whether `decl` is the binding in argument register `argument`.
    fn is_argument_binding(&self, decl: DeclRef, argument: u32) -> bool {
        self.vars.get(&decl) == Some(&argument)
    }
}

/// Whether a binding of `kind` starts in the TDZ.
pub(super) fn lexical(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Let | DeclKind::Const | DeclKind::Class | DeclKind::Hidden | DeclKind::Param
    )
}

impl DeclKind {
    pub(crate) fn is_private(self) -> bool {
        matches!(
            self,
            DeclKind::PrivateField
                | DeclKind::PrivateMethod
                | DeclKind::PrivateGetter
                | DeclKind::PrivateSetter
                | DeclKind::PrivateAccessor
        )
    }
}
