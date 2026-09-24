//! Link-time sealing of private binding roles.
use crate::engine::api::error::Error;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::code::function::metadata::ClosureVariableKind;
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

/// Incremental name-aware sealing of private binding roles during the
/// publication walk.  The walk already visits every unlinked definition in
/// source order, so one pass both matches setter primary/storage cells and
/// assigns roles; the linked names are available in the same loop, so no
/// second scan over materialized metadata is needed.
pub(in crate::engine::code) struct PrivateBindingScanner {
    pairs: Vec<Option<u16>>,
    unmatched_primaries: HashMap<Vec<u16>, Vec<u16>>,
}

impl PrivateBindingScanner {
    pub(in crate::engine::code) fn new(definitions: usize) -> Self {
        Self {
            pairs: vec![None; definitions],
            unmatched_primaries: HashMap::new(),
        }
    }

    /// Records one local definition while its name is interned and returns
    /// the sealed role when the definition declares a private binding.
    pub(in crate::engine::code) fn observe_local(
        &mut self,
        index: usize,
        kind: ClosureVariableKind,
        name: Option<&JsString>,
    ) -> Result<Option<PrivateBindingRole>, RuntimeError> {
        if !kind.is_private() {
            return Ok(None);
        }
        let name = name.ok_or(RuntimeError::Invariant(
            "verified private local lost its source name before publication",
        ))?;
        let info = private_binding_info(kind, name).ok_or(RuntimeError::Invariant(
            "verified private local lost its authenticated role before publication",
        ))?;
        let index = u16::try_from(index).map_err(|_| {
            RuntimeError::Engine(Error::internal(
                "private-name local index exceeds bytecode range",
            ))
        })?;
        match info {
            PrivateBindingInfo {
                kind: ClosureVariableKind::PrivateSetter | ClosureVariableKind::PrivateGetterSetter,
                role: PrivateBindingRole::Primary,
            } => self
                .unmatched_primaries
                .entry(name.utf16_units().collect())
                .or_default()
                .push(index),
            PrivateBindingInfo {
                kind: ClosureVariableKind::PrivateSetter,
                role: PrivateBindingRole::SetterStorage,
            } => {
                let Some(base) = setter_storage_base(name) else {
                    return Err(unpaired_private_setter_error());
                };
                let Some(primary) = self.unmatched_primaries.get_mut(&base).and_then(Vec::pop)
                else {
                    return Err(unpaired_private_setter_error());
                };
                self.pairs[usize::from(primary)] = Some(index);
                self.pairs[usize::from(index)] = Some(primary);
            }
            _ => {}
        }
        Ok(Some(info.role))
    }

    pub(in crate::engine::code) fn pair_of(&self, index: usize) -> Option<u16> {
        self.pairs.get(index).and_then(|pair| *pair)
    }

    pub(in crate::engine::code) fn finish(&self) -> Result<(), RuntimeError> {
        if self
            .unmatched_primaries
            .values()
            .any(|primaries| !primaries.is_empty())
        {
            return Err(unpaired_private_setter_error());
        }
        Ok(())
    }
}

fn unpaired_private_setter_error() -> RuntimeError {
    RuntimeError::Engine(Error::internal(
        "private setter primary/storage cells are not paired",
    ))
}

/// Seals the role of one private closure descriptor from its unlinked source
/// spelling; the caller resolved that spelling to a string constant already.
pub(in crate::engine::code) fn private_closure_role(
    kind: ClosureVariableKind,
    name: &JsString,
) -> Result<PrivateBindingRole, RuntimeError> {
    private_binding_info(kind, name)
        .map(|info| info.role)
        .ok_or(RuntimeError::Invariant(
            "verified private closure lost its authenticated role before publication",
        ))
}
