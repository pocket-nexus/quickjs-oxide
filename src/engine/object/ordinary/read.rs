//! Non-callback property lookup under the caller's existing State lease.
use crate::engine::{
    api::{error::ErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    atom::{Atom, pinned::PinnedAtom},
    builtins::{SharedTypedOwnWord, TypedOwnWord, native::PrimitiveKind},
    heap::{
        ContextId, ObjectId, ObjectPayload, RawValue,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::{
        CallableRef, LinkedNativeSelection, ObjectRef,
        own_properties::{OwnPropertySelection, ReadyOwnProperty},
        property::CompletePropertyDescriptor,
    },
    value::{JsValue, Value},
};
use std::cell::Cell;

/// All edges in a ready reply are owned. Adoption or explicit retirement must
/// follow immediately; a short-lived layout fact is never saved in this record.
#[must_use]
pub(crate) enum OwnedRead {
    Complete(Option<JsValue>),
    Getter {
        function: ObjectId,
        receiver: JsValue,
    },
    Proxy {
        object: ObjectId,
        receiver: JsValue,
    },
}

#[must_use]
pub(crate) enum ReadStep {
    Ready(OwnedRead),
    /// An AutoInit Object edge is committed and owned in this reply. The
    /// consumer publishes it in execution storage before servicing pressure.
    CyclePublished(OwnedRead),
    /// This owns the actual shared backing Arc and selected word bounds.
    /// Only the mutex read leaves State; no property or prototype is replayed.
    Shared(SharedTypedOwnWord),
}

impl OwnedRead {
    pub(crate) fn retire(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(poisoned);
        self.retire_with(|value| state.release_owned_jsvalue(poisoned, value))
    }

    /// Abandoned selected effects leave State through an actual pending
    /// boundary. A conflicting borrow uses the existing Runtime coordinator.
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            self.retire(&mut state, &runtime.0.poisoned)
        } else {
            self.retire_with(|value| {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()
            })
        }
    }

    fn retire_with(
        self,
        mut release: impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Some(value)) => release(value),
            Self::Complete(None) => Ok(()),
            Self::Getter { function, receiver }
            | Self::Proxy {
                object: function,
                receiver,
            } => {
                release(JsValue::Object(function))?;
                release(receiver)
            }
        }
    }
}

impl RuntimeState {
    fn own_read_effect(
        &mut self,
        poisoned: &Cell<bool>,
        function: ObjectId,
        receiver: &JsValue,
        proxy: bool,
    ) -> Result<OwnedRead, RuntimeError> {
        self.heap.retain_object(function)?;
        let mut selected = OwnedValueGuard::new(self, poisoned, JsValue::Object(function));
        let (state, edge) = selected.parts();
        let receiver = match state.dup_jsvalue(receiver) {
            Ok(receiver) => receiver,
            Err(error) => {
                state
                    .release_owned_jsvalue(poisoned, edge.take().expect("selected read effect"))?;
                return Err(error);
            }
        };
        let JsValue::Object(function) = edge.take().expect("selected read effect") else {
            unreachable!("read effect owns an object")
        };
        Ok(if proxy {
            OwnedRead::Proxy {
                object: function,
                receiver,
            }
        } else {
            OwnedRead::Getter { function, receiver }
        })
    }

    fn own_read_primitive(&mut self, value: Value) -> Result<JsValue, RuntimeError> {
        Ok(match value {
            Value::Int(value) => JsValue::Int(value),
            Value::Float(value) => JsValue::Float(value),
            Value::String(value) => JsValue::String(self.heap.allocate_string(value)?),
            Value::BigInt(value) if value.as_i64().is_some() => {
                JsValue::ShortBigInt(value.as_i64().expect("short BigInt"))
            }
            Value::BigInt(value) => JsValue::BigInt(self.heap.allocate_bigint(value)?),
            _ => {
                return Err(RuntimeError::Invariant(
                    "virtual property produced another value kind",
                ));
            }
        })
    }

    pub(crate) fn own_typed_read_word(
        &mut self,
        word: TypedOwnWord,
    ) -> Result<OwnedRead, RuntimeError> {
        Ok(OwnedRead::Complete(Some(
            self.own_read_primitive(word.into_public_value())?,
        )))
    }

    /// The caller protects the admitted initial object and its prototype chain.
    /// Primitive lookup protects that object through the admitted realm owner.
    /// This algorithm invokes
    /// no JS, changes no prototype edge and performs no GC checkpoint; every
    /// intermediate identity stays live without an independent owner.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_ordinary_read_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        domain: u64,
        object: ObjectId,
        atom: Atom,
        receiver: &JsValue,
        own_only: bool,
        native: Option<&mut Option<LinkedNativeSelection>>,
    ) -> Result<ReadStep, RuntimeError> {
        self.prepare_ordinary_read(
            poisoned,
            object,
            atom,
            receiver,
            own_only,
            native.map(|hint| (domain, hint)),
        )
    }

    fn prepare_ordinary_read(
        &mut self,
        poisoned: &Cell<bool>,
        mut object: ObjectId,
        atom: Atom,
        receiver: &JsValue,
        own_only: bool,
        mut native: Option<(u64, &mut Option<LinkedNativeSelection>)>,
    ) -> Result<ReadStep, RuntimeError> {
        loop {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "property_storage_read_probe",
            );
            if matches!(self.heap.object(object)?.payload, ObjectPayload::Proxy(_)) {
                return Ok(ReadStep::Ready(
                    self.own_read_effect(poisoned, object, receiver, true)?,
                ));
            }
            let (ready, cycle_published) = match self.select_own_property(poisoned, object, atom)? {
                OwnPropertySelection::TerminalMissing => {
                    return Ok(ReadStep::Ready(OwnedRead::Complete(Some(
                        JsValue::Undefined,
                    ))));
                }
                OwnPropertySelection::Missing => {
                    if !own_only
                        && let Some(next) = self
                            .heap
                            .shape(self.heap.object(object)?.shape)?
                            .prototype()
                    {
                        object = next;
                        continue;
                    }
                    return Ok(ReadStep::Ready(OwnedRead::Complete(None)));
                }
                OwnPropertySelection::Shared(word) => return Ok(ReadStep::Shared(word)),
                OwnPropertySelection::Ready(ready) => (ready, false),
                OwnPropertySelection::CyclePublished(ready) => (ready, true),
            };
            let read = match ready {
                ReadyOwnProperty::StringIndex(value) => {
                    OwnedRead::Complete(Some(self.own_read_primitive(Value::String(value))?))
                }
                ReadyOwnProperty::TypedWord(word) => self.own_typed_read_word(word)?,
                ReadyOwnProperty::Stored(CompletePropertyDescriptor::Data { value, .. }) => {
                    let borrowed = JsValue::from_raw(value).ok_or(RuntimeError::Invariant(
                        "internal sentinel in ordinary data property",
                    ))?;
                    let owned = self.dup_jsvalue(&borrowed)?;
                    if let Some((domain, hint)) = native.as_mut() {
                        self.link_native_read_fact(*domain, &owned, Some(&mut **hint));
                    }
                    OwnedRead::Complete(Some(owned))
                }
                ReadyOwnProperty::Stored(CompletePropertyDescriptor::Accessor { get, .. }) => {
                    match get {
                        Some(RawValue::Object(function)) => {
                            self.own_read_effect(poisoned, function, receiver, false)?
                        }
                        None => OwnedRead::Complete(Some(JsValue::Undefined)),
                        _ => {
                            return Err(RuntimeError::Invariant(
                                "stored accessor getter was not an object",
                            ));
                        }
                    }
                }
            };
            return Ok(if cycle_published {
                ReadStep::CyclePublished(read)
            } else {
                ReadStep::Ready(read)
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_value_read_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        domain: u64,
        realm: ContextId,
        receiver: &JsValue,
        atom: Atom,
        native: Option<&mut Option<LinkedNativeSelection>>,
    ) -> Result<ReadStep, RuntimeError> {
        self.prepare_value_read(
            poisoned,
            realm,
            receiver,
            atom,
            native.map(|hint| (domain, hint)),
        )
    }

    /// No hint is requested, so no domain fact exists to authenticate. Native
    /// bodies use the same selector without inventing a Runtime/header owner.
    pub(crate) fn prepare_value_read_without_native_hint(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        receiver: &JsValue,
        atom: Atom,
    ) -> Result<ReadStep, RuntimeError> {
        self.prepare_value_read(poisoned, realm, receiver, atom, None)
    }

    fn prepare_value_read(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        receiver: &JsValue,
        atom: Atom,
        native: Option<(u64, &mut Option<LinkedNativeSelection>)>,
    ) -> Result<ReadStep, RuntimeError> {
        let kind = match receiver {
            JsValue::Object(object) => {
                return self
                    .prepare_ordinary_read(poisoned, *object, atom, receiver, false, native);
            }
            JsValue::String(id) => {
                let string = self.heap.string(*id)?;
                if let Some(index) = self.atoms.array_index(atom)?
                    && let Ok(index) = usize::try_from(index)
                    && let Some(unit) = string.code_unit_at(index)
                {
                    let value = self
                        .heap
                        .allocate_string(crate::engine::value::JsString::from_code_unit(unit))?;
                    return Ok(ReadStep::Ready(OwnedRead::Complete(Some(JsValue::String(
                        value,
                    )))));
                }
                if atom == self.pinned_atoms.get(PinnedAtom::Length) {
                    let length = i32::try_from(string.len())
                        .map(JsValue::Int)
                        .unwrap_or_else(|_| JsValue::Float(string.len() as f64));
                    return Ok(ReadStep::Ready(OwnedRead::Complete(Some(length))));
                }
                PrimitiveKind::String
            }
            JsValue::Bool(_) => PrimitiveKind::Boolean,
            JsValue::Int(_) | JsValue::Float(_) => PrimitiveKind::Number,
            JsValue::BigInt(_) | JsValue::ShortBigInt(_) => PrimitiveKind::BigInt,
            JsValue::Symbol(_) => PrimitiveKind::Symbol,
            JsValue::Undefined | JsValue::Null => {
                let suffix = if matches!(receiver, JsValue::Null) {
                    "' of null"
                } else {
                    "' of undefined"
                };
                return Err(RuntimeError::Engine(self.native_atom_error(
                    ErrorKind::Type,
                    "cannot read property '",
                    atom,
                    suffix,
                )?));
            }
        };
        let prototype = self.primitive_prototype_id_for_realm(realm, kind)?;
        self.prepare_ordinary_read(poisoned, prototype, atom, receiver, false, native)
    }
}

impl Runtime {
    /// Migration boundary for existing rooted effect consumers. The State
    /// body never creates these wrappers; no retain follows the owned handoff.
    pub(crate) fn adopt_prepared_read(&self, read: OwnedRead) -> super::OrdinaryRead {
        match read {
            OwnedRead::Complete(value) => super::OrdinaryRead::Complete(value),
            OwnedRead::Getter { function, receiver } => super::OrdinaryRead::Call {
                getter: CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                    self.clone(),
                    function,
                )),
                receiver,
            },
            OwnedRead::Proxy { object, receiver } => super::OrdinaryRead::Special {
                kind: super::super::ordinary_storage::SpecialKind::Proxy,
                object: ObjectRef::from_owned_handle(self.clone(), object),
                receiver,
            },
        }
    }

    pub(in crate::engine::object) fn finish_read_boundary(
        &self,
        step: ReadStep,
    ) -> Result<super::OrdinaryRead, RuntimeError> {
        let owned = match step {
            ReadStep::Ready(read) | ReadStep::CyclePublished(read) => read,
            ReadStep::Shared(word) => {
                let word = word.read()?;
                self.0.state.borrow_mut().own_typed_read_word(word)?
            }
        };
        Ok(self.adopt_prepared_read(owned))
    }
}

#[cfg(test)]
mod tests;
