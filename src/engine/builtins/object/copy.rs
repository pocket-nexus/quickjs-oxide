//! Object spread/rest share the pinned enumerable snapshot and live-read rules.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    atom::PropertyKeyKind,
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::Completion,
};
pub(crate) enum CopyStep {
    Complete(Completion),
    Keys {
        object: ObjectRef,
        resume: CopyResume,
    },
    Enumerable {
        object: ObjectRef,
        key: PropertyKey,
        resume: CopyResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: CopyResume,
    },
}
pub(crate) struct CopyResume {
    target: ObjectRef,
    source: ObjectRef,
    excluded: Option<ObjectRef>,
    snapshot: bool,
    remaining: std::vec::IntoIter<PropertyKey>,
    key: Option<PropertyKey>,
    rejection: &'static str,
}
impl CopyStep {
    pub(crate) fn start(
        runtime: &Runtime,
        target: ObjectRef,
        source: Value,
        excluded: Option<ObjectRef>,
    ) -> Result<Self, RuntimeError> {
        let Value::Object(source) = source else {
            if excluded.is_some() {
                return Err(RuntimeError::Invariant(
                    "object-rest source was not an Object after ToObject",
                ));
            }
            return Ok(Self::Complete(Completion::Return(Value::Undefined)));
        };
        if !target.belongs_to(runtime)
            || !source.belongs_to(runtime)
            || excluded
                .as_ref()
                .is_some_and(|value| !value.belongs_to(runtime))
        {
            return Err(RuntimeError::WrongRuntime("CopyDataProperties object"));
        }
        let snapshot = !runtime.is_proxy_object(&source)?;
        let rejection = if excluded.is_some() {
            "fresh Object rest result rejected a copied data property"
        } else {
            "fresh Object literal rejected a spread data property"
        };
        Ok(Self::Keys {
            object: source.clone(),
            resume: CopyResume {
                target,
                source,
                excluded,
                snapshot,
                remaining: Vec::new().into_iter(),
                key: None,
                rejection,
            },
        })
    }
}
impl CopyResume {
    pub(crate) fn keys(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<CopyStep, RuntimeError> {
        let keys = match reply {
            NativeConversion::Value(keys) => keys,
            NativeConversion::Throw(value) => {
                return Ok(CopyStep::Complete(Completion::Throw(value)));
            }
        };
        let mut selected = Vec::new();
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
            // This optimization is only selected for non-Proxy sources. It
            // observes every descriptor before the first value getter runs.
            if self.snapshot && !runtime.own_property_is_enumerable(&self.source, &key)? {
                continue;
            }
            selected.push(key);
        }
        self.remaining = selected.into_iter();
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<CopyStep, RuntimeError> {
        for key in self.remaining.by_ref() {
            // The exclusion carrier is the compiler's private fresh Object;
            // its own membership test never walks prototypes or calls getters.
            if let Some(excluded) = &self.excluded
                && runtime.has_own_property(excluded, &key)?
            {
                continue;
            }
            self.key = Some(key.clone());
            return Ok(if self.snapshot {
                CopyStep::Read {
                    object: self.source.clone(),
                    key,
                    resume: self,
                }
            } else {
                CopyStep::Enumerable {
                    object: self.source.clone(),
                    key,
                    resume: self,
                }
            });
        }
        Ok(CopyStep::Complete(Completion::Return(Value::Undefined)))
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        reply: NativeConversion<bool>,
    ) -> Result<CopyStep, RuntimeError> {
        match reply {
            NativeConversion::Throw(value) => Ok(CopyStep::Complete(Completion::Throw(value))),
            NativeConversion::Value(false) => self.next(runtime),
            NativeConversion::Value(true) => Ok(CopyStep::Read {
                object: self.source.clone(),
                key: self
                    .key
                    .clone()
                    .ok_or(RuntimeError::Invariant("Object copy key missing"))?,
                resume: self,
            }),
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CopyStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(CopyStep::Complete(Completion::Throw(value))),
        };
        let key = self
            .key
            .as_ref()
            .ok_or(RuntimeError::Invariant("Object copy key missing"))?;
        // The target stays unpublished through the entire copy. Definition
        // uses its own C_W_E data slot and bypasses inherited setters.
        runtime.define_fresh_object_descriptor_property(
            &self.target,
            key,
            value,
            self.rejection,
        )?;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CopyStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CopyStep::Complete(result) => return Ok(result),
            CopyStep::Keys { object, resume } => {
                resume.keys(runtime, runtime.internal_own_property_keys(realm, &object)?)?
            }
            CopyStep::Enumerable {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_own_property_is_enumerable(realm, &object, &key)?,
            )?,
            CopyStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
        };
    }
}
