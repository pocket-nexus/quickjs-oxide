//! Classes, as QuickJS evaluates a class definition: the heritage, the
//! constructor and `MakeClass`, the elements in order, the aggregate
//! instance initializer, the inner name, then the static initializer.
//! Private names are variables of the enclosing function; `super` reads go
//! through the home object.

use super::{Access, Builder, Result, internal};
use crate::engine::hir::schema::{
    Class, ClassMember, DeclKind, DeclRef, Expr, FieldKey, FunctionKind, MethodRole, PropKey,
};
use crate::engine::mir::ir::{Imm, Value};
use crate::engine::mir::op::Op;
use oxide_core::bytecode::opcode::method;

impl Builder<'_, '_> {
    /// Evaluates `cls`; `inferred` names an anonymous class (NamedEvaluation).
    pub(super) fn class(&mut self, cls: &Class, inferred: Option<u32>) -> Result<Value> {
        // The class scope: the inner name, the private names and the
        // computed field keys start in the TDZ each time the class is
        // evaluated.
        let mut scope = Vec::new();
        scope.extend(cls.name);
        for member in &cls.members {
            match member {
                ClassMember::PrivateMethod { decl, .. }
                | ClassMember::PrivateField { decl, .. }
                | ClassMember::FieldKey { decl, .. } => {
                    if !scope.contains(decl) {
                        scope.push(*decl);
                    }
                }
                ClassMember::Method { .. } => {}
            }
        }
        self.enter_class_scope(&scope)?;
        let parent = match &cls.extends {
            Some(heritage) => self.expr(heritage)?,
            None => self.undefined(),
        };
        let ctor = self.closure(cls.ctor);
        let name = match cls.name {
            Some(decl) => self.decl(decl).name.0,
            None => match inferred {
                Some(name) if self.is_default_export_name(name) => name,
                _ => self.static_key(""),
            },
        };
        let (class, proto) = self.op2(
            Op::MakeClass,
            &[parent, ctor],
            Imm::KeyFlags(name, u32::from(cls.extends.is_some())),
        );
        let has_private_instance = cls
            .members
            .iter()
            .any(|member| matches!(member, ClassMember::PrivateMethod { static_: false, .. }));
        let has_private_static = cls
            .members
            .iter()
            .any(|member| matches!(member, ClassMember::PrivateMethod { static_: true, .. }));
        for member in &cls.members {
            match member {
                ClassMember::Method {
                    key,
                    fn_,
                    static_,
                    role,
                } => {
                    let target = if *static_ { class } else { proto };
                    let flags = match role {
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
                                &[target, closure],
                                Imm::KeyFlags(name.0, flags),
                            );
                        }
                        PropKey::Computed { e } => {
                            let key = self.expr(e)?;
                            let key = self.op(Op::ToPropertyKey, &[key], Imm::None);
                            let closure = self.closure(*fn_);
                            self.effect(
                                Op::DefineMethodComputed,
                                &[target, key, closure],
                                Imm::Flags(flags),
                            );
                        }
                        PropKey::Private { .. } => return internal("a private key on a method"),
                    }
                }
                ClassMember::PrivateMethod {
                    decl,
                    fn_,
                    static_,
                    role,
                } => {
                    let closure = self.closure(*fn_);
                    let home = if *static_ { class } else { proto };
                    let at = self.decl(*decl).span.start;
                    self.effect_at(Op::SetHomeObject, &[closure, home], Imm::None, at);
                    let prefix = match role {
                        MethodRole::Method => 0,
                        MethodRole::Get => 1,
                        MethodRole::Set => 2,
                    };
                    let key = self.decl(*decl).name.0;
                    self.effect(
                        Op::NamePrivateMethod,
                        &[closure],
                        Imm::KeyFlags(key, prefix),
                    );
                    let setter_half = *role == MethodRole::Set
                        && self.decl(*decl).kind == DeclKind::PrivateAccessor;
                    self.init_private(*decl, setter_half, closure)?;
                }
                ClassMember::PrivateField { decl, .. } => {
                    let at = self.decl(*decl).span.start;
                    let key = self.decl(*decl).name.0;
                    let name = self.op_at(Op::NewPrivateName, &[], Imm::Key(key), at);
                    self.init_private(*decl, false, name)?;
                }
                ClassMember::FieldKey { key, decl, .. } => {
                    let value = self.expr(key)?;
                    let value = self.op(Op::ToPropertyKey, &[value], Imm::None);
                    self.mark(Some(self.decl(*decl).span.start));
                    self.init_binding(*decl, value)?;
                }
            }
        }
        if let Some(init) = cls.instance_init {
            if has_private_instance {
                self.cx.brand[init.index()] = true;
            }
            let initializer = self.closure(init);
            self.effect(Op::InstallFields, &[class, proto, initializer], Imm::None);
        }
        if let Some(decl) = cls.name {
            self.init_binding(decl, class)?;
        }
        if cls.name.is_none()
            && let Some(name) = inferred
        {
            let name = self.string(name);
            self.effect(Op::SetFunctionName, &[class, name], Imm::None);
        }
        if let Some(init) = cls.static_init {
            if has_private_static {
                self.cx.brand[init.index()] = true;
            }
            let initializer = self.closure(init);
            self.effect(Op::RunStaticInit, &[class, initializer], Imm::None);
        }
        self.leave_scope();
        Ok(class)
    }

    fn is_default_export_name(&self, _name: u32) -> bool {
        false
    }

    /// The class scope's entry: like a block's, for bindings that are not
    /// in any block.
    fn enter_class_scope(&mut self, decls: &[DeclRef]) -> Result<()> {
        let mut close = Vec::new();
        for &decl in decls.iter().rev() {
            let Some(&var) = self.vars.get(&decl) else {
                continue;
            };
            let captured = self.captured(decl);
            let private = self.decl(decl).kind.is_private();
            if captured {
                let cell = self.get_var(var);
                self.effect(Op::ClearCell, &[cell], Imm::None);
                if let Some(&setter) = self.setter_vars.get(&decl) {
                    let cell = self.get_var(setter);
                    self.effect(Op::ClearCell, &[cell], Imm::None);
                }
                close.push(decl);
            } else if self.decl(decl).tdz || private {
                let empty = self.op(Op::Empty, &[], Imm::None);
                self.set_var(var, empty);
            }
        }
        self.checked.clear();
        self.scopes.push(close);
        Ok(())
    }

    /// Stores a private name's value (a field's private name, a method or
    /// accessor function) in its variable.
    fn init_private(&mut self, decl: DeclRef, setter: bool, value: Value) -> Result<()> {
        let var = if setter {
            self.setter_vars.get(&decl).copied()
        } else {
            self.vars.get(&decl).copied()
        };
        let Some(var) = var else {
            return internal("a private name without its variable");
        };
        if self.captured(decl) {
            let cell = self.get_var(var);
            self.effect(Op::StoreCell, &[cell, value], Imm::None);
        } else {
            self.set_var(var, value);
        }
        Ok(())
    }

    /// The value a private name holds: its private name, method, getter or
    /// (`setter`) setter.
    pub(super) fn read_private(&mut self, decl: DeclRef, setter: bool) -> Result<Value> {
        let local = if setter {
            self.setter_vars.get(&decl).copied()
        } else {
            self.vars.get(&decl).copied()
        };
        if let Some(var) = local {
            let value = self.get_var(var);
            if self.captured(decl) {
                return Ok(self.op(Op::LoadCell, &[value], Imm::None));
            }
            return Ok(value);
        }
        match self.captures.get(&(decl, setter)) {
            Some(&index) => Ok(self.op(Op::LoadCaptured, &[], Imm::Capture(index))),
            None => internal(format!("private name #{} is not reachable", decl.0)),
        }
    }

    /// `obj.#name`.
    pub(super) fn get_private(&mut self, obj: Value, decl: DeclRef, at: u32) -> Result<Value> {
        let kind = self.decl(decl).kind;
        Ok(match kind {
            DeclKind::PrivateField => {
                let name = self.read_private(decl, false)?;
                self.op_at(Op::GetPrivate, &[obj, name], Imm::None, at)
            }
            DeclKind::PrivateMethod => {
                let function = self.read_private(decl, false)?;
                self.op_at(Op::GetPrivateMethod, &[obj, function], Imm::None, at)
            }
            DeclKind::PrivateGetter | DeclKind::PrivateAccessor => {
                let getter = self.read_private(decl, false)?;
                self.op_at(Op::CallPrivateGetter, &[obj, getter], Imm::None, at)
            }
            DeclKind::PrivateSetter => {
                self.read_only_private(decl, at);
                self.f.new_value(Default::default())
            }
            _ => return internal("not a private name"),
        })
    }

    /// `obj.#name = value`.
    pub(super) fn put_private(
        &mut self,
        obj: Value,
        decl: DeclRef,
        value: Value,
        at: u32,
    ) -> Result<()> {
        let kind = self.decl(decl).kind;
        match kind {
            DeclKind::PrivateField => {
                let name = self.read_private(decl, false)?;
                self.effect_at(Op::PutPrivate, &[obj, value, name], Imm::None, at);
            }
            DeclKind::PrivateSetter => {
                let setter = self.read_private(decl, false)?;
                self.effect_at(Op::CallPrivateSetter, &[obj, value, setter], Imm::None, at);
            }
            DeclKind::PrivateAccessor => {
                let setter = self.read_private(decl, true)?;
                self.effect_at(Op::CallPrivateSetter, &[obj, value, setter], Imm::None, at);
            }
            DeclKind::PrivateMethod | DeclKind::PrivateGetter => self.read_only_private(decl, at),
            _ => return internal("not a private name"),
        }
        Ok(())
    }

    /// QuickJS's `'#name' is read-only` for a write or read the private
    /// name has no function for.
    fn read_only_private(&mut self, decl: DeclRef, at: u32) {
        let key = self.decl(decl).name.0;
        self.end_at(
            crate::engine::mir::ir::Terminator::ThrowError {
                kind: crate::engine::mir::ir::ThrowKind::ConstAssign,
                key,
            },
            at,
        );
    }

    /// `#name in obj`.
    pub(super) fn private_in(&mut self, decl: DeclRef, obj: Value, at: u32) -> Result<Value> {
        let kind = self.decl(decl).kind;
        Ok(match kind {
            DeclKind::PrivateField => {
                let name = self.read_private(decl, false)?;
                self.op_at(Op::PrivateIn, &[obj, name], Imm::None, at)
            }
            _ => {
                let function = self.read_private(decl, false)?;
                self.op_at(Op::PrivateBrandIn, &[obj, function], Imm::None, at)
            }
        })
    }

    // ---- aggregate initializers ----

    /// `this.key = value` in an instance or static field initializer.
    pub(super) fn define_field(&mut self, key: &FieldKey, value: Option<&Expr>) -> Result<()> {
        let this = self.this_value()?;
        match key {
            FieldKey::Name { name } => {
                let value = match value {
                    Some(value) => self.expr(value)?,
                    None => self.undefined(),
                };
                self.effect(Op::DefineField, &[this, value], Imm::Key(name.0));
            }
            FieldKey::Computed { decl } => {
                let key = self.read_decl(*decl, Some(self.decl(*decl).span.start))?;
                let anonymous = value.is_some_and(|value| self.anonymous_definition(value));
                let value = match value {
                    Some(value) => self.expr(value)?,
                    None => self.undefined(),
                };
                if anonymous {
                    self.effect(Op::SetFunctionName, &[value, key], Imm::None);
                }
                self.effect(Op::DefineElem, &[this, key, value], Imm::None);
            }
        }
        Ok(())
    }

    /// `this.#name = value` defining a private field.
    pub(super) fn define_private(&mut self, decl: DeclRef, value: Option<&Expr>) -> Result<()> {
        let this = self.this_value()?;
        let value = match value {
            Some(value) => self.expr(value)?,
            None => self.undefined(),
        };
        let name = self.read_private(decl, false)?;
        let at = self.decl(decl).span.start;
        self.effect_at(Op::DefinePrivate, &[this, value, name], Imm::None, at);
        Ok(())
    }

    /// The function's own `this`.
    pub(super) fn this_value(&mut self) -> Result<Value> {
        let this = self
            .hf
            .decls
            .iter()
            .copied()
            .find(|&decl| self.decl(decl).kind == DeclKind::This);
        match this {
            Some(decl) => self.read_decl(decl, None),
            None => {
                let var = self.this_register();
                Ok(self.get_var(var))
            }
        }
    }

    /// A variable for register 0.
    pub(super) fn this_register(&mut self) -> u32 {
        if let Some(index) = self
            .f
            .vars
            .iter()
            .position(|var| var.kind == crate::engine::mir::ir::VarKind::This)
        {
            return index as u32;
        }
        self.new_var(crate::engine::mir::ir::VarKind::This, None)
    }

    fn anonymous_definition(&self, e: &Expr) -> bool {
        use crate::engine::hir::schema::ExprKind;
        match &e.kind {
            ExprKind::Fn { fn_ } => self.cx.function(*fn_).name.is_none(),
            ExprKind::Class { cls } => cls.name.is_none(),
            _ => false,
        }
    }

    // ---- constructors ----

    /// A class constructor's entry before its parameters: the `new` check,
    /// and in a base class the instance fields.
    pub(super) fn constructor_entry(&mut self) -> Result<()> {
        let register = self.new_target_register();
        let new_target = self.get_var(register);
        self.effect(Op::CheckCtor, &[new_target], Imm::None);
        if self.hf.kind == FunctionKind::Constructor {
            let this = self.this_value()?;
            let function = self.op(Op::Function, &[], Imm::None);
            self.effect(Op::InitFields, &[this, function], Imm::None);
        }
        Ok(())
    }

    /// The synthesized constructor's body: a base class's returns
    /// `undefined`; a derived class's constructs the parent with its own
    /// arguments.
    pub(super) fn default_constructor(&mut self) -> Result<()> {
        if self.hf.kind == FunctionKind::Constructor {
            self.end(crate::engine::mir::ir::Terminator::ReturnUndefined);
            return Ok(());
        }
        let register = self.new_target_register();
        let new_target = self.get_var(register);
        let result = self.op(Op::InitDerived, &[new_target], Imm::None);
        self.f.unit.flags |= oxide_core::bytecode::flags::ARGUMENTS;
        if self.own_this().is_none() {
            // Nothing reads `this`: a variable of its own holds it.
            let var = self.new_var(crate::engine::mir::ir::VarKind::Local, None);
            let empty = self.op(Op::Empty, &[], Imm::None);
            self.set_var(var, empty);
            let bound = self.op(Op::InitThis, &[empty, result], Imm::None);
            self.set_var(var, bound);
            let function = self.active_function()?;
            self.effect(Op::InitFields, &[result, function], Imm::None);
            let this = self.get_var(var);
            let checked = self.op(Op::CheckDerivedReturn, &[result, this], Imm::None);
            self.end(crate::engine::mir::ir::Terminator::Return(checked));
            return Ok(());
        }
        self.bind_this(result)?;
        let function = self.active_function()?;
        self.effect(Op::InitFields, &[result, function], Imm::None);
        self.derived_return(result, None)
    }

    fn own_this(&self) -> Option<DeclRef> {
        self.hf
            .decls
            .iter()
            .copied()
            .find(|&decl| self.decl(decl).kind == DeclKind::This)
    }

    /// The active function: the implicit binding, or a fresh read.
    fn active_function(&mut self) -> Result<Value> {
        let decl = self
            .hf
            .decls
            .iter()
            .copied()
            .find(|&decl| self.decl(decl).kind == DeclKind::ActiveFunction);
        match decl {
            Some(decl) => self.read_decl(decl, None),
            None => Ok(self.op(Op::Function, &[], Imm::None)),
        }
    }

    /// Binds a derived constructor's `this` (`decl` of the function that
    /// owns it, reached from here) to `value`; a second binding throws.
    fn bind_this(&mut self, value: Value) -> Result<()> {
        let decl = self
            .hf
            .decls
            .iter()
            .copied()
            .find(|&decl| self.decl(decl).kind == DeclKind::This);
        match decl {
            Some(decl) => self.bind_this_decl(decl, value),
            None => internal("a derived constructor without its this"),
        }
    }

    fn bind_this_decl(&mut self, decl: DeclRef, value: Value) -> Result<()> {
        match self.access(decl)? {
            Access::Var { var, cell: false } => {
                let current = self.get_var(var);
                let bound = self.op(Op::InitThis, &[current, value], Imm::None);
                self.set_var(var, bound);
            }
            Access::Var { var, cell: true } => {
                let cell = self.get_var(var);
                self.effect(Op::InitThisCell, &[cell, value], Imm::None);
            }
            Access::Captured { index } => {
                self.effect(Op::InitThisCaptured, &[value], Imm::Capture(index));
            }
            Access::Global { .. } => return internal("a global this binding"),
        }
        Ok(())
    }

    /// A derived constructor's return of `value` (`None`: `undefined`):
    /// an object stays, `undefined` becomes `this`, anything else throws.
    pub(super) fn derived_return(&mut self, value: Value, at: Option<u32>) -> Result<()> {
        let decl = self
            .hf
            .decls
            .iter()
            .copied()
            .find(|&decl| self.decl(decl).kind == DeclKind::This);
        let this = match decl {
            Some(decl) => self.read_this_unchecked(decl)?,
            // Nothing binds `this`: it is still unbound.
            None => self.op(Op::Empty, &[], Imm::None),
        };
        let result = self.op(Op::CheckDerivedReturn, &[value, this], Imm::None);
        let term = crate::engine::mir::ir::Terminator::Return(result);
        match at {
            Some(at) => self.end_at(term, at),
            None => self.end(term),
        }
        Ok(())
    }

    /// `this` as `CheckDerivedReturn` reads it: maybe still the TDZ marker.
    fn read_this_unchecked(&mut self, decl: DeclRef) -> Result<Value> {
        Ok(match self.access(decl)? {
            Access::Var { var, cell: false } => self.get_var(var),
            Access::Var { var, cell: true } => {
                let cell = self.get_var(var);
                self.op(Op::LoadCell, &[cell], Imm::None)
            }
            Access::Captured { index } => self.op(Op::LoadCaptured, &[], Imm::Capture(index)),
            Access::Global { .. } => return internal("a global this binding"),
        })
    }

    /// `super(args)`.
    pub(super) fn super_call(
        &mut self,
        args: &[crate::engine::hir::schema::Arg],
        this: DeclRef,
        new_target: DeclRef,
        func: DeclRef,
        at: u32,
    ) -> Result<Value> {
        let function = self.read_decl(func, None)?;
        let parent = self.op(Op::GetSuper, &[function], Imm::None);
        let new_target = self.read_decl(new_target, None)?;
        let args = self.arguments(args)?;
        let result = match args {
            super::expr::Args::Fixed(args) => {
                let mut operands = vec![parent, new_target];
                operands.extend(args);
                self.op_at(Op::New, &operands, Imm::None, at)
            }
            super::expr::Args::Spread(array) => {
                self.op_at(Op::NewSpread, &[parent, new_target, array], Imm::None, at)
            }
        };
        self.bind_this_decl(this, result)?;
        let function = self.read_decl(func, None)?;
        self.effect(Op::InitFields, &[result, function], Imm::None);
        Ok(result)
    }
}
