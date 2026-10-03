//! Finite complete-descriptor ownership under the existing State borrow.

use super::super::property::CompletePropertyDescriptor;
use crate::engine::api::runtime::RuntimeUnwindGuard;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::RawValue;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::{JsValue, Value};
use std::cell::Cell;

/// One descriptor plus its optional virtual-value producer. Normal paths
/// retire explicitly so cleanup errors stop before an owned result escapes.
/// Drop is only an unwind/early-return fallback and never reborrows Runtime.
struct CompleteDescriptorGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    record: Option<CompletePropertyDescriptor<RawValue>>,
    producer: Option<JsValue>,
}

impl<'a> CompleteDescriptorGuard<'a> {
    fn new(state: &'a mut RuntimeState, poisoned: &'a Cell<bool>) -> Self {
        Self {
            state,
            poisoned,
            record: Some(CompletePropertyDescriptor::Accessor {
                get: None,
                set: None,
                enumerable: false,
                configurable: false,
            }),
            producer: None,
        }
    }

    fn duplicate(
        &mut self,
        source: &CompletePropertyDescriptor<RawValue>,
    ) -> Result<(), RuntimeError> {
        match source {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => {
                let value =
                    self.state
                        .dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                            RuntimeError::Invariant("descriptor held internal sentinel"),
                        )?)?;
                self.record = Some(CompletePropertyDescriptor::Data {
                    value: value.into_raw(),
                    writable: *writable,
                    enumerable: *enumerable,
                    configurable: *configurable,
                });
            }
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => {
                let Some(CompletePropertyDescriptor::Accessor {
                    get: owned_get,
                    set: owned_set,
                    enumerable: e,
                    configurable: c,
                }) = &mut self.record
                else {
                    unreachable!("complete descriptor starts as an empty accessor")
                };
                *e = *enumerable;
                *c = *configurable;
                if let Some(value) = get {
                    *owned_get = Some(
                        self.state
                            .dup_jsvalue(
                                &JsValue::from_raw(value.clone())
                                    .ok_or(RuntimeError::Invariant("invalid getter sentinel"))?,
                            )?
                            .into_raw(),
                    );
                }
                if let Some(value) = set {
                    *owned_set = Some(
                        self.state
                            .dup_jsvalue(
                                &JsValue::from_raw(value.clone())
                                    .ok_or(RuntimeError::Invariant("invalid setter sentinel"))?,
                            )?
                            .into_raw(),
                    );
                }
            }
        }
        Ok(())
    }

    fn retire_producer(&mut self) -> Result<(), RuntimeError> {
        if let Some(value) = self.producer.take() {
            self.state.release_owned_jsvalue(self.poisoned, value)?;
        }
        Ok(())
    }

    fn retire(&mut self) -> Result<(), RuntimeError> {
        self.retire_producer()?;
        match self.record.take() {
            Some(CompletePropertyDescriptor::Data { value, .. }) => {
                if let Some(value) = JsValue::from_raw(value) {
                    self.state.release_owned_jsvalue(self.poisoned, value)?;
                }
            }
            Some(CompletePropertyDescriptor::Accessor { get, set, .. }) => {
                // Getter first, then setter, including two independent edges
                // when both names alias one callable. A first error stops.
                for value in get.into_iter().chain(set) {
                    if let Some(value) = JsValue::from_raw(value) {
                        self.state.release_owned_jsvalue(self.poisoned, value)?;
                    }
                }
            }
            None => {}
        }
        Ok(())
    }

    fn finish(mut self) -> Result<CompletePropertyDescriptor<RawValue>, RuntimeError> {
        self.retire_producer()?;
        Ok(self.record.take().expect("complete descriptor is owned"))
    }
}

impl Drop for CompleteDescriptorGuard<'_> {
    fn drop(&mut self) {
        if self.record.is_none() && self.producer.is_none() {
            return;
        }
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
        if self.retire().is_err() {
            self.poisoned.set(true);
        }
    }
}

impl RuntimeState {
    /// The source is borrowed and admitted; success owns one checked copy of
    /// each descriptor edge. Retain failure is recoverable, cleanup failure
    /// takes precedence and quarantines before another owner is traversed.
    pub(crate) fn retain_complete_descriptor(
        &mut self,
        poisoned: &Cell<bool>,
        source: &CompletePropertyDescriptor<RawValue>,
    ) -> Result<CompletePropertyDescriptor<RawValue>, RuntimeError> {
        let mut owned = CompleteDescriptorGuard::new(self, poisoned);
        if let Err(error) = owned.duplicate(source) {
            owned.retire()?;
            return Err(error);
        }
        owned.finish()
    }

    /// Materialize the on-demand primitive from a String index or TypedArray
    /// word. Preserve producer -> descriptor retain -> producer retirement;
    /// an older zero-queue cleanup failure cannot return an apparent success.
    pub(crate) fn own_virtual_data_descriptor(
        &mut self,
        poisoned: &Cell<bool>,
        value: Value,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    ) -> Result<CompletePropertyDescriptor<RawValue>, RuntimeError> {
        let mut owned = CompleteDescriptorGuard::new(self, poisoned);
        let raw = match value {
            Value::Int(value) => RawValue::Int(value),
            Value::Float(value) => RawValue::Float(value),
            Value::BigInt(value) if value.as_i64().is_some() => {
                RawValue::ShortBigInt(value.as_i64().expect("short BigInt"))
            }
            Value::BigInt(value) => {
                let id = owned.state.heap.allocate_bigint(value)?;
                owned.producer = Some(JsValue::BigInt(id));
                RawValue::BigInt(id)
            }
            Value::String(value) => {
                let id = owned.state.heap.allocate_string(value)?;
                owned.producer = Some(JsValue::String(id));
                RawValue::String(id)
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "virtual own property produced another value kind",
                ));
            }
        };
        let source = CompletePropertyDescriptor::Data {
            value: raw,
            writable,
            enumerable,
            configurable,
        };
        if let Err(error) = owned.duplicate(&source) {
            owned.retire()?;
            return Err(error);
        }
        owned.finish()
    }
}
