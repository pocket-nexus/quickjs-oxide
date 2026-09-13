//! Shared Object/Reflect property algorithms. Requests retain their evaluated inputs.
use crate::engine::atom::PropertyKeyKind;
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::{NativeFunctionId, ObjectExtensibilityKind, ReflectKind};
use crate::engine::builtins::native::{
    ObjectIntegrityKind, ObjectKeysKind, ObjectOwnPropertyKeysKind,
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::operations::{InternalDefineResult, InternalSetResult},
    object::{
        CompleteOrdinaryPropertyDescriptor, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
    },
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::NativeArguments},
};

#[derive(Clone, Copy)]
pub(crate) enum PropertyKind {
    Integrity(ObjectIntegrityKind),
    Assign,
    Keys,
    Get,
    Set,
    Has,
    Delete,
    Define,
    Descriptor,
    Extensible,
    Prevent,
    ObjectKeys(ObjectKeysKind),
    ObjectOwnKeys(ObjectOwnPropertyKeysKind),
    ObjectDescriptors,
    ObjectDefine,
    ObjectDescriptor,
    ObjectExtensible,
    ObjectPrevent,
}
impl PropertyKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::Reflect(kind) => match kind {
                ReflectKind::Get => Self::Get,
                ReflectKind::OwnKeys => Self::Keys,
                ReflectKind::Set => Self::Set,
                ReflectKind::Has => Self::Has,
                ReflectKind::DeleteProperty => Self::Delete,
                ReflectKind::DefineProperty => Self::Define,
                ReflectKind::GetOwnPropertyDescriptor => Self::Descriptor,
                ReflectKind::IsExtensible => Self::Extensible,
                ReflectKind::PreventExtensions => Self::Prevent,
                _ => return None,
            },
            NativeFunctionId::ObjectIntegrity(kind) => Self::Integrity(kind),
            NativeFunctionId::ObjectAssign => Self::Assign,
            NativeFunctionId::ObjectKeys(kind) => Self::ObjectKeys(kind),
            NativeFunctionId::ObjectGetOwnPropertyKeys(kind) => Self::ObjectOwnKeys(kind),
            NativeFunctionId::ObjectGetOwnPropertyDescriptors => Self::ObjectDescriptors,
            NativeFunctionId::ObjectDefineProperty => Self::ObjectDefine,
            NativeFunctionId::ObjectGetOwnPropertyDescriptor => Self::ObjectDescriptor,
            NativeFunctionId::ObjectExtensibility(ObjectExtensibilityKind::IsExtensible) => {
                Self::ObjectExtensible
            }
            NativeFunctionId::ObjectExtensibility(ObjectExtensibilityKind::PreventExtensions) => {
                Self::ObjectPrevent
            }
            _ => return None,
        })
    }
}
pub(crate) enum PropertyStep {
    Keys {
        object: ObjectRef,
        resume: PropertyResume,
    },
    Complete(Completion),
    Key {
        value: Value,
        resume: PropertyResume,
    },
    Convert {
        value: Value,
        resume: PropertyResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: PropertyResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: PropertyResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: PropertyResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: PropertyResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: PropertyResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: PropertyResume,
    },
    Extensible {
        object: ObjectRef,
        resume: PropertyResume,
    },
    Prevent {
        object: ObjectRef,
        resume: PropertyResume,
    },
}
pub(crate) struct PropertyResume {
    realm: ContextId,
    kind: PropertyKind,
    object: ObjectRef,
    phase: Phase,
}
enum Phase {
    Key {
        value: Value,
        receiver: Value,
    },
    Descriptor(PropertyKey),
    Defined(PropertyKey),
    Result,
    IntegrityDescriptor {
        remaining: std::vec::IntoIter<PropertyKey>,
        key: PropertyKey,
    },
    IntegrityDefine {
        remaining: std::vec::IntoIter<PropertyKey>,
        key: PropertyKey,
    },
    AssignKeys {
        sources: std::vec::IntoIter<Value>,
        source: ObjectRef,
        snapshot: bool,
    },
    AssignDescriptor {
        state: Assignment,
        key: PropertyKey,
    },
    AssignRead {
        state: Assignment,
        key: PropertyKey,
    },
    AssignSet {
        state: Assignment,
        key: PropertyKey,
    },
    Enumerate {
        state: Enumeration,
        key: PropertyKey,
    },
    Entry {
        state: Enumeration,
        pair: Option<ObjectRef>,
    },
}
struct Assignment {
    sources: std::vec::IntoIter<Value>,
    source: ObjectRef,
    remaining: std::vec::IntoIter<PropertyKey>,
    snapshot: bool,
}
struct Enumeration {
    remaining: std::vec::IntoIter<PropertyKey>,
    result: ObjectRef,
    index: u32,
}
impl PropertyStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: PropertyKind,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let target = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "property builtin argv was not padded",
            ))?;
        let object = match (kind, target) {
            (_, Value::Object(object)) => object,
            (PropertyKind::Integrity(kind), value) => {
                return Ok(Self::Complete(Completion::Return(match kind {
                    ObjectIntegrityKind::Seal | ObjectIntegrityKind::Freeze => value,
                    ObjectIntegrityKind::IsSealed | ObjectIntegrityKind::IsFrozen => {
                        Value::Bool(true)
                    }
                })));
            }
            (PropertyKind::ObjectExtensible, _) => {
                return Ok(Self::Complete(Completion::Return(Value::Bool(false))));
            }
            (PropertyKind::ObjectPrevent, value) => {
                return Ok(Self::Complete(Completion::Return(value)));
            }
            (
                PropertyKind::Assign
                | PropertyKind::ObjectDescriptor
                | PropertyKind::ObjectKeys(_)
                | PropertyKind::ObjectOwnKeys(_)
                | PropertyKind::ObjectDescriptors,
                value,
            ) => match runtime.native_to_object(realm, value)? {
                NativeConversion::Value(object) => object,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            },
            _ => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
                )));
            }
        };
        let resume = PropertyResume {
            realm,
            kind,
            object: object.clone(),
            phase: Phase::Result,
        };
        match kind {
            PropertyKind::Integrity(ObjectIntegrityKind::Seal | ObjectIntegrityKind::Freeze) => {
                Ok(Self::Prevent { object, resume })
            }
            PropertyKind::Integrity(_) => Ok(Self::Keys { object, resume }),
            PropertyKind::Assign => {
                let mut sources = Vec::new();
                let count = arguments.actual_arg_count.saturating_sub(1);
                sources.try_reserve_exact(count).map_err(|_| {
                    RuntimeError::Invariant("Object.assign sources allocation failed")
                })?;
                sources.extend(arguments.readable.iter().skip(1).take(count).cloned());
                resume.assign_source(runtime, sources.into_iter())
            }
            PropertyKind::Keys
            | PropertyKind::ObjectKeys(_)
            | PropertyKind::ObjectOwnKeys(_)
            | PropertyKind::ObjectDescriptors => Ok(Self::Keys { object, resume }),
            PropertyKind::Extensible | PropertyKind::ObjectExtensible => {
                Ok(Self::Extensible { object, resume })
            }
            PropertyKind::Prevent | PropertyKind::ObjectPrevent => {
                Ok(Self::Prevent { object, resume })
            }
            _ => {
                let key = arguments
                    .readable
                    .get(1)
                    .cloned()
                    .ok_or(RuntimeError::Invariant(
                        "property builtin key argv was not padded",
                    ))?;
                let receiver_index = if matches!(kind, PropertyKind::Get) {
                    2
                } else {
                    3
                };
                let receiver = if arguments.actual_arg_count > receiver_index {
                    arguments.readable[receiver_index].clone()
                } else {
                    Value::Object(object)
                };
                let value = arguments
                    .readable
                    .get(2)
                    .cloned()
                    .unwrap_or(Value::Undefined);
                Ok(Self::Key {
                    value: key,
                    resume: PropertyResume {
                        phase: Phase::Key { value, receiver },
                        ..resume
                    },
                })
            }
        }
    }
}
impl PropertyResume {
    pub(crate) fn keys(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<PropertyStep, RuntimeError> {
        if !matches!(self.phase, Phase::Result | Phase::AssignKeys { .. }) {
            return Err(RuntimeError::Invariant("key-list reply has wrong phase"));
        }
        let keys = match result {
            NativeConversion::Throw(value) => {
                return Ok(PropertyStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(keys) => keys,
        };
        if let Phase::AssignKeys {
            sources,
            source,
            snapshot,
        } = self.phase
        {
            let mut selected = Vec::new();
            selected
                .try_reserve_exact(keys.len())
                .map_err(|_| RuntimeError::Invariant("Object.assign keys allocation failed"))?;
            for key in keys {
                let kind = runtime
                    .0
                    .state
                    .borrow()
                    .atoms
                    .property_key_kind(key.atom())?;
                if !matches!(kind, PropertyKeyKind::String | PropertyKeyKind::Symbol) {
                    continue;
                }
                if snapshot {
                    // This branch was selected only for non-Proxy objects. It
                    // preserves QuickJS's shape-only ENUM_ONLY snapshot.
                    match runtime
                        .internal_snapshot_own_property_is_enumerable(self.realm, &source, &key)?
                    {
                        NativeConversion::Throw(value) => {
                            return Ok(PropertyStep::Complete(Completion::Throw(value)));
                        }
                        NativeConversion::Value(false) => continue,
                        NativeConversion::Value(true) => {}
                    }
                }
                selected.push(key);
            }
            return Self {
                phase: Phase::Result,
                ..self
            }
            .assign_next(
                runtime,
                Assignment {
                    sources,
                    source,
                    remaining: selected.into_iter(),
                    snapshot,
                },
            );
        }
        match self.kind {
            PropertyKind::Integrity(_) => self.integrity_next(runtime, keys.into_iter()),
            PropertyKind::Keys | PropertyKind::ObjectOwnKeys(_) => {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(keys.len())
                    .map_err(|_| RuntimeError::Invariant("own key result allocation failed"))?;
                for key in keys {
                    let key_kind = runtime
                        .0
                        .state
                        .borrow()
                        .atoms
                        .property_key_kind(key.atom())?;
                    let include = match self.kind {
                        PropertyKind::Keys => true,
                        PropertyKind::ObjectOwnKeys(ObjectOwnPropertyKeysKind::Names) => {
                            key_kind == PropertyKeyKind::String
                        }
                        PropertyKind::ObjectOwnKeys(ObjectOwnPropertyKeysKind::Symbols) => {
                            key_kind == PropertyKeyKind::Symbol
                        }
                        _ => unreachable!(),
                    };
                    if include {
                        values.push(runtime.object_property_key_value(&key)?);
                    }
                }
                Ok(PropertyStep::Complete(Completion::Return(Value::Object(
                    runtime.new_array_from_values(self.realm, values)?,
                ))))
            }
            PropertyKind::ObjectKeys(_) | PropertyKind::ObjectDescriptors => {
                let result = if matches!(self.kind, PropertyKind::ObjectDescriptors) {
                    runtime.new_ordinary_object_in_realm(self.realm)?
                } else {
                    runtime.new_array(self.realm)?
                };
                self.enumerate(
                    runtime,
                    Enumeration {
                        remaining: keys.into_iter(),
                        result,
                        index: 0,
                    },
                )
            }
            _ => Err(RuntimeError::Invariant("unexpected key-list reply")),
        }
    }
    fn integrity_next(
        self,
        runtime: &Runtime,
        mut remaining: std::vec::IntoIter<PropertyKey>,
    ) -> Result<PropertyStep, RuntimeError> {
        for key in remaining.by_ref() {
            let kind = runtime
                .0
                .state
                .borrow()
                .atoms
                .property_key_kind(key.atom())?;
            if !matches!(kind, PropertyKeyKind::String | PropertyKeyKind::Symbol) {
                continue;
            }
            return Ok(PropertyStep::Descriptor {
                object: self.object.clone(),
                key: key.clone(),
                resume: Self {
                    phase: Phase::IntegrityDescriptor { remaining, key },
                    ..self
                },
            });
        }
        if matches!(
            self.kind,
            PropertyKind::Integrity(ObjectIntegrityKind::Seal | ObjectIntegrityKind::Freeze)
        ) {
            Ok(PropertyStep::Complete(Completion::Return(Value::Object(
                self.object,
            ))))
        } else {
            Ok(PropertyStep::Extensible {
                object: self.object.clone(),
                resume: Self {
                    phase: Phase::Result,
                    ..self
                },
            })
        }
    }
    fn assign_source(
        self,
        runtime: &Runtime,
        mut sources: std::vec::IntoIter<Value>,
    ) -> Result<PropertyStep, RuntimeError> {
        for value in sources.by_ref() {
            if matches!(value, Value::Null | Value::Undefined) {
                continue;
            }
            let source = match runtime.native_to_object(self.realm, value)? {
                NativeConversion::Value(source) => source,
                NativeConversion::Throw(_) => {
                    return Err(RuntimeError::Invariant(
                        "non-nullish Object.assign source failed ToObject",
                    ));
                }
            };
            let snapshot = !runtime.is_proxy_object(&source)?;
            return Ok(PropertyStep::Keys {
                object: source.clone(),
                resume: Self {
                    phase: Phase::AssignKeys {
                        sources,
                        source,
                        snapshot,
                    },
                    ..self
                },
            });
        }
        Ok(PropertyStep::Complete(Completion::Return(Value::Object(
            self.object,
        ))))
    }
    fn assign_next(
        self,
        runtime: &Runtime,
        mut state: Assignment,
    ) -> Result<PropertyStep, RuntimeError> {
        let Some(key) = state.remaining.next() else {
            return self.assign_source(runtime, state.sources);
        };
        if state.snapshot {
            return self.assign_read(state, key);
        }
        Ok(PropertyStep::Descriptor {
            object: state.source.clone(),
            key: key.clone(),
            resume: Self {
                phase: Phase::AssignDescriptor { state, key },
                ..self
            },
        })
    }
    fn assign_read(
        self,
        state: Assignment,
        key: PropertyKey,
    ) -> Result<PropertyStep, RuntimeError> {
        Ok(PropertyStep::Read {
            object: state.source.clone(),
            receiver: Value::Object(state.source.clone()),
            key: key.clone(),
            resume: Self {
                phase: Phase::AssignRead { state, key },
                ..self
            },
        })
    }
    fn enumerate(
        self,
        runtime: &Runtime,
        mut state: Enumeration,
    ) -> Result<PropertyStep, RuntimeError> {
        for key in state.remaining.by_ref() {
            if matches!(self.kind, PropertyKind::ObjectKeys(_))
                && runtime
                    .0
                    .state
                    .borrow()
                    .atoms
                    .property_key_kind(key.atom())?
                    != PropertyKeyKind::String
            {
                continue;
            }
            return Ok(PropertyStep::Descriptor {
                object: self.object.clone(),
                key: key.clone(),
                resume: Self {
                    phase: Phase::Enumerate { state, key },
                    ..self
                },
            });
        }
        Ok(PropertyStep::Complete(Completion::Return(Value::Object(
            state.result,
        ))))
    }
    fn emit(
        self,
        runtime: &Runtime,
        mut state: Enumeration,
        value: Value,
    ) -> Result<PropertyStep, RuntimeError> {
        runtime.define_fresh_object_keys_array_element(
            &state.result,
            state.index,
            value,
            "fresh Object keys result rejected an element",
        )?;
        state.index = state.index.checked_add(1).ok_or_else(|| {
            RuntimeError::Engine(crate::engine::api::error::Error::new(
                crate::engine::api::error::ErrorKind::Range,
                "invalid array length",
            ))
        })?;
        self.enumerate(runtime, state)
    }
    pub(crate) fn key(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PropertyStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(PropertyStep::Complete(Completion::Throw(value)));
            }
        };
        let key = match runtime.property_key_from_primitive(self.realm, value)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(PropertyStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Key { value, receiver } = self.phase else {
            return Err(RuntimeError::Invariant(
                "property key reply has wrong phase",
            ));
        };
        let object = self.object.clone();
        let resume = Self {
            phase: Phase::Result,
            ..self
        };
        Ok(match resume.kind {
            PropertyKind::Get => PropertyStep::Read {
                object,
                key,
                receiver,
                resume,
            },
            PropertyKind::Set => PropertyStep::Set {
                object,
                key,
                value,
                receiver,
                resume,
            },
            PropertyKind::Has => PropertyStep::Has {
                object,
                key,
                resume,
            },
            PropertyKind::Delete => PropertyStep::Delete {
                object,
                key,
                resume,
            },
            PropertyKind::Descriptor | PropertyKind::ObjectDescriptor => PropertyStep::Descriptor {
                object,
                key,
                resume,
            },
            PropertyKind::Define | PropertyKind::ObjectDefine => PropertyStep::Convert {
                value,
                resume: Self {
                    phase: Phase::Descriptor(key),
                    ..resume
                },
            },
            _ => {
                return Err(RuntimeError::Invariant(
                    "property builtin does not accept a key",
                ));
            }
        })
    }
    pub(crate) fn converted(
        self,
        result: NativeConversion<OrdinaryPropertyDescriptor>,
    ) -> Result<PropertyStep, RuntimeError> {
        let Phase::Descriptor(key) = self.phase else {
            return Err(RuntimeError::Invariant(
                "descriptor conversion reply has wrong phase",
            ));
        };
        Ok(match result {
            NativeConversion::Throw(value) => PropertyStep::Complete(Completion::Throw(value)),
            NativeConversion::Value(descriptor) => PropertyStep::Define {
                object: self.object.clone(),
                key: key.clone(),
                descriptor,
                resume: Self {
                    phase: Phase::Defined(key),
                    ..self
                },
            },
        })
    }
    pub(crate) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<PropertyStep, RuntimeError> {
        if let Phase::IntegrityDefine { remaining, key } = self.phase {
            if let Some(value) =
                runtime.finish_define_property_or_throw(self.realm, &key, result)?
            {
                return Ok(PropertyStep::Complete(Completion::Throw(value)));
            }
            return Self {
                phase: Phase::Result,
                ..self
            }
            .integrity_next(runtime, remaining);
        }
        let Phase::Defined(key) = self.phase else {
            return Err(RuntimeError::Invariant(
                "property definition reply has wrong phase",
            ));
        };
        Ok(PropertyStep::Complete(
            if matches!(self.kind, PropertyKind::ObjectDefine) {
                match runtime.finish_define_property_or_throw(self.realm, &key, result)? {
                    Some(value) => Completion::Throw(value),
                    None => Completion::Return(Value::Object(self.object)),
                }
            } else {
                match result {
                    NativeConversion::Value(result) => Completion::Return(Value::Bool(matches!(
                        result,
                        InternalDefineResult::Defined
                    ))),
                    NativeConversion::Throw(value) => Completion::Throw(value),
                }
            },
        ))
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<PropertyStep, RuntimeError> {
        if let Phase::IntegrityDescriptor { remaining, key } = self.phase {
            let current = match result {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(PropertyStep::Complete(Completion::Throw(value)));
                }
            };
            let PropertyKind::Integrity(kind) = self.kind else {
                return Err(RuntimeError::Invariant(
                    "integrity descriptor has wrong kind",
                ));
            };
            if matches!(
                kind,
                ObjectIntegrityKind::IsFrozen | ObjectIntegrityKind::IsSealed
            ) {
                let violates = current.is_some_and(|descriptor| match descriptor {
                    CompleteOrdinaryPropertyDescriptor::Data {
                        configurable,
                        writable,
                        ..
                    } => configurable || (kind == ObjectIntegrityKind::IsFrozen && writable),
                    CompleteOrdinaryPropertyDescriptor::Accessor { configurable, .. } => {
                        configurable
                    }
                });
                return if violates {
                    Ok(PropertyStep::Complete(Completion::Return(Value::Bool(
                        false,
                    ))))
                } else {
                    Self {
                        phase: Phase::Result,
                        ..self
                    }
                    .integrity_next(runtime, remaining)
                };
            }
            let mut descriptor = OrdinaryPropertyDescriptor {
                configurable: crate::engine::object::DescriptorField::Present(false),
                ..OrdinaryPropertyDescriptor::new()
            };
            if kind == ObjectIntegrityKind::Freeze
                && matches!(
                    current,
                    Some(CompleteOrdinaryPropertyDescriptor::Data { writable: true, .. })
                )
            {
                descriptor.writable = crate::engine::object::DescriptorField::Present(false);
            }
            return Ok(PropertyStep::Define {
                object: self.object.clone(),
                key: key.clone(),
                descriptor,
                resume: Self {
                    phase: Phase::IntegrityDefine { remaining, key },
                    ..self
                },
            });
        }
        if let Phase::AssignDescriptor { state, key } = self.phase {
            let enumerable = match result {
                NativeConversion::Value(descriptor) => {
                    descriptor.is_some_and(|descriptor| descriptor.enumerable())
                }
                NativeConversion::Throw(value) => {
                    return Ok(PropertyStep::Complete(Completion::Throw(value)));
                }
            };
            let resume = Self {
                phase: Phase::Result,
                ..self
            };
            return if enumerable {
                resume.assign_read(state, key)
            } else {
                resume.assign_next(runtime, state)
            };
        }
        if let Phase::Enumerate { state, key } = self.phase {
            let resume = Self {
                phase: Phase::Result,
                ..self
            };
            let descriptor = match result {
                NativeConversion::Throw(value) => {
                    return Ok(PropertyStep::Complete(Completion::Throw(value)));
                }
                NativeConversion::Value(None) => return resume.enumerate(runtime, state),
                NativeConversion::Value(Some(descriptor)) => descriptor,
            };
            if matches!(resume.kind, PropertyKind::ObjectDescriptors) {
                let value =
                    Value::Object(runtime.complete_descriptor_to_object(resume.realm, descriptor)?);
                runtime.define_fresh_object_descriptor_property(
                    &state.result,
                    &key,
                    value,
                    "fresh Object.getOwnPropertyDescriptors result rejected a property",
                )?;
                return resume.enumerate(runtime, state);
            }
            if !descriptor.enumerable() {
                return resume.enumerate(runtime, state);
            }
            if matches!(resume.kind, PropertyKind::ObjectKeys(ObjectKeysKind::Keys)) {
                return resume.emit(runtime, state, runtime.object_property_key_value(&key)?);
            }
            let pair = if matches!(
                resume.kind,
                PropertyKind::ObjectKeys(ObjectKeysKind::Entries)
            ) {
                let pair = runtime.new_array(resume.realm)?;
                runtime.define_fresh_object_keys_array_element(
                    &pair,
                    0,
                    runtime.object_property_key_value(&key)?,
                    "fresh Object.entries pair rejected its key",
                )?;
                Some(pair)
            } else {
                None
            };
            return Ok(PropertyStep::Read {
                object: resume.object.clone(),
                receiver: Value::Object(resume.object.clone()),
                key,
                resume: Self {
                    phase: Phase::Entry { state, pair },
                    ..resume
                },
            });
        }
        if !matches!(
            self.kind,
            PropertyKind::Descriptor | PropertyKind::ObjectDescriptor
        ) || !matches!(self.phase, Phase::Result)
        {
            return Err(RuntimeError::Invariant(
                "property descriptor reply has wrong phase",
            ));
        }
        Ok(PropertyStep::Complete(match result {
            NativeConversion::Throw(value) => Completion::Throw(value),
            NativeConversion::Value(None) => Completion::Return(Value::Undefined),
            NativeConversion::Value(Some(descriptor)) => Completion::Return(Value::Object(
                runtime.complete_descriptor_to_object(self.realm, descriptor)?,
            )),
        }))
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<PropertyStep, RuntimeError> {
        if let PropertyKind::Integrity(kind) = self.kind {
            if !matches!(self.phase, Phase::Result) {
                return Err(RuntimeError::Invariant(
                    "integrity boolean reply has wrong phase",
                ));
            }
            let value = match result {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(PropertyStep::Complete(Completion::Throw(value)));
                }
            };
            return if matches!(
                kind,
                ObjectIntegrityKind::IsSealed | ObjectIntegrityKind::IsFrozen
            ) {
                Ok(PropertyStep::Complete(Completion::Return(Value::Bool(
                    !value,
                ))))
            } else if !value {
                Ok(PropertyStep::Complete(Completion::Throw(
                    runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "proxy preventExtensions handler returned false",
                    )?,
                )))
            } else {
                Ok(PropertyStep::Keys {
                    object: self.object.clone(),
                    resume: self,
                })
            };
        }
        if !matches!(self.phase, Phase::Result)
            || !matches!(
                self.kind,
                PropertyKind::Set
                    | PropertyKind::Has
                    | PropertyKind::Delete
                    | PropertyKind::Extensible
                    | PropertyKind::Prevent
                    | PropertyKind::ObjectExtensible
                    | PropertyKind::ObjectPrevent
            )
        {
            return Err(RuntimeError::Invariant("boolean reply has wrong phase"));
        }
        Ok(PropertyStep::Complete(match result {
            NativeConversion::Throw(value) => Completion::Throw(value),
            NativeConversion::Value(accepted)
                if matches!(self.kind, PropertyKind::ObjectPrevent) =>
            {
                if accepted {
                    Completion::Return(Value::Object(self.object))
                } else {
                    Completion::Throw(runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "proxy preventExtensions handler returned false",
                    )?)
                }
            }
            NativeConversion::Value(value) => Completion::Return(Value::Bool(value)),
        }))
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<PropertyStep, RuntimeError> {
        if let Phase::AssignSet { state, key } = self.phase {
            if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
                return Ok(PropertyStep::Complete(Completion::Throw(value)));
            }
            return Self {
                phase: Phase::Result,
                ..self
            }
            .assign_next(runtime, state);
        }
        if !matches!(self.kind, PropertyKind::Set) {
            return Err(RuntimeError::Invariant("Set reply has wrong builtin"));
        }
        self.boolean(
            runtime,
            match result {
                NativeConversion::Throw(value) => NativeConversion::Throw(value),
                NativeConversion::Value(result) => {
                    NativeConversion::Value(matches!(result, InternalSetResult::Accepted))
                }
            },
        )
    }
    pub(crate) fn read(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PropertyStep, RuntimeError> {
        if let Phase::AssignRead { state, key } = self.phase {
            let value = match result {
                Completion::Return(value) => value,
                Completion::Throw(value) => {
                    return Ok(PropertyStep::Complete(Completion::Throw(value)));
                }
            };
            return Ok(PropertyStep::Set {
                object: self.object.clone(),
                receiver: Value::Object(self.object.clone()),
                key: key.clone(),
                value,
                resume: Self {
                    phase: Phase::AssignSet { state, key },
                    ..self
                },
            });
        }
        if let Phase::Entry { state, pair } = self.phase {
            let value = match result {
                Completion::Throw(value) => {
                    return Ok(PropertyStep::Complete(Completion::Throw(value)));
                }
                Completion::Return(value) => value,
            };
            let value = if let Some(pair) = pair {
                runtime.define_fresh_object_keys_array_element(
                    &pair,
                    1,
                    value,
                    "fresh Object.entries pair rejected its value",
                )?;
                Value::Object(pair)
            } else {
                value
            };
            return Self {
                phase: Phase::Result,
                ..self
            }
            .emit(runtime, state, value);
        }
        if !matches!(self.kind, PropertyKind::Get) || !matches!(self.phase, Phase::Result) {
            return Err(RuntimeError::Invariant("Get reply has wrong phase"));
        }
        Ok(PropertyStep::Complete(result))
    }
}

/// Consumer retained for the previous execution configuration and host APIs.
pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: PropertyStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            PropertyStep::Complete(result) => return Ok(result),
            PropertyStep::Keys { object, resume } => {
                resume.keys(runtime, runtime.internal_own_property_keys(realm, &object)?)?
            }
            PropertyStep::Key { value, resume } => resume.key(
                runtime,
                runtime.to_primitive(realm, value, crate::engine::vm::ToPrimitiveHint::String)?,
            )?,
            PropertyStep::Convert { value, resume } => {
                resume.converted(runtime.native_to_property_descriptor(realm, value)?)?
            }
            PropertyStep::Read {
                object,
                key,
                receiver,
                resume,
            } => resume.read(
                runtime,
                runtime.internal_get(realm, &object, &key, receiver)?,
            )?,
            PropertyStep::Set {
                object,
                key,
                value,
                receiver,
                resume,
            } => resume.set(
                runtime,
                runtime.internal_set(realm, &object, &key, value, receiver)?,
            )?,
            PropertyStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            PropertyStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
            PropertyStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            PropertyStep::Descriptor {
                object,
                key,
                resume,
            } => resume.descriptor(
                runtime,
                runtime.internal_get_own_property(realm, &object, &key)?,
            )?,
            PropertyStep::Extensible { object, resume } => {
                resume.boolean(runtime, runtime.internal_is_extensible(realm, &object)?)?
            }
            PropertyStep::Prevent { object, resume } => resume.boolean(
                runtime,
                runtime.internal_prevent_extensions(realm, &object)?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entries_keep_unpublished_pair_and_target_alive_until_reply_or_abandonment() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let context = runtime.new_context();
        let target = runtime.new_object(None).unwrap();
        let target_id = target.object_id();
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![Value::Object(target)],
        };
        let PropertyStep::Keys { resume, .. } = PropertyStep::start(
            &runtime,
            context.realm,
            PropertyKind::ObjectKeys(ObjectKeysKind::Entries),
            &arguments,
        )
        .unwrap() else {
            panic!("expected key request")
        };
        drop(arguments);
        let key = runtime.intern_property_key("x").unwrap();
        let PropertyStep::Descriptor { resume, .. } = resume
            .keys(&runtime, NativeConversion::Value(vec![key]))
            .unwrap()
        else {
            panic!("expected descriptor")
        };
        let PropertyStep::Read { resume, .. } = resume
            .descriptor(
                &runtime,
                NativeConversion::Value(Some(CompleteOrdinaryPropertyDescriptor::Data {
                    value: Value::Undefined,
                    writable: true,
                    enumerable: true,
                    configurable: true,
                })),
            )
            .unwrap()
        else {
            panic!("expected value request")
        };
        let Phase::Entry {
            state,
            pair: Some(pair),
        } = &resume.phase
        else {
            panic!("expected retained pair")
        };
        let ids = [target_id, state.result.object_id(), pair.object_id()];
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
