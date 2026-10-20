//! ECMAScript modules.
//!
//! A module record is an internal record (tag `Module`) whose slots hold
//! the module's environment (the scope its root function captures: one cell
//! per binding), its root function, its namespace, its evaluation error and
//! its top-level promise capability; the rest is [`ModuleData`]. Every
//! record stays in the registry for the runtime's life (QuickJS keeps its
//! modules too), so the records name each other by `ObjectId`.
//!
//! An import binding is the exporter's cell, installed in the importer's
//! environment while linking: a live binding is a shared cell, read by the
//! loop's `LoadCaptured` with no new opcode. Linking and evaluation are the
//! specification's (`Link`, `InnerModuleLinking`, `Evaluate`,
//! `InnerModuleEvaluation` and the asynchronous-module machinery for
//! top-level `await`). Modules load synchronously through the host's
//! [`ModuleHost`] when they are first requested.

use std::cell::Cell;

use super::compile::{CompileError, Compiler};
use super::native::CallSite;
use super::{Cx, Throw, error};
use crate::PropertyKey;
use crate::builtins::{Native, native_closure, native_data};
use crate::heap::object::{InternalData, InternalTag, ObjectKind, flags};
use crate::runtime::Intrinsic;
use crate::value::{ObjectId, ScopeId, Value};
use crate::{ErrorKind, KeyKind};

// ---- the host's description of a module ----

/// What one binding of a module's root function is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleBinding {
    /// A declaration of the module: `lexical` ones start uninitialized.
    Declaration { lexical: bool },
    /// An import (or a declaration sharing an import's slot): the
    /// exporter's cell, installed while linking.
    Import,
    /// The module's `import.meta` object.
    ImportMeta,
}

/// One requested module.
#[derive(Clone, Debug)]
pub struct ModuleRequest {
    pub specifier: String,
    pub attributes: Vec<(String, String)>,
}

/// One import: from request `request`, the export `name` (`None` for the
/// namespace), into binding `binding`.
#[derive(Clone, Debug)]
pub struct ModuleImport {
    pub request: u32,
    pub name: Option<String>,
    pub binding: u16,
}

/// Where an exported name comes from.
#[derive(Clone, Debug)]
pub enum ExportTarget {
    /// A binding of the module.
    Local(u16),
    /// An export of request `request` (`None` for its namespace).
    Indirect { request: u32, name: Option<String> },
}

#[derive(Clone, Debug)]
pub struct ModuleExport {
    pub name: String,
    pub target: ExportTarget,
}

/// A hoisted declaration initialized while linking: `undefined`, or a
/// function from child `child` of the root unit (named `default` for an
/// anonymous default export).
#[derive(Clone, Copy, Debug)]
pub struct ModuleInitializer {
    pub binding: u16,
    pub function: Option<(u32, bool)>,
}

/// The tables of a compiled module.
#[derive(Clone, Debug, Default)]
pub struct ModuleTables {
    pub bindings: Vec<ModuleBinding>,
    pub requests: Vec<ModuleRequest>,
    pub imports: Vec<ModuleImport>,
    pub exports: Vec<ModuleExport>,
    pub star_exports: Vec<u32>,
    pub initializers: Vec<ModuleInitializer>,
    pub has_top_level_await: bool,
}

/// A loaded module: its root function (instantiated in the [`Compiler`])
/// and its tables, or the text of a JSON module.
pub enum ModuleSource {
    Compiled {
        function: ObjectId,
        tables: ModuleTables,
        /// Properties of the module's `import.meta`, in order (qjs's `url`
        /// and `main`); the values must be rooted by the loader.
        import_meta: Vec<(String, Value)>,
    },
    Json(JsonText),
    /// A JSON5 module (QuickJS's extended JSON, `type: "json5"`).
    Json5(JsonText),
}

/// A JSON module's text: UTF-16 units, and the units that stand for bytes
/// of invalid UTF-8 in the file, which QuickJS's byte-level parser rejects
/// with `Bad UTF-8 sequence` where a token reads them (not in a comment).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsonText {
    pub units: Vec<u16>,
    /// Sorted indices into `units`.
    pub invalid: Vec<u32>,
    /// Sorted indices of the low halves of four-byte sequences and of stray
    /// continuation bytes, which QuickJS's byte columns do not count (a
    /// CESU-8 pair counts twice).
    pub no_column: Vec<u32>,
}

impl From<Vec<u16>> for JsonText {
    fn from(units: Vec<u16>) -> Self {
        JsonText {
            units,
            invalid: Vec::new(),
            no_column: Vec::new(),
        }
    }
}

impl JsonText {
    /// The text of `bytes` decoded as QuickJS decodes UTF-8 (lone and
    /// paired surrogates as is); each byte of an invalid sequence becomes
    /// one inert unit marked invalid. `decode` is QuickJS's code-point
    /// decoder: the code point and the bytes it used, or `None`.
    pub fn from_bytes(bytes: &[u8], decode: impl Fn(&[u8]) -> Option<(u32, usize)>) -> Self {
        let mut text = JsonText {
            units: Vec::with_capacity(bytes.len()),
            invalid: Vec::new(),
            no_column: Vec::new(),
        };
        let mut offset = 0;
        while offset < bytes.len() {
            let byte = bytes[offset];
            if byte < 0x80 {
                text.units.push(u16::from(byte));
                offset += 1;
                continue;
            }
            match decode(&bytes[offset..]) {
                Some((code, used)) if code <= 0x10_ffff => {
                    if code <= 0xffff {
                        text.units.push(code as u16);
                    } else {
                        let scalar = code - 0x1_0000;
                        text.units.push(0xd800 | (scalar >> 10) as u16);
                        text.no_column.push(text.units.len() as u32);
                        text.units.push(0xdc00 | (scalar & 0x3ff) as u16);
                    }
                    offset += used;
                }
                _ => {
                    // A stray continuation byte adds no column either.
                    if (0x80..0xc0).contains(&byte) {
                        text.no_column.push(text.units.len() as u32);
                    }
                    text.invalid.push(text.units.len() as u32);
                    text.units.push(0x7f);
                    offset += 1;
                }
            }
        }
        text
    }
}

/// Resolves `specifier` against the module or script named `referrer`.
pub type NormalizeFn =
    fn(&mut Compiler<'_>, &str, &str, &[(String, String)]) -> Result<String, CompileError>;

/// Loads and compiles the module named `name`.
pub type LoadModuleFn =
    fn(&mut Compiler<'_>, &str, &[(String, String)]) -> Result<ModuleSource, CompileError>;

/// The step of running a module that threw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulePhase {
    Load,
    Link,
    Evaluate,
}

/// The embedder's module loader.
#[derive(Clone, Copy)]
pub struct ModuleHost {
    pub normalize: NormalizeFn,
    pub load: LoadModuleFn,
}

// ---- records ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Unlinked,
    Linking,
    Linked,
    Evaluating,
    EvaluatingAsync,
    Evaluated,
}

/// A module record's non-value state.
#[derive(Debug)]
pub struct ModuleData {
    name: String,
    tables: ModuleTables,
    status: Status,
    /// The records of `tables.requests`, once loaded.
    requested: Vec<ObjectId>,
    dfs_index: u32,
    dfs_ancestor_index: u32,
    /// `[[AsyncEvaluation]]`'s order, when true.
    async_evaluation: Option<u64>,
    pending_async_dependencies: u32,
    async_parents: Vec<ObjectId>,
    cycle_root: Option<ObjectId>,
}

// Slots of a module record.
const ENVIRONMENT: usize = 0;
const FUNCTION: usize = 1;
const NAMESPACE: usize = 2;
const ERROR: usize = 3;
const CAPABILITY: usize = 4;
const SLOTS: usize = 7;

thread_local! {
    static ASYNC_ORDER: Cell<u64> = const { Cell::new(0) };
}

fn data<'a>(cx: &'a Cx<'_>, module: ObjectId) -> &'a ModuleData {
    match &cx.heap.internal(module).data {
        InternalData::Module(data) => data,
        _ => unreachable!("a module record"),
    }
}

fn data_mut<'a>(cx: &'a mut Cx<'_>, module: ObjectId) -> &'a mut ModuleData {
    match &mut cx.heap.internal_mut(module).data {
        InternalData::Module(data) => data,
        _ => unreachable!("a module record"),
    }
}

fn slot(cx: &Cx<'_>, module: ObjectId, index: usize) -> Value {
    cx.heap.internal_slot(module, index)
}

fn set_slot(cx: &mut Cx<'_>, module: ObjectId, index: usize, value: Value) {
    cx.heap.set_internal_slot(module, index, value);
}

fn environment(cx: &Cx<'_>, module: ObjectId) -> ScopeId {
    ScopeId(
        slot(cx, module, ENVIRONMENT)
            .as_object()
            .expect("an environment")
            .index(),
    )
}

/// The cell of binding `binding` of `module`.
fn cell(cx: &Cx<'_>, module: ObjectId, binding: u16) -> Value {
    cx.heap
        .scope_get(environment(cx, module), 0, u32::from(binding))
}

fn compile_error(cx: &mut Cx<'_>, failure: CompileError) -> Throw {
    super::compile::throw_failure(cx, failure)
}

/// The record of the module named `name`, loading and compiling it (and,
/// recursively, what it requests) on first use.
fn load(cx: &mut Cx<'_>, name: &str, attributes: &[(String, String)]) -> Result<ObjectId, Throw> {
    if let Some(&module) = cx.host.modules.get(name) {
        return Ok(module);
    }
    let Some(host) = cx.host.module_host else {
        return Err(error::throw(
            cx,
            ErrorKind::Internal,
            "unsupported: modules",
        ));
    };
    let source = (host.load)(
        &mut Compiler::new(cx.heap, cx.static_code, cx.realm),
        name,
        attributes,
    );
    let source = source.map_err(|failure| compile_error(cx, failure))?;
    let mark = cx.heap.roots().mark();
    let result = match source {
        ModuleSource::Compiled {
            function,
            tables,
            import_meta,
        } => {
            cx.heap.roots().push(Value::object(function));
            create(cx, name, Some(function), tables, &import_meta)
        }
        ModuleSource::Json(text) => json_module(cx, name, &text, false),
        ModuleSource::Json5(text) => json_module(cx, name, &text, true),
    };
    cx.heap.roots().truncate(mark);
    let module = result?;
    // The requested modules, depth first.
    let requests = data(cx, module).tables.requests.clone();
    let mut requested = Vec::with_capacity(requests.len());
    for request in &requests {
        let normalized = (host.normalize)(
            &mut Compiler::new(cx.heap, cx.static_code, cx.realm),
            name,
            &request.specifier,
            &request.attributes,
        );
        let normalized = normalized.map_err(|failure| compile_error(cx, failure))?;
        requested.push(load(cx, &normalized, &request.attributes)?);
    }
    data_mut(cx, module).requested = requested;
    Ok(module)
}

/// A record for a freshly compiled module, with its environment: a cell per
/// binding (an uninitialized lexical one empty, `import.meta` its object),
/// imports filled while linking.
/// A JSON module: its one binding, the default export, is the parsed text.
fn json_module(
    cx: &mut Cx<'_>,
    name: &str,
    text: &JsonText,
    extended: bool,
) -> Result<ObjectId, Throw> {
    let value = crate::builtins::json_module_text(cx, text, name, extended)?;
    cx.heap.roots().push(value);
    let tables = ModuleTables {
        bindings: vec![ModuleBinding::Declaration { lexical: false }],
        exports: vec![ModuleExport {
            name: "default".into(),
            target: ExportTarget::Local(0),
        }],
        ..ModuleTables::default()
    };
    let module = create(cx, name, None, tables, &[])?;
    let holder = cell(cx, module, 0);
    cx.heap
        .scope_set(ScopeId(holder.as_object().unwrap().index()), 0, 0, value);
    Ok(module)
}

fn create(
    cx: &mut Cx<'_>,
    name: &str,
    function: Option<ObjectId>,
    tables: ModuleTables,
    import_meta: &[(String, Value)],
) -> Result<ObjectId, Throw> {
    let source = ModuleParts { function, tables };
    let heap_error = |cx: &mut Cx<'_>, error| error::heap_error(cx, error);
    let count = source.tables.bindings.len() as u32;
    let scope = cx
        .heap
        .alloc_scope(None, count)
        .map_err(|e| heap_error(cx, e))?;
    cx.heap.roots().push(Value::object(scope.object()));
    for (index, binding) in source.tables.bindings.iter().enumerate() {
        let initial = match binding {
            ModuleBinding::Declaration { lexical: true } => Value::EMPTY,
            ModuleBinding::Declaration { lexical: false } | ModuleBinding::Import => {
                Value::UNDEFINED
            }
            ModuleBinding::ImportMeta => {
                let meta = cx
                    .heap
                    .alloc_object(ObjectKind::Ordinary, None, 2)
                    .map_err(|e| heap_error(cx, e))?;
                cx.heap.roots().push(Value::object(meta));
                for (key, value) in import_meta {
                    let key = cx.heap.property_key(key).map_err(|e| heap_error(cx, e))?;
                    cx.heap
                        .add_property(meta, key, crate::PropertyFlags::DEFAULT, *value)
                        .map_err(|e| heap_error(cx, e))?;
                }
                Value::object(meta)
            }
        };
        if *binding == ModuleBinding::Import {
            continue;
        }
        let cell = cx
            .heap
            .alloc_scope(None, 1)
            .map_err(|e| heap_error(cx, e))?;
        cx.heap.scope_set(cell, 0, 0, initial);
        cx.heap
            .scope_set(scope, 0, index as u32, Value::object(cell.object()));
    }
    if let Some(function) = source.function {
        cx.heap.set_function_scope(function, scope);
    }
    let mut slots = vec![Value::UNDEFINED; SLOTS];
    slots[ENVIRONMENT] = Value::object(scope.object());
    slots[FUNCTION] = source.function.map_or(Value::UNDEFINED, Value::object);
    slots[ERROR] = Value::EMPTY;
    let record = ModuleData {
        name: name.to_owned(),
        tables: source.tables,
        status: Status::Unlinked,
        requested: Vec::new(),
        dfs_index: 0,
        dfs_ancestor_index: 0,
        async_evaluation: None,
        pending_async_dependencies: 0,
        async_parents: Vec::new(),
        cycle_root: None,
    };
    let module = cx
        .heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::Module,
            None,
            0,
            &slots,
            InternalData::Module(Box::new(record)),
            0,
        )
        .map_err(|e| heap_error(cx, e))?;
    cx.heap.modules.push(Value::object(module));
    cx.host.modules.insert(name.to_owned(), module);
    Ok(module)
}

struct ModuleParts {
    function: Option<ObjectId>,
    tables: ModuleTables,
}

// ---- exports ----

/// What an export resolves to: a module's binding, or its namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Resolution {
    Binding(ObjectId, u16),
    Namespace(ObjectId),
}

enum Resolved {
    Found(Resolution),
    NotFound,
    /// The walk met a `(module, name)` pair it is resolving already.
    Circular,
    Ambiguous,
}

/// `ResolveExport(exportName, resolveSet)`.
fn resolve_export(
    cx: &Cx<'_>,
    module: ObjectId,
    name: &str,
    set: &mut Vec<(ObjectId, String)>,
) -> Resolved {
    if set.iter().any(|(m, n)| *m == module && n == name) {
        return Resolved::Circular;
    }
    set.push((module, name.to_owned()));
    let record = data(cx, module);
    for export in &record.tables.exports {
        if export.name != name {
            continue;
        }
        return match export.target.clone() {
            ExportTarget::Local(binding) => {
                // A re-exported import resolves where the import does.
                match record
                    .tables
                    .imports
                    .iter()
                    .find(|import| import.binding == binding)
                {
                    Some(import) => {
                        let imported = record.requested[import.request as usize];
                        let import_name = import.name.clone();
                        match import_name {
                            None => Resolved::Found(Resolution::Namespace(imported)),
                            Some(import_name) => resolve_export(cx, imported, &import_name, set),
                        }
                    }
                    None => Resolved::Found(Resolution::Binding(module, binding)),
                }
            }
            ExportTarget::Indirect {
                request,
                name: import_name,
            } => {
                let imported = record.requested[request as usize];
                match import_name {
                    None => Resolved::Found(Resolution::Namespace(imported)),
                    Some(import_name) => resolve_export(cx, imported, &import_name, set),
                }
            }
        };
    }
    if name == "default" {
        return Resolved::NotFound;
    }
    let mut star: Option<Resolution> = None;
    for &request in &record.tables.star_exports {
        let imported = record.requested[request as usize];
        match resolve_export(cx, imported, name, set) {
            Resolved::Ambiguous => return Resolved::Ambiguous,
            Resolved::NotFound | Resolved::Circular => {}
            Resolved::Found(resolution) => match star {
                None => star = Some(resolution),
                Some(existing) if existing != resolution => return Resolved::Ambiguous,
                Some(_) => {}
            },
        }
    }
    match star {
        Some(resolution) => Resolved::Found(resolution),
        None => Resolved::NotFound,
    }
}

/// `GetExportedNames(exportStarSet)`.
fn exported_names(cx: &Cx<'_>, module: ObjectId, set: &mut Vec<ObjectId>) -> Vec<String> {
    if set.contains(&module) {
        return Vec::new();
    }
    set.push(module);
    let record = data(cx, module);
    let mut names: Vec<String> = record
        .tables
        .exports
        .iter()
        .map(|export| export.name.clone())
        .collect();
    for &request in &record.tables.star_exports {
        let imported = record.requested[request as usize];
        for name in exported_names(cx, imported, set) {
            if name != "default" && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

fn syntax(cx: &mut Cx<'_>, message: String) -> Throw {
    error::throw(cx, ErrorKind::Syntax, &message)
}

/// QuickJS's `js_resolve_export_throw_error` for `name` in `module`.
#[cold]
#[inline(never)]
fn resolve_error(cx: &mut Cx<'_>, resolved: &Resolved, module: ObjectId, name: &str) -> Throw {
    let name = super::property::str_atom_text(name);
    let module = super::property::str_atom_text(&data(cx, module).name);
    let message = match resolved {
        Resolved::Circular => {
            format!("circular reference when looking for export '{name}' in module '{module}'")
        }
        Resolved::Ambiguous => format!("export '{name}' in module '{module}' is ambiguous"),
        _ => format!("Could not find export '{name}' in module '{module}'"),
    };
    syntax(cx, message)
}

/// `GetModuleNamespace(module)`.
pub(crate) fn namespace(cx: &mut Cx<'_>, module: ObjectId) -> Result<ObjectId, Throw> {
    if let Some(namespace) = slot(cx, module, NAMESPACE).as_object() {
        return Ok(namespace);
    }
    let names = exported_names(cx, module, &mut Vec::new());
    let mut exports: Vec<(Vec<u16>, Resolution)> = Vec::new();
    for name in names {
        if let Resolved::Found(resolution) = resolve_export(cx, module, &name, &mut Vec::new()) {
            exports.push((name.encode_utf16().collect(), resolution));
        }
    }
    exports.sort_by(|a, b| a.0.cmp(&b.0));
    let mut keys = Vec::with_capacity(exports.len());
    let mut slots = vec![Value::object(module)];
    // The keys are rooted until the namespace holds them.
    let mark = cx.heap.roots().mark();
    let namespace = (|| {
        for (name, resolution) in &exports {
            let key = cx
                .heap
                .property_key_units(name)
                .map_err(|e| error::heap_error(cx, e))?;
            if let Some(value) = key.heap_value() {
                cx.heap.roots().push(value);
            }
            keys.push(key);
            slots.push(match *resolution {
                Resolution::Binding(owner, binding) => cell(cx, owner, binding),
                Resolution::Namespace(owner) => Value::object(owner),
            });
        }
        cx.heap
            .alloc_internal(
                ObjectKind::ModuleNamespace,
                InternalTag::None,
                None,
                1,
                &slots,
                InternalData::Exports(std::mem::take(&mut keys).into_boxed_slice()),
                0,
            )
            .map_err(|e| error::heap_error(cx, e))
    })();
    cx.heap.roots().truncate(mark);
    let namespace = namespace?;
    cx.heap.clear_object_flag(namespace, flags::EXTENSIBLE);
    // Interning may collect: the namespace is not reachable yet.
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(namespace));
    let tag = cx.heap.intern("Module");
    cx.heap.roots().truncate(mark);
    let tag = tag.map_err(|e| error::heap_error(cx, e))?;
    let key = crate::runtime::WellKnown::key(cx.realm.symbols.to_string_tag);
    cx.heap
        .add_property(
            namespace,
            key,
            crate::PropertyFlags::data(false, false, false),
            Value::string(tag),
        )
        .map_err(|e| error::heap_error(cx, e))?;
    set_slot(cx, module, NAMESPACE, Value::object(namespace));
    Ok(namespace)
}

/// The value of a namespace's export at `position`, or a `ReferenceError`
/// while it is uninitialized.
pub(crate) fn namespace_value(
    cx: &mut Cx<'_>,
    object: ObjectId,
    position: usize,
) -> Result<Value, Throw> {
    let holder = cx.heap.internal_slot(object, position + 1);
    let holder = holder.as_object().expect("a binding holder");
    if cx.heap.internal_tag(holder) == InternalTag::Module {
        return namespace(cx, holder).map(Value::object);
    }
    let value = cx.heap.scope_get(ScopeId(holder.index()), 0, 0);
    if value.is_empty() {
        let key = namespace_keys(cx, object)[position];
        let message = format!(
            "{} is not initialized",
            super::property::atom_text(cx.heap, key)
        );
        return Err(error::throw(cx, ErrorKind::Reference, &message));
    }
    Ok(value)
}

/// A namespace's export keys, sorted.
pub(crate) fn namespace_keys<'a>(cx: &'a Cx<'_>, namespace: ObjectId) -> &'a [PropertyKey] {
    match &cx.heap.internal(namespace).data {
        InternalData::Exports(keys) => keys,
        _ => unreachable!("a namespace"),
    }
}

/// The position of export `key` of a namespace.
pub(crate) fn namespace_position(
    cx: &Cx<'_>,
    namespace: ObjectId,
    key: PropertyKey,
) -> Option<usize> {
    if matches!(key.kind(), KeyKind::Symbol(_)) {
        return None;
    }
    namespace_keys(cx, namespace)
        .iter()
        .position(|&export| export == key)
}

// ---- linking ----

/// `Link()`.
fn link(cx: &mut Cx<'_>, module: ObjectId) -> Result<(), Throw> {
    let mut stack = Vec::new();
    if let Err(throw) = inner_link(cx, module, &mut stack, 0) {
        for module in stack {
            data_mut(cx, module).status = Status::Unlinked;
        }
        return Err(throw);
    }
    Ok(())
}

/// `InnerModuleLinking(module, stack, index)`.
fn inner_link(
    cx: &mut Cx<'_>,
    module: ObjectId,
    stack: &mut Vec<ObjectId>,
    mut index: u32,
) -> Result<u32, Throw> {
    if data(cx, module).status != Status::Unlinked {
        return Ok(index);
    }
    let record = data_mut(cx, module);
    record.status = Status::Linking;
    record.dfs_index = index;
    record.dfs_ancestor_index = index;
    index += 1;
    stack.push(module);
    let requested = data(cx, module).requested.clone();
    for required in requested {
        index = inner_link(cx, required, stack, index)?;
        let required = data(cx, required);
        if required.status == Status::Linking {
            let ancestor = required.dfs_ancestor_index;
            let record = data_mut(cx, module);
            record.dfs_ancestor_index = record.dfs_ancestor_index.min(ancestor);
        }
    }
    initialize_environment(cx, module)?;
    let record = data(cx, module);
    if record.dfs_ancestor_index == record.dfs_index {
        while let Some(member) = stack.pop() {
            data_mut(cx, member).status = Status::Linked;
            if member == module {
                break;
            }
        }
    }
    Ok(index)
}

/// `InitializeEnvironment()`: checks the indirect exports, installs the
/// imports' cells and creates the hoisted functions.
fn initialize_environment(cx: &mut Cx<'_>, module: ObjectId) -> Result<(), Throw> {
    let tables = data(cx, module).tables.clone();
    let requested = data(cx, module).requested.clone();
    for export in &tables.exports {
        if let ExportTarget::Indirect { name: Some(_), .. } = export.target {
            match resolve_export(cx, module, &export.name, &mut Vec::new()) {
                Resolved::Found(_) => {}
                failure => return Err(resolve_error(cx, &failure, module, &export.name)),
            }
        }
    }
    let scope = environment(cx, module);
    for import in &tables.imports {
        let imported = requested[import.request as usize];
        let holder = match &import.name {
            None => {
                let namespace = namespace(cx, imported)?;
                let cell = cx
                    .heap
                    .alloc_scope(None, 1)
                    .map_err(|e| error::heap_error(cx, e))?;
                cx.heap.scope_set(cell, 0, 0, Value::object(namespace));
                Value::object(cell.object())
            }
            Some(name) => match resolve_export(cx, imported, name, &mut Vec::new()) {
                Resolved::Found(Resolution::Binding(owner, binding)) => cell(cx, owner, binding),
                Resolved::Found(Resolution::Namespace(owner)) => {
                    let namespace = namespace(cx, owner)?;
                    let cell = cx
                        .heap
                        .alloc_scope(None, 1)
                        .map_err(|e| error::heap_error(cx, e))?;
                    cx.heap.scope_set(cell, 0, 0, Value::object(namespace));
                    Value::object(cell.object())
                }
                failure => return Err(resolve_error(cx, &failure, imported, name)),
            },
        };
        cx.heap
            .scope_set(scope, 0, u32::from(import.binding), holder);
    }
    let Some(function) = slot(cx, module, FUNCTION).as_object() else {
        return Ok(());
    };
    for initializer in &tables.initializers {
        let value = match initializer.function {
            None => Value::UNDEFINED,
            Some((child, default_name)) => {
                let closure = super::interp::closure_over_captures(cx, function, child)?;
                if default_name {
                    let name = cx
                        .heap
                        .property_key("default")
                        .map_err(|e| error::heap_error(cx, e))?;
                    super::interp::set_function_name(cx, closure, name, "", false)?;
                }
                Value::object(closure)
            }
        };
        let holder = cell(cx, module, initializer.binding);
        cx.heap
            .scope_set(ScopeId(holder.as_object().unwrap().index()), 0, 0, value);
    }
    Ok(())
}

// ---- evaluation ----

/// `Evaluate()`: the promise of the module's evaluation.
fn evaluate(cx: &mut Cx<'_>, module: ObjectId) -> Result<ObjectId, Throw> {
    let mut module = module;
    let status = data(cx, module).status;
    if matches!(status, Status::EvaluatingAsync | Status::Evaluated)
        && let Some(root) = data(cx, module).cycle_root
    {
        module = root;
    }
    if let Some(promise) = slot(cx, module, CAPABILITY).as_object() {
        return Ok(promise);
    }
    let prototype = cx.realm.intrinsic(Intrinsic::PromisePrototype);
    let promise = super::jobs::new_promise(cx, prototype)?;
    set_slot(cx, module, CAPABILITY, Value::object(promise));
    let mut stack = Vec::new();
    match inner_evaluate(cx, module, &mut stack, 0) {
        Err(Throw(thrown)) => {
            for member in stack {
                data_mut(cx, member).status = Status::Evaluated;
                set_slot(cx, member, ERROR, thrown);
            }
            super::jobs::reject_promise(cx, promise, thrown);
        }
        Ok(_) => {
            if data(cx, module).async_evaluation.is_none() {
                super::jobs::fulfill_promise(cx, promise, Value::UNDEFINED);
            }
        }
    }
    Ok(promise)
}

/// The evaluation error of an evaluated module, if any.
fn evaluation_error(cx: &Cx<'_>, module: ObjectId) -> Option<Value> {
    let error = slot(cx, module, ERROR);
    (!error.is_empty()).then_some(error)
}

/// `InnerModuleEvaluation(module, stack, index)`.
fn inner_evaluate(
    cx: &mut Cx<'_>,
    module: ObjectId,
    stack: &mut Vec<ObjectId>,
    mut index: u32,
) -> Result<u32, Throw> {
    match data(cx, module).status {
        Status::EvaluatingAsync | Status::Evaluated => {
            return match evaluation_error(cx, module) {
                Some(error) => Err(Throw(error)),
                None => Ok(index),
            };
        }
        Status::Evaluating => return Ok(index),
        _ => {}
    }
    let record = data_mut(cx, module);
    record.status = Status::Evaluating;
    record.dfs_index = index;
    record.dfs_ancestor_index = index;
    record.pending_async_dependencies = 0;
    index += 1;
    stack.push(module);
    let requested = data(cx, module).requested.clone();
    for required in requested {
        index = inner_evaluate(cx, required, stack, index)?;
        let mut required = required;
        if data(cx, required).status == Status::Evaluating {
            let ancestor = data(cx, required).dfs_ancestor_index;
            let record = data_mut(cx, module);
            record.dfs_ancestor_index = record.dfs_ancestor_index.min(ancestor);
        } else {
            required = data(cx, required).cycle_root.unwrap_or(required);
            if let Some(error) = evaluation_error(cx, required) {
                return Err(Throw(error));
            }
        }
        if data(cx, required).async_evaluation.is_some() {
            data_mut(cx, module).pending_async_dependencies += 1;
            data_mut(cx, required).async_parents.push(module);
        }
    }
    let record = data(cx, module);
    if record.pending_async_dependencies > 0 || record.tables.has_top_level_await {
        let order = ASYNC_ORDER.with(|order| {
            order.set(order.get() + 1);
            order.get()
        });
        data_mut(cx, module).async_evaluation = Some(order);
        if data(cx, module).pending_async_dependencies == 0 {
            execute_async(cx, module)?;
        }
    } else {
        execute(cx, module)?;
    }
    let record = data(cx, module);
    if record.dfs_ancestor_index == record.dfs_index {
        while let Some(member) = stack.pop() {
            let record = data_mut(cx, member);
            record.status = if record.async_evaluation.is_none() {
                Status::Evaluated
            } else {
                Status::EvaluatingAsync
            };
            record.cycle_root = Some(module);
            if member == module {
                break;
            }
        }
    }
    Ok(index)
}

/// Runs a module's body: synchronous, or for one with top-level `await`
/// the promise its asynchronous root returns.
fn execute(cx: &mut Cx<'_>, module: ObjectId) -> Result<Value, Throw> {
    let function = slot(cx, module, FUNCTION);
    if function.is_undefined() {
        return Ok(Value::UNDEFINED);
    }
    let result = super::call(cx, function, Value::UNDEFINED, &[])?;
    // A module's root is an asynchronous function; without top-level
    // `await` it has settled when the call returns.
    if !data(cx, module).tables.has_top_level_await
        && let Some(promise) = result.as_object()
        && super::jobs::is_promise(cx, result)
        && super::jobs::state(cx, promise) == super::jobs::REJECTED
    {
        cx.heap
            .set_internal_slot(promise, super::jobs::HANDLED, Value::TRUE);
        return Err(Throw(super::jobs::result(cx, promise)));
    }
    Ok(result)
}

/// `ExecuteAsyncModule(module)`.
fn execute_async(cx: &mut Cx<'_>, module: ObjectId) -> Result<(), Throw> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let data_object = native_data(cx, &[Value::object(module)])?;
        cx.heap.roots().push(Value::object(data_object));
        let fulfilled = native_closure(cx, Native::ModuleAsyncFulfilled, data_object)?;
        cx.heap.roots().push(Value::object(fulfilled));
        let rejected = native_closure(cx, Native::ModuleAsyncRejected, data_object)?;
        cx.heap.roots().push(Value::object(rejected));
        let promise = execute(cx, module)?;
        cx.heap.roots().push(promise);
        let promise = match promise.as_object() {
            Some(promise) if super::jobs::is_promise(cx, Value::object(promise)) => promise,
            _ => {
                // A module the host compiled synchronously: settled now.
                let prototype = cx.realm.intrinsic(Intrinsic::PromisePrototype);
                let settled = super::jobs::new_promise(cx, prototype)?;
                super::jobs::fulfill_promise(cx, settled, Value::UNDEFINED);
                settled
            }
        };
        super::jobs::perform_then(
            cx,
            promise,
            Value::object(fulfilled),
            Value::object(rejected),
            None,
            Value::UNDEFINED,
        )
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `AsyncModuleExecutionFulfilled(module)`.
fn async_fulfilled(cx: &mut Cx<'_>, module: ObjectId) -> Result<(), Throw> {
    if data(cx, module).status == Status::Evaluated {
        return Ok(());
    }
    let record = data_mut(cx, module);
    record.async_evaluation = None;
    record.status = Status::Evaluated;
    if let Some(promise) = slot(cx, module, CAPABILITY).as_object() {
        super::jobs::fulfill_promise(cx, promise, Value::UNDEFINED);
    }
    let mut list = Vec::new();
    gather_available_ancestors(cx, module, &mut list);
    list.sort_by_key(|&member| data(cx, member).async_evaluation.unwrap_or(u64::MAX));
    for member in list {
        if data(cx, member).status == Status::Evaluated {
            continue;
        }
        if data(cx, member).tables.has_top_level_await {
            execute_async(cx, member)?;
        } else {
            match execute(cx, member) {
                Err(Throw(thrown)) => async_rejected(cx, member, thrown),
                Ok(_) => {
                    let record = data_mut(cx, member);
                    record.async_evaluation = None;
                    record.status = Status::Evaluated;
                    if let Some(promise) = slot(cx, member, CAPABILITY).as_object() {
                        super::jobs::fulfill_promise(cx, promise, Value::UNDEFINED);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `GatherAvailableAncestors(module, execList)`.
fn gather_available_ancestors(cx: &mut Cx<'_>, module: ObjectId, list: &mut Vec<ObjectId>) {
    let parents = data(cx, module).async_parents.clone();
    for parent in parents {
        let root = data(cx, parent).cycle_root.unwrap_or(parent);
        if list.contains(&parent) || evaluation_error(cx, root).is_some() {
            continue;
        }
        let record = data_mut(cx, parent);
        record.pending_async_dependencies = record.pending_async_dependencies.saturating_sub(1);
        if record.pending_async_dependencies == 0 {
            list.push(parent);
            if !data(cx, parent).tables.has_top_level_await {
                gather_available_ancestors(cx, parent, list);
            }
        }
    }
}

/// `AsyncModuleExecutionRejected(module, error)`.
fn async_rejected(cx: &mut Cx<'_>, module: ObjectId, error: Value) {
    if data(cx, module).status == Status::Evaluated {
        return;
    }
    set_slot(cx, module, ERROR, error);
    let record = data_mut(cx, module);
    record.status = Status::Evaluated;
    record.async_evaluation = None;
    if let Some(promise) = slot(cx, module, CAPABILITY).as_object() {
        super::jobs::reject_promise(cx, promise, error);
    }
    let parents = data(cx, module).async_parents.clone();
    for parent in parents {
        async_rejected(cx, parent, error);
    }
}

pub(crate) fn module_async_fulfilled(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let module = site.data(cx, 0).as_object().unwrap();
    async_fulfilled(cx, module)?;
    Ok(Value::UNDEFINED)
}

pub(crate) fn module_async_rejected(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let module = site.data(cx, 0).as_object().unwrap();
    let error = site.arg(cx, 0);
    async_rejected(cx, module, error);
    Ok(Value::UNDEFINED)
}

// ---- entry points ----

/// Loads, links and evaluates the module named `name`: the promise of its
/// evaluation.
pub(crate) fn run(cx: &mut Cx<'_>, name: &str) -> Result<ObjectId, Throw> {
    let module = load(cx, name, &[])?;
    link(cx, module)?;
    evaluate(cx, module)
}

/// The name of the module or script running `function`'s code, for
/// `import()`.
///
/// As QuickJS's `JS_GetScriptOrModuleName`, the root of `eval` code takes the
/// name of the first frame below it that is not `eval` code.
fn referrer(cx: &Cx<'_>, function: ObjectId) -> String {
    let unit_of = |function: ObjectId| {
        cx.heap
            .function_bytecode(function)
            .map(|bytecode| cx.unit(cx.heap.bytecode_unit(bytecode)))
    };
    let name = |unit: &crate::bytecode::CodeUnit| {
        unit.debug
            .filename
            .as_deref()
            .unwrap_or_default()
            .to_owned()
    };
    let Some(unit) = unit_of(function) else {
        return String::new();
    };
    if !unit.has(crate::bytecode::flags::EVAL) {
        return name(&unit);
    }
    let frames = &cx.heap.vm.frames;
    let below = frames
        .iter()
        .rposition(|frame| frame.function == function)
        .map_or(&frames[..0], |at| &frames[..at]);
    below
        .iter()
        .rev()
        .map(|frame| cx.unit(frame.unit))
        .find(|unit| !unit.has(crate::bytecode::flags::EVAL))
        .map(|unit| name(&unit))
        .unwrap_or_default()
}

/// `import(specifier, options)` from code of `function`: a promise of the
/// module's namespace. The specifier and options convert now; the loading,
/// linking and evaluation run in a job, as QuickJS's
/// `js_dynamic_import_job` does, so they never preempt an evaluation in
/// progress.
pub(crate) fn import(
    cx: &mut Cx<'_>,
    function: ObjectId,
    specifier: Value,
    options: Value,
) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::PromisePrototype);
    let promise = super::jobs::new_promise(cx, prototype)?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(promise));
    let outcome = (|| -> Result<(), Throw> {
        let specifier = super::ops::to_string(cx, specifier)?;
        cx.heap.roots().push(specifier);
        let attributes = import_attributes(cx, options)?;
        let kind = match attributes.iter().find(|(key, _)| key == "type") {
            Some((_, value)) => crate::builtins::string_text(cx, value)?,
            None => Value::UNDEFINED,
        };
        cx.heap.roots().push(kind);
        let referrer = referrer(cx, function);
        let referrer = crate::builtins::string_text(cx, &referrer)?;
        cx.heap.roots().push(referrer);
        let data_object = native_data(cx, &[Value::object(promise), specifier, referrer, kind])?;
        cx.heap.roots().push(Value::object(data_object));
        let job = native_closure(cx, Native::ModuleImportJob, data_object)?;
        cx.heap.jobs.push_back(super::jobs::Job::Call {
            function: Value::object(job),
            this: Value::UNDEFINED,
            args: Vec::new(),
        });
        Ok(())
    })();
    if let Err(Throw(thrown)) = outcome {
        super::jobs::reject_promise(cx, promise, thrown);
    }
    cx.heap.roots().truncate(mark);
    Ok(Value::object(promise))
}

/// The job of an `import()`: load, link and evaluate, then settle its
/// promise with the namespace once the evaluation has.
pub(crate) fn module_import_job(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let promise = site.data(cx, 0).as_object().unwrap();
    let specifier = site.data(cx, 1);
    let referrer = site.data(cx, 2);
    let kind = site.data(cx, 3);
    let outcome = (|| -> Result<(), Throw> {
        let specifier = cx.heap.string_text(specifier.as_string().unwrap());
        let referrer = cx.heap.string_text(referrer.as_string().unwrap());
        let attributes = match kind.as_string() {
            Some(kind) => vec![("type".to_owned(), cx.heap.string_text(kind))],
            None => Vec::new(),
        };
        let Some(host) = cx.host.module_host else {
            return Err(error::throw(
                cx,
                ErrorKind::Internal,
                "unsupported: modules",
            ));
        };
        let name = (host.normalize)(
            &mut Compiler::new(cx.heap, cx.static_code, cx.realm),
            &referrer,
            &specifier,
            &attributes,
        );
        let name = name.map_err(|failure| compile_error(cx, failure))?;
        let module = load(cx, &name, &attributes)?;
        link(cx, module)?;
        let evaluation = evaluate(cx, module)?;
        cx.heap.roots().push(Value::object(evaluation));
        let data_object = native_data(cx, &[Value::object(module), Value::object(promise)])?;
        cx.heap.roots().push(Value::object(data_object));
        let fulfilled = native_closure(cx, Native::ModuleImportFulfilled, data_object)?;
        cx.heap.roots().push(Value::object(fulfilled));
        let rejected = native_closure(cx, Native::ModuleImportRejected, data_object)?;
        cx.heap.roots().push(Value::object(rejected));
        super::jobs::perform_then(
            cx,
            evaluation,
            Value::object(fulfilled),
            Value::object(rejected),
            None,
            Value::UNDEFINED,
        )
    })();
    if let Err(Throw(thrown)) = outcome {
        super::jobs::reject_promise(cx, promise, thrown);
    }
    Ok(Value::UNDEFINED)
}

/// The `with` attributes of `import()`'s options.
fn import_attributes(cx: &mut Cx<'_>, options: Value) -> Result<Vec<(String, String)>, Throw> {
    if options.is_undefined() {
        return Ok(Vec::new());
    }
    let Some(options) = options.as_object() else {
        return Err(error::type_error(cx, "options must be an object"));
    };
    let key = cx
        .heap
        .property_key("with")
        .map_err(|e| error::heap_error(cx, e))?;
    let with = super::object::get_property(cx, options, key)?;
    if with.is_undefined() {
        return Ok(Vec::new());
    }
    let Some(with) = with.as_object() else {
        return Err(error::type_error(cx, "options.with must be an object"));
    };
    let mut attributes = Vec::new();
    for key in super::object::enumerable_own_keys(cx, with)? {
        let value = super::object::get_property(cx, with, key)?;
        let Some(value) = value.as_string() else {
            return Err(error::type_error(
                cx,
                "module attribute values must be strings",
            ));
        };
        let value = cx.heap.string_text(value);
        attributes.push((super::property::key_text(cx.heap, key), value));
    }
    Ok(attributes)
}

pub(crate) fn module_import_fulfilled(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let module = site.data(cx, 0).as_object().unwrap();
    let promise = site.data(cx, 1).as_object().unwrap();
    match namespace(cx, module) {
        Ok(namespace) => super::jobs::resolve_promise(cx, promise, Value::object(namespace))?,
        Err(Throw(thrown)) => super::jobs::reject_promise(cx, promise, thrown),
    }
    Ok(Value::UNDEFINED)
}

pub(crate) fn module_import_rejected(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let promise = site.data(cx, 1).as_object().unwrap();
    let error = site.arg(cx, 0);
    super::jobs::reject_promise(cx, promise, error);
    Ok(Value::UNDEFINED)
}

impl crate::Runtime {
    /// Installs the embedder's module loader.
    pub fn set_module_host(&mut self, host: ModuleHost) {
        self.host.module_host = Some(host);
    }

    /// Drops the module named `name` from the name map: a later import of
    /// the name loads it again. Modules that imported it keep the record.
    pub fn forget_module(&mut self, name: &str) {
        self.native_cx().forget_module(name);
    }

    /// Loads, links and evaluates the module named `name` (already
    /// normalized); returns the promise of its evaluation, unrooted.
    pub fn run_module(&mut self, name: &str) -> Result<Value, Throw> {
        self.native_cx().run_module(name)
    }

    /// [`Runtime::run_module`](crate::Runtime::run_module) with the step
    /// that threw: loading (compiling the graph), linking or evaluating.
    pub fn run_module_phased(&mut self, name: &str) -> Result<Value, (ModulePhase, Throw)> {
        let mut cx = self.cx();
        let module = load(&mut cx, name, &[]).map_err(|throw| (ModulePhase::Load, throw))?;
        link(&mut cx, module).map_err(|throw| (ModulePhase::Link, throw))?;
        evaluate(&mut cx, module)
            .map(Value::object)
            .map_err(|throw| (ModulePhase::Evaluate, throw))
    }

    /// The state of a promise (0 pending, 1 fulfilled, 2 rejected) and its
    /// result.
    pub fn promise_state(&mut self, promise: Value) -> Option<(i32, Value)> {
        self.native_cx().promise_state(promise)
    }
}

impl crate::NativeCx<'_> {
    /// [`Runtime::forget_module`](crate::Runtime::forget_module).
    pub fn forget_module(&mut self, name: &str) {
        self.host.modules.remove(name);
    }

    /// [`Runtime::run_module`](crate::Runtime::run_module).
    pub fn run_module(&mut self, name: &str) -> Result<Value, Throw> {
        run(&mut self.cx(), name).map(Value::object)
    }

    /// [`Runtime::promise_state`](crate::Runtime::promise_state).
    pub fn promise_state(&mut self, promise: Value) -> Option<(i32, Value)> {
        let cx = self.cx();
        let object = promise.as_object()?;
        if !super::jobs::is_promise(&cx, promise) {
            return None;
        }
        Some((
            super::jobs::state(&cx, object),
            super::jobs::result(&cx, object),
        ))
    }
}
