use super::*;

/// Object publication may succeed before weak-registry linking fails. A state
/// caller must distinguish that case from rejected preparation before running
/// temporary-owner cleanup.
#[derive(Debug)]
pub(crate) struct ObjectAllocationError {
    pub(crate) error: HeapError,
    pub(crate) published: bool,
}

impl Heap {
    /// Allocate and publish a shape, retaining its prototype edge.
    ///
    /// The caller owns one returned shape reference and must eventually call
    /// [`Heap::release_shape`].  Atom references are owned by the caller until
    /// this succeeds, then by the shape until returned as cleanup.
    pub fn allocate_shape(&mut self, shape: Shape) -> Result<ShapeId, HeapError> {
        let id = self.shapes.reserve()?;
        let edges = shape_edges(&shape);

        if let Err(error) = self.retain_edges_transactionally(&edges) {
            self.shapes.abort_initializing(id)?;
            return Err(error);
        }

        if let Some(prototype) = shape.prototype() {
            // The bit is monotonic for this generational object identity.
            // It creates no GC edge and ignores mutations of ordinary newborns.
            self.object_mut(prototype)?.used_as_prototype = true;
        }
        self.shapes.publish(id, shape)?;
        self.record_cycle_allocation();
        Ok(id)
    }

    /// Allocate and publish an object, retaining its shape and property edges.
    ///
    /// The caller owns one returned object reference and must eventually call
    /// [`Heap::release_object`].
    pub fn allocate_object(&mut self, object: ObjectData) -> Result<ObjectId, HeapError> {
        self.allocate_object_with_status(object)
            .map_err(|failure| failure.error)
    }

    pub(crate) fn allocate_object_with_status(
        &mut self,
        object: ObjectData,
    ) -> Result<ObjectId, ObjectAllocationError> {
        if matches!(
            &object.payload,
            ObjectPayload::NativeFunction {
                data: NativeFunctionData { realm: None, .. },
                ..
            }
        ) {
            return Err(ObjectAllocationError {
                error: HeapError::Invariant(
                    "an unbound native function may only be allocated during realm bootstrap",
                ),
                published: false,
            });
        }
        self.allocate_object_inner_with_status(object)
    }

    /// Publish an empty ordinary object under a selected empty shared shape.
    /// With no slots and an ordinary payload, the shape is its only edge, so
    /// no edge list is needed. With `owned_shape` the
    /// caller's shape reference moves into the object; otherwise the object
    /// acquires its own. Nothing is published or retained on error.
    pub(crate) fn allocate_empty_ordinary_object(
        &mut self,
        shape: ShapeId,
        owned_shape: bool,
    ) -> Result<ObjectId, HeapError> {
        debug_assert!(
            self.shape(shape)
                .is_ok_and(|shape| shape.entries().is_empty() && !shape.is_dictionary())
        );
        let (index, generation) = self.reserve(HeapNodeKind::Object)?;
        if !owned_shape {
            let retained = self.shapes.live_mut(shape).and_then(|node| {
                let count = node
                    .strong
                    .get()
                    .checked_add(1)
                    .ok_or(HeapError::Overflow {
                        operation: "retaining a heap reference",
                    })?;
                node.strong.set(count);
                Ok(())
            });
            if let Err(error) = retained {
                self.abort_initializing(index)?;
                return Err(error);
            }
        }
        self.publish(
            index,
            NodeData::Object(ObjectData::ordinary(shape, Vec::new())),
        )?;
        let id = ObjectId { index, generation };
        #[cfg(debug_assertions)]
        if super::ownership::trace_object_matches(id) {
            eprintln!(
                "[o-alloc] {id:?}\n{}",
                std::backtrace::Backtrace::force_capture()
            );
        }
        Ok(id)
    }

    /// Allocate a genuine WeakRef behind the runtime intrinsic surface. The
    /// target remains a non-owning generational identity.
    pub(crate) fn allocate_weak_ref_object(
        &mut self,
        shape: ShapeId,
        slots: Vec<PropertySlot>,
        target: WeakCollectionKey,
    ) -> Result<ObjectId, HeapError> {
        self.validate_live_weak_target(target)?;
        self.allocate_object_inner(ObjectData::weak_ref(shape, slots, target))
    }

    /// Allocate a genuine FinalizationRegistry behind its public runtime
    /// constructor. Callback and realm are traced by the payload itself.
    pub(crate) fn allocate_finalization_registry_object(
        &mut self,
        shape: ShapeId,
        slots: Vec<PropertySlot>,
        callback: ObjectId,
        realm: ContextId,
    ) -> Result<ObjectId, HeapError> {
        if !object_data_is_callable(self.object(callback)?) {
            return Err(HeapError::Invariant(
                "FinalizationRegistry callback is not callable",
            ));
        }
        self.context(realm)?;
        self.allocate_object_inner(ObjectData::finalization_registry(
            shape, slots, callback, realm,
        ))
    }

    /// Allocate the one provisional native callable needed to bootstrap a
    /// realm. The caller must synchronously finish it with
    /// [`Self::attach_native_function_realm`] before exposing it.
    pub(crate) fn allocate_bootstrap_native_function(
        &mut self,
        object: ObjectData,
    ) -> Result<ObjectId, HeapError> {
        if !matches!(
            &object.payload,
            ObjectPayload::NativeFunction {
                data: NativeFunctionData {
                    target: NativeFunctionId::FunctionPrototype,
                    realm: None,
                    ..
                },
                ..
            }
        ) {
            return Err(HeapError::Invariant(
                "bootstrap native-function allocation requires an unbound Function.prototype",
            ));
        }
        self.allocate_object_inner(object)
    }

    pub(in crate::engine::heap) fn allocate_object_inner(
        &mut self,
        object: ObjectData,
    ) -> Result<ObjectId, HeapError> {
        self.allocate_object_inner_with_status(object)
            .map_err(|failure| failure.error)
    }

    /// Allocate an Arguments object whose builder laid out `slots` against a
    /// shape it owns through the realm's arguments-shape cache. Every edge is
    /// still retained with its overflow check.
    pub(crate) fn allocate_arguments_object_with_status(
        &mut self,
        object: ObjectData,
    ) -> Result<ObjectId, ObjectAllocationError> {
        if !matches!(object.payload, ObjectPayload::Arguments { .. }) {
            return Err(ObjectAllocationError {
                error: HeapError::Invariant("arguments allocation received another payload"),
                published: false,
            });
        }
        self.allocate_object_inner_with_status(object)
    }

    #[inline(always)]
    fn allocate_object_inner_with_status(
        &mut self,
        object: ObjectData,
    ) -> Result<ObjectId, ObjectAllocationError> {
        let is_weak_object = matches!(
            &object.payload,
            ObjectPayload::WeakMap { .. }
                | ObjectPayload::WeakSet { .. }
                | ObjectPayload::WeakRef { .. }
                | ObjectPayload::FinalizationRegistry(_)
        );
        let prepared = (|| {
            let (index, generation) = self.reserve(HeapNodeKind::Object)?;
            let edges = object_edges(&object);
            if let Err(error) = self.retain_edges_transactionally(&edges) {
                self.abort_initializing(index)?;
                return Err(error);
            }
            Ok((index, generation))
        })();
        let (index, generation) = prepared.map_err(|error| ObjectAllocationError {
            error,
            published: false,
        })?;
        let id = ObjectId { index, generation };
        self.publish(index, NodeData::Object(object))
            .map_err(|error| ObjectAllocationError {
                error,
                published: false,
            })?;
        #[cfg(debug_assertions)]
        if super::ownership::trace_object_matches(id) {
            eprintln!(
                "[o-alloc] {id:?}\n{}",
                std::backtrace::Backtrace::force_capture()
            );
        }
        if is_weak_object {
            self.link_weak_object(id)
                .map_err(|error| ObjectAllocationError {
                    error,
                    published: true,
                })?;
        }
        Ok(id)
    }

    /// Allocate and publish a string node owning one `JsString` payload.
    ///
    /// The caller owns one returned string reference and must eventually call
    /// [`Heap::release_string`].  String nodes have no outgoing heap edges, so
    /// publication cannot fail after the slot is reserved.
    pub fn allocate_string(&mut self, value: JsString) -> Result<StringId, HeapError> {
        self.allocate_string_leaf(value)
    }

    /// Allocate and publish a BigInt node owning one `JsBigInt` payload.
    ///
    /// The caller owns one returned BigInt reference and must eventually call
    /// [`Heap::release_bigint`].  BigInt nodes have no outgoing heap edges, so
    /// publication cannot fail after the slot is reserved.
    pub fn allocate_bigint(&mut self, value: JsBigInt) -> Result<BigIntId, HeapError> {
        self.allocate_bigint_leaf(value)
    }

    /// Allocate and publish a realm/context node, retaining all realm roots.
    /// Symbol atoms in `intrinsics` transfer to the node on success.
    pub fn allocate_context(&mut self, context: ContextData) -> Result<ContextId, HeapError> {
        if context
            .intrinsics
            .iter()
            .any(|value| matches!(value, RawValue::Private(_)))
        {
            return Err(HeapError::Invariant(
                "private-name identity escaped into a realm intrinsic",
            ));
        }
        let (index, generation) = self.reserve(HeapNodeKind::Context)?;
        let id = ContextId { index, generation };
        let edges = context_edges(&context);

        if let Err(error) = self.retain_edges_transactionally(&edges) {
            self.abort_initializing(index)?;
            return Err(error);
        }

        self.publish(index, NodeData::Context(Box::new(context)))?;
        Ok(id)
    }

    /// Allocate and publish immutable function bytecode, retaining its realm
    /// and every GC edge in its constant pool. `auxiliary_atoms` and symbol
    /// constants transfer to the node on success. The draft comes from the
    /// in-process compiler and runtime linker, whose output is trusted.
    pub fn allocate_function_bytecode(
        &mut self,
        bytecode: FunctionBytecodeDraft,
    ) -> Result<FunctionBytecodeId, HeapError> {
        #[cfg(feature = "profiling")]
        let _encode_timer = crate::engine::api::profiling::PhaseTimer::start(
            crate::engine::api::profiling::CompilePhase::Encode,
        );
        let exec = crate::engine::code::exec::ExecCode::encode_with_locals(
            &bytecode.code,
            &bytecode.local_definitions,
            &bytecode.argument_definitions,
            &bytecode.numeric_regions,
        )
        .map_err(|_| HeapError::Invariant("execution encoding rejected authenticated bytecode"))?;
        let bytecode = FunctionBytecodeData {
            executable: Default::default(),
            exec,
            constants: bytecode.constants,
            property_key_atoms: bytecode.property_key_atoms,
            realm: bytecode.realm,
            metadata: bytecode.metadata,
            parameter_environment: bytecode.parameter_environment,
            func_name: bytecode.func_name,
            argument_definitions: bytecode.argument_definitions,
            local_definitions: bytecode.local_definitions,
            closure_variables: bytecode.closure_variables,
            eval_environments: bytecode.eval_environments,
            debug: bytecode.debug,
            auxiliary_atoms: bytecode.auxiliary_atoms,
        };

        let (index, generation) = self.reserve(HeapNodeKind::FunctionBytecode)?;
        let id = FunctionBytecodeId { index, generation };
        let edges = function_bytecode_edges(&bytecode);

        if let Err(error) = self.retain_edges_transactionally(&edges) {
            self.abort_initializing(index)?;
            return Err(error);
        }

        self.publish(index, NodeData::FunctionBytecode(Box::new(bytecode)))?;
        Ok(id)
    }

    /// Allocate a captured-variable cell and transfer ownership of its value
    /// to the heap. The returned reference is normally owned by the active
    /// frame; closure objects retain the same `VarRefId` when published.
    /// Publish a captured cell by adopting the complete value edge. Rejection
    /// returns the input record unchanged; no partial edge retain is possible.
    pub(crate) fn allocate_var_ref_owned(
        &mut self,
        var_ref: VarRefData,
    ) -> Result<VarRefId, (HeapError, VarRefData)> {
        if let Err(error) = validate_var_ref_payload(&var_ref) {
            return Err((error, var_ref));
        }
        let id = match self.var_refs.reserve() {
            Ok(id) => id,
            Err(error) => return Err((error, var_ref)),
        };
        self.var_refs
            .publish(id, var_ref)
            .expect("fresh VarRef reservation must publish exactly once");
        self.record_cycle_allocation();
        Ok(id)
    }

    pub fn allocate_var_ref(&mut self, var_ref: VarRefData) -> Result<VarRefId, HeapError> {
        validate_var_ref_payload(&var_ref)?;
        let id = self.var_refs.reserve()?;
        let edges = var_ref_edges(&var_ref);

        if let Err(error) = self.retain_edges_transactionally(&edges) {
            self.var_refs.abort_initializing(id)?;
            return Err(error);
        }

        self.var_refs.publish(id, var_ref)?;
        self.record_cycle_allocation();
        Ok(id)
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;

    #[test]
    fn checked_object_allocation_rejection_does_not_publish() {
        let mut heap = Heap::new();
        let shape = heap
            .allocate_shape(crate::engine::object::shape::Shape::new(None, []).unwrap())
            .unwrap();
        heap.set_strong_count_for_test(RawId::Shape(shape), u32::MAX);
        let before = heap.counts();
        let failure = heap
            .allocate_object_with_status(ObjectData::ordinary(shape, Vec::new()))
            .unwrap_err();
        assert!(!failure.published);
        assert!(matches!(failure.error, HeapError::Overflow { .. }));
        assert_eq!(heap.counts().object_nodes, before.object_nodes);
        assert_eq!(heap.counts().initializing, before.initializing);
        assert_eq!(heap.shape_strong_count(shape), Ok(u32::MAX));
    }

    #[test]
    fn weak_registry_failure_reports_the_published_object() {
        let mut heap = Heap::new();
        let shape = heap
            .allocate_shape(crate::engine::object::shape::Shape::new(None, []).unwrap())
            .unwrap();
        heap.allocate_object(ObjectData::weak_map(shape, Vec::new()))
            .unwrap();
        heap.weak_head = None;
        let before = heap.counts();
        let failure = heap
            .allocate_object_with_status(ObjectData::weak_map(shape, Vec::new()))
            .unwrap_err();
        assert!(failure.published);
        assert_eq!(
            failure.error,
            HeapError::Invariant("weak-collection registry endpoints disagreed")
        );
        assert_eq!(heap.counts().object_nodes, before.object_nodes + 1);
        assert_eq!(heap.counts().initializing, before.initializing);
    }
}
