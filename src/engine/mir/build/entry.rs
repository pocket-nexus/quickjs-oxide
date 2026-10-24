//! A function's entry: its variables, QuickJS's prologue, the parameters,
//! and the body with its implicit return.

use super::pattern::Mode;
use super::{Builder, Result, internal};
use crate::engine::hir::schema::{self as hir, DeclKind, DeclRef, FunctionKind, PatternKind};
use crate::engine::mir::ir::{Imm, SuspendKind, Terminator, Value, VarKind};
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
            FunctionKind::Module => self.module_entry(),
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

    /// A variable for every binding this function owns that is not global
    /// or a module binding.
    fn allocate_vars(&mut self) -> Result<()> {
        let hf = self.hf;
        if hf.kind == FunctionKind::Module {
            // The module's bindings are cells its record supplies: the root's
            // captures, in declaration order.
            for &decl in &hf.decls {
                if self.is_module_binding(decl) {
                    let index = self.f.captures.len() as u32;
                    self.f.captures.push((decl, false));
                    self.captures.insert((decl, false), index);
                }
            }
        }
        for &decl in &hf.captures {
            let pair = self.decl(decl).kind == DeclKind::PrivateAccessor;
            for setter in [false, true].into_iter().take(1 + usize::from(pair)) {
                let index = self.f.captures.len() as u32;
                self.f.captures.push((decl, setter));
                self.captures.insert((decl, setter), index);
            }
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
            if self.vars.contains_key(&decl)
                || self.is_global(decl)
                || self.captures.contains_key(&(decl, false))
            {
                continue;
            }
            let d = self.decl(decl);
            if d.kind == DeclKind::Global {
                continue;
            }
            if d.kind == DeclKind::PrivateAccessor {
                let setter = self.new_var(VarKind::Local, Some(decl));
                self.f.vars[setter as usize].setter = true;
                self.setter_vars.insert(decl, setter);
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
        self.this_and_new_target()?;
        self.hoist_functions(&hf.body.stmts)?;
        self.statements(&hf.body.stmts)?;
        if self.reachable() {
            let value = self.get_var(completion);
            self.end(Terminator::Return(value));
        }
        Ok(())
    }

    /// Whether `decl`, of the module function, is a binding of the module
    /// record: an import, `import.meta`, or a top-level declaration.
    fn is_module_binding(&self, decl: DeclRef) -> bool {
        let d = self.decl(decl);
        match d.kind {
            DeclKind::Import | DeclKind::Var | DeclKind::Implicit => true,
            DeclKind::Let | DeclKind::Const | DeclKind::Class | DeclKind::Hidden => {
                self.hf.body.decls.contains(&decl)
            }
            _ => false,
        }
    }

    /// A module's root: the record's tables, then the body. Linking makes
    /// the hoisted bindings, so the body does not.
    fn module_entry(&mut self) -> Result<()> {
        let hf = self.hf;
        let Some(module) = self.cx.hir.modules.first() else {
            return internal("a module function without its module");
        };
        let mut info = super::ModuleInfo {
            has_top_level_await: module.has_top_level_await,
            ..Default::default()
        };
        let binding = |b: &Self, decl: DeclRef| -> Result<u16> {
            match b.captures.get(&(decl, false)) {
                Some(&index) => Ok(index as u16),
                None => internal(format!("decl #{} is not a module binding", decl.0)),
            }
        };
        let imports: Vec<DeclRef> = module.imports.iter().map(|import| import.decl).collect();
        for &(decl, _) in &self.f.captures {
            let d = self.decl(decl);
            info.bindings.push(match d.kind {
                DeclKind::Import => {
                    let named = module
                        .imports
                        .iter()
                        .any(|import| import.decl == decl && import.name.is_some());
                    if named {
                        super::ModuleBinding::Import
                    } else {
                        super::ModuleBinding::Declaration { lexical: true }
                    }
                }
                DeclKind::Implicit => super::ModuleBinding::ImportMeta,
                DeclKind::Var => super::ModuleBinding::Declaration { lexical: false },
                _ => super::ModuleBinding::Declaration { lexical: true },
            });
        }
        let _ = imports;
        for request in &module.requests {
            info.requests.push((
                request.specifier.0,
                request
                    .attributes
                    .iter()
                    .map(|attribute| (attribute.key.0, attribute.value.0))
                    .collect(),
            ));
        }
        for import in &module.imports {
            info.imports.push((
                import.request,
                import.name.map(|name| name.0),
                binding(self, import.decl)?,
            ));
        }
        for reexport in &module.reexports {
            info.exports.push((
                reexport.as_.0,
                super::ExportTarget::Indirect {
                    request: reexport.request,
                    name: reexport.name.map(|name| name.0),
                },
            ));
        }
        for export in &module.exports {
            info.exports.push((
                export.name.0,
                super::ExportTarget::Local(binding(self, export.decl)?),
            ));
        }
        info.star_exports = module.star_exports.clone();
        // Hoisted: vars start as `undefined`, functions as closures.
        let functions = top_level_functions(hf);
        for &decl in &hf.decls {
            let d = self.decl(decl);
            let Some(&index) = self.captures.get(&(decl, false)) else {
                continue;
            };
            if d.kind != DeclKind::Var {
                continue;
            }
            match functions.iter().rev().find(|(f, _)| *f == decl) {
                Some(&(_, function)) => {
                    let child = self.child(function);
                    let anonymous = self.cx.function(function).name.is_none();
                    info.initializers
                        .push((index as u16, Some((child, anonymous))));
                }
                None => info.initializers.push((index as u16, None)),
            }
        }
        self.module = Some(info);
        if hf.async_ {
            self.effect(Op::AsyncStart, &[], Imm::None);
        }
        let decls = hf.decls.clone();
        self.cells_and_tdz(&decls)?;
        self.statements(&hf.body.stmts)?;
        if self.reachable() {
            self.end(Terminator::ReturnUndefined);
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
        let functions = top_level_functions(hf);
        for &decl in &hf.decls {
            if !self.is_global(decl) {
                continue;
            }
            let d = self.decl(decl);
            let (op, flags) = match d.kind {
                DeclKind::Var if functions.iter().any(|(f, _)| *f == decl) => {
                    (Op::DeclareGlobal, eval | global::FUNCTION)
                }
                DeclKind::Var => (Op::DeclareGlobal, eval),
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
        let constructor = matches!(
            hf.kind,
            FunctionKind::Constructor | FunctionKind::DerivedConstructor
        );
        if constructor {
            self.constructor_entry()?;
            if hf.default_ctor {
                return self.default_constructor();
            }
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
            if hf.kind == FunctionKind::DerivedConstructor {
                let undefined = self.undefined();
                self.derived_return(undefined, None)?;
            } else {
                self.end(Terminator::ReturnUndefined);
            }
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
            // A derived constructor's `this` waits for `super()`.
            let unbound_this =
                d.kind == DeclKind::This && self.hf.kind == FunctionKind::DerivedConstructor;
            let lexical = lexical(d.kind) || unbound_this;
            if self.captured(decl) {
                let cell = if lexical {
                    self.op(Op::NewCellEmpty, &[], Imm::None)
                } else {
                    let value = self.get_var(var);
                    self.op(Op::NewCell, &[value], Imm::None)
                };
                self.set_var(var, cell);
            } else if lexical && (d.tdz || unbound_this) {
                let empty = self.op(Op::Empty, &[], Imm::None);
                self.set_var(var, empty);
            }
            if let Some(&setter) = self.setter_vars.get(&decl)
                && self.captured(decl)
            {
                let cell = self.op(Op::NewCellEmpty, &[], Imm::None);
                self.set_var(setter, cell);
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
        // A pattern's bindings: the parameter environment's lexicals, or
        // the function's vars.
        let mode = Mode::Declare {
            var: !has_environment,
            catch: false,
        };
        for (index, param) in hf.params.iter().enumerate() {
            let argument = index as u32;
            match (&param.pattern.kind, &param.default) {
                (PatternKind::Decl { decl }, None) => {
                    if has_environment && !self.is_argument_binding(*decl, argument) {
                        let value = self.argument_value(argument);
                        self.init_binding(*decl, value)?;
                    }
                }
                (PatternKind::Decl { decl }, Some(default)) => {
                    let value = self.argument_value(argument);
                    let undefined = self.undefined();
                    let is_undefined = self.op(Op::StrictEq, &[value, undefined], Imm::None);
                    let missing = self.block();
                    let present = self.block();
                    self.end(Terminator::Branch {
                        cond: is_undefined,
                        then: missing,
                        else_: present,
                    });
                    self.start(missing);
                    let defaulted = self.expr(default)?;
                    self.set_argument(argument, defaulted);
                    let from_default = self.branch_end();
                    self.start(present);
                    let from_argument = self.branch_end();
                    let value = self.join(vec![(from_default, defaulted), (from_argument, value)]);
                    if !self.is_argument_binding(*decl, argument) {
                        self.init_binding(*decl, value)?;
                    }
                }
                (_, default) => {
                    let value = self.argument_value(argument);
                    self.nested_element(&param.pattern, value, default.as_deref(), mode)?;
                }
            }
        }
        if let Some(rest) = &hf.rest {
            let argument = hf.params.len() as u32;
            let value = self.op(Op::Rest, &[], Imm::Int(argument as i32));
            self.f.unit.flags |= flags::ARGUMENTS;
            match rest.kind {
                PatternKind::Decl { decl } => {
                    self.set_argument(argument, value);
                    if !self.is_argument_binding(decl, argument) {
                        self.init_binding(decl, value)?;
                    }
                }
                _ => self.destructure(rest, value, mode)?,
            }
        }
        // A parameter environment's pattern bindings reach the body's, last
        // first.
        for copy in hf.param_copies.iter().rev() {
            if self.decl(copy.to).kind == DeclKind::Param {
                continue;
            }
            let value = self.read_decl(copy.from, None)?;
            self.put_decl(copy.to, value)?;
        }
        Ok(())
    }

    /// The value in argument register `argument`, through its cell.
    fn argument_value(&mut self, argument: u32) -> Value {
        let var = self.f.vars[argument as usize];
        let value = self.get_var(argument);
        match var.decl {
            Some(decl) if self.var_is_cell(decl) => self.op(Op::LoadCell, &[value], Imm::None),
            _ => value,
        }
    }

    fn set_argument(&mut self, argument: u32, value: Value) {
        let var = self.f.vars[argument as usize];
        match var.decl {
            Some(decl) if self.var_is_cell(decl) => {
                let cell = self.get_var(argument);
                self.effect(Op::StoreCell, &[cell, value], Imm::None);
            }
            _ => self.set_var(argument, value),
        }
    }

    /// Whether `decl` is the binding in argument register `argument`.
    fn is_argument_binding(&self, decl: DeclRef, argument: u32) -> bool {
        self.vars.get(&decl) == Some(&argument)
    }
}

/// The function declarations of a body's top level, with their bindings.
fn top_level_functions(hf: &hir::Function) -> Vec<(DeclRef, hir::FunctionRef)> {
    hf.body
        .stmts
        .iter()
        .filter_map(|stmt| match stmt.kind {
            hir::StmtKind::FnDecl { decl, fn_, .. } => Some((decl, fn_)),
            _ => None,
        })
        .collect()
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
