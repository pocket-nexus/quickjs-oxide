//! Atom linking helpers for bytecode publication.
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::function::metadata::{EvalBinding, EvalEnvironment, EvalScope};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{BytecodeConstant, RawValue};
use crate::engine::value::JsString;

mod private_elements;
pub(in crate::engine::code) use private_elements::{
    PrivateBindingRole, PrivateBindingScanner, private_closure_role,
};

/// Link each distinct static-name constant index once. The caller owns the
/// atom transaction: any later failure releases `auxiliary_atoms`, while
/// successful publication transfers those references to the bytecode node.
pub(crate) fn link_constant_property_keys(
    state: &mut RuntimeState,
    code: &[Instruction],
    constants: &[BytecodeConstant],
    auxiliary_atoms: &mut Vec<Atom>,
) -> Result<Vec<Atom>, RuntimeError> {
    let mut keys = Vec::new();
    for instruction in code {
        let Some(index) = instruction.constant_property_key_index() else {
            continue;
        };
        let index = usize::try_from(index)
            .map_err(|_| RuntimeError::Invariant("static name index did not fit usize"))?;
        let Some(BytecodeConstant::Value(RawValue::String(name))) = constants.get(index) else {
            return Err(RuntimeError::Invariant(
                "static name did not reference a string constant",
            ));
        };
        if keys.len() <= index {
            keys.resize(index + 1, Atom::NULL);
        }
        if keys[index].is_null() {
            let text = state.heap.string(*name)?.clone();
            let atom = state.atoms.intern_property_key_js_string(&text)?;
            auxiliary_atoms.push(atom);
            keys[index] = atom;
        }
    }
    Ok(keys)
}

/// Intern every semantically retained direct-eval binding name while keeping
/// the parent publication routine's atom transaction authoritative. The
/// caller releases `auxiliary_atoms` on any later failure and transfers the
/// complete list to the bytecode node on success.
pub(crate) fn link_eval_environments(
    state: &mut RuntimeState,
    environments: Vec<EvalEnvironment<JsString>>,
    auxiliary_atoms: &mut Vec<Atom>,
) -> Result<Vec<EvalEnvironment<Atom>>, RuntimeError> {
    let mut linked_environments = Vec::with_capacity(environments.len());
    for environment in environments {
        let mut linked_scopes = Vec::with_capacity(environment.scopes.len());
        for scope in environment.scopes {
            let mut linked_bindings = Vec::with_capacity(scope.bindings.len());
            for binding in scope.bindings {
                let name = state.atoms.intern_property_key_js_string(&binding.name)?;
                auxiliary_atoms.push(name);
                linked_bindings.push(EvalBinding {
                    name,
                    source: binding.source,
                    is_lexical: binding.is_lexical,
                    is_const: binding.is_const,
                    kind: binding.kind,
                    is_catch_parameter: binding.is_catch_parameter,
                });
            }
            linked_scopes.push(EvalScope {
                kind: scope.kind,
                bindings: linked_bindings.into_boxed_slice(),
            });
        }
        linked_environments.push(EvalEnvironment {
            scopes: linked_scopes.into_boxed_slice(),
            variable_environment: environment.variable_environment,
            caller_strict: environment.caller_strict,
            super_call_allowed: environment.super_call_allowed,
            super_allowed: environment.super_allowed,
        });
    }
    Ok(linked_environments)
}
