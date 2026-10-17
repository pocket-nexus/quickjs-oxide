//! Module loading (phase 6, sections 2.2, 2.3 and 2.7): the embedder's
//! [`ModuleLoader`], the [`Scope`] a host callback receives, and the shims
//! the core's module host calls.

use std::rc::Rc;

use oxide_core::vm::compile::{CompileError, Compiler};

use super::RuntimeError;
use super::compile::{
    ModuleCompileError, compile_module, front_compile_error, normalize_module_name,
};
use super::runtime::{Preloaded, Shared, current, slot};
use super::value::{ObjectRef, SymbolRef, Value};
use crate::engine::api::error::NativeErrorKind;
use crate::engine::code::module::{ModuleImportAttribute, ModuleImportAttributes};
use crate::engine::value::JsString;

/// What a loader returns for a module name.
pub enum ModuleSource {
    /// JavaScript module source, with the properties its `import.meta`
    /// starts with.
    Source {
        text: Vec<u8>,
        import_meta: Vec<(String, Value)>,
    },
    /// A JSON module.
    Json(Vec<u8>),
    /// A JSON5 module (`type: "json5"`).
    Json5(Vec<u8>),
    /// A module compiled through [`Scope::compile_module`].
    Compiled(CompiledModule),
}

impl ModuleSource {
    /// Module source with an empty `import.meta`.
    pub fn source(text: impl Into<Vec<u8>>) -> Self {
        ModuleSource::Source {
            text: text.into(),
            import_meta: Vec::new(),
        }
    }
}

/// A module compiled through a [`Scope`], to return from the loader.
pub struct CompiledModule {
    function: ObjectRef,
    tables: oxide_core::vm::module::ModuleTables,
    import_meta: Vec<(String, Value)>,
}

/// The embedder's module loader. Errors are values to throw, made through
/// the [`Scope`].
pub trait ModuleLoader {
    /// The normalized name of `specifier` imported by the module or script
    /// named `referrer`. The default is QuickJS's: a specifier starting with
    /// `.` resolves against the referrer's directory.
    fn normalize(
        &mut self,
        scope: &mut Scope<'_>,
        referrer: &str,
        specifier: &str,
        attributes: &ModuleImportAttributes,
    ) -> Result<String, Value> {
        let _ = (scope, attributes);
        Ok(normalize_module_name(referrer, specifier))
    }

    /// The module named `name` (normalized).
    fn load(
        &mut self,
        scope: &mut Scope<'_>,
        name: &str,
        attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value>;
}

/// A host callback's view of the runtime (section 2.3): creating strings,
/// objects and errors, compiling a module source. Values made through it are
/// handles like any other and outlive the callback.
pub struct Scope<'a> {
    compiler: Compiler<'a>,
    shared: &'a Rc<Shared>,
}

impl Scope<'_> {
    fn hold(&mut self, value: oxide_core::Value) -> Value {
        use oxide_core::value::Kind;
        match value.kind() {
            Kind::Object => Value::Object(ObjectRef {
                slot: slot(self.shared, self.compiler.hold(value), value),
            }),
            Kind::Symbol => Value::Symbol(SymbolRef {
                slot: slot(self.shared, self.compiler.hold(value), value),
            }),
            Kind::String => {
                let units: Vec<u16> = self
                    .compiler
                    .heap()
                    .string(value.as_string().expect("a string"))
                    .repr()
                    .units()
                    .collect();
                JsString::try_from_utf16(units).map_or(Value::Undefined, Value::String)
            }
            Kind::Bool => Value::Bool(value.as_bool().unwrap_or(false)),
            Kind::Int32 => Value::Int(value.as_int32().unwrap_or(0)),
            Kind::Number => Value::Float(value.as_f64().unwrap_or(f64::NAN)),
            Kind::Null => Value::Null,
            _ => Value::Undefined,
        }
    }

    /// A new ordinary object.
    pub fn new_object(&mut self) -> Result<ObjectRef, RuntimeError> {
        let prototype = self
            .compiler
            .intrinsic(oxide_core::Intrinsic::ObjectPrototype);
        let object = self.compiler.heap_mut().alloc_object(
            oxide_core::ObjectKind::Ordinary,
            Some(prototype),
            4,
        )?;
        match self.hold(oxide_core::Value::object(object)) {
            Value::Object(object) => Ok(object),
            _ => Err(RuntimeError::Invariant("a new object")),
        }
    }

    /// A new error object of `kind` with `message`.
    pub fn new_native_error(
        &mut self,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<Value, RuntimeError> {
        let error = self.compiler.new_error(core_error_kind(kind), message)?;
        Ok(self.hold(error))
    }

    /// Compiles `text` as the module `name` (its position tables and
    /// functions name it). A syntax error is returned as the error to throw.
    pub fn compile_module(
        &mut self,
        name: &str,
        text: &[u8],
        import_meta: Vec<(String, Value)>,
    ) -> Result<CompiledModule, Value> {
        let debug_info = self.shared.debug_info.get();
        match compile_module(&mut self.compiler, name, text, debug_info) {
            Ok((
                oxide_core::vm::module::ModuleSource::Compiled {
                    function, tables, ..
                },
                _,
            )) => {
                let Value::Object(function) = self.hold(oxide_core::Value::object(function)) else {
                    unreachable!("a module function is an object");
                };
                Ok(CompiledModule {
                    function,
                    tables,
                    import_meta,
                })
            }
            Ok(_) => Err(Value::Undefined),
            Err(error) => {
                let failure = match error {
                    ModuleCompileError::Front(error) => front_compile_error(&error, text, name),
                    ModuleCompileError::Core(error) => error,
                };
                let thrown = throw_value(&mut self.compiler, &failure);
                Err(self.hold(thrown))
            }
        }
    }

    /// The core value of `value`, kept alive until the next entry.
    fn lower(&mut self, value: &Value) -> oxide_core::Value {
        let lowered = match value {
            Value::Undefined => oxide_core::Value::UNDEFINED,
            Value::Null => oxide_core::Value::NULL,
            Value::Bool(value) => oxide_core::Value::bool(*value),
            Value::Int(value) => oxide_core::Value::int32(*value),
            Value::Float(value) => oxide_core::Value::number(*value),
            Value::String(text) => {
                let units: Vec<u16> = text.utf16_units().collect();
                match self.compiler.heap_mut().alloc_string_units(&units) {
                    Ok(id) => oxide_core::Value::string(id),
                    Err(_) => oxide_core::Value::UNDEFINED,
                }
            }
            Value::BigInt(_) => oxide_core::Value::UNDEFINED,
            Value::Symbol(symbol) => symbol.core_value(),
            Value::Object(object) => object.core_value(),
        };
        // A temporary handle, released by the next entry.
        if lowered.is_heap() {
            let handle = self.compiler.hold(lowered);
            self.shared.release(handle);
        }
        lowered
    }
}

fn core_error_kind(kind: NativeErrorKind) -> oxide_core::ErrorKind {
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

/// The error a failed module compilation throws, as a value (no backtrace:
/// no frame runs the module yet).
fn throw_value(compiler: &mut Compiler<'_>, failure: &CompileError) -> oxide_core::Value {
    let Ok(error) = compiler.new_error(failure.kind, &failure.message) else {
        return oxide_core::Value::UNDEFINED;
    };
    if let (Some((file, line, column)), Some(object)) = (&failure.location, error.as_object()) {
        let stack = format!("    at {file}:{line}:{column}\n");
        let handle = compiler.hold(error);
        if let (Ok(key), Ok(stack)) = (
            compiler.heap_mut().property_key("stack"),
            compiler.heap_mut().alloc_string(&stack),
        ) {
            let _ = compiler.heap_mut().add_property(
                object,
                key,
                oxide_core::PropertyFlags::data(true, false, true),
                oxide_core::Value::string(stack),
            );
        }
        compiler.heap_mut().handles().release(handle);
    }
    error
}

/// The attributes the core passes, in the front end's form.
fn attributes(pairs: &[(String, String)]) -> ModuleImportAttributes {
    if pairs.is_empty() {
        return ModuleImportAttributes::Absent;
    }
    let attributes: Vec<ModuleImportAttribute> = pairs
        .iter()
        .filter_map(|(key, value)| {
            Some(ModuleImportAttribute {
                key: JsString::try_from_utf8(key).ok()?,
                value: JsString::try_from_utf8(value).ok()?,
            })
        })
        .collect();
    ModuleImportAttributes::Present(attributes.into_boxed_slice())
}

/// The core's `normalize` hook: the embedder's loader, or QuickJS's rule.
pub(crate) fn normalize_shim(
    compiler: &mut Compiler<'_>,
    referrer: &str,
    specifier: &str,
    pairs: &[(String, String)],
) -> Result<String, CompileError> {
    let Some(shared) = current() else {
        return Ok(normalize_module_name(referrer, specifier));
    };
    let Ok(mut loader) = shared.loader.try_borrow_mut() else {
        return Err(CompileError::new(
            oxide_core::ErrorKind::Internal,
            "the module loader was re-entered",
        ));
    };
    let Some(loader) = loader.as_mut() else {
        return Ok(normalize_module_name(referrer, specifier));
    };
    let mut scope = Scope {
        compiler: compiler.reborrow(),
        shared: &shared,
    };
    loader
        .normalize(&mut scope, referrer, specifier, &attributes(pairs))
        .map_err(|thrown| CompileError::thrown(scope.lower(&thrown)))
}

/// The core's `load` hook: a module `Context::compile_module` compiled, or
/// the embedder's loader.
pub(crate) fn load_shim(
    compiler: &mut Compiler<'_>,
    name: &str,
    pairs: &[(String, String)],
) -> Result<oxide_core::vm::module::ModuleSource, CompileError> {
    let Some(shared) = current() else {
        return Err(CompileError::new(
            oxide_core::ErrorKind::Internal,
            "unsupported: modules",
        ));
    };
    let preloaded = shared.modules.borrow_mut().remove(name);
    if let Some(Preloaded {
        function,
        tables,
        import_meta,
    }) = preloaded
    {
        let mut scope = Scope {
            compiler: compiler.reborrow(),
            shared: &shared,
        };
        return Ok(compiled_source(&mut scope, function, tables, &import_meta));
    }
    let Ok(mut loader) = shared.loader.try_borrow_mut() else {
        return Err(CompileError::new(
            oxide_core::ErrorKind::Internal,
            "the module loader was re-entered",
        ));
    };
    let Some(loader) = loader.as_mut() else {
        return Err(CompileError::new(
            oxide_core::ErrorKind::Reference,
            format!("could not load module filename '{name}'"),
        ));
    };
    let mut scope = Scope {
        compiler: compiler.reborrow(),
        shared: &shared,
    };
    let source = loader
        .load(&mut scope, name, &attributes(pairs))
        .map_err(|thrown| CompileError::thrown(scope.lower(&thrown)))?;
    match source {
        ModuleSource::Source { text, import_meta } => {
            let module = scope
                .compile_module(name, &text, import_meta)
                .map_err(|thrown| CompileError::thrown(scope.lower(&thrown)))?;
            Ok(compiled_source(
                &mut scope,
                module.function,
                module.tables,
                &module.import_meta,
            ))
        }
        ModuleSource::Compiled(module) => Ok(compiled_source(
            &mut scope,
            module.function,
            module.tables,
            &module.import_meta,
        )),
        ModuleSource::Json(text) => Ok(oxide_core::vm::module::ModuleSource::Json(
            String::from_utf8_lossy(&text).encode_utf16().collect(),
        )),
        ModuleSource::Json5(text) => Ok(oxide_core::vm::module::ModuleSource::Json5(
            String::from_utf8_lossy(&text).encode_utf16().collect(),
        )),
    }
}

fn compiled_source(
    scope: &mut Scope<'_>,
    function: ObjectRef,
    tables: oxide_core::vm::module::ModuleTables,
    import_meta: &[(String, Value)],
) -> oxide_core::vm::module::ModuleSource {
    let import_meta = import_meta
        .iter()
        .map(|(key, value)| (key.clone(), scope.lower(value)))
        .collect();
    let id = function
        .core_value()
        .as_object()
        .expect("a module function");
    // The function stays rooted by a handle released at the next entry.
    let _ = scope.lower(&Value::Object(function));
    oxide_core::vm::module::ModuleSource::Compiled {
        function: id,
        tables,
        import_meta,
    }
}
