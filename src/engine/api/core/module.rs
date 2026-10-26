//! Module loading: the embedder's
//! [`ModuleLoader`], the [`ModuleScope`] a host callback receives, and the shims
//! the core's module host calls.

use std::rc::Rc;

use oxide_core::vm::compile::{CompileError, Compiler};

use super::RuntimeError;
use super::compile::{
    ModuleCompileError, compile_module, front_compile_error, normalize_module_name,
};
use super::function::{FunctionOptions, HostFunction, host_body};
use super::runtime::{Preloaded, Shared, current, slot};
use super::value::{CallableRef, ObjectRef, SymbolRef, Value};
use crate::engine::api::error::NativeErrorKind;
use crate::engine::value::JsString;

/// One decoded entry from a static import declaration's `with` clause.
///
/// QuickJS builds the corresponding null-prototype object in source order.
/// Keeping the ordered entries structural here preserves that observable
/// enumeration order without introducing a JavaScript heap object during
/// parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleImportAttribute {
    /// Decoded property key, after identifier and string-literal escapes.
    pub key: JsString,
    /// Decoded StringLiteral value.
    pub value: JsString,
}

/// Syntactic import-attribute state for one requested module.
///
/// `Present([])` deliberately differs from [`Self::Absent`] so compiler and
/// source tooling can retain whether `with {}` was authored. Pinned QuickJS
/// does not allocate an attributes object until the first entry, however, so
/// both states collapse to `None` through [`Self::effective`] at the host
/// checker and loader boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleImportAttributes {
    /// No `with` clause was authored.
    Absent,
    /// An authored `with` clause, possibly empty.
    Present(Box<[ModuleImportAttribute]>),
}

impl ModuleImportAttributes {
    /// Attributes exposed to QuickJS-compatible host hooks.
    #[must_use]
    pub fn effective(&self) -> Option<&[ModuleImportAttribute]> {
        match self {
            Self::Present(attributes) if !attributes.is_empty() => Some(attributes),
            Self::Absent | Self::Present(_) => None,
        }
    }

    /// Entries from an authored `with` clause, including an empty clause.
    #[must_use]
    pub fn syntactic(&self) -> Option<&[ModuleImportAttribute]> {
        match self {
            Self::Absent => None,
            Self::Present(attributes) => Some(attributes),
        }
    }
}

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
    /// A module compiled through [`ModuleScope::compile_module`].
    Compiled(CompiledModule),
    /// A module whose exports are these values, each under its name;
    /// `default` is the default export.
    Synthetic { exports: Vec<(String, Value)> },
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

/// A module compiled through a [`ModuleScope`], to return from the loader.
pub struct CompiledModule {
    function: ObjectRef,
    tables: oxide_core::vm::module::ModuleTables,
    import_meta: Vec<(String, Value)>,
}

/// The embedder's module loader. Errors are values to throw, made through
/// the [`ModuleScope`].
pub trait ModuleLoader {
    /// The normalized name of `specifier` imported by the module or script
    /// named `referrer`. The default is QuickJS's: a specifier starting with
    /// `.` resolves against the referrer's directory.
    fn normalize(
        &mut self,
        scope: &mut ModuleScope<'_>,
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
        scope: &mut ModuleScope<'_>,
        name: &str,
        attributes: &ModuleImportAttributes,
    ) -> Result<ModuleSource, Value>;
}

/// A module loader's view of the runtime: creating objects, errors, host
/// functions and host objects, compiling a module source. It is the
/// compile-time view (no job queue, no running script), so it has these
/// operations and not those of [`Js`](super::Js). Values made through it are
/// handles like any other and outlive the callback.
pub struct ModuleScope<'a> {
    compiler: Compiler<'a>,
    shared: &'a Rc<Shared>,
}

impl ModuleScope<'_> {
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

    /// A host function, as [`Js::new_function`](super::Js::new_function)
    /// makes one.
    pub fn new_function<Args>(
        &mut self,
        options: FunctionOptions,
        function: impl HostFunction<Args>,
    ) -> Result<CallableRef, RuntimeError> {
        let mut data = Vec::with_capacity(options.data.len());
        for value in &options.data {
            data.push(self.lower(value));
        }
        let body = host_body(self.shared, function);
        let object = self.compiler.new_host_function(
            &options.name,
            options.length,
            options.constructor,
            &data,
            body,
        )?;
        match self.hold(oxide_core::Value::object(object)) {
            Value::Object(object) => Ok(CallableRef(object)),
            _ => Err(RuntimeError::Invariant("a new function")),
        }
    }

    /// A host object, as [`Js::new_host_object`](super::Js::new_host_object)
    /// makes one.
    pub fn new_host_object<T: 'static>(
        &mut self,
        value: T,
        prototype: Option<&ObjectRef>,
    ) -> Result<ObjectRef, RuntimeError> {
        let prototype = prototype.and_then(|prototype| prototype.core_value().as_object());
        let object = self
            .compiler
            .new_host_object(prototype, std::rc::Rc::new(value))?;
        match self.hold(oxide_core::Value::object(object)) {
            Value::Object(object) => Ok(object),
            _ => Err(RuntimeError::Invariant("a new host object")),
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
    let created = match &failure.location {
        Some((file, line, column)) => {
            compiler.new_located_error(failure.kind, &failure.message, file, *line, *column)
        }
        None => compiler.new_error(failure.kind, &failure.message),
    };
    created.unwrap_or(oxide_core::Value::UNDEFINED)
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
    let mut scope = ModuleScope {
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
    if let Some(preloaded) = preloaded {
        let mut scope = ModuleScope {
            compiler: compiler.reborrow(),
            shared: &shared,
        };
        return Ok(match preloaded {
            Preloaded::Compiled {
                function,
                tables,
                import_meta,
            } => compiled_source(&mut scope, function, tables, &import_meta),
            Preloaded::Synthetic(exports) => synthetic_source(&mut scope, &exports),
        });
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
    let mut scope = ModuleScope {
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
        ModuleSource::Synthetic { exports } => Ok(synthetic_source(&mut scope, &exports)),
        ModuleSource::Json(text) => {
            Ok(oxide_core::vm::module::ModuleSource::Json(json_text(&text)))
        }
        ModuleSource::Json5(text) => Ok(oxide_core::vm::module::ModuleSource::Json5(json_text(
            &text,
        ))),
    }
}

/// The core's form of a synthetic module: its values, rooted until the next
/// entry.
fn synthetic_source(
    scope: &mut ModuleScope<'_>,
    exports: &[(String, Value)],
) -> oxide_core::vm::module::ModuleSource {
    oxide_core::vm::module::ModuleSource::Synthetic {
        exports: exports
            .iter()
            .map(|(name, value)| (name.clone(), scope.lower(value)))
            .collect(),
    }
}

/// A JSON module's bytes as QuickJS's byte-level parser reads them.
fn json_text(bytes: &[u8]) -> oxide_core::vm::module::JsonText {
    oxide_core::vm::module::JsonText::from_bytes(bytes, crate::engine::value::decode_quickjs_utf8)
}

fn compiled_source(
    scope: &mut ModuleScope<'_>,
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
