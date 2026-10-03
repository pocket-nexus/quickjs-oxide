//! Fresh bytecode function construction uses the caller's state and owners.
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::code::function::metadata::{ConstructorKind, FunctionKind, FunctionMetadata};
use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{ContextId, FunctionBytecodeId, ObjectData, ObjectId, VarRefId};
use crate::engine::value::{JsString, JsValue};
use std::cell::Cell;

impl RuntimeState {
    /// The caller owns the bytecode and each capture edge while this factory
    /// retains their stored copies. The Vec contains only indices, not owners.
    pub(crate) fn new_bytecode_closure_with_slots(
        &mut self,
        poisoned: &Cell<bool>,
        caller_realm: ContextId,
        function: FunctionBytecodeId,
        closure_slots: Vec<VarRefId>,
    ) -> Result<ObjectId, RuntimeError> {
        let (metadata, func_name) = {
            let bytecode = self.heap.function_bytecode(function)?;
            (bytecode.metadata, bytecode.func_name.clone())
        };
        let context = self.heap.context(caller_realm)?;
        // Module async bytecode is intentionally entered through an ordinary
        // hidden function; the module evaluator selects its async driver.
        let prototype = if metadata.is_module {
            context.function_prototype
        } else {
            match metadata.function_kind {
                FunctionKind::Normal => context.function_prototype,
                FunctionKind::Generator => {
                    context
                        .generator
                        .ok_or(RuntimeError::Invariant(
                            "generator closure realm has no Generator intrinsics",
                        ))?
                        .function_prototype
                }
                FunctionKind::Async => {
                    context
                        .async_function
                        .ok_or(RuntimeError::Invariant(
                            "async closure realm has no AsyncFunction intrinsics",
                        ))?
                        .function_prototype
                }
                FunctionKind::AsyncGenerator => {
                    context
                        .async_generator
                        .ok_or(RuntimeError::Invariant(
                            "async-generator closure realm has no AsyncGenerator intrinsics",
                        ))?
                        .function_prototype
                }
            }
        };
        let object = self.allocate_object_with_layout(
            poisoned,
            Some(prototype),
            &[],
            Vec::new(),
            |shape, slots| {
                ObjectData::bytecode_function_with_closures(
                    shape,
                    slots,
                    function,
                    None,
                    closure_slots,
                    metadata.constructor_kind != ConstructorKind::None,
                )
            },
        )?;
        let mut owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(object));
        let (state, owner) = owner.parts();
        state.initialize_bytecode_function_properties(
            poisoned,
            caller_realm,
            object,
            metadata,
            func_name,
        )?;
        let JsValue::Object(object) = owner.take().expect("new bytecode function owner") else {
            unreachable!("bytecode function factory allocated an object")
        };
        Ok(object)
    }

    fn initialize_bytecode_function_properties(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        function: ObjectId,
        metadata: FunctionMetadata,
        func_name: Option<JsString>,
    ) -> Result<(), RuntimeError> {
        self.define_fresh_function_integer_property(
            poisoned,
            function,
            "length",
            i32::from(metadata.defined_argument_count),
            false,
            true,
        )?;
        self.define_fresh_function_string_property(
            poisoned,
            function,
            "name",
            func_name.unwrap_or_else(|| JsString::from_static("")),
            false,
            true,
        )?;
        if matches!(
            metadata.function_kind,
            FunctionKind::Generator | FunctionKind::AsyncGenerator
        ) {
            if !metadata.has_prototype || metadata.constructor_kind != ConstructorKind::None {
                return Err(RuntimeError::Invariant(
                    "generator-family bytecode has invalid prototype/constructor metadata",
                ));
            }
            return self.define_generator_function_prototype(
                poisoned,
                function,
                realm,
                metadata.function_kind,
            );
        }
        if !metadata.has_prototype {
            return Ok(());
        }
        self.define_function_auto_init_prototype(poisoned, function, realm)
    }

    fn define_generator_function_prototype(
        &mut self,
        poisoned: &Cell<bool>,
        function: ObjectId,
        realm: ContextId,
        function_kind: FunctionKind,
    ) -> Result<(), RuntimeError> {
        let context = self.heap.context(realm)?;
        let prototype = match function_kind {
            FunctionKind::Generator => {
                context
                    .generator
                    .ok_or(RuntimeError::Invariant(
                        "generator function realm has no Generator intrinsics",
                    ))?
                    .prototype
            }
            FunctionKind::AsyncGenerator => {
                context
                    .async_generator
                    .ok_or(RuntimeError::Invariant(
                        "async-generator function realm has no AsyncGenerator intrinsics",
                    ))?
                    .prototype
            }
            FunctionKind::Normal | FunctionKind::Async => {
                return Err(RuntimeError::Invariant(
                    "ordinary function requested a generator prototype",
                ));
            }
        };
        // Preserve the former borrowed ObjectRef's checked temporary edge.
        self.heap.retain_object(prototype)?;
        let mut prototype_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(prototype));
        let (state, prototype_owner) = prototype_owner.parts();
        let instance = state.allocate_object_with_layout(
            poisoned,
            Some(prototype),
            &[],
            Vec::new(),
            ObjectData::ordinary,
        )?;
        // The field helper consumes this fresh producer after the canonical
        // descriptor transaction has retained its independent stored edge.
        state.define_fresh_function_object_property(
            poisoned,
            function,
            "prototype",
            instance,
            true,
            false,
        )?;
        state.release_owned_jsvalue(
            poisoned,
            prototype_owner.take().expect("generator prototype owner"),
        )
    }
}
