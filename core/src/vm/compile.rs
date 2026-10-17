//! Code compiled at run time (phase 4, section 5.5): `eval`, the `Function`
//! constructors, `$262.evalScript` and `Function.prototype.toString`'s
//! source text, through the compile host the embedder installs.
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
    /// A tagged template's frozen site object (section 5.2).
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
    let Some(compile) = cx.host.compile else {
        return Err(super::object::unsupported(cx, "eval"));
    };
    let request = CompileRequest {
        source,
        kind,
        filename: "<input>",
    };
    let result = compile(&mut Compiler::new(cx.heap, cx.code, cx.realm), &request);
    match result {
        Ok(function) => Ok(function),
        Err(failure) => Err(error::throw(cx, failure.kind, &failure.message)),
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

/// `%eval%(x)`: a direct eval runs as an indirect one (phase 0).
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

/// The source text of a bytecode function, if it was recorded.
pub(crate) fn function_source(cx: &Cx<'_>, function: ObjectId) -> Option<Vec<u16>> {
    if cx.heap.object_kind(function) != crate::ObjectKind::Function {
        return None;
    }
    let bytecode = cx.heap.function_bytecode(function)?;
    let unit = cx.heap.bytecode_unit(bytecode);
    let source = cx.code.get(unit).source.as_ref()?;
    Some(source.encode_utf16().collect())
}

/// `$262.evalScript(source)`: the source as a separate script.
pub(crate) fn eval_script(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let source = site.arg(cx, 0);
    let source = ops::to_string(cx, source)?;
    let units = ops::units(cx.heap, source.as_string().unwrap());
    run(cx, &units, CompileKind::Script)
}
