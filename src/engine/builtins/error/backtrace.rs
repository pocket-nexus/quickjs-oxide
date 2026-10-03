use crate::engine::api::error::ErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{ObjectId, ObjectPayload};
use crate::engine::object::access::raw_string_property_one_level;

use crate::engine::object::property::PropertyDescriptor;
use crate::engine::object::{DescriptorField, ObjectRef, OrdinaryPropertyDescriptor};
use crate::engine::value::{JsString, JsStringBuilder, JsStringError, JsValue, Value};
use crate::engine::vm::frames::{ActiveFrameKind, ExplicitBacktraceLocation};
use std::cell::Cell;

impl Runtime {
    /// QuickJS `is_backtrace_needed` plus `build_backtrace`.
    ///
    /// Only real Error-class objects without any own `stack` property are
    /// eligible. Function names are read from raw ordinary data slots so this
    /// path never invokes user code while an exception is already in flight.
    pub(crate) fn ensure_error_backtrace(
        &self,
        value: &Value,
        skip_first_frame: bool,
        explicit_location: Option<ExplicitBacktraceLocation>,
    ) -> Result<(), RuntimeError> {
        let Value::Object(object) = value else {
            return Ok(());
        };
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("backtrace Error object"));
        }
        self.ensure_error_backtrace_object(object.object_id(), skip_first_frame, explicit_location)
    }

    /// Internal-value form of [`Runtime::ensure_error_backtrace`].
    pub(crate) fn ensure_error_backtrace_jsvalue(
        &self,
        value: &crate::engine::value::JsValue,
        skip_first_frame: bool,
        explicit_location: Option<ExplicitBacktraceLocation>,
    ) -> Result<(), RuntimeError> {
        let crate::engine::value::JsValue::Object(object) = value else {
            return Ok(());
        };
        self.ensure_error_backtrace_object(*object, skip_first_frame, explicit_location)
    }

    fn ensure_error_backtrace_object(
        &self,
        object: ObjectId,
        skip_first_frame: bool,
        explicit_location: Option<ExplicitBacktraceLocation>,
    ) -> Result<(), RuntimeError> {
        let _operation = self.operation()?;
        let stack_key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Stack)?;
        let needs_backtrace = self
            .0
            .state
            .borrow()
            .error_needs_backtrace(object, stack_key.atom())?;
        if !needs_backtrace {
            return Ok(());
        }

        let name_key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Name)?;
        let Some(stack) = backtrace_string_or_none(self.0.state.borrow().build_backtrace_string(
            name_key.atom(),
            skip_first_frame,
            explicit_location.as_ref(),
        ))?
        else {
            return Ok(());
        };

        let object_ref = ObjectRef::from_borrowed_handle(self.clone(), object)?;
        // Parse errors add SpiderMonkey-compatible metadata before `stack`,
        // exactly as QuickJS does. Rejection (for example after
        // preventExtensions) is intentionally silent: build_backtrace must
        // not replace the original JavaScript completion.
        for (name, property_value) in backtrace_properties(stack, explicit_location)?
            .into_iter()
            .flatten()
        {
            let property_value = match property_value {
                BacktraceFieldValue::String(value) => Value::String(value),
                BacktraceFieldValue::Int(value) => Value::Int(value),
            };
            if !self.define_backtrace_property(&object_ref, name, property_value)? {
                return Ok(());
            }
        }
        Ok(())
    }

    pub(crate) fn define_backtrace_property(
        &self,
        object: &ObjectRef,
        name: &str,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        let key = self.intern_property_key(name)?;
        self.define_own_property(
            object,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(value),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )
    }
}

impl RuntimeState {
    fn error_needs_backtrace(
        &self,
        object: ObjectId,
        stack_atom: Atom,
    ) -> Result<bool, RuntimeError> {
        let data = self.heap.object(object)?;
        Ok(matches!(data.payload, ObjectPayload::Error)
            && self
                .heap
                .shape(data.shape)?
                .find(AtomIdx::from_raw(stack_atom.raw()))
                .is_none())
    }

    /// Complete only the freshly allocated native Error, before it is exposed
    /// to callbacks or mutation. Its slots cannot contain AutoInit/VarRef
    /// metadata. Existing Error objects stay in Runtime's general adapter.
    pub(super) fn complete_fresh_native_error_backtrace(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        skip_first_frame: bool,
        explicit_location: Option<ExplicitBacktraceLocation>,
    ) -> Result<(), RuntimeError> {
        let stack_atom = self
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Stack);
        if !self.error_needs_backtrace(object, stack_atom)? {
            return Ok(());
        }
        let name_atom = self
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Name);
        let Some(stack) = backtrace_string_or_none(self.build_backtrace_string(
            name_atom,
            skip_first_frame,
            explicit_location.as_ref(),
        ))?
        else {
            return Ok(());
        };

        // Keep the old checked ObjectRef temporary retain, after rendering
        // and before metadata validation/definition, even for fresh objects.
        self.heap.retain_object(object)?;
        let mut temporary = OwnedValueGuard::new(self, poisoned, JsValue::Object(object));
        let (state, temporary) = temporary.parts();
        for (name, value) in backtrace_properties(stack, explicit_location)?
            .into_iter()
            .flatten()
        {
            if !state.define_fresh_error_backtrace_property(poisoned, object, name, value)? {
                break;
            }
        }
        state.release_owned_jsvalue(
            poisoned,
            temporary.take().expect("backtrace object temporary"),
        )
    }

    fn define_fresh_error_backtrace_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        name: &str,
        value: BacktraceFieldValue,
    ) -> Result<bool, RuntimeError> {
        let atom = self.intern_property_key_js_string(&JsString::try_from_utf8(name)?)?;
        let defined: Result<bool, RuntimeError> = (|| {
            let value = match value {
                BacktraceFieldValue::String(value) => {
                    JsValue::String(self.heap.allocate_string(value)?)
                }
                BacktraceFieldValue::Int(value) => JsValue::Int(value),
            };
            let mut producer = OwnedValueGuard::new(self, poisoned, value);
            let (state, producer) = producer.parts();
            let defined = state.define_raw_property_with_poison(
                poisoned,
                object,
                atom,
                &PropertyDescriptor {
                    value: Some(producer.as_ref().expect("backtrace value").as_raw()),
                    writable: Some(true),
                    enumerable: Some(false),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                },
            )?;
            state.release_owned_jsvalue(poisoned, producer.take().expect("backtrace value"))?;
            Ok(defined)
        })();
        // The slot/shape transaction owns accepted copies. Retire the key
        // producer after the value producer, including a rejected definition.
        if !poisoned.get()
            && let Err(error) = self.atoms.release(atom)
        {
            poisoned.set(true);
            return defined.and(Err(error.into()));
        }
        defined
    }

    pub(super) fn build_backtrace_string(
        &self,
        name_atom: Atom,
        mut skip_first_frame: bool,
        explicit_location: Option<&ExplicitBacktraceLocation>,
    ) -> Result<JsString, RuntimeError> {
        let state = self;
        let mut output = JsStringBuilder::new(0);

        if let Some(location) = explicit_location {
            let (line, column) = location
                .position
                .one_based()
                .ok_or(RuntimeError::Invariant(
                    "backtrace location cannot be represented one-based",
                ))?;
            append_backtrace_ascii(&mut output, "    at ")?;
            append_backtrace_string(&mut output, &location.filename)?;
            append_backtrace_ascii(&mut output, ":")?;
            append_backtrace_ascii(&mut output, &line.to_string())?;
            append_backtrace_ascii(&mut output, ":")?;
            append_backtrace_ascii(&mut output, &column.to_string())?;
            append_backtrace_ascii(&mut output, "\n")?;
        }

        for frame in state.active_frames.iter().rev() {
            if frame.flags.backtrace_barrier {
                break;
            }
            if frame.flags.backtrace_hidden {
                continue;
            }
            if skip_first_frame {
                skip_first_frame = false;
                continue;
            }

            let name = raw_string_property_one_level(state, frame.function, name_atom)?
                .map(truncate_backtrace_c_string)
                .transpose()?
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| JsString::from_static("<anonymous>"));
            append_backtrace_ascii(&mut output, "    at ")?;
            append_backtrace_string(&mut output, &name)?;

            match frame.kind {
                ActiveFrameKind::Native { .. } => {
                    append_backtrace_ascii(&mut output, " (native)")?;
                }
                ActiveFrameKind::Bytecode { bytecode, pc } => {
                    let bytecode = state.heap.function_bytecode(bytecode)?;
                    if let Some(debug) = &bytecode.debug {
                        let filename = state.atoms.to_js_string(debug.filename)?;
                        append_backtrace_ascii(&mut output, " (")?;
                        append_backtrace_string(&mut output, &filename)?;
                        if let Some(table) = &debug.pc2line {
                            let pc = pc
                                .map(|pc| {
                                    let exec_pc = u32::try_from(pc.index()).map_err(|_| {
                                        RuntimeError::Invariant(
                                            "active execution PC does not fit word offsets",
                                        )
                                    })?;
                                    bytecode
                                        .exec
                                        .source_pc(exec_pc)
                                        .ok_or(RuntimeError::Invariant(
                                            "active execution PC is not an instruction boundary",
                                        ))
                                })
                                .transpose()?;
                            let (line, column) =
                                table.lookup(pc).one_based().ok_or(RuntimeError::Invariant(
                                    "bytecode debug position cannot be represented one-based",
                                ))?;
                            append_backtrace_ascii(&mut output, ":")?;
                            append_backtrace_ascii(&mut output, &line.to_string())?;
                            append_backtrace_ascii(&mut output, ":")?;
                            append_backtrace_ascii(&mut output, &column.to_string())?;
                        }
                        append_backtrace_ascii(&mut output, ")")?;
                    }
                }
            }
            append_backtrace_ascii(&mut output, "\n")?;
        }

        Ok(output.finish()?)
    }
}

fn backtrace_string_or_none(
    result: Result<JsString, RuntimeError>,
) -> Result<Option<JsString>, RuntimeError> {
    match result {
        Ok(stack) => Ok(Some(stack)),
        Err(RuntimeError::Engine(error))
            if error.kind() == ErrorKind::JsInternal && error.message() == "string too long" =>
        {
            // QuickJS's void builder must preserve the Error being completed.
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Only the two primitive kinds emitted by diagnostic metadata.
enum BacktraceFieldValue {
    String(JsString),
    Int(i32),
}

/// One fixed field sequence is shared by the general adapter and fresh native
/// completion. Call only after the checked object temporary has been retained.
fn backtrace_properties(
    stack: JsString,
    explicit_location: Option<ExplicitBacktraceLocation>,
) -> Result<[Option<(&'static str, BacktraceFieldValue)>; 4], RuntimeError> {
    let mut fields = [
        None,
        None,
        None,
        Some(("stack", BacktraceFieldValue::String(stack))),
    ];
    if let Some(location) = explicit_location {
        let Some((line, column)) = location.position.one_based() else {
            return Err(RuntimeError::Invariant(
                "backtrace location cannot be represented one-based",
            ));
        };
        let line = i32::try_from(line).map_err(|_| {
            RuntimeError::Invariant("backtrace line does not fit an ECMAScript Int32")
        })?;
        let column = i32::try_from(column).map_err(|_| {
            RuntimeError::Invariant("backtrace column does not fit an ECMAScript Int32")
        })?;
        fields[0] = Some(("fileName", BacktraceFieldValue::String(location.filename)));
        fields[1] = Some(("lineNumber", BacktraceFieldValue::Int(line)));
        fields[2] = Some(("columnNumber", BacktraceFieldValue::Int(column)));
    }
    Ok(fields)
}

pub(crate) fn append_backtrace_ascii(
    output: &mut JsStringBuilder,
    value: &str,
) -> Result<(), JsStringError> {
    output.push_utf8(value)
}

pub(crate) fn append_backtrace_string(
    output: &mut JsStringBuilder,
    value: &JsString,
) -> Result<(), JsStringError> {
    output.push_js_string(value)
}

pub(crate) fn truncate_backtrace_c_string(value: JsString) -> Result<JsString, RuntimeError> {
    if !value.utf16_units().any(|unit| unit == 0) {
        return Ok(value);
    }
    let prefix = value.utf16_units().take_while(|unit| *unit != 0);
    Ok(JsString::try_from_utf16(prefix)?)
}

#[cfg(test)]
mod fresh_backtrace_state_tests {
    use super::*;
    use crate::engine::api::error::{NativeErrorKind, NativeErrorMessage};
    use crate::engine::atom::pinned::PinnedAtom;
    use crate::engine::heap::{ContextId, HeapError, PropertySlot, RawId, RawValue};
    use crate::source::LineColumn;

    fn new_error(state: &mut RuntimeState, poisoned: &Cell<bool>, realm: ContextId) -> ObjectId {
        state
            .new_native_error_without_backtrace_from_message(
                poisoned,
                realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8("message"),
            )
            .unwrap()
    }

    fn has_stack(state: &RuntimeState, object: ObjectId) -> bool {
        let object = state.heap.object(object).unwrap();
        state
            .heap
            .shape(object.shape)
            .unwrap()
            .find(AtomIdx::from_raw(
                state.pinned_atoms.get(PinnedAtom::Stack).raw(),
            ))
            .is_some()
    }

    #[test]
    fn fresh_backtrace_orders_metadata_and_transfers_exact_string_owners() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let keys = ["message", "fileName", "lineNumber", "columnNumber", "stack"]
            .map(|name| runtime.intern_property_key(name).unwrap());
        let filename = JsString::try_from_utf16([0xd800, u16::from(b'x')]).unwrap();
        let expected_stack = JsString::try_from_utf16(
            "    at "
                .encode_utf16()
                .chain([0xd800, u16::from(b'x')])
                .chain(":5:9\n".encode_utf16()),
        )
        .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let object = new_error(&mut state, &runtime.0.poisoned, context.realm);
        state
            .complete_fresh_native_error_backtrace(
                &runtime.0.poisoned,
                object,
                false,
                Some(ExplicitBacktraceLocation {
                    filename: filename.clone(),
                    position: LineColumn::new(4, 8),
                }),
            )
            .unwrap();
        assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
        let data = state.heap.object(object).unwrap();
        let shape = state.heap.shape(data.shape).unwrap();
        assert_eq!(
            shape
                .entries()
                .iter()
                .map(|entry| entry.atom.raw())
                .collect::<Vec<_>>(),
            keys.each_ref().map(|key| key.atom().raw())
        );
        assert!(shape.entries().iter().all(|entry| entry.flags.writable
            && !entry.flags.enumerable
            && entry.flags.configurable));
        assert!(matches!(
            data.slots[2],
            PropertySlot::Data(RawValue::Int(5))
        ));
        assert!(matches!(
            data.slots[3],
            PropertySlot::Data(RawValue::Int(9))
        ));
        let strings: Vec<_> = data
            .slots
            .iter()
            .filter_map(|slot| match slot {
                PropertySlot::Data(RawValue::String(string)) => Some(*string),
                _ => None,
            })
            .collect();
        assert_eq!(strings.len(), 3);
        assert_eq!(state.heap.string(strings[1]).unwrap(), &filename);
        assert_eq!(state.heap.string(strings[2]).unwrap(), &expected_stack);
        for string in &strings {
            assert_eq!(state.heap.strong_count(RawId::String(*string)).unwrap(), 1);
        }
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
        assert!(state.heap.object(object).is_err());
        assert!(
            strings
                .iter()
                .all(|string| state.heap.string(*string).is_err())
        );
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.0.poisoned.get());
    }

    #[test]
    fn fresh_backtrace_checked_object_retain_precedes_metadata_int32_validation() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let object = new_error(&mut state, &runtime.0.poisoned, context.realm);
        let location = ExplicitBacktraceLocation {
            filename: JsString::from_static("overflow.js"),
            position: LineColumn::new(i32::MAX as u32, 0),
        };
        state
            .heap
            .set_strong_count_for_test(RawId::Object(object), u32::MAX);
        let blocked = state.complete_fresh_native_error_backtrace(
            &runtime.0.poisoned,
            object,
            false,
            Some(location.clone()),
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(object), 1);
        assert!(matches!(
            blocked,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert!(matches!(
            state.complete_fresh_native_error_backtrace(
                &runtime.0.poisoned,
                object,
                false,
                Some(location)
            ),
            Err(RuntimeError::Invariant(
                "backtrace line does not fit an ECMAScript Int32"
            ))
        ));
        assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
        assert!(!has_stack(&state, object));
        assert_eq!(state.heap.object(object).unwrap().slots.len(), 1);
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
        assert!(!runtime.0.poisoned.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn rejected_fresh_backtrace_releases_string_and_atom_producers() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let filename_key = runtime.intern_property_key("fileName").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let object = new_error(&mut state, &runtime.0.poisoned, context.realm);
        state.heap.set_object_extensible(object, false).unwrap();
        let before = state.heap.counts();
        let atom_count = state.atoms.resolve(filename_key.atom()).unwrap().ref_count;
        state
            .complete_fresh_native_error_backtrace(
                &runtime.0.poisoned,
                object,
                false,
                Some(ExplicitBacktraceLocation {
                    filename: JsString::from_static("sealed.js"),
                    position: LineColumn::new(1, 2),
                }),
            )
            .unwrap();
        let after = state.heap.counts();
        assert_eq!(after.live, before.live);
        assert_eq!(after.string_nodes, before.string_nodes);
        assert_eq!(
            state.atoms.resolve(filename_key.atom()).unwrap().ref_count,
            atom_count
        );
        assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
        assert_eq!(state.heap.object(object).unwrap().slots.len(), 1);
        assert!(!has_stack(&state, object));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
        assert!(!runtime.0.poisoned.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn existing_error_adapter_preserves_autoinit_failure_and_own_stack_suppression() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let Value::Object(error) = runtime
            .new_native_error_without_backtrace_from_message(
                context.realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8("message"),
            )
            .unwrap()
        else {
            panic!("native Error object")
        };
        let filename_key = runtime.intern_property_key("fileName").unwrap();
        let line_key = runtime.intern_property_key("lineNumber").unwrap();
        runtime
            .define_failure_auto_init(&error, context.realm, "lineNumber")
            .unwrap();
        let realm_count = runtime
            .0
            .state
            .borrow()
            .heap
            .context_strong_count(context.realm)
            .unwrap();
        let location = ExplicitBacktraceLocation {
            filename: JsString::from_static("lazy.js"),
            position: LineColumn::new(2, 3),
        };
        assert!(matches!(
            runtime.ensure_error_backtrace_jsvalue(
                &JsValue::Object(error.object_id()),
                false,
                Some(location.clone())
            ),
            Err(RuntimeError::Invariant("autoinit failure probe"))
        ));
        {
            let state = runtime.0.state.borrow();
            let data = state.heap.object(error.object_id()).unwrap();
            let shape = state.heap.shape(data.shape).unwrap();
            let file_slot = shape
                .find(AtomIdx::from_raw(filename_key.atom().raw()))
                .unwrap() as usize;
            let line_slot = shape
                .find(AtomIdx::from_raw(line_key.atom().raw()))
                .unwrap() as usize;
            assert!(matches!(
                data.slots[file_slot],
                PropertySlot::Data(RawValue::String(_))
            ));
            assert!(matches!(
                data.slots[line_slot],
                PropertySlot::Data(RawValue::Undefined)
            ));
            assert_eq!(
                state.heap.context_strong_count(context.realm).unwrap(),
                realm_count - 1
            );
            assert_eq!(
                state.heap.object_strong_count(error.object_id()).unwrap(),
                1
            );
            assert!(!has_stack(&state, error.object_id()));
        }
        runtime
            .ensure_error_backtrace_jsvalue(
                &JsValue::Object(error.object_id()),
                false,
                Some(location),
            )
            .unwrap();
        let stack_key = runtime.intern_property_key("stack").unwrap();
        assert!(runtime.delete_property(&error, &stack_key).unwrap());
        runtime
            .define_failure_auto_init(&error, context.realm, "stack")
            .unwrap();
        runtime
            .ensure_error_backtrace_jsvalue(
                &JsValue::Object(error.object_id()),
                false,
                Some(ExplicitBacktraceLocation {
                    filename: JsString::from_static("ignored.js"),
                    position: LineColumn::new(u32::MAX, u32::MAX),
                }),
            )
            .unwrap();
        let state = runtime.0.state.borrow();
        let data = state.heap.object(error.object_id()).unwrap();
        let shape = state.heap.shape(data.shape).unwrap();
        let slot = shape
            .find(AtomIdx::from_raw(stack_key.atom().raw()))
            .unwrap() as usize;
        assert!(matches!(data.slots[slot], PropertySlot::AutoInit(_)));
        assert!(!runtime.0.poisoned.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn oversized_backtrace_preserves_error_before_checked_object_retain() {
        // A shared rope reaches the length ceiling without allocating its
        // gigabyte logical payload. Rendering rejects the added prefix before
        // iterating the filename or retaining the Error temporary.
        let mut powers = Vec::with_capacity(30);
        let mut power = JsString::from_static("x");
        powers.push(power.clone());
        for _ in 1..30 {
            power = power.try_concat(&power).unwrap();
            powers.push(power.clone());
        }
        let mut filename = JsString::from_static("");
        for power in powers.into_iter().rev() {
            filename = filename.try_concat(&power).unwrap();
        }
        assert_eq!(filename.len(), JsString::MAX_LEN);
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let location = ExplicitBacktraceLocation {
            filename,
            position: LineColumn::new(0, 0),
        };
        let object = {
            let mut state = runtime.0.state.borrow_mut();
            let object = new_error(&mut state, &runtime.0.poisoned, context.realm);
            state
                .heap
                .set_strong_count_for_test(RawId::Object(object), u32::MAX);
            let result = state.complete_fresh_native_error_backtrace(
                &runtime.0.poisoned,
                object,
                false,
                Some(location.clone()),
            );
            state
                .heap
                .set_strong_count_for_test(RawId::Object(object), 1);
            result.unwrap();
            assert!(!has_stack(&state, object));
            object
        };
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(object), u32::MAX);
        let result =
            runtime.ensure_error_backtrace_jsvalue(&JsValue::Object(object), false, Some(location));
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(object), 1);
        result.unwrap();
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(state.heap.object(object).unwrap().slots.len(), 1);
        assert!(!has_stack(&state, object));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
        assert!(!runtime.0.poisoned.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }
}
