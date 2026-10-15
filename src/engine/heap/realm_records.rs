use super::*;

/// Realm-owned identities needed to allocate and dispatch genuine RegExp
/// objects and their string iterators. QuickJS roots the constructor, iterator
/// prototype, and initial instance shape independently from their public
/// property graph because user code may replace or delete those properties
/// after bootstrap.
#[derive(Clone, Copy, Debug)]
pub struct RegExpRealmData {
    pub prototype: ObjectId,
    pub constructor: ObjectId,
    /// Realm-local `%RegExpStringIteratorPrototype%`, created with the RegExp
    /// intrinsic and inheriting from this realm's `%IteratorPrototype%`.
    pub string_iterator_prototype: ObjectId,
    pub object_shape: ShapeId,
    /// Result, result-with-indices, and indices-array final layouts.
    pub result_shapes: Option<[ShapeId; 3]>,
}

/// Realm-owned identities required to allocate genuine Map objects and their
/// iterators. QuickJS roots the two class prototypes, but not the public Map
/// constructor: deleting the global and `Map.prototype.constructor` edges may
/// therefore make that constructor collectible.
#[derive(Clone, Copy, Debug)]
pub struct MapRealmData {
    pub prototype: ObjectId,
    /// Realm-local `%MapIteratorPrototype%`, inheriting from this realm's
    /// `%IteratorPrototype%`.
    pub iterator_prototype: ObjectId,
}

/// Realm-owned identities required to allocate genuine Set objects and their
/// iterators. As with Map, QuickJS roots the two class prototypes without
/// independently rooting the public Set constructor.
#[derive(Clone, Copy, Debug)]
pub struct SetRealmData {
    pub prototype: ObjectId,
    /// Realm-local `%SetIteratorPrototype%`, inheriting from this realm's
    /// `%IteratorPrototype%`.
    pub iterator_prototype: ObjectId,
}

/// Realm-owned `%WeakMap.prototype%` class root.
///
/// Weak collections have no iterator prototype. As in pinned QuickJS, the
/// public constructor remains reachable through the ordinary property graph
/// rather than through an additional Context edge.
#[derive(Clone, Copy, Debug)]
pub struct WeakMapRealmData {
    pub prototype: ObjectId,
}

/// Realm-owned `%WeakSet.prototype%` class root.
#[derive(Clone, Copy, Debug)]
pub struct WeakSetRealmData {
    pub prototype: ObjectId,
}

/// Realm-owned class prototypes installed together by QuickJS's
/// `JS_AddIntrinsicWeakRef` bootstrap step. The public constructors remain
/// reachable through their ordinary prototype/global property graph and are
/// therefore not duplicated as Context roots.
#[derive(Clone, Copy, Debug)]
pub struct WeakRefRealmData {
    pub weak_ref_prototype: ObjectId,
    pub finalization_registry_prototype: ObjectId,
}

/// Realm-owned identities required by synchronous generator functions and
/// generator instances.
///
/// QuickJS roots both class prototypes independently: generator function
/// objects inherit from `function_prototype`, while generator instances use
/// `prototype` as the cross-realm fallback when a callable's public
/// `.prototype` is not an object.
#[derive(Clone, Copy, Debug)]
pub struct GeneratorRealmData {
    pub prototype: ObjectId,
    pub function_prototype: ObjectId,
}

/// Realm-owned `%AsyncFunction.prototype%` identity.
///
/// Async function objects inherit from this ordinary object, which is itself
/// a direct child of the realm's `%Function.prototype%`. The hidden
/// `AsyncFunction` constructor remains reachable through the reciprocal
/// property graph and therefore needs no independent Context root.
#[derive(Clone, Copy, Debug)]
pub struct AsyncFunctionRealmData {
    pub function_prototype: ObjectId,
}

/// Realm-owned identities required by async-generator functions and objects.
///
/// QuickJS keeps `%AsyncIteratorPrototype%`, `%AsyncGeneratorPrototype%`, and
/// `%AsyncGeneratorFunction.prototype%` as independent context roots. The
/// hidden dynamic constructor remains reachable from the reciprocal graph.
#[derive(Clone, Copy, Debug)]
pub struct AsyncGeneratorRealmData {
    pub async_iterator_prototype: ObjectId,
    /// Realm-local `%AsyncFromSyncIteratorPrototype%`, inheriting from this
    /// realm's `%AsyncIteratorPrototype%`.
    pub async_from_sync_iterator_prototype: ObjectId,
    pub prototype: ObjectId,
    pub function_prototype: ObjectId,
}

/// Realm-owned Promise identities used by allocation and species fallback.
///
/// Both identities remain explicit Context roots.  User code may delete the
/// public global and constructor/prototype properties without changing the
/// intrinsic identities used by Promise abstract operations.
#[derive(Clone, Copy, Debug)]
pub struct PromiseRealmData {
    pub prototype: ObjectId,
    pub constructor: ObjectId,
}

/// Realm-owned identities used by the synchronous Iterator helpers proposal.
///
/// `%IteratorPrototype%` is already a mandatory [`ContextData`] root because
/// concrete built-in iterators depend on it before the public `%Iterator%`
/// constructor is installed. The four identities here are attached later as
/// one transaction, matching QuickJS's `iterator_ctor` and class-prototype
/// roots for `JS_CLASS_ITERATOR_CONCAT`, `JS_CLASS_ITERATOR_HELPER`, and
/// `JS_CLASS_ITERATOR_WRAP`.
#[derive(Clone, Copy, Debug)]
pub struct IteratorRealmData {
    pub constructor: ObjectId,
    pub concat_prototype: ObjectId,
    pub helper_prototype: ObjectId,
    pub wrap_prototype: ObjectId,
}

/// Realm-local `%ArrayBuffer.prototype%` class root.
///
/// The backing bytes live on each branded object, but constructor-realm
/// fallback must retain the original prototype even after authored code
/// replaces or deletes the writable global `ArrayBuffer` binding.
#[derive(Clone, Copy, Debug)]
pub struct ArrayBufferRealmData {
    pub prototype: ObjectId,
}

/// Realm-local `%SharedArrayBuffer.prototype%` class root.
///
/// Shared wrappers may outlive and share backing stores across runtimes, but
/// their JavaScript prototype identity remains owned by the importing realm.
#[derive(Clone, Copy, Debug)]
pub struct SharedArrayBufferRealmData {
    pub prototype: ObjectId,
}

/// Realm-local `%DataView.prototype%` class root.
///
/// DataView instances retain their backing ArrayBuffer-family object directly. The realm
/// keeps only the original prototype identity used by constructor fallback.
#[derive(Clone, Copy, Debug)]
pub struct DataViewRealmData {
    pub prototype: ObjectId,
}

/// Realm-local concrete TypedArray class prototypes.
///
/// QuickJS roots these twelve identities in `ctx->class_proto`. The hidden
/// abstract prototype stays reachable through their `[[Prototype]]` edges;
/// retaining it separately here would add an arena root that upstream lacks.
#[derive(Clone, Copy, Debug)]
pub struct TypedArrayRealmData {
    pub prototypes: [ObjectId; TypedArrayElementKind::COUNT],
}

/// Optional realm-owned intrinsic root slots, in Context GC edge order.
///
/// Ranged entries name the first of `PrimitiveKind::COUNT` primitive wrapper
/// prototypes, `TypedArrayElementKind::COUNT` concrete TypedArray prototypes,
/// and `NativeErrorKind::COUNT` native error prototypes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Intrinsic {
    PrimitivePrototypes = 0,
    DatePrototype = 5,
    RegExpPrototype,
    RegExpConstructor,
    RegExpStringIteratorPrototype,
    MapPrototype,
    MapIteratorPrototype,
    SetPrototype,
    SetIteratorPrototype,
    WeakMapPrototype,
    WeakSetPrototype,
    WeakRefPrototype,
    FinalizationRegistryPrototype,
    ArrayBufferPrototype,
    SharedArrayBufferPrototype,
    DataViewPrototype,
    TypedArrayPrototypes,
    GeneratorPrototype = 32,
    GeneratorFunctionPrototype,
    AsyncFunctionPrototype,
    AsyncIteratorPrototype,
    AsyncFromSyncIteratorPrototype,
    AsyncGeneratorPrototype,
    AsyncGeneratorFunctionPrototype,
    PromisePrototype,
    PromiseConstructor,
    IteratorConstructor,
    IteratorConcatPrototype,
    IteratorHelperPrototype,
    IteratorWrapPrototype,
    FunctionConstructor,
    ArrayConstructor,
    /// Original `Array.prototype.values`, cached like QuickJS's
    /// `JSContext.array_proto_values` for every later arguments object.
    ArrayPrototypeValues,
    /// Shared frozen poison callable of restricted function accessors.
    ThrowTypeError,
    /// Original `%eval%`, compared by direct-eval dispatch after user code
    /// mutates the global property.
    EvalFunction,
    ErrorPrototype,
    NativeErrorPrototypes,
}

const _: () = assert!(Intrinsic::DatePrototype as usize == PrimitiveKind::COUNT);
const _: () = assert!(
    Intrinsic::GeneratorPrototype as usize
        == Intrinsic::TypedArrayPrototypes as usize + TypedArrayElementKind::COUNT
);

/// Number of optional intrinsic root slots.
pub(crate) const INTRINSIC_COUNT: usize =
    Intrinsic::NativeErrorPrototypes as usize + NativeErrorKind::COUNT;
/// Presence bit of the three RegExp result shapes, past every slot bit.
const REGEXP_RESULT_SHAPES: u64 = 1 << 63;
const _: () = assert!(INTRINSIC_COUNT < 63);

/// Optional intrinsic roots, each present only after its attach succeeded.
///
/// Slots are a bare array plus a presence mask rather than
/// `[Option<ObjectId>; N]`, which would add a discriminant word per slot.
#[derive(Debug)]
pub struct IntrinsicRoots {
    objects: [ObjectId; INTRINSIC_COUNT],
    /// RegExp object shape, then the result, result-with-indices, and
    /// indices-array shapes; rooted with `Intrinsic::RegExpPrototype`.
    regexp_shapes: [ShapeId; 4],
    present: u64,
}

impl IntrinsicRoots {
    const fn new() -> Self {
        Self {
            objects: [ObjectId {
                index: 0,
                generation: 0,
            }; INTRINSIC_COUNT],
            regexp_shapes: [ShapeId {
                index: 0,
                generation: 0,
            }; 4],
            present: 0,
        }
    }

    #[inline]
    pub(crate) const fn get(&self, slot: usize) -> Option<ObjectId> {
        if self.present & (1 << slot) == 0 {
            None
        } else {
            Some(self.objects[slot])
        }
    }

    pub(super) const fn set(&mut self, slot: usize, object: ObjectId) {
        self.objects[slot] = object;
        self.present |= 1 << slot;
    }

    /// Upper bound of the owned edges visited by [`Self::visit`], counting
    /// all four RegExp shapes once the RegExp roots exist.
    pub(super) const fn len(&self) -> usize {
        self.present.count_ones() as usize + 3
    }

    pub(super) fn set_regexp_shapes(&mut self, shapes: &[ShapeId]) {
        self.regexp_shapes[..shapes.len()].copy_from_slice(shapes);
        if shapes.len() == 4 {
            self.present |= REGEXP_RESULT_SHAPES;
        }
    }

    /// Owned edges in Context GC order: every present slot, with the RegExp
    /// shapes after the RegExp roots, and `globals` before the Error roots.
    pub(super) fn visit<E>(
        &self,
        globals: [ObjectId; 2],
        visit: &mut impl FnMut(RawId) -> Result<(), E>,
    ) -> Result<(), E> {
        let objects = |slots: std::ops::Range<usize>| {
            slots.filter_map(|slot| self.get(slot)).map(RawId::Object)
        };
        for edge in objects(0..Intrinsic::MapPrototype as usize) {
            visit(edge)?;
        }
        if self.get(Intrinsic::RegExpPrototype as usize).is_some() {
            let shapes = if self.present & REGEXP_RESULT_SHAPES == 0 {
                1
            } else {
                4
            };
            for shape in &self.regexp_shapes[..shapes] {
                visit(RawId::Shape(*shape))?;
            }
        }
        let globals = globals.into_iter().map(RawId::Object);
        for edge in objects(Intrinsic::MapPrototype as usize..Intrinsic::ErrorPrototype as usize)
            .chain(globals)
            .chain(objects(Intrinsic::ErrorPrototype as usize..INTRINSIC_COUNT))
        {
            visit(edge)?;
        }
        Ok(())
    }
}

/// Single-slot `ContextData` accessors named after their intrinsics.
macro_rules! intrinsic_accessors {
    ($($name:ident => $slot:ident),* $(,)?) => {$(
        #[inline]
        #[must_use]
        pub fn $name(&self) -> Option<ObjectId> {
            self.intrinsic(Intrinsic::$slot)
        }
    )*};
}

/// `ContextData` accessors assembling one attached intrinsic group.
macro_rules! intrinsic_groups {
    ($($name:ident -> $group:ident { $($field:ident: $slot:ident),* $(,)? }),* $(,)?) => {$(
        #[must_use]
        pub fn $name(&self) -> Option<$group> {
            Some($group { $($field: self.intrinsic(Intrinsic::$slot)?),* })
        }
    )*};
}

/// Realm-owned roots which participate in QuickJS's cycle graph.
///
/// The bootstrap roots needed by ordinary script evaluation are explicit;
/// additional intrinsic and module roots can extend the vectors without
/// changing `ContextId` ownership.
/// Arguments objects with up to this many actual arguments share a cached shape.
pub(crate) const ARGUMENTS_SHAPE_COUNTS: usize = 8;

#[derive(Debug)]
pub struct ContextData {
    /// Stable public handle identity assigned by `Runtime::new_context`.
    ///
    /// Low-level heap tests and internal job-only realms deliberately leave
    /// this absent; only a user-created Context may be reconstructed for a
    /// host callback.
    pub(crate) public_id: Option<u64>,
    pub object_prototype: ObjectId,
    pub function_prototype: ObjectId,
    /// Realm-local `%Array.prototype%`. QuickJS creates the prototype itself
    /// as a genuine Array exotic object, so this root is never substituted by
    /// an ordinary object even before the global `%Array%` constructor is
    /// published.
    pub array_prototype: ObjectId,
    /// Realm-local `%IteratorPrototype%`.  It is rooted independently rather
    /// than rediscovered through a concrete iterator so empty realms retain
    /// the intrinsic identity required by cross-realm iterator creation.
    pub iterator_prototype: ObjectId,
    /// Realm-local `%ArrayIteratorPrototype%`, whose prototype is the realm's
    /// `%IteratorPrototype%`.
    pub array_iterator_prototype: ObjectId,
    /// Realm-local `%StringIteratorPrototype%`, whose prototype is the realm's
    /// `%IteratorPrototype%`.
    pub string_iterator_prototype: ObjectId,
    pub global_object: ObjectId,
    /// Null-prototype storage for global lexical bindings (`let`/`const`).
    pub global_var_object: ObjectId,
    /// Optional intrinsic roots attached after the cyclic Context is
    /// published; see [`Intrinsic`].
    pub(crate) intrinsic_roots: IntrinsicRoots,
    /// QuickJS keeps Math.random's xorshift64* state on each JSContext.  Zero
    /// is reserved for the not-yet-seeded bootstrap state.
    pub(in crate::engine::heap) math_random_state: u64,
    pub global_objects: Vec<ObjectId>,
    pub intrinsics: Vec<RawValue>,
    pub initial_shapes: Vec<ShapeId>,
    /// Shapes of arguments objects created in this realm, by mapped kind and
    /// actual argument count, so creating one does not build a shape anew.
    pub(crate) arguments_shapes: [[Option<ShapeId>; ARGUMENTS_SHAPE_COUNTS]; 2],
    pub(crate) regexp_group_shapes: std::collections::HashMap<Vec<Atom>, ShapeId>,
    /// Context-local `JSModuleDef` ownership, matching QuickJS's
    /// `JSContext.loaded_modules` rather than a Rust-side parallel graph.
    pub(crate) loaded_modules: LoadedModuleCache,
}

// Optional roots share one presence mask rather than per-slot `Option`s.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<ContextData>() <= 984);

impl ContextData {
    /// Construct the complete mandatory realm root set.
    ///
    /// Iterator prototype identities are required arguments rather than
    /// builder-populated placeholders so the public low-level allocator
    /// cannot publish a realm whose `%IteratorPrototype%` silently aliases
    /// `%Object.prototype%`.
    #[must_use]
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        object_prototype: ObjectId,
        function_prototype: ObjectId,
        array_prototype: ObjectId,
        iterator_prototype: ObjectId,
        array_iterator_prototype: ObjectId,
        string_iterator_prototype: ObjectId,
        global_object: ObjectId,
        global_var_object: ObjectId,
    ) -> Self {
        Self {
            public_id: None,
            object_prototype,
            function_prototype,
            array_prototype,
            iterator_prototype,
            array_iterator_prototype,
            string_iterator_prototype,
            global_object,
            global_var_object,
            intrinsic_roots: IntrinsicRoots::new(),
            math_random_state: 0,
            global_objects: Vec::new(),
            intrinsics: Vec::new(),
            initial_shapes: Vec::new(),
            arguments_shapes: [[None; ARGUMENTS_SHAPE_COUNTS]; 2],
            regexp_group_shapes: std::collections::HashMap::new(),
            loaded_modules: LoadedModuleCache::new(),
        }
    }

    /// Attach the stable public identity of a user-created Context.
    #[must_use]
    pub(crate) const fn with_public_id(mut self, id: u64) -> Self {
        self.public_id = Some(id);
        self
    }

    /// Attach one implemented primitive wrapper prototype to this realm.
    #[must_use]
    pub const fn with_primitive_prototype(
        mut self,
        kind: PrimitiveKind,
        prototype: ObjectId,
    ) -> Self {
        self.intrinsic_roots.set(
            Intrinsic::PrimitivePrototypes as usize + kind.index(),
            prototype,
        );
        self
    }

    /// Attach the ordinary Date prototype to this realm before publish.
    #[must_use]
    pub const fn with_date_prototype(mut self, prototype: ObjectId) -> Self {
        self.intrinsic_roots
            .set(Intrinsic::DatePrototype as usize, prototype);
        self
    }

    /// Attach the Error intrinsic prototype graph to this realm.
    #[must_use]
    pub const fn with_error_prototypes(
        mut self,
        error_prototype: ObjectId,
        native_error_prototypes: [ObjectId; NativeErrorKind::COUNT],
    ) -> Self {
        self.intrinsic_roots
            .set(Intrinsic::ErrorPrototype as usize, error_prototype);
        let mut index = 0;
        while index < NativeErrorKind::COUNT {
            let slot = Intrinsic::NativeErrorPrototypes as usize + index;
            self.intrinsic_roots
                .set(slot, native_error_prototypes[index]);
            index += 1;
        }
        self
    }
    #[inline]
    #[must_use]
    pub fn intrinsic(&self, slot: Intrinsic) -> Option<ObjectId> {
        self.intrinsic_roots.get(slot as usize)
    }

    #[inline]
    #[must_use]
    pub fn primitive_prototype(&self, kind: PrimitiveKind) -> Option<ObjectId> {
        self.intrinsic_roots
            .get(Intrinsic::PrimitivePrototypes as usize + kind.index())
    }

    intrinsic_accessors! {
        date_prototype => DatePrototype,
        function_constructor => FunctionConstructor,
        array_constructor => ArrayConstructor,
        array_prototype_values => ArrayPrototypeValues,
        throw_type_error => ThrowTypeError,
        eval_function => EvalFunction,
        error_prototype => ErrorPrototype,
    }

    intrinsic_groups! {
        map -> MapRealmData { prototype: MapPrototype, iterator_prototype: MapIteratorPrototype },
        set -> SetRealmData { prototype: SetPrototype, iterator_prototype: SetIteratorPrototype },
        weak_map -> WeakMapRealmData { prototype: WeakMapPrototype },
        weak_set -> WeakSetRealmData { prototype: WeakSetPrototype },
        weak_ref -> WeakRefRealmData {
            weak_ref_prototype: WeakRefPrototype,
            finalization_registry_prototype: FinalizationRegistryPrototype,
        },
        array_buffer -> ArrayBufferRealmData { prototype: ArrayBufferPrototype },
        shared_array_buffer -> SharedArrayBufferRealmData { prototype: SharedArrayBufferPrototype },
        data_view -> DataViewRealmData { prototype: DataViewPrototype },
        generator -> GeneratorRealmData {
            prototype: GeneratorPrototype,
            function_prototype: GeneratorFunctionPrototype,
        },
        async_function -> AsyncFunctionRealmData { function_prototype: AsyncFunctionPrototype },
        async_generator -> AsyncGeneratorRealmData {
            async_iterator_prototype: AsyncIteratorPrototype,
            async_from_sync_iterator_prototype: AsyncFromSyncIteratorPrototype,
            prototype: AsyncGeneratorPrototype,
            function_prototype: AsyncGeneratorFunctionPrototype,
        },
        promise -> PromiseRealmData {
            prototype: PromisePrototype,
            constructor: PromiseConstructor,
        },
        iterator -> IteratorRealmData {
            constructor: IteratorConstructor,
            concat_prototype: IteratorConcatPrototype,
            helper_prototype: IteratorHelperPrototype,
            wrap_prototype: IteratorWrapPrototype,
        },
    }

    #[must_use]
    pub fn native_error_prototype(&self, kind: NativeErrorKind) -> Option<ObjectId> {
        self.intrinsic_roots
            .get(Intrinsic::NativeErrorPrototypes as usize + kind.index())
    }

    #[must_use]
    pub fn regexp(&self) -> Option<RegExpRealmData> {
        let shapes = &self.intrinsic_roots.regexp_shapes;
        Some(RegExpRealmData {
            prototype: self.intrinsic(Intrinsic::RegExpPrototype)?,
            constructor: self.intrinsic(Intrinsic::RegExpConstructor)?,
            string_iterator_prototype: self.intrinsic(Intrinsic::RegExpStringIteratorPrototype)?,
            object_shape: shapes[0],
            result_shapes: (self.intrinsic_roots.present & REGEXP_RESULT_SHAPES != 0)
                .then(|| [shapes[1], shapes[2], shapes[3]]),
        })
    }

    #[must_use]
    pub fn typed_array(&self) -> Option<TypedArrayRealmData> {
        let first = Intrinsic::TypedArrayPrototypes as usize;
        self.intrinsic_roots.get(first)?;
        Some(TypedArrayRealmData {
            prototypes: std::array::from_fn(|index| self.intrinsic_roots.objects[first + index]),
        })
    }
}
