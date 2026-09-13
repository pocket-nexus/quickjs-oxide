//! Array sorting owns the collected slots, cached strings, and exact rqsort cursor.
use super::{
    ArraySortSlot,
    rqsort::{SortAction, SortMachine},
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum SortStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: SortResume,
    },
    Number {
        value: Value,
        resume: SortResume,
    },
    String {
        value: Value,
        resume: SortResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: SortResume,
    },
    Call {
        callable: CallableRef,
        arguments: Vec<Value>,
        resume: SortResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: SortResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: SortResume,
    },
}
enum Phase {
    Length,
    LengthNumber,
    CollectHas,
    CollectRead,
    CompareCall,
    CompareNumber,
    LeftString,
    RightString,
    Write,
    Delete,
}
pub(crate) struct SortResume {
    realm: ContextId,
    copying: bool,
    object: ObjectRef,
    comparator: Option<CallableRef>,
    phase: Phase,
    length: u64,
    cursor: u64,
    undefined_count: u64,
    defined_count: u64,
    values: Vec<Value>,
    slots: Vec<ArraySortSlot>,
    logical_capacity: usize,
    // Keep rqsort's fixed partition stack outside each copied domain reply.
    machine: Box<SortMachine>,
    left: usize,
    right: usize,
}
impl SortStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        copying: bool,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let comparator = match runtime.native_sort_comparator(realm, arguments)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array sort requires generic invocation",
            ));
        };
        let object = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: SortResume {
                realm,
                copying,
                object,
                comparator,
                phase: Phase::Length,
                length: 0,
                cursor: 0,
                undefined_count: 0,
                defined_count: 0,
                values: Vec::new(),
                slots: Vec::new(),
                logical_capacity: 0,
                machine: Box::new(SortMachine::new(0)),
                left: 0,
                right: 0,
            },
        })
    }
}
impl SortResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<SortStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(SortStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::LengthNumber;
                Ok(SortStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::CollectRead => {
                self.collect_value(value);
                self.cursor += 1;
                self.collect_next(runtime)
            }
            Phase::CompareCall => {
                if let Value::Int(value) = value {
                    return self.compared(runtime, order_from_number(f64::from(value)));
                }
                self.phase = Phase::CompareNumber;
                Ok(SortStep::Number {
                    value,
                    resume: self,
                })
            }
            _ => Err(RuntimeError::Invariant("Array sort value phase mismatch")),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<SortStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(SortStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::LengthNumber => {
                self.length = Runtime::length_from_number(number);
                if self.copying {
                    self.values =
                        match runtime.native_allocate_fast_array_values(self.realm, self.length)? {
                            NativeConversion::Value(values) => values,
                            NativeConversion::Throw(value) => {
                                return Ok(SortStep::Complete(Completion::Throw(value)));
                            }
                        };
                }
                self.collect_next(runtime)
            }
            Phase::CompareNumber => self.compared(runtime, order_from_number(number)),
            _ => Err(RuntimeError::Invariant("Array sort number phase mismatch")),
        }
    }
    fn collect_next(mut self, runtime: &Runtime) -> Result<SortStep, RuntimeError> {
        if self.cursor == self.length {
            if self.copying {
                (self.slots, self.undefined_count) =
                    Runtime::collect_dense_array_sort_slots(&self.values)?;
                self.object =
                    runtime.new_array_from_values(self.realm, std::mem::take(&mut self.values))?;
            }
            *self.machine = SortMachine::new(self.slots.len());
            return self.sort_next(runtime, None);
        }
        if !self.copying {
            Runtime::reserve_array_sort_slot_capacity(&mut self.slots, &mut self.logical_capacity)?;
        }
        self.phase = Phase::CollectHas;
        Ok(SortStep::Has {
            object: self.object.clone(),
            key: runtime.property_key_for_index(self.cursor)?,
            resume: self,
        })
    }
    fn collect_value(&mut self, value: Value) {
        if self.copying {
            self.values[self.cursor as usize] = value;
        } else if matches!(value, Value::Undefined) {
            self.undefined_count += 1;
        } else {
            self.slots.push(ArraySortSlot {
                value,
                cached_string: None,
                original_position: self.cursor,
            });
        }
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<SortStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(SortStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::CollectHas => {
                if !value {
                    self.cursor += 1;
                    return self.collect_next(runtime);
                }
                self.phase = Phase::CollectRead;
                Ok(SortStep::Read {
                    object: self.object.clone(),
                    key: runtime.property_key_for_index(self.cursor)?,
                    resume: self,
                })
            }
            Phase::Delete => {
                if !value {
                    return Ok(SortStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                self.cursor += 1;
                self.write_next(runtime)
            }
            _ => Err(RuntimeError::Invariant("Array sort boolean phase mismatch")),
        }
    }
    fn sort_next(
        mut self,
        runtime: &Runtime,
        mut reply: Option<std::cmp::Ordering>,
    ) -> Result<SortStep, RuntimeError> {
        loop {
            match self.machine.advance(reply.take()) {
                SortAction::Complete => {
                    self.cursor = 0;
                    self.defined_count = self.slots.len() as u64;
                    return self.write_next(runtime);
                }
                SortAction::Swap(left, right) => self.slots.swap(left, right),
                SortAction::Compare(left, right) => {
                    self.left = left;
                    self.right = right;
                    if let Some(callable) = &self.comparator {
                        if self.slots[left]
                            .value
                            .same_quickjs_representation(&self.slots[right].value)
                        {
                            reply = Some(
                                self.slots[left]
                                    .original_position
                                    .cmp(&self.slots[right].original_position),
                            );
                            continue;
                        }
                        let callable = callable.clone();
                        self.phase = Phase::CompareCall;
                        return Ok(SortStep::Call {
                            callable,
                            arguments: vec![
                                self.slots[left].value.clone(),
                                self.slots[right].value.clone(),
                            ],
                            resume: self,
                        });
                    }
                    if let (Some(left_string), Some(right_string)) = (
                        &self.slots[left].cached_string,
                        &self.slots[right].cached_string,
                    ) {
                        let ordering = left_string.utf16_units().cmp(right_string.utf16_units());
                        reply = Some(if ordering.is_eq() {
                            self.slots[left]
                                .original_position
                                .cmp(&self.slots[right].original_position)
                        } else {
                            ordering
                        });
                        continue;
                    }
                    return self.compare_strings(runtime);
                }
            }
        }
    }
    fn compare_strings(mut self, runtime: &Runtime) -> Result<SortStep, RuntimeError> {
        if self.slots[self.left].cached_string.is_none() {
            self.phase = Phase::LeftString;
            return Ok(SortStep::String {
                value: self.slots[self.left].value.clone(),
                resume: self,
            });
        }
        if self.slots[self.right].cached_string.is_none() {
            self.phase = Phase::RightString;
            return Ok(SortStep::String {
                value: self.slots[self.right].value.clone(),
                resume: self,
            });
        }
        let ordering = self.slots[self.left]
            .cached_string
            .as_ref()
            .ok_or(RuntimeError::Invariant(
                "Array sort left string cache missing",
            ))?
            .utf16_units()
            .cmp(
                self.slots[self.right]
                    .cached_string
                    .as_ref()
                    .ok_or(RuntimeError::Invariant(
                        "Array sort right string cache missing",
                    ))?
                    .utf16_units(),
            );
        self.compared(runtime, ordering)
    }
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<SortStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(SortStep::Complete(Completion::Throw(value)));
            }
        };
        let index = match self.phase {
            Phase::LeftString => self.left,
            Phase::RightString => self.right,
            _ => return Err(RuntimeError::Invariant("Array sort string phase mismatch")),
        };
        self.slots[index].cached_string = Some(value);
        self.compare_strings(runtime)
    }
    fn compared(
        self,
        runtime: &Runtime,
        ordering: std::cmp::Ordering,
    ) -> Result<SortStep, RuntimeError> {
        let ordering = if ordering.is_eq() {
            self.slots[self.left]
                .original_position
                .cmp(&self.slots[self.right].original_position)
        } else {
            ordering
        };
        self.sort_next(runtime, Some(ordering))
    }
    fn write_next(mut self, runtime: &Runtime) -> Result<SortStep, RuntimeError> {
        while self.cursor < self.defined_count {
            let slot = &mut self.slots[self.cursor as usize];
            slot.cached_string.take();
            let value = std::mem::replace(&mut slot.value, Value::Undefined);
            if slot.original_position == self.cursor {
                self.cursor += 1;
                continue;
            }
            self.phase = Phase::Write;
            return Ok(SortStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.cursor)?,
                value,
                resume: self,
            });
        }
        drop(std::mem::take(&mut self.slots));
        if self.cursor < self.defined_count + self.undefined_count {
            self.phase = Phase::Write;
            return Ok(SortStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.cursor)?,
                value: Value::Undefined,
                resume: self,
            });
        }
        if self.cursor < self.length {
            self.phase = Phase::Delete;
            return Ok(SortStep::Delete {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.cursor)?,
                resume: self,
            });
        }
        Ok(SortStep::Complete(Completion::Return(Value::Object(
            self.object,
        ))))
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<SortStep, RuntimeError> {
        if !matches!(self.phase, Phase::Write) {
            return Err(RuntimeError::Invariant("Array sort set phase mismatch"));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(SortStep::Complete(Completion::Throw(value)));
        }
        self.cursor += 1;
        self.write_next(runtime)
    }
}
fn order_from_number(number: f64) -> std::cmp::Ordering {
    if number > 0.0 {
        std::cmp::Ordering::Greater
    } else if number < 0.0 {
        std::cmp::Ordering::Less
    } else {
        std::cmp::Ordering::Equal
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: SortStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            SortStep::Complete(result) => return Ok(result),
            SortStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            SortStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            SortStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            SortStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            SortStep::Call {
                callable,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, Value::Undefined, &arguments)?,
            )?,
            SortStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let result = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, result)?
            }
            SortStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
        };
    }
}
