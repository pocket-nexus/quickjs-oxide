//! Code compiled at run time: `eval`, the `Function` constructors,
//! `$262.evalScript` and `Function.prototype.toString`'s source text, through
//! the compile host the embedder installs.
//!
//! The core has no front end. The embedder installs a [`CompileFn`] that
//! compiles and lowers source into a [`Compiler`], the part of the runtime a
//! lowering needs (the heap, the code store, the realm), which the natives
//! can lend while the loop holds the code store: units compiled at run time
//! are published into the store's dynamic chunks.

use super::native::CallSite;
use super::{Cx, Throw, error, ops};
use crate::ErrorKind;
use crate::bytecode::{CodeStore, CodeUnit, CodeUnitId, ValidateError};
use crate::heap::hash::FxHashMap;
use crate::heap::{BytecodeTemplate, Heap, HeapError};
use crate::runtime::{Intrinsic, Realm};
use crate::value::{ObjectId, Value};

/// The code store a compilation publishes into: owned before the program
/// runs (units go in the static store), shared while it runs (the dynamic
/// chunks).
enum Store<'a> {
    Owned(&'a mut CodeStore),
    Shared(&'a CodeStore),
}

/// The runtime's parts a compilation uses.
pub struct Compiler<'a> {
    heap: &'a mut Heap,
    code: Store<'a>,
    realm: &'a Realm,
}

impl<'a> Compiler<'a> {
    pub(crate) fn new(heap: &'a mut Heap, code: &'a CodeStore, realm: &'a Realm) -> Self {
        Compiler {
            heap,
            code: Store::Shared(code),
            realm,
        }
    }

    pub(crate) fn owned(heap: &'a mut Heap, code: &'a mut CodeStore, realm: &'a Realm) -> Self {
        Compiler {
            heap,
            code: Store::Owned(code),
            realm,
        }
    }

    /// The same parts for a shorter borrow (a host callback's view).
    pub fn reborrow(&mut self) -> Compiler<'_> {
        Compiler {
            heap: self.heap,
            code: match &mut self.code {
                Store::Owned(code) => Store::Owned(code),
                Store::Shared(code) => Store::Shared(code),
            },
            realm: self.realm,
        }
    }

    /// Holds `value` for the embedder until released (the heap's handle
    /// table).
    pub fn hold(&mut self, value: Value) -> crate::Handle {
        self.heap.handles().create(value)
    }

    /// A new error object of `kind` with `message` and an empty `stack`: an
    /// error a host callback raises, with no JavaScript frame, as QuickJS's.
    pub fn new_error(&mut self, kind: ErrorKind, message: &str) -> Result<Value, HeapError> {
        let prototype = self.realm.intrinsic(kind.prototype());
        let message = self.heap.alloc_string(message)?;
        let mark = self.heap.roots().mark();
        self.heap.roots().push(Value::string(message));
        let result = (|| {
            let object = self
                .heap
                .alloc_object(crate::ObjectKind::Ordinary, Some(prototype), 1)?;
            self.heap
                .set_object_flag(object, crate::heap::object::flags::ERROR);
            let key = crate::runtime::Names::key(self.realm.names.message);
            self.heap.add_property(
                object,
                key,
                crate::PropertyFlags::data(true, false, true),
                Value::string(message),
            )?;
            let empty = self.heap.intern("")?;
            let stack = crate::runtime::Names::key(self.realm.names.stack);
            self.heap.add_property(
                object,
                stack,
                crate::PropertyFlags::data(true, false, true),
                Value::string(empty),
            )?;
            Ok(Value::object(object))
        })();
        self.heap.roots().truncate(mark);
        result
    }

    /// [`Compiler::new_error`] located at `file:line:column` (a module's
    /// compile error): `fileName`, `lineNumber`, `columnNumber`, and a
    /// `stack` of the location alone.
    pub fn new_located_error(
        &mut self,
        kind: ErrorKind,
        message: &str,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<Value, HeapError> {
        let error = self.new_error(kind, message)?;
        let object = error.as_object().expect("an error object");
        let names = self.realm.names;
        let mark = self.heap.roots().mark();
        self.heap.roots().push(error);
        let result = (|| {
            let flags = crate::PropertyFlags::data(true, false, true);
            let stack_key = crate::runtime::Names::key(names.stack);
            // `fileName`, `lineNumber` and `columnNumber` precede `stack`,
            // as QuickJS's `build_backtrace` defines them: the stack is
            // re-added after them.
            self.heap.delete_property(object, stack_key)?;
            let file_text = self.heap.alloc_string(file)?;
            self.heap.add_property(
                object,
                crate::runtime::Names::key(names.file_name),
                flags,
                Value::string(file_text),
            )?;
            self.heap.add_property(
                object,
                crate::runtime::Names::key(names.line_number),
                flags,
                Value::int32(line as i32),
            )?;
            self.heap.add_property(
                object,
                crate::runtime::Names::key(names.column_number),
                flags,
                Value::int32(column as i32),
            )?;
            let stack = self
                .heap
                .alloc_string(&format!("    at {file}:{line}:{column}\n"))?;
            self.heap
                .add_property(object, stack_key, flags, Value::string(stack))?;
            Ok(error)
        })();
        self.heap.roots().truncate(mark);
        result
    }

    pub fn heap(&self) -> &Heap {
        self.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        self.heap
    }

    pub fn code(&self) -> &CodeStore {
        match &self.code {
            Store::Owned(code) => code,
            Store::Shared(code) => code,
        }
    }

    pub fn intrinsic(&self, intrinsic: Intrinsic) -> ObjectId {
        self.realm.intrinsic(intrinsic)
    }

    /// Validates `unit` and appends it to the code store. Its children must
    /// be published first.
    pub fn publish(&mut self, unit: CodeUnit) -> Result<CodeUnitId, ValidateError> {
        let (id, code) = match &mut self.code {
            Store::Owned(code) => (code.publish(unit)?, &**code),
            Store::Shared(code) => (code.publish_shared(unit)?, *code),
        };
        self.heap.ics.reserve(code.get(id));
        Ok(id)
    }

    /// A function object for the published unit `root`, with a bytecode
    /// record for it and every unit below it. `constants` holds one table per
    /// unit, in the pre-order of [`CodeStore::tree`].
    pub fn instantiate(
        &mut self,
        root: CodeUnitId,
        constants: &[&[Value]],
    ) -> Result<ObjectId, HeapError> {
        let code = match &self.code {
            Store::Owned(code) => &**code,
            Store::Shared(code) => *code,
        };
        let order = code.tree(root);
        assert_eq!(order.len(), constants.len(), "one constant table per unit");
        let position: FxHashMap<CodeUnitId, u32> = order
            .iter()
            .enumerate()
            .map(|(index, &unit)| (unit, index as u32))
            .collect();
        let children: Vec<Vec<u32>> = order
            .iter()
            .map(|&id| {
                let unit = code.get(id);
                unit.children.iter().map(|child| position[child]).collect()
            })
            .collect();
        let tree: Vec<BytecodeTemplate<'_>> = order
            .iter()
            .zip(constants)
            .zip(&children)
            .map(|((&unit, &constants), children)| {
                assert_eq!(
                    constants.len(),
                    code.get(unit).constant_count as usize,
                    "constant table length"
                );
                BytecodeTemplate {
                    unit,
                    constants,
                    children,
                }
            })
            .collect();
        let prototype = self.realm.intrinsic(Intrinsic::FunctionPrototype);
        self.heap
            .alloc_bytecode_function(Some(prototype), None, &tree)
    }
}

impl Compiler<'_> {
    /// A tagged template's frozen site object.
    pub fn template_object(
        &mut self,
        cooked: &[Value],
        raw: &[Value],
    ) -> Result<ObjectId, HeapError> {
        use crate::heap::object::flags;
        let prototype = self.intrinsic(Intrinsic::ArrayPrototype);
        let names = self.realm.names;
        let heap = &mut *self.heap;
        let mark = heap.roots().mark();
        let result = (|| {
            let raw_array = heap.alloc_array(Some(prototype), raw.len() as u32)?;
            heap.roots().push(Value::object(raw_array));
            let array = heap.alloc_array(Some(prototype), cooked.len() as u32)?;
            heap.roots().push(Value::object(array));
            for (target, values) in [(raw_array, raw), (array, cooked)] {
                for (index, &value) in values.iter().enumerate() {
                    let key =
                        crate::PropertyKey::index(index as u32).ok_or(HeapError::OutOfMemory)?;
                    heap.add_property(
                        target,
                        key,
                        crate::PropertyFlags::data(false, true, false),
                        value,
                    )?;
                }
                heap.set_array_length(target, values.len() as u32);
            }
            let raw_key = crate::runtime::Names::key(names.raw);
            heap.add_property(
                array,
                raw_key,
                crate::PropertyFlags::data(false, false, false),
                Value::object(raw_array),
            )?;
            for target in [raw_array, array] {
                let header = heap.header(target);
                let _ = header;
                heap.set_object_flag(target, flags::SPARSE | flags::LENGTH_READONLY);
                heap.clear_object_flag(target, flags::EXTENSIBLE);
            }
            Ok(array)
        })();
        heap.roots().truncate(mark);
        result
    }
}

/// What to compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileKind {
    /// A script (`$262.evalScript`): its declarations are global.
    Script,
    /// Indirect eval code (`eval`, and the `Function` constructors' source):
    /// the root returns the completion value.
    IndirectEval,
}

/// A compilation request.
#[derive(Clone, Copy, Debug)]
pub struct CompileRequest<'s> {
    pub source: &'s [u16],
    pub kind: CompileKind,
    pub filename: &'s str,
}

/// A failed compilation: the error to throw.
#[derive(Clone, Debug)]
pub struct CompileError {
    pub kind: ErrorKind,
    pub message: String,
    /// A value the embedder threw instead (a module loader's own error
    /// object): thrown as is, `kind` and `message` unused. It must stay
    /// rooted until the core throws it (a handle the embedder releases
    /// after the entry).
    pub thrown: Option<Value>,
    /// Where in which source the error is (a syntax error's file, line and
    /// column, one-based): the error's `stack` starts with it, as
    /// QuickJS's does.
    pub location: Option<(Box<str>, u32, u32)>,
}

impl CompileError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        CompileError {
            kind,
            message: message.into(),
            thrown: None,
            location: None,
        }
    }

    /// The error located at `line` and `column` of `file`.
    pub fn at(mut self, file: &str, line: u32, column: u32) -> Self {
        self.location = Some((file.into(), line, column));
        self
    }

    pub fn thrown(value: Value) -> Self {
        CompileError {
            kind: ErrorKind::Internal,
            message: String::new(),
            thrown: Some(value),
            location: None,
        }
    }
}

/// Throws a compile failure.
pub(crate) fn throw_failure(cx: &mut Cx<'_>, failure: CompileError) -> Throw {
    match (failure.thrown, failure.location) {
        (Some(value), _) => Throw(value),
        (None, Some((file, line, column))) => {
            error::throw_located(cx, failure.kind, &failure.message, &file, line, column)
        }
        (None, None) => error::throw(cx, failure.kind, &failure.message),
    }
}

/// The embedder's compiler: the root function of the compiled code, to be
/// called with the global object as `this`.
pub type CompileFn = fn(&mut Compiler<'_>, &CompileRequest<'_>) -> Result<ObjectId, CompileError>;

/// Compiles `source` through the host, throwing its error.
pub(crate) fn compile(
    cx: &mut Cx<'_>,
    source: &[u16],
    kind: CompileKind,
) -> Result<ObjectId, Throw> {
    compile_named(cx, source, kind, "<input>")
}

/// [`compile`] with the filename the code's positions name.
pub(crate) fn compile_named(
    cx: &mut Cx<'_>,
    source: &[u16],
    kind: CompileKind,
    filename: &str,
) -> Result<ObjectId, Throw> {
    let Some(compile) = cx.host.compile else {
        return Err(super::object::unsupported(cx, "eval"));
    };
    let request = CompileRequest {
        source,
        kind,
        filename,
    };
    let result = compile(&mut Compiler::new(cx.heap, cx.code, cx.realm), &request);
    match result {
        Ok(function) => Ok(function),
        Err(failure) => Err(throw_failure(cx, failure)),
    }
}

/// Compiles and runs `source` on the global object.
fn run(cx: &mut Cx<'_>, source: &[u16], kind: CompileKind) -> Result<Value, Throw> {
    let function = compile(cx, source, kind)?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(function));
    let global = Value::object(cx.realm.global());
    let result = super::call(cx, Value::object(function), global, &[]);
    cx.heap.roots().truncate(mark);
    result
}

/// `%eval%(x)`: a direct eval runs as an indirect one.
pub(crate) fn eval(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let source = site.arg(cx, 0);
    let Some(string) = source.as_string() else {
        return Ok(source);
    };
    let units = ops::units(cx.heap, string);
    run(cx, &units, CompileKind::IndirectEval)
}

/// Which `Function` constructor runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FunctionKind {
    Normal,
    Generator,
    Async,
    AsyncGenerator,
}

/// `CreateDynamicFunction`, as QuickJS does it: the parameters and body in
/// `(function anonymous(…\n) {\n…\n})`, evaluated as indirect eval code.
pub(crate) fn function_constructor(
    cx: &mut Cx<'_>,
    site: CallSite,
    kind: FunctionKind,
) -> Result<Value, Throw> {
    let mut source: Vec<u16> = Vec::new();
    let text = |source: &mut Vec<u16>, text: &str| source.extend(text.encode_utf16());
    text(&mut source, "(");
    if matches!(kind, FunctionKind::Async | FunctionKind::AsyncGenerator) {
        text(&mut source, "async ");
    }
    text(&mut source, "function");
    if matches!(kind, FunctionKind::Generator | FunctionKind::AsyncGenerator) {
        text(&mut source, "*");
    }
    text(&mut source, " anonymous(");
    let parameters = site.argc.saturating_sub(1);
    for index in 0..parameters {
        if index > 0 {
            text(&mut source, ",");
        }
        let parameter = site.arg(cx, index);
        let parameter = ops::to_string(cx, parameter)?;
        source.extend(ops::units(cx.heap, parameter.as_string().unwrap()));
    }
    text(&mut source, "\n) {\n");
    if site.argc > 0 {
        let body = site.arg(cx, site.argc - 1);
        let body = ops::to_string(cx, body)?;
        source.extend(ops::units(cx.heap, body.as_string().unwrap()));
    }
    text(&mut source, "\n})");
    let function = run(cx, &source, CompileKind::IndirectEval)?;
    let Some(object) = function.as_object() else {
        return Err(error::type_error(cx, "not an object"));
    };
    let default = match kind {
        FunctionKind::Normal => Intrinsic::FunctionPrototype,
        FunctionKind::Generator => Intrinsic::GeneratorFunctionPrototype,
        FunctionKind::Async => Intrinsic::AsyncFunctionPrototype,
        FunctionKind::AsyncGenerator => Intrinsic::AsyncGeneratorFunctionPrototype,
    };
    if site.construct && !site.new_target.is_undefined() {
        let mark = cx.heap.roots().mark();
        cx.heap.roots().push(function);
        let prototype = super::object::prototype_from_constructor(cx, site.new_target, default);
        cx.heap.roots().truncate(mark);
        let prototype = prototype?;
        if !super::object::set_prototype_of(cx, object, Value::object(prototype))? {
            return Err(error::type_error(cx, "prototype is immutable"));
        }
    }
    Ok(function)
}

/// The unit flags of a bytecode function (its kind), or `None` for any
/// other function.
pub(crate) fn function_kind_flags(cx: &Cx<'_>, function: ObjectId) -> Option<u16> {
    if cx.heap.object_kind(function) != crate::ObjectKind::Function {
        return None;
    }
    let bytecode = cx.heap.function_bytecode(function)?;
    let unit = cx.heap.bytecode_unit(bytecode);
    Some(cx.code.get(unit).flags)
}

/// The source text of a bytecode function, if it was recorded.
pub(crate) fn function_source(cx: &Cx<'_>, function: ObjectId) -> Option<Vec<u16>> {
    if cx.heap.object_kind(function) != crate::ObjectKind::Function {
        return None;
    }
    let bytecode = cx.heap.function_bytecode(function)?;
    let unit = cx.heap.bytecode_unit(bytecode);
    let source = cx.code.get(unit).source.as_ref()?;
    Some(wtf8_units(source))
}

/// The UTF-16 units of source `bytes` as QuickJS's `JS_NewStringLen` reads
/// them (`Function.prototype.toString` of a function's debug source): a
/// surrogate's three-byte form gives the lone unit back, and an invalid
/// sequence gives one U+FFFD, skipping its continuation bytes, the next
/// byte and that byte's continuation bytes.
fn wtf8_units(bytes: &[u8]) -> Vec<u16> {
    let continuation = |byte: u8| (0x80..0xc0).contains(&byte);
    let mut units = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let lead = bytes[index];
        if lead < 0x80 {
            units.push(u16::from(lead));
            index += 1;
            continue;
        }
        // QuickJS's `unicode_from_utf8`.
        let (length, mask, minimum) = match lead {
            0xc0..=0xdf => (1, 0x1f, 0x80),
            0xe0..=0xef => (2, 0x0f, 0x800),
            0xf0..=0xf7 => (3, 0x07, 0x1_0000),
            0xf8..=0xfb => (4, 0x03, 0x20_0000),
            0xfc..=0xfd => (5, 0x01, 0x400_0000),
            _ => (0, 0, 0),
        };
        let decoded = (length > 0 && index + length < bytes.len())
            .then(|| &bytes[index + 1..=index + length])
            .filter(|tail| tail.iter().all(|&byte| continuation(byte)))
            .map(|tail| {
                tail.iter().fold(u32::from(lead & mask), |code, &byte| {
                    code << 6 | u32::from(byte & 0x3f)
                })
            })
            .filter(|&code| code >= minimum && code <= 0x10_ffff);
        match decoded {
            Some(code) if code >= 0x1_0000 => {
                let code = code - 0x1_0000;
                units.push(0xd800 | (code >> 10) as u16);
                units.push(0xdc00 | (code & 0x3ff) as u16);
                index += length + 1;
            }
            Some(code) => {
                units.push(code as u16);
                index += length + 1;
            }
            None => {
                units.push(0xfffd);
                while index < bytes.len() && continuation(bytes[index]) {
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                    while index < bytes.len() && continuation(bytes[index]) {
                        index += 1;
                    }
                }
            }
        }
    }
    units
}

/// `$262.evalScript(source)`: the source as a separate script. qjs's
/// `load(path)` (`qjs --std`): the file, run as a script.
pub(crate) fn load(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    load_script(cx, site).map(|_| Value::UNDEFINED)
}

/// qjs's `__loadScript(path)`: the file, run as a script; its completion
/// value.
pub(crate) fn load_script(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let path = site.arg(cx, 0);
    let path = ops::to_string(cx, path)?;
    let path = cx.heap.string_text(path.as_string().unwrap());
    let Some(text) = cx.host.hooks.read_file(&path) else {
        let message = format!("could not load '{path}'");
        return Err(error::throw(cx, ErrorKind::Reference, &message));
    };
    let units: Vec<u16> = text.encode_utf16().collect();
    let function = compile_named(cx, &units, CompileKind::Script, &path)?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(function));
    let global = Value::object(cx.realm.global());
    let result = super::call(cx, Value::object(function), global, &[]);
    cx.heap.roots().truncate(mark);
    result
}

/// qjs's `read(path)` (`qjs --std`): the file's text.
pub(crate) fn read(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let path = site.arg(cx, 0);
    let path = ops::to_string(cx, path)?;
    let path = cx.heap.string_text(path.as_string().unwrap());
    let Some(text) = cx.host.hooks.read_file(&path) else {
        let message = format!("could not read '{path}'");
        return Err(error::throw(cx, ErrorKind::Error, &message));
    };
    crate::builtins::string_text(cx, &text)
}

pub(crate) fn eval_script(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let source = site.arg(cx, 0);
    let source = ops::to_string(cx, source)?;
    let units = ops::units(cx.heap, source.as_string().unwrap());
    run(cx, &units, CompileKind::Script)
}
