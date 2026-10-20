//! The operations `Context` and a host function's `Scope` share, written
//! once on [`View`] and listed once in [`js_ops!`], which makes each an
//! inherent method of both handles and a method of the [`Js`] trait.

use std::any::Any;
use std::rc::Rc;

use oxide_core::{Intrinsic, NativeCx, RootMark};

use super::RuntimeError;
use super::buffer::{TypedArrayKind, TypedArrayView};
use super::compile::{ScriptCompileError, compile_script, syntax_location};
use super::context::{Context, EvalOptions, FunctionBytecodeRef, ModuleBytecodeRef, QjsHelpers};
use super::function::{FunctionOptions, HostFunction, Raw, Scope, host_body};
use super::promise::PromiseCapability;
use super::property::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, DescriptorField, Key,
    OrdinaryPropertyDescriptor, PropertyKey, WellKnownSymbol,
};
use super::runtime::{Preloaded, PromiseSnapshot, PromiseState, Shared, slot};
use super::value::{CallableRef, ObjectRef, Slot, SymbolRef, Value};
use crate::engine::api::error::{Error, NativeErrorKind};
use crate::engine::compiler::CompileOptions;
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;

/// What every operation returns.
pub type JsResult<T> = Result<T, RuntimeError>;

/// The engine borrowed for operations: the core's view, the runtime it
/// belongs to, and the root-stack mark its temporaries are cut back to when
/// it drops.
pub(crate) struct View<'a> {
    pub(crate) core: NativeCx<'a>,
    pub(crate) shared: &'a Rc<Shared>,
    mark: RootMark,
}

impl Drop for View<'_> {
    fn drop(&mut self) {
        self.core.heap_mut().roots().truncate(self.mark);
    }
}

impl<'a> View<'a> {
    pub(crate) fn new(mut core: NativeCx<'a>, shared: &'a Rc<Shared>) -> Self {
        let mark = core.heap_mut().roots().mark();
        View { core, shared, mark }
    }
}

impl View<'_> {
    // ---- values ----

    /// Copies a core value out: primitives as data, objects and symbols as
    /// new handles. `value` must still be live (just returned or rooted).
    pub(crate) fn lift(&mut self, value: oxide_core::Value) -> JsResult<Value> {
        use oxide_core::value::Kind;
        Ok(match value.kind() {
            Kind::Undefined => Value::Undefined,
            Kind::Null => Value::Null,
            Kind::Bool => Value::Bool(value.as_bool().unwrap_or(false)),
            Kind::Int32 => Value::Int(value.as_int32().unwrap_or(0)),
            Kind::Number => Value::Float(value.as_f64().unwrap_or(f64::NAN)),
            Kind::String => {
                let units = self.core.string_units(value).unwrap_or_default();
                Value::String(JsString::try_from_utf16(units)?)
            }
            Kind::BigInt => {
                let decimal = self.core.bigint_decimal(value).unwrap_or_default();
                Value::BigInt(
                    JsBigInt::from_decimal_unchecked(&decimal)
                        .ok_or(RuntimeError::Invariant("a BigInt's decimal text"))?,
                )
            }
            Kind::Symbol => Value::Symbol(SymbolRef {
                slot: slot(self.shared, self.core.hold(value), value),
            }),
            Kind::Object => Value::Object(ObjectRef {
                slot: slot(self.shared, self.core.hold(value), value),
            }),
            _ => Value::Undefined,
        })
    }

    pub(crate) fn lift_object(&mut self, object: oxide_core::ObjectId) -> ObjectRef {
        let value = oxide_core::Value::object(object);
        ObjectRef {
            slot: slot(self.shared, self.core.hold(value), value),
        }
    }

    /// The core value of `value`, rooted until the view drops.
    pub(crate) fn lower(&mut self, value: &Value) -> JsResult<oxide_core::Value> {
        let lowered = match value {
            Value::Undefined => oxide_core::Value::UNDEFINED,
            Value::Null => oxide_core::Value::NULL,
            Value::Bool(value) => oxide_core::Value::bool(*value),
            Value::Int(value) => oxide_core::Value::int32(*value),
            Value::Float(value) => oxide_core::Value::number(*value),
            Value::String(text) => {
                let units: Vec<u16> = text.utf16_units().collect();
                let value = self.core.new_string_units(&units)?;
                self.temp(value);
                value
            }
            Value::BigInt(big) => {
                let decimal = big
                    .to_string_radix(10)
                    .map_err(|error| RuntimeError::Internal(format!("{error:?}")))?;
                let value = self
                    .core
                    .new_bigint(&decimal)
                    .ok_or(RuntimeError::Invariant("a BigInt's decimal text"))??;
                self.temp(value);
                value
            }
            Value::Symbol(symbol) => self.own(symbol.slot.as_ref())?,
            Value::Object(object) => self.own(object.slot.as_ref())?,
        };
        Ok(lowered)
    }

    pub(crate) fn lower_all(&mut self, values: &[Value]) -> JsResult<Vec<oxide_core::Value>> {
        values.iter().map(|value| self.lower(value)).collect()
    }

    /// The value a handle of this runtime holds.
    pub(crate) fn own(&self, slot: &Slot) -> JsResult<oxide_core::Value> {
        if slot.of(self.shared) {
            Ok(slot.value)
        } else if slot.shared.strong_count() == 0 {
            Err(RuntimeError::Invariant("a value of a dropped runtime"))
        } else {
            Err(RuntimeError::Invariant("a value of another runtime"))
        }
    }

    pub(crate) fn object(&self, object: &ObjectRef) -> JsResult<oxide_core::ObjectId> {
        self.own(&object.slot)?
            .as_object()
            .ok_or(RuntimeError::Invariant("an object reference"))
    }

    fn function(&mut self, callable: &CallableRef) -> JsResult<oxide_core::Value> {
        Ok(oxide_core::Value::object(
            self.object(callable.as_object())?,
        ))
    }

    fn temp(&mut self, value: oxide_core::Value) {
        self.core.heap_mut().roots().push(value);
    }

    /// The core key of `key`, rooted until the view drops.
    pub(crate) fn key(&mut self, key: &PropertyKey) -> JsResult<oxide_core::PropertyKey> {
        let key = match &key.0 {
            Key::Index(index) => oxide_core::PropertyKey::index(*index).map_or_else(
                || {
                    let units: Vec<u16> = index.to_string().encode_utf16().collect();
                    self.core.key_from_units(&units)
                },
                Ok,
            )?,
            Key::String(text) => {
                let units: Vec<u16> = text.utf16_units().collect();
                self.core.key_from_units(&units)?
            }
            // The symbol's handle roots it.
            Key::Symbol(symbol) => {
                let value = self.own(&symbol.slot)?;
                return self
                    .core
                    .key_from_symbol(value)
                    .ok_or(RuntimeError::Invariant("a symbol reference"));
            }
        };
        // An atom lives only while something names it.
        if let Some(value) = key.heap_value() {
            self.temp(value);
        }
        Ok(key)
    }

    /// The API key of a core key.
    pub(crate) fn lift_key(&mut self, key: oxide_core::PropertyKey) -> JsResult<PropertyKey> {
        Ok(match self.core.key_name(key) {
            oxide_core::KeyName::Index(index) => PropertyKey(Key::Index(index)),
            oxide_core::KeyName::String(units) => {
                PropertyKey(Key::String(JsString::try_from_utf16(units)?))
            }
            oxide_core::KeyName::Symbol(symbol) => match self.lift(symbol)? {
                Value::Symbol(symbol) => PropertyKey(Key::Symbol(symbol)),
                _ => return Err(RuntimeError::Invariant("a symbol key")),
            },
        })
    }

    /// The error a throw returns: the thrown value, lifted. An interrupt is
    /// `Interrupted`, and the outermost entry ends its unwinding.
    pub(crate) fn thrown(&mut self, thrown: oxide_core::Throw) -> RuntimeError {
        if self.core.is_interrupted() {
            if self.core.owns_code() {
                self.core.clear_interrupted();
            }
            return RuntimeError::Interrupted;
        }
        let out_of_memory = self.core.intrinsic(Intrinsic::OutOfMemoryError);
        if thrown.0.as_object() == Some(out_of_memory) {
            return RuntimeError::OutOfMemory;
        }
        match self.lift(thrown.0) {
            Ok(value) => RuntimeError::Exception(value),
            Err(error) => error,
        }
    }

    /// Converts a core result, lifting a throw.
    pub(crate) fn complete(
        &mut self,
        result: Result<oxide_core::Value, oxide_core::Throw>,
    ) -> JsResult<Value> {
        match result {
            Ok(value) => self.lift(value),
            Err(thrown) => Err(self.thrown(thrown)),
        }
    }

    pub(crate) fn complete_with<T>(&mut self, result: Result<T, oxide_core::Throw>) -> JsResult<T> {
        result.map_err(|thrown| self.thrown(thrown))
    }

    /// A descriptor in the core's form, its values rooted until the view
    /// drops. Accessor fields that are not callable were rejected by the
    /// type already.
    fn lower_descriptor(
        &mut self,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> JsResult<oxide_core::PropertyDescriptor> {
        let accessor = |view: &mut Self, field: &DescriptorField<AccessorValue>| match field {
            DescriptorField::Absent => Ok(None),
            DescriptorField::Present(AccessorValue::Undefined) => {
                Ok(Some(oxide_core::Value::UNDEFINED))
            }
            DescriptorField::Present(AccessorValue::Callable(function)) => {
                view.own(&function.0.slot).map(Some)
            }
        };
        Ok(oxide_core::PropertyDescriptor {
            value: match &descriptor.value {
                DescriptorField::Absent => None,
                DescriptorField::Present(value) => Some(self.lower(value)?),
            },
            get: accessor(self, &descriptor.get)?,
            set: accessor(self, &descriptor.set)?,
            writable: descriptor.writable.into_option(),
            enumerable: descriptor.enumerable.into_option(),
            configurable: descriptor.configurable.into_option(),
        })
    }

    fn lift_descriptor(
        &mut self,
        descriptor: oxide_core::PropertyDescriptor,
    ) -> JsResult<CompleteOrdinaryPropertyDescriptor> {
        if descriptor.is_accessor() {
            let callable = |view: &mut Self, value: Option<oxide_core::Value>| {
                Ok::<_, RuntimeError>(match value {
                    Some(value) if !value.is_undefined() => match view.lift(value)? {
                        Value::Object(object) => Some(CallableRef(object)),
                        _ => None,
                    },
                    _ => None,
                })
            };
            Ok(CompleteOrdinaryPropertyDescriptor::Accessor {
                get: callable(self, descriptor.get)?,
                set: callable(self, descriptor.set)?,
                enumerable: descriptor.enumerable.unwrap_or(false),
                configurable: descriptor.configurable.unwrap_or(false),
            })
        } else {
            let value = descriptor.value.unwrap_or(oxide_core::Value::UNDEFINED);
            Ok(CompleteOrdinaryPropertyDescriptor::Data {
                value: self.lift(value)?,
                writable: descriptor.writable.unwrap_or(false),
                enumerable: descriptor.enumerable.unwrap_or(false),
                configurable: descriptor.configurable.unwrap_or(false),
            })
        }
    }

    /// The error for an operation a host function cannot run, because the
    /// running loop borrows what it needs.
    fn modules_supported(&mut self) -> JsResult<()> {
        if self.core.owns_code() {
            Ok(())
        } else {
            Err(RuntimeError::Unsupported("modules inside a host function"))
        }
    }

    // ---- scripts ----

    pub(crate) fn eval(&mut self, source: &str) -> JsResult<Value> {
        self.eval_bytes(source.as_bytes())
    }

    pub(crate) fn eval_bytes(&mut self, source: &[u8]) -> JsResult<Value> {
        self.eval_bytes_with_filename(source, crate::engine::compiler::DEFAULT_EVAL_FILENAME)
    }

    pub(crate) fn eval_with_filename(&mut self, source: &str, filename: &str) -> JsResult<Value> {
        self.eval_bytes_with_filename(source.as_bytes(), filename)
    }

    pub(crate) fn eval_with_options(
        &mut self,
        source: &str,
        options: &EvalOptions,
    ) -> JsResult<Value> {
        self.eval_bytes_with_options(source.as_bytes(), options)
    }

    pub(crate) fn eval_bytes_with_options(
        &mut self,
        source: &[u8],
        options: &EvalOptions,
    ) -> JsResult<Value> {
        let script = self.compile_script_bytes(source, &options.filename, options.transient)?;
        self.execute(&script)
    }

    pub(crate) fn eval_bytes_with_filename(
        &mut self,
        source: &[u8],
        filename: &str,
    ) -> JsResult<Value> {
        let script = self.compile_bytes_with_filename(source, filename)?;
        self.execute(&script)
    }

    pub(crate) fn compile(&mut self, source: &str) -> JsResult<FunctionBytecodeRef> {
        self.compile_bytes(source.as_bytes())
    }

    pub(crate) fn compile_bytes(&mut self, source: &[u8]) -> JsResult<FunctionBytecodeRef> {
        self.compile_bytes_with_filename(source, crate::engine::compiler::DEFAULT_EVAL_FILENAME)
    }

    pub(crate) fn compile_with_filename(
        &mut self,
        source: &str,
        filename: &str,
    ) -> JsResult<FunctionBytecodeRef> {
        self.compile_bytes_with_filename(source.as_bytes(), filename)
    }

    pub(crate) fn compile_with_options(
        &mut self,
        source: &str,
        options: &CompileOptions,
    ) -> JsResult<FunctionBytecodeRef> {
        self.compile_bytes_with_options(source.as_bytes(), options)
    }

    pub(crate) fn compile_bytes_with_options(
        &mut self,
        source: &[u8],
        options: &CompileOptions,
    ) -> JsResult<FunctionBytecodeRef> {
        self.compile_script_bytes(source, &options.filename, options.transient)
    }

    pub(crate) fn compile_bytes_with_filename(
        &mut self,
        source: &[u8],
        filename: &str,
    ) -> JsResult<FunctionBytecodeRef> {
        self.compile_script_bytes(source, filename, false)
    }

    /// Compiles `source` as a script named `filename`; `transient` code goes
    /// in the heap, which frees it once no function names it. Inside a host
    /// function all code is transient.
    fn compile_script_bytes(
        &mut self,
        source: &[u8],
        filename: &str,
        transient: bool,
    ) -> JsResult<FunctionBytecodeRef> {
        let debug_info = self.shared.debug_info.get();
        let lowered = {
            let mut compiler = self.core.compiler(transient);
            compile_script(&mut compiler, source, filename, debug_info)
        };
        match lowered {
            Ok(lowered) => Ok(FunctionBytecodeRef::new(
                self.lift_object(lowered.function),
                #[cfg(test)]
                source,
                #[cfg(test)]
                filename,
                #[cfg(test)]
                debug_info,
            )),
            Err(ScriptCompileError::Front(error)) => {
                Err(self.throw_compile_error(&error, source, filename))
            }
            Err(ScriptCompileError::Lower(error)) => match error {
                crate::engine::compiler::register::LowerError::Syntax(message) => {
                    let error = Error::new(crate::engine::api::error::ErrorKind::Syntax, message);
                    Err(self.throw_compile_error(&error, source, filename))
                }
                other => Err(RuntimeError::Internal(other.to_string())),
            },
        }
    }

    /// A front-end error thrown as QuickJS's `JS_Eval` throws it: an error
    /// object of its kind whose `stack` is its location.
    fn throw_compile_error(
        &mut self,
        error: &Error,
        source: &[u8],
        filename: &str,
    ) -> RuntimeError {
        use crate::engine::api::error::ErrorKind;
        let kind = match error.kind() {
            ErrorKind::Syntax => oxide_core::ErrorKind::Syntax,
            ErrorKind::Type => oxide_core::ErrorKind::Type,
            ErrorKind::Reference => oxide_core::ErrorKind::Reference,
            ErrorKind::Range => oxide_core::ErrorKind::Range,
            ErrorKind::JsInternal => oxide_core::ErrorKind::Internal,
            ErrorKind::Unsupported => {
                return RuntimeError::Unsupported("constructs of this source");
            }
            ErrorKind::Internal | ErrorKind::Io => return RuntimeError::Compile(error.clone()),
        };
        let created = match syntax_location(error, source) {
            Some((line, column)) => {
                self.core
                    .new_located_error(kind, error.message(), filename, line, column)
            }
            None => self.core.new_error(kind, error.message()),
        };
        match created {
            Ok(thrown) => self.thrown(oxide_core::Throw(thrown)),
            Err(error) => error.into(),
        }
    }

    pub(crate) fn execute(&mut self, script: &FunctionBytecodeRef) -> JsResult<Value> {
        let function = self.object(script.function())?;
        let result = self.core.run_script(function);
        self.complete(result)
    }

    // ---- modules ----

    pub(crate) fn compile_module(&mut self, source: &str) -> JsResult<ModuleBytecodeRef> {
        self.compile_module_bytes(source.as_bytes())
    }

    pub(crate) fn compile_module_bytes(&mut self, source: &[u8]) -> JsResult<ModuleBytecodeRef> {
        self.compile_module_bytes_with_filename(
            source,
            crate::engine::compiler::DEFAULT_EVAL_FILENAME,
        )
    }

    pub(crate) fn compile_module_with_filename(
        &mut self,
        source: &str,
        filename: &str,
    ) -> JsResult<ModuleBytecodeRef> {
        self.compile_module_bytes_with_filename(source.as_bytes(), filename)
    }

    pub(crate) fn compile_module_bytes_with_options(
        &mut self,
        source: &[u8],
        options: &CompileOptions,
    ) -> JsResult<ModuleBytecodeRef> {
        self.compile_module_bytes_with_filename(source, &options.filename)
    }

    pub(crate) fn compile_module_bytes_with_filename(
        &mut self,
        source: &[u8],
        filename: &str,
    ) -> JsResult<ModuleBytecodeRef> {
        self.compile_module_bytes_with_import_meta(source, filename, Vec::new())
    }

    pub(crate) fn compile_module_bytes_with_import_meta(
        &mut self,
        source: &[u8],
        filename: &str,
        import_meta: Vec<(String, Value)>,
    ) -> JsResult<ModuleBytecodeRef> {
        self.modules_supported()?;
        let debug_info = self.shared.debug_info.get();
        let compiled = {
            let mut compiler = self.core.compiler(false);
            super::compile::compile_module(&mut compiler, filename, source, debug_info)
        };
        match compiled {
            Ok((
                oxide_core::vm::module::ModuleSource::Compiled {
                    function, tables, ..
                },
                _,
            )) => {
                let function = self.lift_object(function);
                // A second module of the same name replaces the first for
                // later imports, as a fresh `JS_Eval` does.
                self.core.forget_module(filename);
                self.shared.modules.borrow_mut().insert(
                    filename.to_owned(),
                    Preloaded::Compiled {
                        function,
                        tables,
                        import_meta,
                    },
                );
                Ok(ModuleBytecodeRef::new(filename))
            }
            Ok(_) => Err(RuntimeError::Invariant("a compiled module")),
            Err(super::compile::ModuleCompileError::Front(error)) => {
                Err(self.throw_compile_error(&error, source, filename))
            }
            Err(super::compile::ModuleCompileError::Core(failure)) => {
                let thrown = self.core.new_error(failure.kind, &failure.message)?;
                Err(self.thrown(oxide_core::Throw(thrown)))
            }
        }
    }

    pub(crate) fn execute_module(&mut self, module: &ModuleBytecodeRef) -> JsResult<Value> {
        self.modules_supported()?;
        let result = self.core.run_module(module.name());
        self.complete(result)
    }

    // ---- calls ----

    pub(crate) fn call(
        &mut self,
        callable: &CallableRef,
        this_value: Value,
        arguments: &[Value],
    ) -> JsResult<Value> {
        let function = self.function(callable)?;
        let this = self.lower(&this_value)?;
        let args = self.lower_all(arguments)?;
        let result = self.core.call(function, this, &args);
        self.complete(result)
    }

    pub(crate) fn construct(
        &mut self,
        constructor: &CallableRef,
        arguments: &[Value],
    ) -> JsResult<Value> {
        self.construct_with_new_target(constructor, constructor, arguments)
    }

    pub(crate) fn construct_with_new_target(
        &mut self,
        constructor: &CallableRef,
        new_target: &CallableRef,
        arguments: &[Value],
    ) -> JsResult<Value> {
        let function = self.function(constructor)?;
        let target = self.function(new_target)?;
        let args = self.lower_all(arguments)?;
        if !self.core.is_constructor(function) {
            let error = self
                .core
                .new_error(oxide_core::ErrorKind::Type, "not a constructor")?;
            return Err(self.thrown(oxide_core::Throw(error)));
        }
        let result = self.core.construct(function, &args, target);
        self.complete(result)
    }

    // ---- objects ----

    pub(crate) fn new_object(&mut self) -> JsResult<ObjectRef> {
        let object = self.core.new_object()?;
        Ok(self.lift_object(object))
    }

    pub(crate) fn new_object_with_prototype(
        &mut self,
        prototype: Option<&ObjectRef>,
    ) -> JsResult<ObjectRef> {
        let prototype = prototype.map(|p| self.object(p)).transpose()?;
        let object = self.core.new_object_with_prototype(prototype)?;
        Ok(self.lift_object(object))
    }

    pub(crate) fn new_array(&mut self) -> JsResult<ObjectRef> {
        self.new_array_from_values(Vec::new())
    }

    pub(crate) fn new_array_from_values(&mut self, values: Vec<Value>) -> JsResult<ObjectRef> {
        let values = self.lower_all(&values)?;
        let result = self.core.new_array_from(&values);
        let array = self.complete_with(result)?;
        Ok(self.lift_object(array))
    }

    pub(crate) fn get_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> JsResult<Option<CompleteOrdinaryPropertyDescriptor>> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let result = self.core.get_own_property(id, key);
        match self.complete_with(result)? {
            Some(descriptor) => self.lift_descriptor(descriptor).map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn define_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> JsResult<bool> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let descriptor = self.lower_descriptor(descriptor)?;
        let result = self.core.define_own_property(id, key, &descriptor);
        self.complete_with(result)
    }

    pub(crate) fn get_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> JsResult<Value> {
        self.get_property_with_receiver(object, key, Value::Object(object.clone()))
    }

    pub(crate) fn get_property_with_receiver(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: Value,
    ) -> JsResult<Value> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let receiver = self.lower(&receiver)?;
        let result = self.core.get(id, key, receiver);
        self.complete(result)
    }

    pub(crate) fn set_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> JsResult<bool> {
        self.set_property_with_receiver(object, key, value, Value::Object(object.clone()))
    }

    pub(crate) fn set_property_with_receiver(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
        receiver: Value,
    ) -> JsResult<bool> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let value = self.lower(&value)?;
        let receiver = self.lower(&receiver)?;
        let result = self.core.set(id, key, value, receiver);
        self.complete_with(result)
    }

    pub(crate) fn delete_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> JsResult<bool> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let result = self.core.delete_property(id, key);
        self.complete_with(result)
    }

    pub(crate) fn has_property(&mut self, object: &ObjectRef, key: &PropertyKey) -> JsResult<bool> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let result = self.core.has_property(id, key);
        self.complete_with(result)
    }

    pub(crate) fn has_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> JsResult<bool> {
        let id = self.object(object)?;
        let key = self.key(key)?;
        let result = self.core.has_own_property(id, key);
        self.complete_with(result)
    }

    pub(crate) fn own_property_keys(&mut self, object: &ObjectRef) -> JsResult<Vec<PropertyKey>> {
        let id = self.object(object)?;
        let result = self.core.own_property_keys(id);
        let keys = self.complete_with(result)?;
        keys.into_iter().map(|key| self.lift_key(key)).collect()
    }

    pub(crate) fn get_prototype_of(&mut self, object: &ObjectRef) -> JsResult<Option<ObjectRef>> {
        let id = self.object(object)?;
        let result = self.core.get_prototype_of(id);
        let prototype = self.complete_with(result)?;
        Ok(prototype.map(|prototype| self.lift_object(prototype)))
    }

    pub(crate) fn set_prototype_of(
        &mut self,
        object: &ObjectRef,
        prototype: Option<&ObjectRef>,
    ) -> JsResult<bool> {
        let id = self.object(object)?;
        let prototype = prototype.map(|p| self.object(p)).transpose()?;
        let result = self.core.set_prototype_of(id, prototype);
        self.complete_with(result)
    }

    pub(crate) fn as_callable(&mut self, object: &ObjectRef) -> JsResult<Option<CallableRef>> {
        let value = self.own(&object.slot)?;
        Ok(self
            .core
            .is_callable(value)
            .then(|| CallableRef(object.clone())))
    }

    pub(crate) fn is_constructor(&mut self, object: &ObjectRef) -> JsResult<bool> {
        let value = self.own(&object.slot)?;
        Ok(self.core.is_constructor(value))
    }

    pub(crate) fn is_array_object(&mut self, object: &ObjectRef) -> JsResult<bool> {
        let value = self.own(&object.slot)?;
        let result = self.core.is_array(value);
        self.complete_with(result)
    }

    pub(crate) fn is_error_object(&mut self, object: &ObjectRef) -> JsResult<bool> {
        let value = self.own(&object.slot)?;
        Ok(self.core.is_error(value))
    }

    // ---- conversions and printing ----

    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_js_string(&mut self, value: &Value) -> JsResult<JsString> {
        let value = self.lower(value)?;
        let result = self.core.to_string(value);
        match self.complete(result)? {
            Value::String(text) => Ok(text),
            _ => Err(RuntimeError::Invariant("ToString returned a string")),
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_number(&mut self, value: &Value) -> JsResult<f64> {
        let value = self.lower(value)?;
        let result = self.core.to_number(value);
        self.complete_with(result)
    }

    pub(crate) fn detach_array_buffer(&mut self, value: &Value) -> JsResult<()> {
        let value = self.lower(value)?;
        let result = self.core.detach_array_buffer(value);
        self.complete_with(result)
    }

    pub(crate) fn type_of(&mut self, value: &Value) -> JsResult<String> {
        let value = self.lower(value)?;
        Ok(self.core.type_of(value))
    }

    pub(crate) fn print_value(&mut self, value: &Value) -> JsResult<String> {
        let value = self.lower(value)?;
        Ok(self.core.print_value(value))
    }

    pub(crate) fn qjs_print_value_bytes(&mut self, value: &Value) -> JsResult<Vec<u8>> {
        let value = self.lower(value)?;
        Ok(self.core.print_value_bytes(value))
    }

    // ---- symbols ----

    pub(crate) fn new_symbol(&mut self, description: Option<JsString>) -> JsResult<SymbolRef> {
        let units: Option<Vec<u16>> = description.map(|text| text.utf16_units().collect());
        let symbol = self.core.new_symbol(units.as_deref())?;
        match self.lift(symbol)? {
            Value::Symbol(symbol) => Ok(symbol),
            _ => Err(RuntimeError::Invariant("a new symbol")),
        }
    }

    pub(crate) fn symbol_description(&mut self, symbol: &SymbolRef) -> JsResult<Option<JsString>> {
        let value = self.own(&symbol.slot)?;
        self.core
            .symbol_description(value)
            .map(JsString::try_from_utf16)
            .transpose()
            .map_err(RuntimeError::from)
    }

    pub(crate) fn well_known_symbol(&mut self, symbol: WellKnownSymbol) -> JsResult<SymbolRef> {
        let value = self
            .core
            .well_known_symbol(symbol.name())
            .ok_or(RuntimeError::Invariant("a well-known symbol"))?;
        match self.lift(value)? {
            Value::Symbol(symbol) => Ok(symbol),
            _ => Err(RuntimeError::Invariant("a well-known symbol")),
        }
    }

    // ---- promises ----

    pub(crate) fn promise_snapshot(
        &mut self,
        promise: &ObjectRef,
    ) -> JsResult<Option<PromiseSnapshot>> {
        let value = oxide_core::Value::object(self.object(promise)?);
        let Some((state, result)) = self.core.promise_state(value) else {
            return Ok(None);
        };
        let state = match state {
            1 => PromiseState::Fulfilled,
            2 => PromiseState::Rejected,
            _ => PromiseState::Pending,
        };
        let result = if state == PromiseState::Pending {
            Value::Undefined
        } else {
            self.lift(result)?
        };
        Ok(Some(PromiseSnapshot::new(state, result)))
    }

    pub(crate) fn new_promise(&mut self) -> JsResult<PromiseCapability> {
        let result = self.core.new_promise_capability();
        let (promise, resolve, reject) = self.complete_with(result)?;
        Ok(PromiseCapability {
            promise: self.lift_object(promise),
            resolve: CallableRef(self.lift_object(resolve)),
            reject: CallableRef(self.lift_object(reject)),
        })
    }

    pub(crate) fn promise_then(
        &mut self,
        promise: &ObjectRef,
        on_fulfilled: impl HostFunction<Raw>,
        on_rejected: impl HostFunction<Raw>,
    ) -> JsResult<ObjectRef> {
        let fulfilled = self.new_function(FunctionOptions::new("").length(1), on_fulfilled)?;
        let rejected = self.new_function(FunctionOptions::new("").length(1), on_rejected)?;
        let key = PropertyKey::from_js_string(JsString::from_static("then"));
        let then = match self.get_property(promise, &key)? {
            Value::Object(then) => self.as_callable(&then)?,
            _ => None,
        };
        let Some(then) = then else {
            return Err(self.throw_error(NativeErrorKind::Type, "not a function"));
        };
        let result = self.call(
            &then,
            Value::Object(promise.clone()),
            &[Value::from(fulfilled), Value::from(rejected)],
        )?;
        match result {
            Value::Object(promise) => Ok(promise),
            _ => Err(RuntimeError::Invariant("then returned a promise")),
        }
    }

    // ---- buffers ----

    pub(crate) fn new_array_buffer(&mut self, bytes: &[u8]) -> JsResult<ObjectRef> {
        let result = self.core.new_array_buffer(bytes.len());
        let id = self.complete_with(result)?;
        let buffer = self.lift_object(id);
        let result = self.core.array_buffer_write(id, 0, bytes);
        self.complete_with(result)?;
        Ok(buffer)
    }

    pub(crate) fn array_buffer_len(&mut self, buffer: &ObjectRef) -> JsResult<Option<usize>> {
        let id = self.object(buffer)?;
        Ok(self.core.array_buffer_len(id))
    }

    pub(crate) fn array_buffer_to_vec(&mut self, buffer: &ObjectRef) -> JsResult<Vec<u8>> {
        let id = self.object(buffer)?;
        let mut bytes = vec![0; self.core.array_buffer_len(id).unwrap_or(0)];
        let result = self.core.array_buffer_read(id, 0, &mut bytes);
        self.complete_with(result)?;
        Ok(bytes)
    }

    pub(crate) fn with_array_buffer_bytes<R>(
        &mut self,
        buffer: &ObjectRef,
        f: impl FnOnce(&[u8]) -> R,
    ) -> JsResult<R> {
        let id = self.object(buffer)?;
        if let Some(bytes) = self.core.array_buffer_bytes(id) {
            return Ok(f(bytes));
        }
        // A shared buffer's bytes are atomics: they are copied first. A
        // detached buffer or another object throws.
        let bytes = self.array_buffer_to_vec(buffer)?;
        Ok(f(&bytes))
    }

    pub(crate) fn write_array_buffer(
        &mut self,
        buffer: &ObjectRef,
        offset: usize,
        data: &[u8],
    ) -> JsResult<()> {
        let id = self.object(buffer)?;
        let result = self.core.array_buffer_write(id, offset, data);
        self.complete_with(result)
    }

    pub(crate) fn typed_array_view(
        &mut self,
        array: &ObjectRef,
    ) -> JsResult<Option<TypedArrayView>> {
        let id = self.object(array)?;
        let Some((kind, buffer, byte_offset, length)) = self.core.typed_array_view(id) else {
            return Ok(None);
        };
        Ok(Some(TypedArrayView {
            kind,
            buffer: self.lift_object(buffer),
            byte_offset,
            length,
        }))
    }

    pub(crate) fn new_typed_array(
        &mut self,
        kind: TypedArrayKind,
        buffer: &ObjectRef,
        byte_offset: usize,
        length: Option<usize>,
    ) -> JsResult<ObjectRef> {
        let constructor = CallableRef(self.intrinsic(typed_array_constructor(kind))?);
        let mut arguments = vec![
            Value::Object(buffer.clone()),
            Value::number(byte_offset as f64),
        ];
        if let Some(length) = length {
            arguments.push(Value::number(length as f64));
        }
        match self.construct(&constructor, &arguments)? {
            Value::Object(array) => Ok(array),
            _ => Err(RuntimeError::Invariant("a typed array")),
        }
    }

    // ---- intrinsics ----

    fn intrinsic(&mut self, intrinsic: Intrinsic) -> JsResult<ObjectRef> {
        let object = self.core.intrinsic(intrinsic);
        Ok(self.lift_object(object))
    }

    pub(crate) fn global_object(&mut self) -> JsResult<ObjectRef> {
        let object = self.core.global_object();
        Ok(self.lift_object(object))
    }

    pub(crate) fn object_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::ObjectPrototype)
    }

    pub(crate) fn array_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::ArrayPrototype)
    }

    pub(crate) fn function_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::FunctionPrototype)
    }

    pub(crate) fn iterator_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::IteratorPrototype)
    }

    pub(crate) fn string_iterator_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::StringIteratorPrototype)
    }

    pub(crate) fn number_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::NumberPrototype)
    }

    pub(crate) fn boolean_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::BooleanPrototype)
    }

    pub(crate) fn string_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::StringPrototype)
    }

    pub(crate) fn symbol_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::SymbolPrototype)
    }

    pub(crate) fn bigint_prototype(&mut self) -> JsResult<ObjectRef> {
        self.intrinsic(Intrinsic::BigIntPrototype)
    }

    pub(crate) fn function_constructor(&mut self) -> JsResult<CallableRef> {
        self.intrinsic(Intrinsic::Function).map(CallableRef)
    }

    // ---- errors ----

    pub(crate) fn new_native_error(
        &mut self,
        kind: NativeErrorKind,
        message: &str,
    ) -> JsResult<Value> {
        let error = self.core.new_error(native_kind(kind), message)?;
        self.lift(error)
    }

    pub(crate) fn throw_error(&mut self, kind: NativeErrorKind, message: &str) -> RuntimeError {
        match self.new_native_error(kind, message) {
            Ok(error) => RuntimeError::Exception(error),
            Err(error) => error,
        }
    }

    // ---- hosts ----

    pub(crate) fn install_qjs_helpers(&mut self, helpers: &QjsHelpers) -> JsResult<()> {
        self.core
            .install_qjs_helpers(helpers.script_args.as_deref())?;
        if helpers.std {
            self.core.install_std()?;
        }
        Ok(())
    }

    // ---- host functions and host objects ----

    pub(crate) fn new_function<Args>(
        &mut self,
        options: FunctionOptions,
        function: impl HostFunction<Args>,
    ) -> JsResult<CallableRef> {
        let data = self.lower_all(&options.data)?;
        let body = host_body(self.shared, function);
        let object = self.core.new_host_function(
            &options.name,
            options.length,
            options.constructor,
            &data,
            body,
        )?;
        Ok(CallableRef(self.lift_object(object)))
    }

    pub(crate) fn define_function<Args>(
        &mut self,
        target: &ObjectRef,
        name: &str,
        length: u32,
        function: impl HostFunction<Args>,
    ) -> JsResult<CallableRef> {
        let function = self.new_function(FunctionOptions::new(name).length(length), function)?;
        let key = PropertyKey::from_js_string(JsString::try_from_utf8(name)?);
        let descriptor = OrdinaryPropertyDescriptor::data(
            Value::Object(function.as_object().clone()),
            true,
            false,
            true,
        );
        if !self.define_own_property(target, &key, &descriptor)? {
            let error = self.throw_error(NativeErrorKind::Type, "property is not configurable");
            return Err(error);
        }
        Ok(function)
    }

    pub(crate) fn new_host_object<T: 'static>(
        &mut self,
        value: T,
        prototype: Option<&ObjectRef>,
    ) -> JsResult<ObjectRef> {
        let prototype = prototype.map(|p| self.object(p)).transpose()?;
        let object = self.core.new_host_object(prototype, Rc::new(value))?;
        Ok(self.lift_object(object))
    }

    pub(crate) fn host_data<T: 'static>(&mut self, object: &ObjectRef) -> JsResult<Option<Rc<T>>> {
        let id = self.object(object)?;
        Ok(self
            .core
            .host_data(id)
            .and_then(|data: Rc<dyn Any>| data.downcast::<T>().ok()))
    }
}

fn typed_array_constructor(kind: TypedArrayKind) -> Intrinsic {
    match kind {
        TypedArrayKind::Int8 => Intrinsic::Int8Array,
        TypedArrayKind::Uint8 => Intrinsic::Uint8Array,
        TypedArrayKind::Uint8Clamped => Intrinsic::Uint8ClampedArray,
        TypedArrayKind::Int16 => Intrinsic::Int16Array,
        TypedArrayKind::Uint16 => Intrinsic::Uint16Array,
        TypedArrayKind::Int32 => Intrinsic::Int32Array,
        TypedArrayKind::Uint32 => Intrinsic::Uint32Array,
        TypedArrayKind::Float16 => Intrinsic::Float16Array,
        TypedArrayKind::Float32 => Intrinsic::Float32Array,
        TypedArrayKind::Float64 => Intrinsic::Float64Array,
        TypedArrayKind::BigInt64 => Intrinsic::BigInt64Array,
        TypedArrayKind::BigUint64 => Intrinsic::BigUint64Array,
    }
}

pub(crate) fn native_kind(kind: NativeErrorKind) -> oxide_core::ErrorKind {
    match kind {
        NativeErrorKind::Eval => oxide_core::ErrorKind::Eval,
        NativeErrorKind::Range => oxide_core::ErrorKind::Range,
        NativeErrorKind::Reference => oxide_core::ErrorKind::Reference,
        NativeErrorKind::Syntax => oxide_core::ErrorKind::Syntax,
        NativeErrorKind::Type => oxide_core::ErrorKind::Type,
        NativeErrorKind::Uri => oxide_core::ErrorKind::Uri,
        NativeErrorKind::Internal => oxide_core::ErrorKind::Internal,
        NativeErrorKind::Aggregate => oxide_core::ErrorKind::Aggregate,
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Context {}
    impl Sealed for super::Scope<'_> {}
}

/// Lists every shared operation once, with its documentation, and emits it
/// as an inherent method of `Context` (one entry each, `Reentered` inside a
/// host function), an inherent method of `Scope` (on the host function's
/// view) and a method of [`Js`]. `Context` takes `&self`: its state is the
/// runtime's, behind the `Rc`.
macro_rules! js_ops {
    ($(
        $(#[$doc:meta])*
        fn $name:ident $([$($generic:tt)*])? (&mut self $(, $arg:ident: $ty:ty)* $(,)?) -> $ret:ty;
    )*) => {
        impl Context {
            $(
                $(#[$doc])*
                pub fn $name $(<$($generic)*>)? (&self $(, $arg: $ty)*) -> $ret {
                    let mut entry = self.runtime.enter()?;
                    entry.view().$name($($arg),*)
                }
            )*
        }

        impl Scope<'_> {
            $(
                $(#[$doc])*
                pub fn $name $(<$($generic)*>)? (&mut self $(, $arg: $ty)*) -> $ret {
                    self.view().$name($($arg),*)
                }
            )*
        }

        /// The operations [`Context`] and a host function's [`Scope`]
        /// share, for code generic over where it runs. Sealed; both types
        /// also have each as an inherent method, so calling one needs no
        /// import.
        pub trait Js: sealed::Sealed {
            $(
                $(#[$doc])*
                fn $name $(<$($generic)*>)? (&mut self $(, $arg: $ty)*) -> $ret;
            )*

            /// An error object of `kind` with `message` and a `stack` from
            /// the running frames, as the error to return:
            /// `return Err(js.throw_error(NativeErrorKind::Type, "x is required"))`.
            fn throw_error(&mut self, kind: NativeErrorKind, message: &str) -> RuntimeError;
        }

        impl Js for Context {
            $(
                fn $name $(<$($generic)*>)? (&mut self $(, $arg: $ty)*) -> $ret {
                    Context::$name(self $(, $arg)*)
                }
            )*

            fn throw_error(&mut self, kind: NativeErrorKind, message: &str) -> RuntimeError {
                Context::throw_error(self, kind, message)
            }
        }

        impl Js for Scope<'_> {
            $(
                fn $name $(<$($generic)*>)? (&mut self $(, $arg: $ty)*) -> $ret {
                    Scope::$name(self $(, $arg)*)
                }
            )*

            fn throw_error(&mut self, kind: NativeErrorKind, message: &str) -> RuntimeError {
                Scope::throw_error(self, kind, message)
            }
        }
    };
}

impl Context {
    /// [`Js::throw_error`].
    pub fn throw_error(&self, kind: NativeErrorKind, message: &str) -> RuntimeError {
        match self.runtime.enter() {
            Ok(mut entry) => entry.view().throw_error(kind, message),
            Err(error) => error,
        }
    }
}

impl Scope<'_> {
    /// [`Js::throw_error`].
    pub fn throw_error(&mut self, kind: NativeErrorKind, message: &str) -> RuntimeError {
        self.view().throw_error(kind, message)
    }
}

js_ops! {
    // ---- scripts ----

    /// Compiles and runs `source` as a script; the completion value.
    fn eval(&mut self, source: &str) -> JsResult<Value>;
    fn eval_bytes(&mut self, source: &[u8]) -> JsResult<Value>;
    fn eval_with_filename(&mut self, source: &str, filename: &str) -> JsResult<Value>;
    fn eval_with_options(&mut self, source: &str, options: &EvalOptions) -> JsResult<Value>;
    fn eval_bytes_with_options(&mut self, source: &[u8], options: &EvalOptions) -> JsResult<Value>;
    /// Compiles and runs `source` as a script named `filename`; the
    /// completion value. A syntax error is thrown (`RuntimeError::Exception`
    /// with a `SyntaxError`), as QuickJS's `JS_Eval` does.
    fn eval_bytes_with_filename(&mut self, source: &[u8], filename: &str) -> JsResult<Value>;
    fn compile(&mut self, source: &str) -> JsResult<FunctionBytecodeRef>;
    fn compile_bytes(&mut self, source: &[u8]) -> JsResult<FunctionBytecodeRef>;
    fn compile_with_filename(&mut self, source: &str, filename: &str) -> JsResult<FunctionBytecodeRef>;
    fn compile_with_options(&mut self, source: &str, options: &CompileOptions) -> JsResult<FunctionBytecodeRef>;
    fn compile_bytes_with_options(&mut self, source: &[u8], options: &CompileOptions) -> JsResult<FunctionBytecodeRef>;
    /// Compiles `source` as a script named `filename`. A syntax error is
    /// thrown (`RuntimeError::Exception` with a `SyntaxError`). Inside a
    /// host function the code is transient whatever the options say.
    fn compile_bytes_with_filename(&mut self, source: &[u8], filename: &str) -> JsResult<FunctionBytecodeRef>;
    /// Runs a compiled script with the global object as `this`.
    fn execute(&mut self, script: &FunctionBytecodeRef) -> JsResult<Value>;

    // ---- modules (`Unsupported` inside a host function) ----

    fn compile_module(&mut self, source: &str) -> JsResult<ModuleBytecodeRef>;
    fn compile_module_bytes(&mut self, source: &[u8]) -> JsResult<ModuleBytecodeRef>;
    fn compile_module_with_filename(&mut self, source: &str, filename: &str) -> JsResult<ModuleBytecodeRef>;
    fn compile_module_bytes_with_options(&mut self, source: &[u8], options: &CompileOptions) -> JsResult<ModuleBytecodeRef>;
    fn compile_module_bytes_with_filename(&mut self, source: &[u8], filename: &str) -> JsResult<ModuleBytecodeRef>;
    /// Compiles `source` as the module `filename`, whose `import.meta`
    /// starts with `import_meta` (qjs's `url` and `main`). A syntax error is
    /// thrown.
    fn compile_module_bytes_with_import_meta(&mut self, source: &[u8], filename: &str, import_meta: Vec<(String, Value)>) -> JsResult<ModuleBytecodeRef>;
    /// Links and evaluates a compiled module and what it imports; the
    /// promise of its evaluation.
    fn execute_module(&mut self, module: &ModuleBytecodeRef) -> JsResult<Value>;

    // ---- calls ----

    fn call(&mut self, callable: &CallableRef, this_value: Value, arguments: &[Value]) -> JsResult<Value>;
    fn construct(&mut self, constructor: &CallableRef, arguments: &[Value]) -> JsResult<Value>;
    fn construct_with_new_target(&mut self, constructor: &CallableRef, new_target: &CallableRef, arguments: &[Value]) -> JsResult<Value>;

    // ---- objects ----

    fn new_object(&mut self) -> JsResult<ObjectRef>;
    fn new_object_with_prototype(&mut self, prototype: Option<&ObjectRef>) -> JsResult<ObjectRef>;
    fn new_array(&mut self) -> JsResult<ObjectRef>;
    fn new_array_from_values(&mut self, values: Vec<Value>) -> JsResult<ObjectRef>;
    fn get_own_property(&mut self, object: &ObjectRef, key: &PropertyKey) -> JsResult<Option<CompleteOrdinaryPropertyDescriptor>>;
    fn define_own_property(&mut self, object: &ObjectRef, key: &PropertyKey, descriptor: &OrdinaryPropertyDescriptor) -> JsResult<bool>;
    fn get_property(&mut self, object: &ObjectRef, key: &PropertyKey) -> JsResult<Value>;
    fn get_property_with_receiver(&mut self, object: &ObjectRef, key: &PropertyKey, receiver: Value) -> JsResult<Value>;
    fn set_property(&mut self, object: &ObjectRef, key: &PropertyKey, value: Value) -> JsResult<bool>;
    fn set_property_with_receiver(&mut self, object: &ObjectRef, key: &PropertyKey, value: Value, receiver: Value) -> JsResult<bool>;
    fn delete_property(&mut self, object: &ObjectRef, key: &PropertyKey) -> JsResult<bool>;
    fn has_property(&mut self, object: &ObjectRef, key: &PropertyKey) -> JsResult<bool>;
    fn has_own_property(&mut self, object: &ObjectRef, key: &PropertyKey) -> JsResult<bool>;
    fn own_property_keys(&mut self, object: &ObjectRef) -> JsResult<Vec<PropertyKey>>;
    fn get_prototype_of(&mut self, object: &ObjectRef) -> JsResult<Option<ObjectRef>>;
    fn set_prototype_of(&mut self, object: &ObjectRef, prototype: Option<&ObjectRef>) -> JsResult<bool>;
    /// The object as a callable reference; `None` when it is not callable.
    fn as_callable(&mut self, object: &ObjectRef) -> JsResult<Option<CallableRef>>;
    fn is_constructor(&mut self, object: &ObjectRef) -> JsResult<bool>;
    /// `IsArray`.
    fn is_array_object(&mut self, object: &ObjectRef) -> JsResult<bool>;
    fn is_error_object(&mut self, object: &ObjectRef) -> JsResult<bool>;

    // ---- conversions and printing ----

    /// `ToString(value)`.
    fn to_js_string(&mut self, value: &Value) -> JsResult<JsString>;
    /// `ToNumber(value)`.
    fn to_number(&mut self, value: &Value) -> JsResult<f64>;
    /// QuickJS's `JS_DetachArrayBuffer`: detaches an ArrayBuffer; anything
    /// else is ignored.
    fn detach_array_buffer(&mut self, value: &Value) -> JsResult<()>;
    /// `typeof value`.
    fn type_of(&mut self, value: &Value) -> JsResult<String>;
    /// QuickJS's `JS_PrintValue`: what `print` and an uncaught-exception dump
    /// show. Runs no JavaScript (no accessor, trap or coercion).
    fn print_value(&mut self, value: &Value) -> JsResult<String>;
    /// `print_value` as the bytes QuickJS writes: a lone surrogate of a raw
    /// name, message or stack as its own three bytes (WTF-8) instead of
    /// U+FFFD.
    fn qjs_print_value_bytes(&mut self, value: &Value) -> JsResult<Vec<u8>>;

    // ---- symbols ----

    fn new_symbol(&mut self, description: Option<JsString>) -> JsResult<SymbolRef>;
    fn symbol_description(&mut self, symbol: &SymbolRef) -> JsResult<Option<JsString>>;
    fn well_known_symbol(&mut self, symbol: WellKnownSymbol) -> JsResult<SymbolRef>;

    // ---- promises ----

    /// A promise's state and result; `None` when `promise` is not one.
    fn promise_snapshot(&mut self, promise: &ObjectRef) -> JsResult<Option<PromiseSnapshot>>;
    /// `NewPromiseCapability(%Promise%)`: a pending promise and its
    /// resolving functions.
    fn new_promise(&mut self) -> JsResult<PromiseCapability>;
    /// `promise.then(on_fulfilled, on_rejected)` with host functions; the
    /// promise `then` returns.
    fn promise_then(&mut self, promise: &ObjectRef, on_fulfilled: impl HostFunction<Raw>, on_rejected: impl HostFunction<Raw>) -> JsResult<ObjectRef>;

    // ---- buffers ----

    /// A new `ArrayBuffer` holding a copy of `bytes`.
    fn new_array_buffer(&mut self, bytes: &[u8]) -> JsResult<ObjectRef>;
    /// A buffer's length in bytes; `None` for a detached buffer or another
    /// object.
    fn array_buffer_len(&mut self, buffer: &ObjectRef) -> JsResult<Option<usize>>;
    /// A copy of a buffer's bytes (`ArrayBuffer` or `SharedArrayBuffer`);
    /// a detached buffer or another object throws a `TypeError`.
    fn array_buffer_to_vec(&mut self, buffer: &ObjectRef) -> JsResult<Vec<u8>>;
    /// Calls `f` with a buffer's bytes, borrowed in place for an
    /// `ArrayBuffer` and copied first for a `SharedArrayBuffer`. `f` runs
    /// inside the engine's borrow, so it cannot call the engine.
    fn with_array_buffer_bytes[R](&mut self, buffer: &ObjectRef, f: impl FnOnce(&[u8]) -> R) -> JsResult<R>;
    /// Writes `data` into a buffer at `offset`: a `TypeError` for a
    /// detached buffer or another object, a `RangeError` past its end.
    fn write_array_buffer(&mut self, buffer: &ObjectRef, offset: usize, data: &[u8]) -> JsResult<()>;
    /// A typed array's kind, buffer, byte offset and current length; `None`
    /// for anything but a typed array (a `DataView` included).
    fn typed_array_view(&mut self, array: &ObjectRef) -> JsResult<Option<TypedArrayView>>;
    /// `new <Kind>Array(buffer, byte_offset, length)`; without `length` the
    /// view tracks a resizable buffer's length.
    fn new_typed_array(&mut self, kind: TypedArrayKind, buffer: &ObjectRef, byte_offset: usize, length: Option<usize>) -> JsResult<ObjectRef>;

    // ---- intrinsics ----

    fn global_object(&mut self) -> JsResult<ObjectRef>;
    fn object_prototype(&mut self) -> JsResult<ObjectRef>;
    fn array_prototype(&mut self) -> JsResult<ObjectRef>;
    fn function_prototype(&mut self) -> JsResult<ObjectRef>;
    fn iterator_prototype(&mut self) -> JsResult<ObjectRef>;
    fn string_iterator_prototype(&mut self) -> JsResult<ObjectRef>;
    fn number_prototype(&mut self) -> JsResult<ObjectRef>;
    fn boolean_prototype(&mut self) -> JsResult<ObjectRef>;
    fn string_prototype(&mut self) -> JsResult<ObjectRef>;
    fn symbol_prototype(&mut self) -> JsResult<ObjectRef>;
    fn bigint_prototype(&mut self) -> JsResult<ObjectRef>;
    fn function_constructor(&mut self) -> JsResult<CallableRef>;

    // ---- errors ----

    /// A new native error object (not thrown).
    fn new_native_error(&mut self, kind: NativeErrorKind, message: &str) -> JsResult<Value>;

    // ---- hosts ----

    /// qjs's `print` and `console.log`, `scriptArgs`, and under `std` its
    /// `load` and `read`.
    fn install_qjs_helpers(&mut self, helpers: &QjsHelpers) -> JsResult<()>;

    // ---- host functions and host objects ----

    /// A function whose body is `function`: a closure over a [`Scope`]
    /// (`|scope: &mut Scope<'_>| …`), named and shaped by `options`.
    fn new_function[Args](&mut self, options: FunctionOptions, function: impl HostFunction<Args>) -> JsResult<CallableRef>;
    /// [`Js::new_function`] defined on `target` as `name`, writable and
    /// configurable, not enumerable, as QuickJS's
    /// `JS_SetPropertyFunctionList` defines a method.
    fn define_function[Args](&mut self, target: &ObjectRef, name: &str, length: u32, function: impl HostFunction<Args>) -> JsResult<CallableRef>;
    /// An object inheriting from `prototype` (`None` for null) holding
    /// `value`, which drops when the object is collected.
    fn new_host_object[T: 'static](&mut self, value: T, prototype: Option<&ObjectRef>) -> JsResult<ObjectRef>;
    /// The value a host object holds; `None` for an ordinary object or a
    /// value of another type.
    fn host_data[T: 'static](&mut self, object: &ObjectRef) -> JsResult<Option<Rc<T>>>;
}
