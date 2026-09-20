//! Dynamic import snapshots attributes before enqueuing the existing load job.
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::atom::PropertyKeyKind;
use crate::engine::builtins::promise::RootedPromiseCapability;
use crate::engine::code::{
    module::{ModuleImportAttribute, ModuleImportAttributes},
    rooted::FunctionBytecodeRef,
};
use crate::engine::heap::ContextId;
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey};
use crate::engine::value::{JsString, JsValue, Value, conversion::NativeConversion};
use crate::engine::vm::Completion;

pub(crate) enum ImportStep {
    Complete(Completion),
    String {
        value: JsValue,
        resume: Box<ImportResume>,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: Box<ImportResume>,
    },
    Keys {
        object: ObjectRef,
        resume: Box<ImportResume>,
    },
    Enumerable {
        object: ObjectRef,
        key: PropertyKey,
        resume: Box<ImportResume>,
    },
    Call {
        callable: CallableRef,
        reason: JsValue,
        resume: Box<ImportResume>,
    },
}
enum Phase {
    Specifier(Value),
    With,
    Descriptors,
    Values,
    Reject,
}
pub(crate) struct ImportResume {
    realm: ContextId,
    base_name: Option<JsString>,
    capability: RootedPromiseCapability,
    phase: Phase,
    specifier: Option<JsString>,
    pending_name: Option<JsString>,
    attributes: Option<ObjectRef>,
    keys: Vec<PropertyKey>,
    enumerable: Vec<PropertyKey>,
    index: usize,
    entries: Vec<ModuleImportAttribute>,
}
impl ImportStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        root: Option<&FunctionBytecodeRef>,
        specifier: Value,
        options: Value,
    ) -> Result<Self, RuntimeError> {
        // Reject host-policy violations before filename observation, allocation,
        // conversion side effects, or any loader callback.
        runtime.ensure_dynamic_import_bytecode_authorized(root)?;
        let base_name = runtime.active_script_or_module_name()?;
        let capability = runtime.new_default_promise_capability(realm)?;
        Ok(Self::String {
            value: runtime.into_jsvalue(specifier)?,
            resume: Box::new(ImportResume {
                realm,
                base_name,
                capability,
                phase: Phase::Specifier(options),
                specifier: None,
                pending_name: None,
                attributes: None,
                keys: Vec::new(),
                enumerable: Vec::new(),
                index: 0,
                entries: Vec::new(),
            }),
        })
    }
}
impl ImportResume {
    fn reject(
        mut self: Box<Self>,
        runtime: &Runtime,
        reason: Value,
    ) -> Result<ImportStep, RuntimeError> {
        self.phase = Phase::Reject;
        Ok(ImportStep::Call {
            callable: self.capability.reject.clone(),
            reason: runtime.into_jsvalue(reason)?,
            resume: self,
        })
    }
    fn type_error(
        self: Box<Self>,
        runtime: &Runtime,
        message: &str,
    ) -> Result<ImportStep, RuntimeError> {
        let reason = runtime.new_native_error(self.realm, NativeErrorKind::Type, message)?;
        self.reject(runtime, reason)
    }
    fn enqueue(
        mut self: Box<Self>,
        runtime: &Runtime,
        attributes: ModuleImportAttributes,
    ) -> Result<ImportStep, RuntimeError> {
        let specifier = self.specifier.take().ok_or(RuntimeError::Invariant(
            "dynamic import lost its converted specifier",
        ))?;
        runtime.enqueue_dynamic_import_load_job(
            self.realm,
            &self.capability,
            self.base_name.take(),
            specifier,
            attributes,
        )?;
        Ok(ImportStep::Complete(Completion::Return(
            runtime.into_jsvalue(Value::Object(self.capability.promise))?,
        )))
    }
    pub(crate) fn resume(
        mut self: Box<Self>,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ImportStep, RuntimeError> {
        if matches!(self.phase, Phase::Reject) {
            return match completion {
                Completion::Return(_) => Ok(ImportStep::Complete(Completion::Return(
                    runtime.into_jsvalue(Value::Object(self.capability.promise))?,
                ))),
                Completion::Throw(_) => Err(RuntimeError::Invariant(
                    "intrinsic dynamic import reject function threw",
                )),
            };
        }
        let value = match completion {
            Completion::Throw(reason) => {
                let reason = runtime.root_and_release_jsvalue(reason)?;
                return self.reject(runtime, reason);
            }
            Completion::Return(value) => runtime.root_and_release_jsvalue(value)?,
        };
        match std::mem::replace(&mut self.phase, Phase::With) {
            Phase::Specifier(options) => {
                let Value::String(specifier) = value else {
                    return Err(RuntimeError::Invariant(
                        "dynamic import conversion returned a non-string",
                    ));
                };
                self.specifier = Some(specifier);
                if matches!(options, Value::Undefined) {
                    return self.enqueue(runtime, ModuleImportAttributes::Absent);
                }
                let Value::Object(object) = options else {
                    return self.type_error(runtime, "options must be an object");
                };
                Ok(ImportStep::Read {
                    object,
                    key: runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::With)?,
                    resume: self,
                })
            }
            Phase::With => {
                if matches!(value, Value::Undefined) {
                    return self.enqueue(runtime, ModuleImportAttributes::Absent);
                }
                let Value::Object(object) = value else {
                    return self.type_error(runtime, "options.with must be an object");
                };
                self.attributes = Some(object.clone());
                self.phase = Phase::Descriptors;
                Ok(ImportStep::Keys {
                    object,
                    resume: self,
                })
            }
            Phase::Values => {
                let Value::String(value) = value else {
                    return self.type_error(runtime, "module attribute values must be strings");
                };
                let name = self.pending_name.take().ok_or(RuntimeError::Invariant(
                    "dynamic import lost attribute name",
                ))?;
                self.entries
                    .push(ModuleImportAttribute { key: name, value });
                self.index += 1;
                self.phase = Phase::Values;
                self.next_value(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "dynamic import received an unexpected value reply",
            )),
        }
    }
    pub(crate) fn keys(
        mut self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<ImportStep, RuntimeError> {
        let keys = match result {
            NativeConversion::Value(keys) => keys,
            NativeConversion::Throw(reason) => return self.reject(runtime, reason),
        };
        if !matches!(self.phase, Phase::Descriptors) {
            return Err(RuntimeError::Invariant(
                "dynamic import received unexpected keys",
            ));
        }
        for key in keys {
            if runtime
                .0
                .state
                .borrow()
                .atoms
                .property_key_kind(key.atom())?
                == PropertyKeyKind::String
            {
                self.keys.push(key);
            }
        }
        self.next_descriptor(runtime)
    }
    pub(crate) fn boolean(
        mut self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ImportStep, RuntimeError> {
        let enumerable = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(reason) => return self.reject(runtime, reason),
        };
        if !matches!(self.phase, Phase::Descriptors) {
            return Err(RuntimeError::Invariant(
                "dynamic import received unexpected enumerability",
            ));
        }
        if enumerable {
            self.enumerable.push(self.keys[self.index].clone());
        }
        self.index += 1;
        self.next_descriptor(runtime)
    }
    fn next_descriptor(mut self: Box<Self>, runtime: &Runtime) -> Result<ImportStep, RuntimeError> {
        if let Some(key) = self.keys.get(self.index).cloned() {
            return Ok(ImportStep::Enumerable {
                object: self
                    .attributes
                    .as_ref()
                    .ok_or(RuntimeError::Invariant("dynamic import lost attributes"))?
                    .clone(),
                key,
                resume: self,
            });
        }
        self.phase = Phase::Values;
        self.index = 0;
        self.next_value(runtime)
    }
    fn next_value(mut self: Box<Self>, runtime: &Runtime) -> Result<ImportStep, RuntimeError> {
        if let Some(key) = self.enumerable.get(self.index).cloned() {
            // Observe key conversion before Get, as in the original algorithm.
            self.pending_name = Some(runtime.property_key_to_js_string(&key)?);
            return Ok(ImportStep::Read {
                object: self
                    .attributes
                    .as_ref()
                    .ok_or(RuntimeError::Invariant("dynamic import lost attributes"))?
                    .clone(),
                key,
                resume: self,
            });
        }
        match runtime.check_dynamic_import_attributes(self.realm, &self.entries)? {
            NativeConversion::Value(()) => {}
            NativeConversion::Throw(reason) => return self.reject(runtime, reason),
        }
        let entries = std::mem::take(&mut self.entries).into_boxed_slice();
        self.enqueue(runtime, ModuleImportAttributes::Present(entries))
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ImportStep>() <= 64);
