//! Install primitive and function intrinsics and their initial property relationships.

mod construction;

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::builtins::native::{
    BigIntAsNKind, DynamicFunctionKind, FunctionDebugPosition, GlobalNumberPredicateKind,
    GlobalUriCodecKind, NativeFunctionId, NumberFormatKind, NumberParseKind, NumberPredicateKind,
    PrimitiveKind, SymbolRegistryKind,
};
use crate::engine::heap::ContextId;
use crate::engine::object::builtin_properties::{
    BuiltinEntry, BuiltinKey, BuiltinValue, METHOD_FLAGS, builtin_getter, method, symbol_method,
    to_string_tag,
};
use crate::engine::object::shape::PropertyFlags;

use crate::engine::object::{
    AccessorValue, CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor,
    PropertyKey, WellKnownSymbol,
};
use crate::engine::value::{JsString, Value};

impl Runtime {
    /// Define `@@toStringTag` as a non-writable, non-enumerable, configurable
    /// string, the shape every intrinsic tag uses.
    pub(crate) fn define_to_string_tag(
        &self,
        object: &ObjectRef,
        value: &'static str,
    ) -> Result<(), RuntimeError> {
        let key = PropertyKey::from(self.well_known_symbol(WellKnownSymbol::ToStringTag)?);
        let defined = self.define_own_property(
            object,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::String(JsString::from_static(value))),
                writable: DescriptorField::Present(false),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )?;
        if defined {
            Ok(())
        } else {
            Err(RuntimeError::Invariant(
                "intrinsic toStringTag definition was rejected",
            ))
        }
    }

    pub(crate) fn initialize_function_constructor(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        // QuickJS publishes Function after its prototype table and the Error
        // family, then closes the constructor/prototype cycle. Its magic
        // selector makes this same handler reusable by the future dynamic
        // GeneratorFunction/AsyncFunction constructors.
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::FunctionConstructor(DynamicFunctionKind::Normal),
            1,
            "Function",
            1,
        )?;
        self.define_function_data_property(
            global_object,
            "Function",
            Value::Object(constructor.as_object().try_clone()?),
            true,
            true,
        )?;
        self.define_constructor_relationship(&constructor, function_prototype)?;
        self.0
            .state
            .borrow_mut()
            .heap
            .attach_function_constructor(realm, constructor.as_object().object_id())?;
        Ok(())
    }

    pub(crate) fn initialize_number_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        number_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId as F;
        use NumberFormatKind as Format;
        use NumberPredicateKind as Predicate;
        const KIND: PrimitiveKind = PrimitiveKind::Number;
        const fn constant(name: &'static str, value: f64) -> BuiltinEntry {
            BuiltinEntry::Value {
                name,
                value: BuiltinValue::Float(value),
                writable: false,
                configurable: false,
            }
        }
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(
                F::NumberPrototypeFormat(Format::Exponential),
                "toExponential",
                1,
                1,
            ),
            method(F::NumberPrototypeFormat(Format::Fixed), "toFixed", 1, 1),
            method(
                F::NumberPrototypeFormat(Format::Precision),
                "toPrecision",
                1,
                1,
            ),
            method(F::PrimitivePrototypeToString(KIND), "toString", 1, 1),
            method(
                F::NumberPrototypeFormat(Format::LocaleString),
                "toLocaleString",
                0,
                0,
            ),
            method(F::PrimitivePrototypeValueOf(KIND), "valueOf", 0, 0),
        ];
        const CONSTRUCTOR: &[BuiltinEntry] = &[
            method(F::NumberPredicate(Predicate::IsNaN), "isNaN", 1, 1),
            method(F::NumberPredicate(Predicate::IsFinite), "isFinite", 1, 1),
            method(F::NumberPredicate(Predicate::IsInteger), "isInteger", 1, 1),
            method(
                F::NumberPredicate(Predicate::IsSafeInteger),
                "isSafeInteger",
                1,
                1,
            ),
            constant("MAX_VALUE", f64::MAX),
            constant("MIN_VALUE", f64::from_bits(1)),
            constant("NaN", f64::NAN),
            constant("NEGATIVE_INFINITY", f64::NEG_INFINITY),
            constant("POSITIVE_INFINITY", f64::INFINITY),
            constant("EPSILON", f64::EPSILON),
            constant("MAX_SAFE_INTEGER", 9_007_199_254_740_991.0),
            constant("MIN_SAFE_INTEGER", -9_007_199_254_740_991.0),
        ];
        self.define_builtin_entries(number_prototype, realm, PROTOTYPE)?;
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            F::PrimitiveConstructor(KIND),
            1,
            "Number",
            1,
        )?;
        // Upstream captures the already-published global parser callables by
        // identity before adding the remaining Number statics.
        for name in ["parseInt", "parseFloat"] {
            let key = self.intern_property_key(name)?;
            self.define_builtin_alias(
                constructor.as_object(),
                realm,
                global_object,
                &key,
                BuiltinKey::Name(name),
            )?;
        }
        self.define_builtin_entries(constructor.as_object(), realm, CONSTRUCTOR)?;
        self.define_global_function(global_object, "Number", &constructor)?;
        self.define_constructor_relationship(&constructor, number_prototype)
    }

    pub(crate) fn initialize_boolean_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        boolean_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        const KIND: PrimitiveKind = PrimitiveKind::Boolean;
        // QuickJS installs the complete Boolean prototype table before the
        // constructor back-reference, which fixes own-key order as
        // `toString,valueOf,constructor`.
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(
                NativeFunctionId::PrimitivePrototypeToString(KIND),
                "toString",
                0,
                0,
            ),
            method(
                NativeFunctionId::PrimitivePrototypeValueOf(KIND),
                "valueOf",
                0,
                0,
            ),
        ];
        self.define_builtin_entries(boolean_prototype, realm, PROTOTYPE)?;
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::PrimitiveConstructor(KIND),
            1,
            "Boolean",
            1,
        )?;
        self.define_global_function(global_object, "Boolean", &constructor)?;
        self.define_constructor_relationship(&constructor, boolean_prototype)
    }

    /// Install the implemented String conversion pair before the constructor
    /// relationship is published later in bootstrap. This is not the prefix of
    /// QuickJS's 53-key table: these two brand methods are also the observable
    /// ordinary-ToPrimitive dependency for every generic String prototype
    /// method. The implemented method slice is already installed first;
    /// earlier missing String entries must continue to enter before this pair,
    /// while case conversion and later entries remain after it in pinned table
    /// order when each fresh context is bootstrapped.
    pub(crate) fn initialize_string_conversion_core(
        &self,
        realm: ContextId,
        string_prototype: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        const KIND: PrimitiveKind = PrimitiveKind::String;
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(
                NativeFunctionId::PrimitivePrototypeToString(KIND),
                "toString",
                0,
                0,
            ),
            method(
                NativeFunctionId::PrimitivePrototypeValueOf(KIND),
                "valueOf",
                0,
                0,
            ),
        ];
        self.define_builtin_entries(string_prototype, realm, PROTOTYPE)
    }

    /// Complete the String iterator class slice: the generic String method,
    /// branded iterator prototype, native-next ABI and configurable tag.
    pub(crate) fn initialize_string_iterator_intrinsics(
        &self,
        realm: ContextId,
        string_prototype: &ObjectRef,
        string_iterator_prototype: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        const PROTOTYPE: &[BuiltinEntry] = &[symbol_method(
            WellKnownSymbol::Iterator,
            NativeFunctionId::StringPrototypeIterator,
            "[Symbol.iterator]",
            0,
            METHOD_FLAGS,
        )];
        const ITERATOR_PROTOTYPE: &[BuiltinEntry] = &[
            method(NativeFunctionId::StringIteratorNext, "next", 0, 0),
            to_string_tag("String Iterator"),
        ];
        self.define_builtin_entries(string_prototype, realm, PROTOTYPE)?;
        self.define_builtin_entries(string_iterator_prototype, realm, ITERATOR_PROTOTYPE)
    }

    pub(crate) fn initialize_symbol_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        symbol_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId as F;
        use WellKnownSymbol as S;
        const KIND: PrimitiveKind = PrimitiveKind::Symbol;
        const fn symbol(name: &'static str, symbol: S) -> BuiltinEntry {
            BuiltinEntry::Value {
                name,
                value: BuiltinValue::Symbol(symbol),
                writable: false,
                configurable: false,
            }
        }
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(F::PrimitivePrototypeToString(KIND), "toString", 0, 0),
            method(F::PrimitivePrototypeValueOf(KIND), "valueOf", 0, 0),
            // This is a pinned QuickJS quirk: the table entry is a C function,
            // but a symbol-named method is installed non-writable.
            symbol_method(
                S::ToPrimitive,
                F::PrimitivePrototypeValueOf(KIND),
                "[Symbol.toPrimitive]",
                1,
                PropertyFlags::data(false, false, true),
            ),
            to_string_tag("Symbol"),
            builtin_getter!(F::SymbolPrototypeDescription, "description"),
        ];
        const CONSTRUCTOR: &[BuiltinEntry] = &[
            method(F::SymbolRegistry(SymbolRegistryKind::For), "for", 1, 1),
            method(
                F::SymbolRegistry(SymbolRegistryKind::KeyFor),
                "keyFor",
                1,
                1,
            ),
            symbol("toPrimitive", S::ToPrimitive),
            symbol("iterator", S::Iterator),
            symbol("match", S::Match),
            symbol("matchAll", S::MatchAll),
            symbol("replace", S::Replace),
            symbol("search", S::Search),
            symbol("split", S::Split),
            symbol("toStringTag", S::ToStringTag),
            symbol("isConcatSpreadable", S::IsConcatSpreadable),
            symbol("hasInstance", S::HasInstance),
            symbol("species", S::Species),
            symbol("unscopables", S::Unscopables),
            symbol("asyncIterator", S::AsyncIterator),
        ];
        self.define_builtin_entries(symbol_prototype, realm, PROTOTYPE)?;
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            F::PrimitiveConstructor(KIND),
            1,
            "Symbol",
            0,
        )?;
        self.define_builtin_entries(constructor.as_object(), realm, CONSTRUCTOR)?;
        self.define_global_function(global_object, "Symbol", &constructor)?;
        self.define_constructor_relationship(&constructor, symbol_prototype)
    }

    pub(crate) fn initialize_bigint_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        bigint_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId as F;
        const KIND: PrimitiveKind = PrimitiveKind::BigInt;
        // `js_bigint_proto_funcs` is installed before the constructor
        // back-reference. toString has observable length zero even though its
        // C handler reads one optional, padded radix argument.
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(F::PrimitivePrototypeToString(KIND), "toString", 0, 1),
            method(F::PrimitivePrototypeValueOf(KIND), "valueOf", 0, 0),
            to_string_tag("BigInt"),
        ];
        const CONSTRUCTOR: &[BuiltinEntry] = &[
            method(F::BigIntAsN(BigIntAsNKind::AsUintN), "asUintN", 2, 2),
            method(F::BigIntAsN(BigIntAsNKind::AsIntN), "asIntN", 2, 2),
        ];
        self.define_builtin_entries(bigint_prototype, realm, PROTOTYPE)?;
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            F::PrimitiveConstructor(KIND),
            1,
            "BigInt",
            1,
        )?;
        self.define_builtin_entries(constructor.as_object(), realm, CONSTRUCTOR)?;
        self.define_global_function(global_object, "BigInt", &constructor)?;
        self.define_constructor_relationship(&constructor, bigint_prototype)
    }

    pub(crate) fn initialize_global_functions_prefix(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use GlobalUriCodecKind as Uri;
        use NativeFunctionId as F;
        const GLOBALS: &[BuiltinEntry] = &[
            method(
                F::GlobalNumberPredicate(GlobalNumberPredicateKind::IsNaN),
                "isNaN",
                1,
                1,
            ),
            method(
                F::GlobalNumberPredicate(GlobalNumberPredicateKind::IsFinite),
                "isFinite",
                1,
                1,
            ),
            method(F::GlobalUriCodec(Uri::DecodeUri), "decodeURI", 1, 1),
            method(
                F::GlobalUriCodec(Uri::DecodeUriComponent),
                "decodeURIComponent",
                1,
                1,
            ),
            method(F::GlobalUriCodec(Uri::EncodeUri), "encodeURI", 1, 1),
            method(
                F::GlobalUriCodec(Uri::EncodeUriComponent),
                "encodeURIComponent",
                1,
                1,
            ),
            method(F::GlobalUriCodec(Uri::Escape), "escape", 1, 1),
            method(F::GlobalUriCodec(Uri::Unescape), "unescape", 1, 1),
        ];
        // QuickJS publishes these entries at the head of `js_global_funcs`,
        // before the frozen global constants and `%Number%`. Number's parser
        // statics later capture the first two callable identities.
        for (kind, name, arity) in [
            (NumberParseKind::ParseInt, "parseInt", 2),
            (NumberParseKind::ParseFloat, "parseFloat", 1),
        ] {
            let callable = self.new_native_builtin(
                function_prototype,
                realm,
                F::GlobalNumberParse(kind),
                arity,
                name,
                i32::from(arity),
            )?;
            self.define_global_function(global_object, name, &callable)?;
        }
        self.define_builtin_entries(global_object, realm, GLOBALS)
    }

    pub(crate) fn initialize_global_primitive_constants(
        &self,
        realm: ContextId,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        const fn constant(name: &'static str, value: BuiltinValue) -> BuiltinEntry {
            BuiltinEntry::Value {
                name,
                value,
                writable: false,
                configurable: false,
            }
        }
        // QuickJS `js_global_funcs` entries immediately before @@toStringTag:
        // all three are non-writable, non-enumerable and non-configurable.
        const GLOBALS: &[BuiltinEntry] = &[
            constant("Infinity", BuiltinValue::Float(f64::INFINITY)),
            constant("NaN", BuiltinValue::Float(f64::NAN)),
            constant("undefined", BuiltinValue::Undefined),
        ];
        self.define_builtin_entries(global_object, realm, GLOBALS)
    }

    pub(crate) fn initialize_global_this(
        &self,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::GlobalThis)?;
        let defined = self.define_own_property(
            global_object,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::Object(global_object.try_clone()?)),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )?;
        if !defined {
            return Err(RuntimeError::Invariant(
                "globalThis definition was rejected",
            ));
        }
        Ok(())
    }

    pub(crate) fn initialize_function_restricted_properties(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        // JS_AddIntrinsicBaseObjects creates one frozen %ThrowTypeError% and
        // installs that same callable as both halves of both legacy poison
        // accessors before publishing the Function prototype method table.
        let throw_type_error = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::ThrowTypeError,
            0,
            "",
            0,
        )?;
        for name in ["length", "name"] {
            let key = self.intern_property_key(name)?;
            let accepted = self.define_own_property(
                throw_type_error.as_object(),
                &key,
                &OrdinaryPropertyDescriptor {
                    writable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(false),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )?;
            if !accepted {
                return Err(RuntimeError::Invariant(
                    "%ThrowTypeError% own property could not be frozen",
                ));
            }
        }
        self.prevent_extensions(throw_type_error.as_object())?;

        for name in ["caller", "arguments"] {
            let key = self.intern_property_key(name)?;
            let accepted = self.define_own_property(
                function_prototype,
                &key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(
                        throw_type_error.try_clone()?,
                    )),
                    set: DescriptorField::Present(AccessorValue::Callable(
                        throw_type_error.try_clone()?,
                    )),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )?;
            if !accepted {
                return Err(RuntimeError::Invariant(
                    "Function.prototype poison accessor definition was rejected",
                ));
            }
        }

        self.0
            .state
            .borrow_mut()
            .heap
            .attach_throw_type_error(realm, throw_type_error.as_object().object_id())?;
        Ok(())
    }

    pub(crate) fn initialize_function_prototype_methods(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId as F;
        // Unlike C functions in JS_SetPropertyFunctionList, QuickJS's
        // CGETSET entries are instantiated eagerly. Keep that distinction so
        // descriptor reads and realm/GC edges match the upstream table.
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(F::FunctionPrototypeCall, "call", 1, 1),
            method(F::FunctionPrototypeApply, "apply", 2, 2),
            method(F::FunctionPrototypeBind, "bind", 1, 1),
            method(F::FunctionPrototypeToString, "toString", 0, 0),
            symbol_method(
                WellKnownSymbol::HasInstance,
                F::FunctionPrototypeHasInstance,
                "[Symbol.hasInstance]",
                1,
                PropertyFlags::data(false, false, false),
            ),
            builtin_getter!(F::FunctionPrototypeFileName, "fileName"),
            builtin_getter!(
                F::FunctionPrototypePosition(FunctionDebugPosition::Line),
                "lineNumber"
            ),
            builtin_getter!(
                F::FunctionPrototypePosition(FunctionDebugPosition::Column),
                "columnNumber"
            ),
        ];
        self.define_builtin_entries(function_prototype, realm, PROTOTYPE)
    }

    /// Publish `function` as a writable, configurable global data property.
    pub(crate) fn define_global_function(
        &self,
        global_object: &ObjectRef,
        name: &str,
        function: &CallableRef,
    ) -> Result<(), RuntimeError> {
        let value = Value::Object(function.as_object().try_clone()?);
        self.define_function_data_property(global_object, name, value, true, true)
    }

    /// Create a hidden dynamic-function constructor (`GeneratorFunction` and
    /// friends) whose prototype is `%Function%`. JS_NewCConstructor keeps the
    /// constructor's own `prototype` fully fixed; JS_NEW_CTOR_READONLY only
    /// changes the reciprocal `prototype.constructor` descriptor.
    pub(crate) fn new_dynamic_function_constructor(
        &self,
        realm: ContextId,
        kind: DynamicFunctionKind,
        name: &str,
        prototype: &ObjectRef,
    ) -> Result<CallableRef, RuntimeError> {
        let function_constructor = self
            .0
            .state
            .borrow()
            .heap
            .context(realm)?
            .function_constructor()
            .ok_or(RuntimeError::Invariant(
                "dynamic function initialization requires the Function constructor",
            ))?;
        let function_constructor =
            ObjectRef::from_borrowed_handle(self.clone(), function_constructor)?;
        let constructor = self.new_native_builtin(
            &function_constructor,
            realm,
            NativeFunctionId::FunctionConstructor(kind),
            1,
            name,
            1,
        )?;
        self.define_readonly_constructor_pair(constructor.as_object(), prototype, false)?;
        Ok(constructor)
    }

    /// Define `constructor.prototype` (non-writable, configurable only when
    /// `prototype_configurable`) and a non-writable `prototype.constructor`.
    pub(crate) fn define_readonly_constructor_pair(
        &self,
        constructor: &ObjectRef,
        prototype: &ObjectRef,
        prototype_configurable: bool,
    ) -> Result<(), RuntimeError> {
        self.define_function_data_property(
            constructor,
            "prototype",
            Value::Object(prototype.try_clone()?),
            false,
            prototype_configurable,
        )?;
        self.define_function_data_property(
            prototype,
            "constructor",
            Value::Object(constructor.try_clone()?),
            false,
            true,
        )
    }

    pub(crate) fn define_constructor_relationship(
        &self,
        constructor: &CallableRef,
        prototype: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        self.define_function_data_property(
            constructor.as_object(),
            "prototype",
            Value::Object(prototype.try_clone()?),
            false,
            false,
        )?;
        self.define_function_data_property(
            prototype,
            "constructor",
            Value::Object(constructor.as_object().try_clone()?),
            true,
            true,
        )
    }
}

pub(crate) mod bindings;

pub(crate) mod prototypes;
