//! Link-time sealing of private binding roles.
use crate::engine::api::error::Error;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::code::function::UnlinkedVariableDefinition;
use crate::engine::code::function::metadata::{
    ClosureVariable, ClosureVariableKind, ClosureVariableName, VariableDefinition,
};
use crate::engine::heap::{
    BytecodeConstant, Heap, PublishedPrivateBinding, PublishedPrivateBindings, RawValue,
};
use crate::engine::value::JsString;
use std::collections::HashMap;

fn is_private_source_name(name: &JsString) -> bool {
    name.utf16_units().next() == Some(u16::from(b'#'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::code) enum PrivateBindingRole {
    Primary,
    SetterStorage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::code) struct PrivateBindingInfo {
    kind: ClosureVariableKind,
    pub(in crate::engine::code) role: PrivateBindingRole,
}

fn setter_storage_base(name: &JsString) -> Option<Vec<u16>> {
    let units = name.utf16_units().collect::<Vec<_>>();
    let suffix = "<set>".encode_utf16().collect::<Vec<_>>();
    let base = units.strip_suffix(suffix.as_slice())?;
    (base.len() > 1 && base.first() == Some(&u16::from(b'#'))).then(|| base.to_vec())
}

pub(in crate::engine::code) fn private_binding_info(
    kind: ClosureVariableKind,
    name: &JsString,
) -> Option<PrivateBindingInfo> {
    if !is_private_source_name(name) {
        return None;
    }
    let role = if setter_storage_base(name).is_some() {
        PrivateBindingRole::SetterStorage
    } else {
        PrivateBindingRole::Primary
    };
    if role == PrivateBindingRole::SetterStorage && kind != ClosureVariableKind::PrivateSetter {
        return None;
    }
    Some(PrivateBindingInfo { kind, role })
}

pub(in crate::engine::code) fn private_setter_local_pairs(
    local_definitions: &[UnlinkedVariableDefinition],
) -> Result<Vec<Option<u16>>, RuntimeError> {
    let mut pairs = vec![None; local_definitions.len()];
    let mut unmatched_primaries = HashMap::<Vec<u16>, Vec<u16>>::new();
    for (index, definition) in local_definitions.iter().enumerate() {
        let Some(name) = definition.name.as_ref() else {
            continue;
        };
        let Some(info) = private_binding_info(definition.kind, name) else {
            continue;
        };
        let index = u16::try_from(index).map_err(|_| {
            RuntimeError::Engine(Error::internal(
                "private-name local index exceeds bytecode range",
            ))
        })?;
        match info {
            PrivateBindingInfo {
                kind: ClosureVariableKind::PrivateSetter | ClosureVariableKind::PrivateGetterSetter,
                role: PrivateBindingRole::Primary,
            } => unmatched_primaries
                .entry(name.utf16_units().collect())
                .or_default()
                .push(index),
            PrivateBindingInfo {
                kind: ClosureVariableKind::PrivateSetter,
                role: PrivateBindingRole::SetterStorage,
            } => {
                let Some(base) = setter_storage_base(name) else {
                    return Err(RuntimeError::Engine(Error::internal(
                        "private setter primary/storage cells are not paired",
                    )));
                };
                let Some(primary) = unmatched_primaries.get_mut(&base).and_then(Vec::pop) else {
                    return Err(RuntimeError::Engine(Error::internal(
                        "private setter primary/storage cells are not paired",
                    )));
                };
                pairs[usize::from(primary)] = Some(index);
                pairs[usize::from(index)] = Some(primary);
            }
            _ => {}
        }
    }
    if unmatched_primaries
        .values()
        .any(|primaries| !primaries.is_empty())
    {
        return Err(RuntimeError::Engine(Error::internal(
            "private setter primary/storage cells are not paired",
        )));
    }
    Ok(pairs)
}

/// Name-aware publication data retained across atom linking. The unlinked
/// verifier has exact source spellings, while the heap deliberately sees only
/// atoms, so setter-primary/storage roles and local pair identities must be
/// sealed at this boundary rather than reconstructed later from `kind`.
pub(crate) struct PrivateBindingPublicationPlan {
    local_roles: Vec<Option<PrivateBindingRole>>,
    local_pairs: Vec<Option<u16>>,
    closure_roles: Vec<Option<PrivateBindingRole>>,
    has_private_bindings: bool,
}

pub(crate) fn prepare_private_binding_publication(
    local_definitions: &[UnlinkedVariableDefinition],
    closure_variables: &[ClosureVariable],
    constants: &[BytecodeConstant],
    heap: &Heap,
) -> Result<PrivateBindingPublicationPlan, RuntimeError> {
    let mut local_roles = vec![None; local_definitions.len()];
    let local_pairs = private_setter_local_pairs(local_definitions)?;
    let mut has_private_bindings = false;

    for (index, definition) in local_definitions.iter().enumerate() {
        if !definition.kind.is_private() {
            continue;
        }
        has_private_bindings = true;
        let name = definition.name.as_ref().ok_or(RuntimeError::Invariant(
            "verified private local lost its source name before publication",
        ))?;
        let info = private_binding_info(definition.kind, name).ok_or(RuntimeError::Invariant(
            "verified private local lost its authenticated role before publication",
        ))?;
        local_roles[index] = Some(info.role);
    }

    let mut closure_roles = vec![None; closure_variables.len()];
    for (index, descriptor) in closure_variables.iter().enumerate() {
        if !descriptor.kind.is_private() {
            continue;
        }
        has_private_bindings = true;
        let ClosureVariableName::Constant(constant) = descriptor.name else {
            return Err(RuntimeError::Invariant(
                "verified private closure lost its unlinked source name",
            ));
        };
        let name = usize::try_from(constant)
            .ok()
            .and_then(|constant| constants.get(constant))
            .and_then(|constant| match constant {
                BytecodeConstant::Value(RawValue::String(name)) => heap.string(*name).ok(),
                BytecodeConstant::Value(_)
                | BytecodeConstant::RegExp { .. }
                | BytecodeConstant::Function(_) => None,
            })
            .ok_or(RuntimeError::Invariant(
                "verified private closure source name was not a string constant",
            ))?;
        let info = private_binding_info(descriptor.kind, name).ok_or(RuntimeError::Invariant(
            "verified private closure lost its authenticated role before publication",
        ))?;
        closure_roles[index] = Some(info.role);
    }

    Ok(PrivateBindingPublicationPlan {
        local_roles,
        local_pairs,
        closure_roles,
        has_private_bindings,
    })
}

impl PrivateBindingPublicationPlan {
    pub(crate) fn authenticate(
        self,
        local_definitions: &[VariableDefinition],
        closure_variables: &[ClosureVariable],
    ) -> Result<PublishedPrivateBindings, RuntimeError> {
        if self.local_roles.len() != local_definitions.len()
            || self.local_pairs.len() != local_definitions.len()
            || self.closure_roles.len() != closure_variables.len()
        {
            return Err(RuntimeError::Invariant(
                "private binding publication plan no longer matches linked metadata",
            ));
        }
        if !self.has_private_bindings {
            return Ok(PublishedPrivateBindings::none());
        }

        let locals = self
            .local_roles
            .into_iter()
            .zip(self.local_pairs)
            .zip(local_definitions)
            .map(|((role, pair), definition)| {
                role.map(|role| {
                    let name = definition.name.ok_or(RuntimeError::Invariant(
                        "linked private local lost its atom name",
                    ))?;
                    Ok(match role {
                        PrivateBindingRole::Primary => PublishedPrivateBinding::primary(name, pair),
                        PrivateBindingRole::SetterStorage => {
                            PublishedPrivateBinding::setter_storage(name, pair)
                        }
                    })
                })
                .transpose()
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let closures = self
            .closure_roles
            .into_iter()
            .zip(closure_variables)
            .map(|(role, descriptor)| {
                role.map(|role| {
                    let ClosureVariableName::Atom(name) = descriptor.name else {
                        return Err(RuntimeError::Invariant(
                            "linked private closure lost its atom name",
                        ));
                    };
                    Ok(match role {
                        PrivateBindingRole::Primary => PublishedPrivateBinding::primary(name, None),
                        PrivateBindingRole::SetterStorage => {
                            PublishedPrivateBinding::setter_storage(name, None)
                        }
                    })
                })
                .transpose()
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;

        Ok(PublishedPrivateBindings::authenticated(locals, closures))
    }
}
