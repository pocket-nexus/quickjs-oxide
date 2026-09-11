//! Eval publication owns topology, names and access modes. Execution validates
//! the caller and actual frame slots before compilation can create captures.
use super::*;

impl RuntimeVmHost {
    pub(super) fn validate_eval_frame_bindings(
        &self,
        environment: &EvalEnvironment<Atom>,
        caller_strict: bool,
    ) -> Result<(), Error> {
        if environment.caller_strict != caller_strict {
            return Err(Error::internal(
                "eval environment caller strictness disagrees with its bytecode frame",
            ));
        }
        for scope in &environment.scopes {
            for binding in &scope.bindings {
                match binding.source {
                    EvalBindingSource::Local(index) => {
                        self.locals.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval local binding index is out of bounds")
                        })?;
                    }
                    EvalBindingSource::Argument(index) => {
                        self.arguments.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval argument binding index is out of bounds")
                        })?;
                    }
                    EvalBindingSource::Closure(index) => {
                        let descriptor = *self
                            .executable
                            .closure_variables
                            .get(usize::from(index))
                            .ok_or_else(|| {
                                Error::internal("eval closure binding index is out of bounds")
                            })?;
                        let root = self.closure_slots.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval closure slot index is out of bounds")
                        })?;
                        self.runtime
                            .validate_var_ref_metadata(root, descriptor)
                            .map_err(|error| Error::internal(error.to_string()))?;
                    }
                }
            }
        }
        Ok(())
    }

    // Synthetic tests have no published owner. Retain the old static rejection
    // contract there; production uses verify_eval_environments and its linked,
    // immutable snapshot rather than revalidating topology on each eval call.
    #[cfg(test)]
    fn validate_eval_definition(
        definition: VariableDefinition,
        binding: &EvalBinding<Atom>,
    ) -> Result<(), Error> {
        if definition.name != Some(binding.name)
            || definition.is_lexical != binding.is_lexical
            || definition.is_const != binding.is_const
            || definition.kind != binding.kind
        {
            return Err(Error::internal(
                "eval binding disagrees with its caller variable definition",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn validate_eval_closure(
        descriptor: ClosureVariable,
        binding: &EvalBinding<Atom>,
    ) -> Result<(), Error> {
        if matches!(
            descriptor.source,
            ClosureSource::GlobalDeclaration
                | ClosureSource::Global
                | ClosureSource::ParentGlobal(_)
        ) {
            return Err(Error::internal(
                "eval environment retained a global closure binding",
            ));
        }
        let name_matches =
            matches!(descriptor.name, ClosureVariableName::Atom(name) if name == binding.name);
        if !name_matches
            || descriptor.is_lexical != binding.is_lexical
            || descriptor.is_const != binding.is_const
            || descriptor.kind != binding.kind
        {
            return Err(Error::internal(
                "eval binding disagrees with its caller closure descriptor",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn validate_eval_environment(
        &self,
        environment: &EvalEnvironment<Atom>,
        caller_strict: bool,
        caller_metadata: FunctionMetadata,
    ) -> Result<(), Error> {
        if environment.caller_strict != caller_strict {
            return Err(Error::internal(
                "eval environment caller strictness disagrees with its bytecode frame",
            ));
        }
        if caller_metadata.super_call_allowed && !caller_metadata.super_allowed {
            return Err(Error::internal(
                "caller bytecode permits super() without SuperProperty",
            ));
        }
        if environment.super_call_allowed && !environment.super_allowed {
            return Err(Error::internal(
                "eval environment permits super() without SuperProperty",
            ));
        }
        if (environment.super_call_allowed, environment.super_allowed)
            != (
                caller_metadata.super_call_allowed,
                caller_metadata.super_allowed,
            )
        {
            return Err(Error::internal(
                "eval environment super capability disagrees with caller bytecode",
            ));
        }
        let first_function_anchor = environment
            .scopes
            .iter()
            .position(|scope| {
                matches!(
                    scope.kind,
                    crate::engine::code::function::metadata::EvalScopeKind::FunctionRoot
                        | crate::engine::code::function::metadata::EvalScopeKind::Parameter
                )
            })
            .and_then(|scope| u16::try_from(scope).ok())
            .ok_or_else(|| {
                Error::internal(
                    "eval environment contains no representable current function anchor",
                )
            })?;
        match environment.variable_environment {
            EvalVariableEnvironment::Global => {
                let current_body_is_program = first_function_anchor
                    .checked_sub(1)
                    .and_then(|scope| environment.scopes.get(usize::from(scope)))
                    .is_some_and(|scope| {
                        scope.kind
                            == crate::engine::code::function::metadata::EvalScopeKind::ProgramBody
                    });
                if caller_metadata.is_module
                    || !current_body_is_program
                    || (caller_strict
                        && caller_metadata.eval_kind
                            != crate::engine::code::function::metadata::EvalKind::None)
                {
                    return Err(Error::internal(
                        "global eval variable environment escaped an authored Script root",
                    ));
                }
            }
            EvalVariableEnvironment::StrictLocal(scope) => {
                if !caller_strict {
                    return Err(Error::internal(
                        "sloppy eval environment selected a strict-local destination",
                    ));
                }
                if scope != first_function_anchor {
                    return Err(Error::internal(
                        "strict eval variable environment selected the wrong current function segment",
                    ));
                }
                let current_body_is_program = first_function_anchor
                    .checked_sub(1)
                    .and_then(|scope| environment.scopes.get(usize::from(scope)))
                    .is_some_and(|scope| {
                        scope.kind
                            == crate::engine::code::function::metadata::EvalScopeKind::ProgramBody
                    });
                if current_body_is_program
                    && caller_metadata.eval_kind
                        == crate::engine::code::function::metadata::EvalKind::None
                    && !caller_metadata.is_module
                {
                    return Err(Error::internal(
                        "authored Script eval environment used a non-canonical strict-local target",
                    ));
                }
                let Some(scope) = environment.scopes.get(usize::from(scope)) else {
                    return Err(Error::internal(
                        "eval variable-environment scope is out of bounds",
                    ));
                };
                if !matches!(
                    scope.kind,
                    crate::engine::code::function::metadata::EvalScopeKind::FunctionRoot
                        | crate::engine::code::function::metadata::EvalScopeKind::Parameter
                ) {
                    return Err(Error::internal(
                        "strict eval variable environment did not select a function anchor",
                    ));
                }
            }
            EvalVariableEnvironment::VariableObject { scope, source } => {
                if caller_strict || matches!(source, EvalBindingSource::Argument(_)) {
                    return Err(Error::internal(
                        "eval variable-object destination is not authentic",
                    ));
                }
                let target_matches_function_segment = if caller_metadata.eval_kind
                    == crate::engine::code::function::metadata::EvalKind::None
                {
                    scope == first_function_anchor && matches!(source, EvalBindingSource::Local(_))
                } else {
                    caller_metadata.eval_kind
                        == crate::engine::code::function::metadata::EvalKind::Direct
                        && scope > first_function_anchor
                        && matches!(source, EvalBindingSource::Closure(_))
                };
                if !target_matches_function_segment {
                    return Err(Error::internal(
                        "eval variable object selected the wrong current function segment",
                    ));
                }
                let target_scope = environment.scopes.get(usize::from(scope)).ok_or_else(|| {
                    Error::internal("eval variable-object scope is out of bounds")
                })?;
                let expected_kind = match target_scope.kind {
                    crate::engine::code::function::metadata::EvalScopeKind::FunctionRoot => {
                        ClosureVariableKind::EvalVariableObject
                    }
                    crate::engine::code::function::metadata::EvalScopeKind::Parameter => {
                        ClosureVariableKind::ArgEvalVariableObject
                    }
                    _ => {
                        return Err(Error::internal(
                            "eval variable object selected a non-function scope",
                        ));
                    }
                };
                if target_scope
                    .bindings
                    .iter()
                    .filter(|binding| {
                        binding.source == source
                            && binding.kind == expected_kind
                            && !binding.is_lexical
                            && !binding.is_const
                            && !binding.is_catch_parameter
                    })
                    .count()
                    != 1
                {
                    return Err(Error::internal("eval variable-object target is not exact"));
                }
                match source {
                    EvalBindingSource::Local(index) => {
                        if self.eval_variable_object_local_kind(index) != Some(expected_kind) {
                            return Err(Error::internal(
                                "eval variable-object local role is not authentic",
                            ));
                        }
                        let definition = self.local_definition(index)?;
                        if definition.kind != expected_kind
                            || definition.is_lexical
                            || definition.is_const
                        {
                            return Err(Error::internal(
                                "eval variable-object local definition is malformed",
                            ));
                        }
                        self.locals.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval variable-object local is out of bounds")
                        })?;
                    }
                    EvalBindingSource::Closure(index) => {
                        let descriptor = *self
                            .executable
                            .closure_variables
                            .get(usize::from(index))
                            .ok_or_else(|| {
                                Error::internal("eval variable-object closure is out of bounds")
                            })?;
                        if descriptor.kind != expected_kind
                            || descriptor.is_lexical
                            || descriptor.is_const
                        {
                            return Err(Error::internal(
                                "eval variable-object closure descriptor is malformed",
                            ));
                        }
                        let root = self.closure_slots.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval variable-object closure slot is out of bounds")
                        })?;
                        self.runtime
                            .validate_var_ref_metadata(root, descriptor)
                            .map_err(|error| Error::internal(error.to_string()))?;
                    }
                    EvalBindingSource::Argument(_) => unreachable!(
                        "argument variable-object source was rejected before validation"
                    ),
                }
            }
        }
        for scope in &environment.scopes {
            for binding in &scope.bindings {
                if binding.kind.is_eval_variable_object()
                    && match scope.kind {
                        crate::engine::code::function::metadata::EvalScopeKind::FunctionRoot => {
                            false
                        }
                        crate::engine::code::function::metadata::EvalScopeKind::Parameter => {
                            binding.kind != ClosureVariableKind::ArgEvalVariableObject
                        }
                        _ => true,
                    }
                {
                    return Err(Error::internal(
                        "eval variable-object binding escaped its authenticated function anchor",
                    ));
                }
                match binding.source {
                    EvalBindingSource::Local(index) => {
                        let definition = self.local_definition(index)?;
                        Self::validate_eval_definition(definition, binding)?;
                        self.locals.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval local binding index is out of bounds")
                        })?;
                    }
                    EvalBindingSource::Argument(index) => {
                        let definition = self.argument_definition(index)?;
                        Self::validate_eval_definition(definition, binding)?;
                        self.arguments.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval argument binding index is out of bounds")
                        })?;
                    }
                    EvalBindingSource::Closure(index) => {
                        let descriptor = *self
                            .executable
                            .closure_variables
                            .get(usize::from(index))
                            .ok_or_else(|| {
                                Error::internal("eval closure binding index is out of bounds")
                            })?;
                        Self::validate_eval_closure(descriptor, binding)?;
                        let root = self.closure_slots.get(usize::from(index)).ok_or_else(|| {
                            Error::internal("eval closure slot index is out of bounds")
                        })?;
                        self.runtime
                            .validate_var_ref_metadata(root, descriptor)
                            .map_err(|error| Error::internal(error.to_string()))?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::function::metadata::{EvalScope, EvalScopeKind};

    #[test]
    fn eval_frame_validation_rejects_missing_slots_and_wrong_cell_metadata() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let key = runtime.intern_property_key("binding").unwrap();
        let mut host = RuntimeVmHost::empty_for_test(runtime.clone(), context.realm);
        let descriptor = ClosureVariable {
            source: ClosureSource::ParentLocal(0),
            name: ClosureVariableName::None,
            is_lexical: true,
            is_const: false,
            kind: ClosureVariableKind::Normal,
        };
        host.executable.closure_variables = Rc::from([descriptor]);
        let mut environment = EvalEnvironment {
            scopes: vec![EvalScope {
                kind: EvalScopeKind::FunctionRoot,
                bindings: vec![EvalBinding {
                    name: key.atom(),
                    source: EvalBindingSource::Closure(0),
                    is_lexical: true,
                    is_const: false,
                    kind: ClosureVariableKind::Normal,
                    is_catch_parameter: false,
                }]
                .into_boxed_slice(),
            }]
            .into_boxed_slice(),
            variable_environment: EvalVariableEnvironment::StrictLocal(0),
            caller_strict: true,
            super_call_allowed: false,
            super_allowed: false,
        };
        assert!(
            host.validate_eval_frame_bindings(&environment, false)
                .is_err()
        );
        assert!(
            host.validate_eval_frame_bindings(&environment, true)
                .is_err()
        );
        host.closure_slots.push(
            runtime
                .new_var_ref(Value::Int(1), false, false, ClosureVariableKind::Normal)
                .unwrap(),
        );
        assert!(
            host.validate_eval_frame_bindings(&environment, true)
                .is_err()
        );
        host.closure_slots[0] = runtime
            .new_var_ref(Value::Int(1), true, false, ClosureVariableKind::Normal)
            .unwrap();
        host.validate_eval_frame_bindings(&environment, true)
            .unwrap();
        for source in [EvalBindingSource::Local(0), EvalBindingSource::Argument(0)] {
            environment.scopes[0].bindings[0].source = source;
            assert!(
                host.validate_eval_frame_bindings(&environment, true)
                    .is_err()
            );
        }
    }
}
