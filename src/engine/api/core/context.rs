//! The realm's handle (phase 6, section 2.1): evaluation, calls, objects and
//! properties, the intrinsics, the pending exception.

use std::rc::Rc;

use oxide_core::Intrinsic;

use super::RuntimeError;
use super::compile::{ScriptCompileError, compile_script, syntax_location};
use super::property::{
    CompleteOrdinaryPropertyDescriptor, OrdinaryPropertyDescriptor, PropertyKey,
};
use super::runtime::{Entry, Preloaded, Runtime};
use super::value::{CallableRef, ObjectRef, Value};
use crate::engine::api::error::{Error, NativeErrorKind};
use crate::engine::compiler::CompileOptions;

/// Options of `Context::eval_with_options`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalOptions {
    pub filename: String,
}

impl EvalOptions {
    pub fn new(filename: impl Into<String>) -> Self {
        EvalOptions {
            filename: filename.into(),
        }
    }
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self::new(crate::engine::compiler::DEFAULT_EVAL_FILENAME)
    }
}

/// A compiled script, run by `Context::execute`.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionBytecodeRef {
    function: ObjectRef,
}

/// A compiled module, run by `Context::execute_module`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleBytecodeRef {
    name: String,
}

impl ModuleBytecodeRef {
    /// The module's name (its filename).
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// qjs's helpers (`Context::install_qjs_helpers`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QjsHelpers {
    /// `load` and `read` (`qjs --std`).
    pub std: bool,
    /// `scriptArgs`, when set.
    pub script_args: Option<Vec<String>>,
}

/// The runtime's one realm.
#[derive(Clone)]
pub struct Context {
    pub(crate) runtime: Runtime,
}

impl Context {
    /// The runtime's realm (a second call returns the same one).
    pub fn new(runtime: &Runtime) -> Self {
        runtime.context()
    }

    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    fn enter(&self) -> Result<Entry<'_>, RuntimeError> {
        self.runtime.enter()
    }

    // ---- scripts ----

    pub fn eval(&mut self, source: &str) -> Result<Value, RuntimeError> {
        self.eval_bytes(source.as_bytes())
    }

    pub fn eval_bytes(&mut self, source: &[u8]) -> Result<Value, RuntimeError> {
        self.eval_bytes_with_filename(source, crate::engine::compiler::DEFAULT_EVAL_FILENAME)
    }

    pub fn eval_with_filename(
        &mut self,
        source: &str,
        filename: &str,
    ) -> Result<Value, RuntimeError> {
        self.eval_bytes_with_filename(source.as_bytes(), filename)
    }

    pub fn eval_with_options(
        &mut self,
        source: &str,
        options: &EvalOptions,
    ) -> Result<Value, RuntimeError> {
        self.eval_bytes_with_filename(source.as_bytes(), &options.filename)
    }

    pub fn eval_bytes_with_options(
        &mut self,
        source: &[u8],
        options: &EvalOptions,
    ) -> Result<Value, RuntimeError> {
        self.eval_bytes_with_filename(source, &options.filename)
    }

    /// Compiles and runs `source` as a script named `filename`; the
    /// completion value. A syntax error is thrown (`RuntimeError::Exception`
    /// with a `SyntaxError` pending), as QuickJS's `JS_Eval` does.
    pub fn eval_bytes_with_filename(
        &mut self,
        source: &[u8],
        filename: &str,
    ) -> Result<Value, RuntimeError> {
        let script = self.compile_bytes_with_filename(source, filename)?;
        self.execute(&script)
    }

    pub fn compile(&mut self, source: &str) -> Result<FunctionBytecodeRef, RuntimeError> {
        self.compile_bytes(source.as_bytes())
    }

    pub fn compile_bytes(&mut self, source: &[u8]) -> Result<FunctionBytecodeRef, RuntimeError> {
        self.compile_bytes_with_filename(source, crate::engine::compiler::DEFAULT_EVAL_FILENAME)
    }

    pub fn compile_with_filename(
        &mut self,
        source: &str,
        filename: &str,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        self.compile_bytes_with_filename(source.as_bytes(), filename)
    }

    pub fn compile_with_options(
        &mut self,
        source: &str,
        options: &CompileOptions,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        self.compile_bytes_with_filename(source.as_bytes(), &options.filename)
    }

    pub fn compile_bytes_with_options(
        &mut self,
        source: &[u8],
        options: &CompileOptions,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        self.compile_bytes_with_filename(source, &options.filename)
    }

    /// Compiles `source` as a script named `filename`. A syntax error is
    /// thrown (`RuntimeError::Exception`, a `SyntaxError` pending).
    pub fn compile_bytes_with_filename(
        &mut self,
        source: &[u8],
        filename: &str,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        let debug_info = self.runtime.shared.debug_info.get();
        let mut entry = self.enter()?;
        let lowered = {
            let mut compiler = entry.inner.core.compiler();
            compile_script(&mut compiler, source, filename, debug_info)
        };
        match lowered {
            Ok(lowered) => Ok(FunctionBytecodeRef {
                function: entry.lift_object(lowered.function),
            }),
            Err(ScriptCompileError::Front(error)) => {
                Err(throw_compile_error(&mut entry, &error, source, filename))
            }
            Err(ScriptCompileError::Lower(error)) => match error {
                crate::engine::compiler::register::LowerError::Syntax(message) => {
                    let error = Error::new(crate::engine::api::error::ErrorKind::Syntax, message);
                    Err(throw_compile_error(&mut entry, &error, source, filename))
                }
                other => Err(RuntimeError::Internal(other.to_string())),
            },
        }
    }

    /// Runs a compiled script with the global object as `this`.
    pub fn execute(&mut self, script: &FunctionBytecodeRef) -> Result<Value, RuntimeError> {
        let mut entry = self.enter()?;
        let function = entry.object(&script.function)?;
        let result = entry.inner.core.run_script(function);
        entry.complete(result)
    }

    // ---- modules ----

    pub fn compile_module(&mut self, source: &str) -> Result<ModuleBytecodeRef, RuntimeError> {
        self.compile_module_bytes(source.as_bytes())
    }

    pub fn compile_module_bytes(
        &mut self,
        source: &[u8],
    ) -> Result<ModuleBytecodeRef, RuntimeError> {
        self.compile_module_bytes_with_filename(
            source,
            crate::engine::compiler::DEFAULT_EVAL_FILENAME,
        )
    }

    pub fn compile_module_with_filename(
        &mut self,
        source: &str,
        filename: &str,
    ) -> Result<ModuleBytecodeRef, RuntimeError> {
        self.compile_module_bytes_with_filename(source.as_bytes(), filename)
    }

    pub fn compile_module_bytes_with_filename(
        &mut self,
        source: &[u8],
        filename: &str,
    ) -> Result<ModuleBytecodeRef, RuntimeError> {
        self.compile_module_bytes_with_import_meta(source, filename, Vec::new())
    }

    /// Compiles `source` as the module `filename`, whose `import.meta`
    /// starts with `import_meta` (qjs's `url` and `main`). A syntax error is
    /// thrown.
    pub fn compile_module_bytes_with_import_meta(
        &mut self,
        source: &[u8],
        filename: &str,
        import_meta: Vec<(String, Value)>,
    ) -> Result<ModuleBytecodeRef, RuntimeError> {
        let debug_info = self.runtime.shared.debug_info.get();
        let shared = Rc::clone(&self.runtime.shared);
        let mut entry = self.enter()?;
        let compiled = {
            let mut compiler = entry.inner.core.compiler();
            super::compile::compile_module(&mut compiler, filename, source, debug_info)
        };
        match compiled {
            Ok((
                oxide_core::vm::module::ModuleSource::Compiled {
                    function, tables, ..
                },
                _,
            )) => {
                let function = entry.lift_object(function);
                // A second module of the same name replaces the first for
                // later imports, as a fresh `JS_Eval` does.
                entry.inner.core.forget_module(filename);
                shared.modules.borrow_mut().insert(
                    filename.to_owned(),
                    Preloaded {
                        function,
                        tables,
                        import_meta,
                    },
                );
                Ok(ModuleBytecodeRef {
                    name: filename.to_owned(),
                })
            }
            Ok(_) => Err(RuntimeError::Invariant("a compiled module")),
            Err(super::compile::ModuleCompileError::Front(error)) => {
                Err(throw_compile_error(&mut entry, &error, source, filename))
            }
            Err(super::compile::ModuleCompileError::Core(failure)) => {
                let thrown = entry.inner.core.new_error(failure.kind, &failure.message)?;
                Err(entry.thrown(oxide_core::Throw(thrown)))
            }
        }
    }

    /// Links and evaluates a compiled module and what it imports; the
    /// promise of its evaluation.
    pub fn execute_module(&mut self, module: &ModuleBytecodeRef) -> Result<Value, RuntimeError> {
        let mut entry = self.enter()?;
        let result = entry.inner.core.run_module(&module.name);
        entry.complete(result)
    }

    // ---- calls ----

    pub fn call(
        &mut self,
        callable: &CallableRef,
        this_value: Value,
        arguments: &[Value],
    ) -> Result<Value, RuntimeError> {
        let mut entry = self.enter()?;
        let function = oxide_core::Value::object(entry.object(callable.as_object())?);
        let this = entry.lower(&this_value)?;
        let args = arguments
            .iter()
            .map(|argument| entry.lower(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let result = entry.inner.core.call(function, this, &args);
        entry.complete(result)
    }

    pub fn construct(
        &mut self,
        constructor: &CallableRef,
        arguments: &[Value],
    ) -> Result<Value, RuntimeError> {
        self.construct_with_new_target(constructor, constructor, arguments)
    }

    pub fn construct_with_new_target(
        &mut self,
        constructor: &CallableRef,
        new_target: &CallableRef,
        arguments: &[Value],
    ) -> Result<Value, RuntimeError> {
        let mut entry = self.enter()?;
        let function = oxide_core::Value::object(entry.object(constructor.as_object())?);
        let target = oxide_core::Value::object(entry.object(new_target.as_object())?);
        let args = arguments
            .iter()
            .map(|argument| entry.lower(argument))
            .collect::<Result<Vec<_>, _>>()?;
        if !entry.core.is_constructor(function) {
            let error = entry
                .inner
                .core
                .new_error(oxide_core::ErrorKind::Type, "not a constructor")?;
            return Err(entry.thrown(oxide_core::Throw(error)));
        }
        let result = entry.inner.core.construct(function, &args, target);
        entry.complete(result)
    }

    // ---- objects ----

    pub fn new_object(&mut self) -> Result<ObjectRef, RuntimeError> {
        let mut entry = self.enter()?;
        let object = entry.inner.core.new_object()?;
        Ok(entry.lift_object(object))
    }

    pub fn new_object_with_prototype(
        &mut self,
        prototype: Option<&ObjectRef>,
    ) -> Result<ObjectRef, RuntimeError> {
        let mut entry = self.enter()?;
        let prototype = prototype.map(|p| entry.object(p)).transpose()?;
        let object = entry.inner.core.new_object_with_prototype(prototype)?;
        Ok(entry.lift_object(object))
    }

    pub fn new_array(&mut self) -> Result<ObjectRef, RuntimeError> {
        self.new_array_from_values(Vec::new())
    }

    pub fn new_array_from_values(&mut self, values: Vec<Value>) -> Result<ObjectRef, RuntimeError> {
        let mut entry = self.enter()?;
        let values = values
            .iter()
            .map(|value| entry.lower(value))
            .collect::<Result<Vec<_>, _>>()?;
        let result = entry.inner.core.new_array_from(&values);
        let array = entry.complete_with(result)?;
        Ok(entry.lift_object(array))
    }

    pub fn get_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<CompleteOrdinaryPropertyDescriptor>, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let result = entry.inner.core.get_own_property(id, key);
        match entry.complete_with(result)? {
            Some(descriptor) => entry.lift_descriptor(descriptor).map(Some),
            None => Ok(None),
        }
    }

    pub fn define_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let descriptor = entry.lower_descriptor(descriptor)?;
        let result = entry.inner.core.define_own_property(id, key, &descriptor);
        entry.complete_with(result)
    }

    pub fn get_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Value, RuntimeError> {
        self.get_property_with_receiver(object, key, Value::Object(object.clone()))
    }

    pub fn get_property_with_receiver(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: Value,
    ) -> Result<Value, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let receiver = entry.lower(&receiver)?;
        let result = entry.inner.core.get(id, key, receiver);
        entry.complete(result)
    }

    pub fn set_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        self.set_property_with_receiver(object, key, value, Value::Object(object.clone()))
    }

    pub fn set_property_with_receiver(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let value = entry.lower(&value)?;
        let receiver = entry.lower(&receiver)?;
        let result = entry.inner.core.set(id, key, value, receiver);
        entry.complete_with(result)
    }

    /// `ToString(value)`.
    pub fn to_js_string(
        &mut self,
        value: &Value,
    ) -> Result<crate::engine::value::JsString, RuntimeError> {
        let mut entry = self.enter()?;
        let value = entry.lower(value)?;
        let result = entry.inner.core.to_string(value);
        match entry.complete(result)? {
            Value::String(text) => Ok(text),
            _ => Err(RuntimeError::Invariant("ToString returned a string")),
        }
    }

    /// `ToNumber(value)`.
    pub fn to_number(&mut self, value: &Value) -> Result<f64, RuntimeError> {
        let mut entry = self.enter()?;
        let value = entry.lower(value)?;
        let result = entry.inner.core.to_number(value);
        entry.complete_with(result)
    }

    // ---- intrinsics ----

    fn intrinsic(&self, intrinsic: Intrinsic) -> Result<ObjectRef, RuntimeError> {
        let mut entry = self.enter()?;
        let object = entry.core.intrinsic(intrinsic);
        Ok(entry.lift_object(object))
    }

    pub fn global_object(&self) -> Result<ObjectRef, RuntimeError> {
        let mut entry = self.enter()?;
        let object = entry.core.global_object();
        Ok(entry.lift_object(object))
    }

    pub fn object_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::ObjectPrototype)
    }

    pub fn array_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::ArrayPrototype)
    }

    pub fn function_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::FunctionPrototype)
    }

    pub fn iterator_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::IteratorPrototype)
    }

    pub fn string_iterator_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::StringIteratorPrototype)
    }

    pub fn number_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::NumberPrototype)
    }

    pub fn boolean_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::BooleanPrototype)
    }

    pub fn string_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::StringPrototype)
    }

    pub fn symbol_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::SymbolPrototype)
    }

    pub fn bigint_prototype(&self) -> Result<ObjectRef, RuntimeError> {
        self.intrinsic(Intrinsic::BigIntPrototype)
    }

    pub fn function_constructor(&self) -> Result<CallableRef, RuntimeError> {
        self.intrinsic(Intrinsic::Function).map(CallableRef)
    }

    // ---- exceptions ----

    pub fn has_exception(&self) -> Result<bool, RuntimeError> {
        Ok(self.enter()?.exception.is_some())
    }

    /// The pending exception, cleared.
    pub fn take_exception(&mut self) -> Result<Option<Value>, RuntimeError> {
        Ok(self.enter()?.exception.take())
    }

    /// A new native error object (not thrown).
    pub fn new_native_error(
        &self,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<Value, RuntimeError> {
        let mut entry = self.enter()?;
        let error = entry.inner.core.new_error(native_kind(kind), message)?;
        entry.lift(error)
    }

    // ---- hosts ----

    /// qjs's `print` and `console.log`, `scriptArgs`, and under `std` its
    /// `load` and `read`.
    pub fn install_qjs_helpers(&mut self, helpers: &QjsHelpers) -> Result<(), RuntimeError> {
        let mut entry = self.enter()?;
        entry.inner.core.install_print()?;
        if helpers.std {
            entry.inner.core.install_std()?;
        }
        if let Some(args) = &helpers.script_args {
            entry.inner.core.install_script_args(args)?;
        }
        Ok(())
    }

    /// Defines the Test262 host object `$262`; returns it.
    #[cfg(feature = "test262-host")]
    pub fn install_test262_host(&mut self) -> Result<ObjectRef, RuntimeError> {
        {
            let mut entry = self.enter()?;
            entry.inner.core.install_test262_host()?;
        }
        let global = self.global_object()?;
        let key = PropertyKey::from_js_string(crate::engine::value::JsString::from_static("$262"));
        match self.get_property(&global, &key)? {
            Value::Object(host) => Ok(host),
            _ => Err(RuntimeError::Invariant("$262 is an object")),
        }
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

/// A front-end error thrown as QuickJS's `JS_Eval` throws it: an error
/// object of its kind whose `stack` is its location.
fn throw_compile_error(
    entry: &mut Entry<'_>,
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
        ErrorKind::Unsupported => return RuntimeError::Unsupported("constructs of this source"),
        ErrorKind::Internal | ErrorKind::Io => return RuntimeError::Compile(error.clone()),
    };
    let thrown = match entry.inner.core.new_error(kind, error.message()) {
        Ok(thrown) => thrown,
        Err(error) => return error.into(),
    };
    if let (Some((line, column)), Some(object)) =
        (syntax_location(error, source), thrown.as_object())
    {
        let stack = format!("    at {filename}:{line}:{column}\n");
        let handle = entry.inner.core.hold(thrown);
        let stack = entry.inner.core.new_string(&stack);
        let key = entry
            .inner
            .core
            .key_from_units(&"stack".encode_utf16().collect::<Vec<_>>());
        if let (Ok(stack), Ok(key)) = (stack, key) {
            let descriptor = oxide_core::PropertyDescriptor {
                value: Some(stack),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Default::default()
            };
            let _ = entry
                .inner
                .core
                .define_own_property(object, key, &descriptor);
        }
        entry.inner.core.release(handle);
    }
    entry.thrown(oxide_core::Throw(thrown))
}
