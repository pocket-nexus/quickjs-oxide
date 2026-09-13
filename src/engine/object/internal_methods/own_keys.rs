//! Proxy ownKeys preserves list conversion, duplicate detection and invariant order.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    atom::Atom,
    heap::ContextId,
    object::{CompleteOrdinaryPropertyDescriptor, ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::DirectCallTarget},
};
use std::collections::HashSet;

pub(crate) enum KeysStep {
    Complete(NativeConversion<Vec<PropertyKey>>),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: KeysResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: KeysResume,
    },
    Number {
        value: Value,
        resume: KeysResume,
    },
    Keys {
        object: ObjectRef,
        resume: KeysResume,
    },
    Extensible {
        object: ObjectRef,
        resume: KeysResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: KeysResume,
    },
}
pub(crate) struct KeysResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Method(MethodResume),
    Forward(RootedProxy),
    Trap(RootedProxy),
    Length {
        rooted: RootedProxy,
        list: Value,
    },
    Number {
        rooted: RootedProxy,
        list: Value,
    },
    Item {
        rooted: RootedProxy,
        list: Value,
        length: u32,
        keys: Vec<PropertyKey>,
    },
    Extensible {
        rooted: RootedProxy,
        keys: Vec<PropertyKey>,
        atoms: HashSet<Atom>,
    },
    TargetKeys {
        rooted: RootedProxy,
        keys: Vec<PropertyKey>,
        atoms: HashSet<Atom>,
        extensible: bool,
    },
    Descriptor {
        state: Check,
        key: PropertyKey,
    },
}
struct Check {
    rooted: RootedProxy,
    keys: Vec<PropertyKey>,
    atoms: HashSet<Atom>,
    extensible: bool,
    remaining: std::vec::IntoIter<PropertyKey>,
}
impl KeysStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
    ) -> Result<Self, RuntimeError> {
        method(realm, MethodStep::start(runtime, realm, object, "ownKeys")?)
    }
}
fn method(realm: ContextId, step: MethodStep) -> Result<KeysStep, RuntimeError> {
    Ok(match step {
        MethodStep::Read {
            object,
            key,
            receiver: _,
            resume,
        } => KeysStep::Read {
            receiver: Value::Object(object),
            key,
            resume: KeysResume {
                realm,
                phase: Phase::Method(resume),
            },
        },
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            KeysStep::Complete(NativeConversion::Throw(value))
        }
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => KeysStep::Keys {
            object: rooted.target.clone(),
            resume: KeysResume {
                realm,
                phase: Phase::Forward(rooted),
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => KeysStep::Call {
            target,
            receiver: Value::Object(rooted.handler.clone()),
            arguments: vec![Value::Object(rooted.target.clone())],
            resume: KeysResume {
                realm,
                phase: Phase::Trap(rooted),
            },
        },
    })
}
fn fail(runtime: &Runtime, realm: ContextId, message: &str) -> Result<KeysStep, RuntimeError> {
    Ok(KeysStep::Complete(NativeConversion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Type, message)?,
    )))
}
fn items(
    runtime: &Runtime,
    realm: ContextId,
    rooted: RootedProxy,
    list: Value,
    length: u32,
    keys: Vec<PropertyKey>,
) -> Result<KeysStep, RuntimeError> {
    if keys.len() < length as usize {
        let key = runtime.intern_property_key(&keys.len().to_string())?;
        return Ok(KeysStep::Read {
            receiver: list.clone(),
            key,
            resume: KeysResume {
                realm,
                phase: Phase::Item {
                    rooted,
                    list,
                    length,
                    keys,
                },
            },
        });
    }
    // Pinned QuickJS reads every list element before it reports duplicate keys.
    let mut atoms = HashSet::new();
    atoms
        .try_reserve(keys.len())
        .map_err(|_| RuntimeError::Invariant("Proxy ownKeys set allocation failed"))?;
    for key in &keys {
        if !atoms.insert(key.atom()) {
            return fail(runtime, realm, "proxy: duplicate property");
        }
    }
    Ok(KeysStep::Extensible {
        object: rooted.target.clone(),
        resume: KeysResume {
            realm,
            phase: Phase::Extensible {
                rooted,
                keys,
                atoms,
            },
        },
    })
}
fn check_next(
    runtime: &Runtime,
    realm: ContextId,
    mut state: Check,
) -> Result<KeysStep, RuntimeError> {
    if let Some(key) = state.remaining.next() {
        if runtime.proxy_is_revoked(&state.rooted.proxy)? {
            return Ok(KeysStep::Complete(runtime.proxy_revoked_throw(realm)?));
        }
        return Ok(KeysStep::Descriptor {
            object: state.rooted.target.clone(),
            key: key.clone(),
            resume: KeysResume {
                realm,
                phase: Phase::Descriptor { state, key },
            },
        });
    }
    if !state.extensible && !state.atoms.is_empty() {
        return fail(
            runtime,
            realm,
            "proxy: property not present in target were returned by non extensible proxy",
        );
    }
    Ok(KeysStep::Complete(NativeConversion::Value(state.keys)))
}
impl KeysResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<KeysStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(KeysStep::Complete(NativeConversion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            Phase::Method(resume) => {
                method(realm, resume.resume(runtime, Completion::Return(value))?)
            }
            Phase::Trap(rooted) => Ok(KeysStep::Read {
                receiver: value.clone(),
                key: runtime.intern_property_key("length")?,
                resume: Self {
                    realm,
                    phase: Phase::Length {
                        rooted,
                        list: value,
                    },
                },
            }),
            Phase::Length { rooted, list } => Ok(KeysStep::Number {
                value,
                resume: Self {
                    realm,
                    phase: Phase::Number { rooted, list },
                },
            }),
            Phase::Item {
                rooted,
                list,
                length,
                mut keys,
            } => {
                let key = match value {
                    Value::String(value) => runtime.intern_property_key_js_string(&value)?,
                    Value::Symbol(value) => PropertyKey::from(value),
                    _ => {
                        return fail(
                            runtime,
                            realm,
                            "proxy: properties must be strings or symbols",
                        );
                    }
                };
                keys.push(key);
                items(runtime, realm, rooted, list, length, keys)
            }
            _ => Err(RuntimeError::Invariant("ownKeys received an untyped reply")),
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<KeysStep, RuntimeError> {
        let Phase::Number { rooted, list } = self.phase else {
            return Err(RuntimeError::Invariant(
                "ownKeys received unexpected numeric reply",
            ));
        };
        let length = match result {
            NativeConversion::Value(value) => Runtime::to_uint32_number(value),
            NativeConversion::Throw(value) => {
                return Ok(KeysStep::Complete(NativeConversion::Throw(value)));
            }
        };
        let mut keys = Vec::new();
        keys.try_reserve_exact(length as usize)
            .map_err(|_| RuntimeError::Invariant("Proxy ownKeys list allocation failed"))?;
        items(runtime, self.realm, rooted, list, length, keys)
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<KeysStep, RuntimeError> {
        let Phase::Extensible {
            rooted,
            keys,
            atoms,
        } = self.phase
        else {
            return Err(RuntimeError::Invariant(
                "ownKeys received unexpected boolean reply",
            ));
        };
        let extensible = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(KeysStep::Complete(NativeConversion::Throw(value)));
            }
        };
        if runtime.proxy_is_revoked(&rooted.proxy)? {
            return Ok(KeysStep::Complete(runtime.proxy_revoked_throw(self.realm)?));
        }
        Ok(KeysStep::Keys {
            object: rooted.target.clone(),
            resume: Self {
                realm: self.realm,
                phase: Phase::TargetKeys {
                    rooted,
                    keys,
                    atoms,
                    extensible,
                },
            },
        })
    }
    pub(crate) fn keys(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<KeysStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(KeysStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Forward(_rooted) => Ok(KeysStep::Complete(NativeConversion::Value(value))),
            Phase::TargetKeys {
                rooted,
                keys,
                atoms,
                extensible,
            } => check_next(
                runtime,
                self.realm,
                Check {
                    rooted,
                    keys,
                    atoms,
                    extensible,
                    remaining: value.into_iter(),
                },
            ),
            _ => Err(RuntimeError::Invariant(
                "ownKeys received unexpected key-list reply",
            )),
        }
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<KeysStep, RuntimeError> {
        let Phase::Descriptor { mut state, key } = self.phase else {
            return Err(RuntimeError::Invariant(
                "ownKeys received unexpected descriptor reply",
            ));
        };
        let descriptor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(KeysStep::Complete(NativeConversion::Throw(value)));
            }
        };
        if let Some(descriptor) = descriptor {
            let missing = if !state.extensible {
                !state.atoms.remove(&key.atom())
            } else {
                !descriptor.configurable() && !state.atoms.contains(&key.atom())
            };
            if missing {
                return fail(
                    runtime,
                    self.realm,
                    "proxy: target property must be present in proxy ownKeys",
                );
            }
        }
        check_next(runtime, self.realm, state)
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: KeysStep,
) -> Result<NativeConversion<Vec<PropertyKey>>, RuntimeError> {
    loop {
        step = match step {
            KeysStep::Complete(result) => return Ok(result),
            KeysStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            KeysStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let completion = match target {
                    DirectCallTarget::Callable(callable) => {
                        runtime.call_internal(realm, &callable, receiver, &arguments)?
                    }
                    DirectCallTarget::NonCallableProxy(proxy) => {
                        runtime.call_proxy(realm, &proxy, receiver, &arguments)?
                    }
                };
                resume.resume(runtime, completion)?
            }
            KeysStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            KeysStep::Keys { object, resume } => {
                resume.keys(runtime, runtime.internal_own_property_keys(realm, &object)?)?
            }
            KeysStep::Extensible { object, resume } => {
                resume.boolean(runtime, runtime.internal_is_extensible(realm, &object)?)?
            }
            KeysStep::Descriptor {
                object,
                key,
                resume,
            } => resume.descriptor(
                runtime,
                runtime.internal_get_own_property(realm, &object, &key)?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn list_and_key_roots_survive_gc_and_release_after_abandonment() {
        for after_list in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let mut context = runtime.new_context();
            let Value::Object(proxy) = context.eval("new Proxy({}, {})").unwrap() else {
                panic!("expected proxy")
            };
            let data = runtime.proxy_snapshot_if_any(&proxy).unwrap().unwrap();
            let ids = [proxy.object_id(), data.target, data.handler];
            let callable = context.eval("(function(){})").unwrap();
            let KeysStep::Read { resume, .. } =
                KeysStep::start(&runtime, context.realm, proxy).unwrap()
            else {
                panic!("expected handler read")
            };
            let KeysStep::Call { resume, .. } = resume
                .resume(&runtime, Completion::Return(callable))
                .unwrap()
            else {
                panic!("expected trap")
            };
            let list = runtime.new_object(None).unwrap();
            let list_id = list.object_id();
            let KeysStep::Read { resume, .. } = resume
                .resume(&runtime, Completion::Return(Value::Object(list)))
                .unwrap()
            else {
                panic!("expected length")
            };
            let KeysStep::Number { mut resume, .. } = resume
                .resume(&runtime, Completion::Return(Value::Int(1)))
                .unwrap()
            else {
                panic!("expected conversion")
            };
            let symbol = context.eval("Symbol('owned-key')").unwrap();
            let Value::Symbol(symbol) = symbol else {
                panic!("expected symbol")
            };
            let atom = symbol.atom();
            if after_list {
                let KeysStep::Read { resume: next, .. } = resume
                    .number(&runtime, NativeConversion::Value(1.0))
                    .unwrap()
                else {
                    panic!("expected item")
                };
                let KeysStep::Extensible { resume: next, .. } = next
                    .resume(&runtime, Completion::Return(Value::Symbol(symbol)))
                    .unwrap()
                else {
                    panic!("expected target query")
                };
                resume = next;
            } else {
                drop(symbol);
            }
            runtime.run_gc().unwrap();
            for id in ids {
                assert!(runtime.0.state.borrow().heap.object(id).is_ok());
            }
            assert_eq!(
                runtime.0.state.borrow().heap.object(list_id).is_ok(),
                !after_list
            );
            if after_list {
                assert!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .atoms
                        .property_key_kind(atom)
                        .is_ok()
                );
            }
            drop(resume);
            runtime.run_gc().unwrap();
            for id in ids {
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
            }
            assert!(runtime.0.state.borrow().heap.object(list_id).is_err());
            if after_list {
                assert!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .atoms
                        .property_key_kind(atom)
                        .is_err()
                );
            }
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }
}
