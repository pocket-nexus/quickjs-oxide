//! Complete descriptors acquire owners only when an effect needs a snapshot.
use super::*;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::object::property::CompletePropertyDescriptor;
use std::cell::Cell;

#[must_use]
pub(crate) struct StateOwnedCompleteDescriptor {
    record: CompletePropertyDescriptor<RawValue>,
}
impl StateOwnedCompleteDescriptor {
    pub(crate) fn from_owned_data(
        value: JsValue,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    ) -> Self {
        Self {
            record: CompletePropertyDescriptor::Data {
                value: value.into_raw(),
                writable,
                enumerable,
                configurable,
            },
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn record(&self) -> &CompletePropertyDescriptor<RawValue> {
        &self.record
    }
    pub(crate) fn retain_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        source: &CompletePropertyDescriptor<RawValue>,
    ) -> Result<Self, RuntimeError> {
        let empty = Self {
            record: CompletePropertyDescriptor::Accessor {
                get: None,
                set: None,
                enumerable: false,
                configurable: false,
            },
        };
        let mut guard = StateCompleteDescriptorGuard {
            state,
            poisoned,
            owned: Some(empty),
        };
        let copy = |state: &mut RuntimeState, raw: &RawValue| {
            state
                .dup_jsvalue(
                    &JsValue::from_raw(raw.clone()).ok_or(RuntimeError::Invariant(
                        "complete descriptor held internal sentinel",
                    ))?,
                )
                .map(JsValue::into_raw)
        };
        match source {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => {
                let value = copy(guard.state, value)?;
                guard.owned.as_mut().unwrap().record = CompletePropertyDescriptor::Data {
                    value,
                    writable: *writable,
                    enumerable: *enumerable,
                    configurable: *configurable,
                };
            }
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => {
                for (source, getter) in [(get, true), (set, false)] {
                    if let Some(source) = source {
                        let value = copy(guard.state, source)?;
                        let CompletePropertyDescriptor::Accessor { get, set, .. } =
                            &mut guard.owned.as_mut().unwrap().record
                        else {
                            unreachable!()
                        };
                        *if getter { get } else { set } = Some(value);
                    }
                }
                let CompletePropertyDescriptor::Accessor {
                    enumerable: e,
                    configurable: c,
                    ..
                } = &mut guard.owned.as_mut().unwrap().record
                else {
                    unreachable!()
                };
                *e = *enumerable;
                *c = *configurable;
            }
        }
        Ok(guard.owned.take().unwrap())
    }
    pub(crate) fn release_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        let release = |state: &mut RuntimeState, raw| {
            state.release_owned_jsvalue(
                poisoned,
                JsValue::from_raw(raw).ok_or(RuntimeError::Invariant(
                    "owned complete descriptor held internal sentinel",
                ))?,
            )
        };
        match self.record {
            CompletePropertyDescriptor::Data { value, .. } => release(state, value)?,
            CompletePropertyDescriptor::Accessor { get, set, .. } => {
                for value in [get, set].into_iter().flatten() {
                    release(state, value)?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn into_legacy(self, runtime: &Runtime) -> OwnedCompletePropertyDescriptor {
        OwnedCompletePropertyDescriptor {
            runtime: runtime.clone(),
            record: self.record,
        }
    }
}

struct StateCompleteDescriptorGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    owned: Option<StateOwnedCompleteDescriptor>,
}
impl Drop for StateCompleteDescriptorGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get()
            && let Some(owned) = self.owned.take()
        {
            let _ = owned.release_in_state(self.state, self.poisoned);
        }
    }
}

impl OwnedCompletePropertyDescriptor {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn into_state_owned(mut self) -> StateOwnedCompleteDescriptor {
        StateOwnedCompleteDescriptor {
            record: std::mem::replace(
                &mut self.record,
                CompletePropertyDescriptor::Accessor {
                    get: None,
                    set: None,
                    enumerable: false,
                    configurable: false,
                },
            ),
        }
    }
}
