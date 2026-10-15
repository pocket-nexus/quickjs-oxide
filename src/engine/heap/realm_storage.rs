use super::*;
use Intrinsic as I;

impl Heap {
    pub(crate) fn cache_regexp_group_shape(
        &mut self,
        realm: ContextId,
        names: Vec<Atom>,
        shape: ShapeId,
    ) -> Result<Option<ShapeId>, HeapError> {
        let NodeData::Context(context) = &mut self.live_node_mut(RawId::Context(realm))?.data
        else {
            unreachable!()
        };
        context
            .regexp_group_shapes
            .try_reserve(1)
            .map_err(|_| HeapError::Allocation {
                operation: "caching RegExp group layout",
            })?;
        self.retain_shape(shape)?;
        let NodeData::Context(context) = &mut self.live_node_mut(RawId::Context(realm))?.data
        else {
            unreachable!()
        };
        // Bound retained pattern layouts. Eviction transfers its owned edge
        // to the caller, which releases it through the runtime cleanup path.
        let evicted = if context.regexp_group_shapes.len() >= 64 {
            let key = context
                .regexp_group_shapes
                .keys()
                .next()
                .cloned()
                .expect("full cache");
            context.regexp_group_shapes.remove(&key)
        } else {
            None
        };
        let previous = context.regexp_group_shapes.insert(names, shape);
        debug_assert!(previous.is_none());
        Ok(evicted)
    }

    /// Deliberately corrupt only native realm metadata for rejection-order tests.
    /// Tests must restore the returned realm before releasing the native owner;
    /// this does not alter the retained realm edge or any production capability.
    #[cfg(all(test, feature = "profiling"))]
    pub(crate) fn replace_native_realm_for_test(
        &mut self,
        object: ObjectId,
        realm: Option<ContextId>,
    ) -> Result<Option<ContextId>, HeapError> {
        let ObjectPayload::NativeFunction { data, .. } = &mut self.object_mut(object)?.payload
        else {
            return Err(HeapError::Invariant(
                "native realm test expected native function",
            ));
        };
        Ok(std::mem::replace(&mut data.realm, realm))
    }

    /// Finish two-phase native-function bootstrap by installing its defining
    /// realm as an owned GC edge.
    ///
    /// Realm construction is necessarily cyclic: the Context owns
    /// `%Function.prototype%`, while that native callable owns its defining
    /// Context. The object is therefore allocated provisionally, the Context
    /// is published, and this operation closes the cycle transactionally.
    pub(crate) fn attach_native_function_realm(
        &mut self,
        object: ObjectId,
        realm: ContextId,
    ) -> Result<(), HeapError> {
        self.context(realm)?;
        match &self.object(object)?.payload {
            ObjectPayload::NativeFunction {
                data: NativeFunctionData { realm: None, .. },
                ..
            } => {}
            ObjectPayload::NativeFunction {
                data: NativeFunctionData { realm: Some(_), .. },
                ..
            } => {
                return Err(HeapError::Invariant(
                    "native function already has a defining realm",
                ));
            }
            ObjectPayload::Ordinary
            | ObjectPayload::RawJson
            | ObjectPayload::Array { .. }
            | ObjectPayload::Arguments { .. }
            | ObjectPayload::ArrayIterator { .. }
            | ObjectPayload::ForInIterator(_)
            | ObjectPayload::Primitive(_)
            | ObjectPayload::Date(_)
            | ObjectPayload::RegExp(_)
            | ObjectPayload::RegExpStringIterator { .. }
            | ObjectPayload::Map { .. }
            | ObjectPayload::MapIterator { .. }
            | ObjectPayload::Set { .. }
            | ObjectPayload::SetIterator { .. }
            | ObjectPayload::WeakMap { .. }
            | ObjectPayload::WeakSet { .. }
            | ObjectPayload::WeakRef { .. }
            | ObjectPayload::FinalizationRegistry(_)
            | ObjectPayload::GlobalObject { .. }
            | ObjectPayload::Error
            | ObjectPayload::StringIterator { .. }
            | ObjectPayload::IteratorHelper(_)
            | ObjectPayload::IteratorWrap(_)
            | ObjectPayload::AsyncFromSyncIterator(_)
            | ObjectPayload::IteratorConcat(_)
            | ObjectPayload::Proxy(_)
            | ObjectPayload::ArrayBuffer(_)
            | ObjectPayload::SharedArrayBuffer(_)
            | ObjectPayload::DataView(_)
            | ObjectPayload::TypedArray(_)
            | ObjectPayload::BoundFunction { .. }
            | ObjectPayload::BytecodeFunction { .. }
            | ObjectPayload::Generator { .. }
            | ObjectPayload::AsyncGenerator(_)
            | ObjectPayload::AsyncFunctionState(_)
            | ObjectPayload::Promise(_) => {
                return Err(HeapError::Invariant(
                    "attempted to attach a native realm to a non-native function",
                ));
            }
        }

        self.retain_raw(RawId::Context(realm), 1)?;
        let ObjectPayload::NativeFunction { data, .. } = &mut self.object_mut(object)?.payload
        else {
            unreachable!("native-function payload was validated before retaining its realm")
        };
        data.realm = Some(realm);
        Ok(())
    }

    /// The realm's cached arguments shape for this kind and count, if any.
    pub(crate) fn arguments_shape(
        &self,
        realm: ContextId,
        mapped: bool,
        count: usize,
    ) -> Result<Option<ShapeId>, HeapError> {
        Ok(self.context(realm)?.arguments_shapes[usize::from(mapped)]
            .get(count)
            .copied()
            .flatten())
    }

    /// Transfer one owned shape reference into the realm's arguments cache.
    pub(crate) fn cache_arguments_shape(
        &mut self,
        realm: ContextId,
        mapped: bool,
        count: usize,
        shape: ShapeId,
    ) -> Result<(), HeapError> {
        let NodeData::Context(context) = &mut self.live_node_mut(RawId::Context(realm))?.data
        else {
            return Err(HeapError::Invariant(
                "arguments shape cache needs a context",
            ));
        };
        let entry = context.arguments_shapes[usize::from(mapped)]
            .get_mut(count)
            .ok_or(HeapError::Invariant(
                "arguments shape cache index out of range",
            ))?;
        if entry.is_some() {
            return Err(HeapError::Invariant("arguments shape is already cached"));
        }
        *entry = Some(shape);
        Ok(())
    }
}

/// One object validated by an intrinsic attach, rooted in `slot` when given.
#[derive(Clone, Copy)]
struct IntrinsicRoot {
    slot: Option<usize>,
    object: ObjectId,
    /// Required `[[Prototype]]`, when checked.
    parent: Option<ObjectId>,
    /// Required native target and `is_constructor`; otherwise the object must
    /// be ordinary.
    native: Option<(NativeFunctionId, bool)>,
}

const fn ordinary(slot: I, object: ObjectId, parent: Option<ObjectId>) -> IntrinsicRoot {
    IntrinsicRoot {
        slot: Some(slot as usize),
        object,
        parent,
        native: None,
    }
}

const fn native(
    slot: Option<I>,
    object: ObjectId,
    target: NativeFunctionId,
    constructor: bool,
) -> IntrinsicRoot {
    IntrinsicRoot {
        slot: match slot {
            Some(slot) => Some(slot as usize),
            None => None,
        },
        object,
        parent: None,
        native: Some((target, constructor)),
    }
}

impl Heap {
    /// Validate and atomically publish one group of realm intrinsic roots.
    ///
    /// Every rooted object becomes an owned Context edge, retained as one
    /// transaction before the Context is mutated; unslotted entries are only
    /// validated. `regexp_shapes` (the RegExp object shape, then optionally
    /// its three result shapes) are retained and rooted with them.
    fn attach_intrinsics(
        &mut self,
        realm: ContextId,
        roots: &[IntrinsicRoot],
        regexp_shapes: &[ShapeId],
    ) -> Result<(), HeapError> {
        let context = self.context(realm)?;
        if roots.iter().any(|root| {
            root.slot
                .is_some_and(|slot| context.intrinsic_roots.get(slot).is_some())
        }) {
            return Err(HeapError::Invariant(
                "context already has these intrinsic roots",
            ));
        }
        let mut edges = Vec::with_capacity(roots.len() + regexp_shapes.len());
        for root in roots {
            let object = self.object(root.object)?;
            let valid = match root.native {
                None => {
                    object.kind == ObjectKind::Ordinary
                        && matches!(object.payload, ObjectPayload::Ordinary)
                }
                Some((target, constructor)) => {
                    object.is_constructor == constructor
                        && matches!(
                            &object.payload,
                            ObjectPayload::NativeFunction { data, internal: None }
                                if data.target == target && data.realm == Some(realm)
                        )
                }
            };
            let parent = match root.parent {
                Some(parent) => self.shape(object.shape)?.prototype() == Some(parent),
                None => true,
            };
            if !valid || !parent {
                return Err(HeapError::Invariant(
                    "intrinsic root has an invalid class graph",
                ));
            }
            if root.slot.is_some() {
                let edge = RawId::Object(root.object);
                if edges.contains(&edge) {
                    return Err(HeapError::Invariant("intrinsic roots share an identity"));
                }
                edges.push(edge);
            }
        }
        edges.extend(regexp_shapes.iter().copied().map(RawId::Shape));
        self.retain_edges_transactionally(&edges)?;

        let NodeData::Context(context) = &mut self.live_node_mut(RawId::Context(realm))?.data
        else {
            unreachable!("context identity was validated before retaining intrinsic roots")
        };
        for root in roots {
            if let Some(slot) = root.slot {
                context.intrinsic_roots.set(slot, root.object);
            }
        }
        context.intrinsic_roots.set_regexp_shapes(regexp_shapes);
        Ok(())
    }

    /// Publish the realm's shared frozen `%ThrowTypeError%` root after the
    /// context exists. Its native callable already owns the context, so this
    /// deliberately closes the same collectable realm cycle as QuickJS.
    pub(crate) fn attach_throw_type_error(
        &mut self,
        realm: ContextId,
        thrower: ObjectId,
    ) -> Result<(), HeapError> {
        let target = NativeFunctionId::ThrowTypeError;
        let root = native(Some(I::ThrowTypeError), thrower, target, false);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Cache the original realm-local `Array.prototype.values` callable.
    /// This is a distinct Context root because the public prototype property
    /// is writable and configurable while arguments creation must keep using
    /// the bootstrap identity.
    pub(crate) fn attach_array_prototype_values(
        &mut self,
        realm: ContextId,
        values: ObjectId,
    ) -> Result<(), HeapError> {
        let target = NativeFunctionId::ArrayPrototypeIterator(ArrayIteratorKind::Value);
        let root = native(Some(I::ArrayPrototypeValues), values, target, false);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Publish the realm's `%Function%` root after its native callable and
    /// constructor/prototype cycle have been fully initialized.
    pub(crate) fn attach_function_constructor(
        &mut self,
        realm: ContextId,
        constructor: ObjectId,
    ) -> Result<(), HeapError> {
        let target = NativeFunctionId::FunctionConstructor(DynamicFunctionKind::Normal);
        let root = native(Some(I::FunctionConstructor), constructor, target, true);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Cache the realm's original `%eval%` callable independently from its
    /// mutable global property, matching `JSContext.eval_obj`.
    pub(crate) fn attach_eval_function(
        &mut self,
        realm: ContextId,
        function: ObjectId,
    ) -> Result<(), HeapError> {
        let target = NativeFunctionId::GlobalEval;
        let root = native(Some(I::EvalFunction), function, target, false);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Publish the realm's `%Array%` root after its native callable and
    /// constructor/prototype cycle have been initialized.
    pub(crate) fn attach_array_constructor(
        &mut self,
        realm: ContextId,
        constructor: ObjectId,
    ) -> Result<(), HeapError> {
        let target = NativeFunctionId::ArrayConstructor;
        let root = native(Some(I::ArrayConstructor), constructor, target, true);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Atomically publish the realm's RegExp intrinsic roots after its native
    /// constructor, ordinary prototype, RegExp String Iterator prototype, and
    /// canonical instance shape exist.
    ///
    /// `last_index_atom` is validation-only: the shape already owns its atom
    /// edge. Passing it explicitly lets this heap layer prove that the sole
    /// instance slot really is `lastIndex` without depending on `AtomTable`
    /// string lookup.
    pub(crate) fn attach_regexp_intrinsics(
        &mut self,
        realm: ContextId,
        regexp: RegExpRealmData,
        last_index_atom: Atom,
    ) -> Result<(), HeapError> {
        let iterator_prototype = self.context(realm)?.iterator_prototype;
        let object_shape = self.shape(regexp.object_shape)?;
        let [last_index] = object_shape.entries() else {
            return Err(HeapError::Invariant(
                "RegExp object shape does not contain exactly one lastIndex property",
            ));
        };
        if object_shape.prototype() != Some(regexp.prototype)
            || last_index.atom != AtomIdx::from_raw(last_index_atom.raw())
            || last_index.flags != PropertyFlags::data(true, false, false)
        {
            return Err(HeapError::Invariant(
                "RegExp object shape has an invalid prototype or lastIndex property",
            ));
        }
        let mut shapes = vec![regexp.object_shape];
        shapes.extend(regexp.result_shapes.into_iter().flatten());
        let target = NativeFunctionId::RegExp(RegExpNativeKind::Constructor);
        self.attach_intrinsics(
            realm,
            &[
                ordinary(I::RegExpPrototype, regexp.prototype, None),
                native(Some(I::RegExpConstructor), regexp.constructor, target, true),
                ordinary(
                    I::RegExpStringIteratorPrototype,
                    regexp.string_iterator_prototype,
                    Some(iterator_prototype),
                ),
            ],
            &shapes,
        )
    }

    /// Atomically publish the realm's ordinary Map prototype and Map Iterator
    /// prototype roots after both have been initialized.
    pub(crate) fn attach_map_intrinsics(
        &mut self,
        realm: ContextId,
        map: MapRealmData,
    ) -> Result<(), HeapError> {
        let iterator_prototype = self.context(realm)?.iterator_prototype;
        self.attach_intrinsics(
            realm,
            &[
                ordinary(I::MapPrototype, map.prototype, None),
                ordinary(
                    I::MapIteratorPrototype,
                    map.iterator_prototype,
                    Some(iterator_prototype),
                ),
            ],
            &[],
        )
    }

    /// Atomically publish the realm's ordinary Set prototype and Set Iterator
    /// prototype roots after both have been initialized.
    pub(crate) fn attach_set_intrinsics(
        &mut self,
        realm: ContextId,
        set: SetRealmData,
    ) -> Result<(), HeapError> {
        let iterator_prototype = self.context(realm)?.iterator_prototype;
        self.attach_intrinsics(
            realm,
            &[
                ordinary(I::SetPrototype, set.prototype, None),
                ordinary(
                    I::SetIteratorPrototype,
                    set.iterator_prototype,
                    Some(iterator_prototype),
                ),
            ],
            &[],
        )
    }

    /// Atomically publish the realm's ordinary WeakMap prototype root.
    pub(crate) fn attach_weak_map_intrinsics(
        &mut self,
        realm: ContextId,
        weak_map: WeakMapRealmData,
    ) -> Result<(), HeapError> {
        let root = ordinary(I::WeakMapPrototype, weak_map.prototype, None);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Atomically publish the realm's ordinary WeakSet prototype root.
    pub(crate) fn attach_weak_set_intrinsics(
        &mut self,
        realm: ContextId,
        weak_set: WeakSetRealmData,
    ) -> Result<(), HeapError> {
        let root = ordinary(I::WeakSetPrototype, weak_set.prototype, None);
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Atomically publish the realm's WeakRef and FinalizationRegistry class
    /// prototype roots. Both are ordinary children of this realm's
    /// Object.prototype and are installed together by pinned QuickJS.
    pub(crate) fn attach_weak_ref_intrinsics(
        &mut self,
        realm: ContextId,
        weak_ref: WeakRefRealmData,
    ) -> Result<(), HeapError> {
        let object_prototype = Some(self.context(realm)?.object_prototype);
        self.attach_intrinsics(
            realm,
            &[
                ordinary(
                    I::WeakRefPrototype,
                    weak_ref.weak_ref_prototype,
                    object_prototype,
                ),
                ordinary(
                    I::FinalizationRegistryPrototype,
                    weak_ref.finalization_registry_prototype,
                    object_prototype,
                ),
            ],
            &[],
        )
    }

    /// Atomically publish the realm's `%ArrayBuffer.prototype%` class root
    /// after validating the public constructor relationship.
    pub(crate) fn attach_array_buffer_intrinsics(
        &mut self,
        realm: ContextId,
        constructor: ObjectId,
        array_buffer: ArrayBufferRealmData,
    ) -> Result<(), HeapError> {
        let object_prototype = Some(self.context(realm)?.object_prototype);
        let target = NativeFunctionId::ArrayBuffer(ArrayBufferNativeKind::Constructor);
        self.attach_intrinsics(
            realm,
            &[
                native(None, constructor, target, true),
                ordinary(
                    I::ArrayBufferPrototype,
                    array_buffer.prototype,
                    object_prototype,
                ),
            ],
            &[],
        )
    }

    /// Atomically publish the realm's `%SharedArrayBuffer.prototype%` class
    /// root after validating its independent public constructor relationship.
    pub(crate) fn attach_shared_array_buffer_intrinsics(
        &mut self,
        realm: ContextId,
        constructor: ObjectId,
        shared_array_buffer: SharedArrayBufferRealmData,
    ) -> Result<(), HeapError> {
        let object_prototype = Some(self.context(realm)?.object_prototype);
        let target = NativeFunctionId::SharedArrayBuffer(SharedArrayBufferNativeKind::Constructor);
        self.attach_intrinsics(
            realm,
            &[
                native(None, constructor, target, true),
                ordinary(
                    I::SharedArrayBufferPrototype,
                    shared_array_buffer.prototype,
                    object_prototype,
                ),
            ],
            &[],
        )
    }

    /// Atomically publish the twelve concrete TypedArray class prototypes
    /// after validating the abstract and concrete constructor class graph.
    pub(crate) fn attach_typed_array_intrinsics(
        &mut self,
        realm: ContextId,
        base_constructor: ObjectId,
        base_prototype: ObjectId,
        constructors: [ObjectId; TypedArrayElementKind::COUNT],
        typed_array: TypedArrayRealmData,
    ) -> Result<(), HeapError> {
        let context = self.context(realm)?;
        let base_target = NativeFunctionId::TypedArray(TypedArrayNativeKind::BaseConstructor);
        let mut roots = vec![
            IntrinsicRoot {
                parent: Some(context.function_prototype),
                ..native(None, base_constructor, base_target, true)
            },
            IntrinsicRoot {
                slot: None,
                object: base_prototype,
                parent: Some(context.object_prototype),
                native: None,
            },
        ];
        for (index, element) in TypedArrayElementKind::ALL.into_iter().enumerate() {
            let target = NativeFunctionId::TypedArray(TypedArrayNativeKind::Constructor(element));
            roots.push(IntrinsicRoot {
                parent: Some(base_constructor),
                ..native(None, constructors[index], target, true)
            });
            roots.push(IntrinsicRoot {
                slot: Some(I::TypedArrayPrototypes as usize + index),
                ..ordinary(
                    I::TypedArrayPrototypes,
                    typed_array.prototypes[index],
                    Some(base_prototype),
                )
            });
        }
        self.attach_intrinsics(realm, &roots, &[])
    }

    /// Atomically publish the realm's `%DataView.prototype%` class root after
    /// validating the public constructor relationship.
    pub(crate) fn attach_data_view_intrinsics(
        &mut self,
        realm: ContextId,
        constructor: ObjectId,
        data_view: DataViewRealmData,
    ) -> Result<(), HeapError> {
        let object_prototype = Some(self.context(realm)?.object_prototype);
        let target = NativeFunctionId::DataView(DataViewNativeKind::Constructor);
        self.attach_intrinsics(
            realm,
            &[
                native(None, constructor, target, true),
                ordinary(I::DataViewPrototype, data_view.prototype, object_prototype),
            ],
            &[],
        )
    }

    /// Atomically publish the synchronous Iterator constructor and the hidden
    /// Iterator Concat/Helper/Wrap class prototypes.
    pub(crate) fn attach_iterator_intrinsics(
        &mut self,
        realm: ContextId,
        iterator: IteratorRealmData,
    ) -> Result<(), HeapError> {
        let parent = Some(self.context(realm)?.iterator_prototype);
        let target = NativeFunctionId::IteratorConstructor;
        self.attach_intrinsics(
            realm,
            &[
                native(
                    Some(I::IteratorConstructor),
                    iterator.constructor,
                    target,
                    true,
                ),
                ordinary(
                    I::IteratorConcatPrototype,
                    iterator.concat_prototype,
                    parent,
                ),
                ordinary(
                    I::IteratorHelperPrototype,
                    iterator.helper_prototype,
                    parent,
                ),
                ordinary(I::IteratorWrapPrototype, iterator.wrap_prototype, parent),
            ],
            &[],
        )
    }

    /// Atomically publish the realm's Promise constructor and ordinary
    /// prototype after their public constructor/prototype links exist.
    pub(crate) fn attach_promise_intrinsics(
        &mut self,
        realm: ContextId,
        promise: PromiseRealmData,
    ) -> Result<(), HeapError> {
        let object_prototype = Some(self.context(realm)?.object_prototype);
        let target = NativeFunctionId::Promise(PromiseNativeKind::Constructor);
        self.attach_intrinsics(
            realm,
            &[
                ordinary(I::PromisePrototype, promise.prototype, object_prototype),
                native(
                    Some(I::PromiseConstructor),
                    promise.constructor,
                    target,
                    true,
                ),
            ],
            &[],
        )
    }

    /// Atomically publish the two realm-local synchronous-generator class
    /// prototypes after their property graph has been initialized.
    pub(crate) fn attach_generator_intrinsics(
        &mut self,
        realm: ContextId,
        generator: GeneratorRealmData,
    ) -> Result<(), HeapError> {
        let context = self.context(realm)?;
        let roots = [
            ordinary(
                I::GeneratorPrototype,
                generator.prototype,
                Some(context.iterator_prototype),
            ),
            ordinary(
                I::GeneratorFunctionPrototype,
                generator.function_prototype,
                Some(context.function_prototype),
            ),
        ];
        self.attach_intrinsics(realm, &roots, &[])
    }

    /// Atomically publish the realm-local `%AsyncFunction.prototype%` root
    /// after its hidden constructor/prototype property graph exists.
    pub(crate) fn attach_async_function_intrinsics(
        &mut self,
        realm: ContextId,
        async_function: AsyncFunctionRealmData,
    ) -> Result<(), HeapError> {
        let root = ordinary(
            I::AsyncFunctionPrototype,
            async_function.function_prototype,
            Some(self.context(realm)?.function_prototype),
        );
        self.attach_intrinsics(realm, &[root], &[])
    }

    /// Atomically publish the realm-local async-iterator/generator graph.
    pub(crate) fn attach_async_generator_intrinsics(
        &mut self,
        realm: ContextId,
        async_generator: AsyncGeneratorRealmData,
    ) -> Result<(), HeapError> {
        let context = self.context(realm)?;
        let async_iterator = async_generator.async_iterator_prototype;
        let roots = [
            ordinary(
                I::AsyncIteratorPrototype,
                async_iterator,
                Some(context.object_prototype),
            ),
            ordinary(
                I::AsyncFromSyncIteratorPrototype,
                async_generator.async_from_sync_iterator_prototype,
                Some(async_iterator),
            ),
            ordinary(
                I::AsyncGeneratorPrototype,
                async_generator.prototype,
                Some(async_iterator),
            ),
            ordinary(
                I::AsyncGeneratorFunctionPrototype,
                async_generator.function_prototype,
                Some(context.function_prototype),
            ),
        ];
        self.attach_intrinsics(realm, &roots, &[])
    }
}
