//! Lowering from the compiler's stack code to `quickjs-oxide-core`'s register code.
//!
//! The input is the [`UnlinkedFunction`] tree the front end compiles:
//! QuickJS-shaped stack code whose operand-stack depth is static at every
//! instruction. The stack depth is the register allocator. Register 0 is
//! `this`, then the parameters, then the locals, then one temporary `T(d)` per
//! stack depth `d`.
//!
//! The lowering walks the code with a symbolic stack. An entry is either
//! *canonical* (its value is in `T(d)` for its own depth `d`), a local or
//! parameter register that was pushed but not copied, or a constant that was
//! pushed but not loaded. Consumers read operands where they are. Two rules
//! keep that correct:
//!
//! 1. Before a jump, at a join point, and before a `Catch` or `Gosub`, every
//!    entry is made canonical, so every path into a join agrees on the layout.
//! 2. Before a write to a local register, every entry that still aliases that
//!    register is made canonical, so `x + (x = 2)` reads the old `x`.
//!
//! Bindings captured by an inner function live in cells (one-slot heap
//! objects): the binding's register holds the cell, a closure holds the cells
//! it captures, and `CloseLocal` replaces the frame's cell with a fresh copy,
//! as QuickJS's `close_loc` detaches a variable reference.

mod analysis;
#[cfg(test)]
mod ic_tests;
#[cfg(test)]
pub(crate) mod run_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod window_tests;

use std::collections::{HashMap, HashSet};
use std::fmt;

use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{
    Capture, CodeBuilder, CodeUnit, CodeUnitId, Handler, Opcode, ValidateError, flags,
};
use oxide_core::vm::compile::Compiler;
use oxide_core::{HeapError, ObjectId, PropertyKey, Runtime, StringId, Value as CoreValue};

use self::analysis::{Analysis, analyze_into};
use crate::engine::code::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, Instruction, IteratorCallKind, PrivateNameSource,
};
use crate::engine::code::function::UnlinkedFunction;
use crate::engine::code::function::metadata::{
    ClosureSource, ClosureVariableKind, ClosureVariableName, ConstructorKind, EvalKind,
    FunctionKind,
};
use crate::engine::value::{JsString, PrimitiveValue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LowerError {
    /// A construct the lowering does not support (direct eval code, or an
    /// instruction or constant it has no form for).
    Unsupported(String),
    /// A dropped feature that is an early `SyntaxError` (`with`).
    Syntax(String),
    /// The input broke an invariant the stack code guarantees.
    Internal(String),
    Heap(HeapError),
    /// The publication validator rejected a lowered unit.
    Invalid(ValidateError),
}

impl LowerError {
    fn unsupported(construct: impl Into<String>) -> LowerError {
        LowerError::Unsupported(construct.into())
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LowerError::Unsupported(construct) => write!(f, "unsupported: {construct}"),
            LowerError::Syntax(message) => write!(f, "SyntaxError: {message}"),
            LowerError::Internal(message) => write!(f, "internal lowering error: {message}"),
            LowerError::Heap(error) => write!(f, "{error}"),
            LowerError::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl From<HeapError> for LowerError {
    fn from(error: HeapError) -> Self {
        LowerError::Heap(error)
    }
}

impl From<ValidateError> for LowerError {
    fn from(error: ValidateError) -> Self {
        LowerError::Invalid(error)
    }
}

/// A script lowered and instantiated on the core.
pub(crate) struct LoweredScript {
    pub(crate) root: CodeUnitId,
    /// The script's function object. Unrooted: the caller holds it before
    /// its next allocation.
    pub(crate) function: ObjectId,
}

/// Lowers a compiled script and every function in it, publishes the units
/// children first, and instantiates the root.
pub(crate) fn lower_script(
    runtime: &mut Runtime,
    script: &UnlinkedFunction,
) -> Result<LoweredScript, LowerError> {
    lower_script_into(&mut runtime.compiler(), script)
}

/// [`lower_script`] into a runtime's [`Compiler`]: from the host before the
/// program runs, or from a native compiling `eval` code while it runs.
pub(crate) fn lower_script_into(
    runtime: &mut Compiler<'_>,
    script: &UnlinkedFunction,
) -> Result<LoweredScript, LowerError> {
    // BigInt constants are heap records but not atoms, which the compilation
    // keeps: they stay on the root stack until `instantiate` attaches the
    // constant tables.
    let mark = runtime.heap_mut().roots().mark();
    let result = (|| {
        let mut constants = HashMap::new();
        let mut tree = TreeCache::default();
        let root = lower_tree(runtime, &mut tree, script, None, None, &mut constants)?;
        let order = runtime.units().tree(root);
        let tables: Vec<&[CoreValue]> = order.iter().map(|id| constants[id].as_slice()).collect();
        let function = runtime.instantiate(root, &tables)?;
        Ok(LoweredScript { root, function })
    })();
    runtime.heap_mut().roots().truncate(mark);
    result
}

/// Lowers a compiled module's function tree into `runtime` and describes
/// its bindings, requests, imports and exports for the core's module
/// records; also returns the root unit.
pub(crate) fn lower_module_tree_into(
    runtime: &mut Compiler<'_>,
    module: &crate::engine::code::module::UnlinkedModule,
) -> Result<(oxide_core::vm::module::ModuleSource, CodeUnitId), LowerError> {
    use crate::engine::code::module::{
        ModuleExportTarget, ModuleImportName, ModuleLinkInitializerValue,
    };
    use oxide_core::vm::module as core;
    let lowered = lower_script_into(runtime, module.function())?;
    let function = module.function();
    let text = |string: &JsString| String::from_utf16_lossy(&string_units(string));
    let name = |name: &ModuleImportName| match name {
        ModuleImportName::Name(name) => Some(text(name)),
        ModuleImportName::Namespace => None,
    };
    let bindings = function
        .closure_variables()
        .iter()
        .map(|variable| match variable.source {
            ClosureSource::ModuleImport | ClosureSource::ModuleImportCollision => {
                core::ModuleBinding::Import
            }
            ClosureSource::ModuleImportMeta => core::ModuleBinding::ImportMeta,
            _ => core::ModuleBinding::Declaration {
                lexical: variable.is_lexical,
            },
        })
        .collect();
    let requests = module
        .requested_modules()
        .iter()
        .map(|request| core::ModuleRequest {
            specifier: text(&request.specifier),
            attributes: request
                .attributes
                .effective()
                .unwrap_or(&[])
                .iter()
                .map(|attribute| (text(&attribute.key), text(&attribute.value)))
                .collect(),
        })
        .collect();
    let imports = module
        .imports()
        .iter()
        .map(|import| core::ModuleImport {
            request: import.request.0,
            name: name(&import.import_name),
            binding: import.closure_index,
        })
        .collect();
    let exports = module
        .exports()
        .iter()
        .map(|export| core::ModuleExport {
            name: text(&export.export_name),
            target: match &export.target {
                ModuleExportTarget::Local { closure_index } => {
                    core::ExportTarget::Local(*closure_index)
                }
                ModuleExportTarget::Indirect {
                    request,
                    import_name,
                } => core::ExportTarget::Indirect {
                    request: request.0,
                    name: name(import_name),
                },
            },
        })
        .collect();
    let star_exports = module
        .star_exports()
        .iter()
        .map(|star| star.request.0)
        .collect();
    // A function constant's position among the root's children.
    let child_of = |constant: u32| {
        function.constants()[..constant as usize]
            .iter()
            .filter(|constant| constant.as_child().is_some())
            .count() as u32
    };
    let initializers = module
        .link_initializers()
        .iter()
        .map(|initializer| core::ModuleInitializer {
            binding: initializer.closure_index,
            function: match initializer.value {
                ModuleLinkInitializerValue::Undefined => None,
                ModuleLinkInitializerValue::Function {
                    constant,
                    inferred_name,
                } => Some((child_of(constant), inferred_name.is_some())),
            },
        })
        .collect();
    let source = core::ModuleSource::Compiled {
        function: lowered.function,
        tables: core::ModuleTables {
            bindings,
            requests,
            imports,
            exports,
            star_exports,
            initializers,
            has_top_level_await: module.has_top_level_await(),
        },
        import_meta: Vec::new(),
    };
    Ok((source, lowered.root))
}

/// Whether a unit of the tree under `root` has an `import()` call.
#[cfg(feature = "test262-host")]
pub(crate) fn tree_has_import_call(units: oxide_core::Units<'_>, root: CodeUnitId) -> bool {
    units.tree(root).into_iter().any(|unit| {
        units
            .get(unit)
            .code
            .iter()
            .any(|insn| insn.op == oxide_core::bytecode::Opcode::ImportCall)
    })
}

/// The compare-jump taken exactly when `op` is not, if there is one.
fn negated_jump(op: Opcode) -> Option<Opcode> {
    use Opcode::*;
    Some(match op {
        JTrue => JFalse,
        JFalse => JTrue,
        JLt => JNlt,
        JNlt => JLt,
        JLte => JNlte,
        JNlte => JLte,
        JGt => JNgt,
        JNgt => JGt,
        JGte => JNgte,
        JNgte => JGte,
        JEq => JNeq,
        JNeq => JEq,
        JStrictEq => JStrictNeq,
        JStrictNeq => JStrictEq,
        JLtI => JNltI,
        JNltI => JLtI,
        JLteI => JNlteI,
        JNlteI => JLteI,
        JGtI => JNgtI,
        JNgtI => JGtI,
        JGteI => JNgteI,
        JNgteI => JGteI,
        JEqI => JNeqI,
        JNeqI => JEqI,
        JStrictEqI => JStrictNeqI,
        JStrictNeqI => JStrictEqI,
        _ => return None,
    })
}

/// Lowers and publishes `function` after its children. `parent` is the
/// parent's layout (`None` for the script), which places the parent's locals
/// for this function's captures; `inferred` the name the parent's code gives
/// it where it creates it ([`inferred_names`]).
fn lower_tree(
    runtime: &mut Compiler<'_>,
    tree: &mut TreeCache,
    function: &UnlinkedFunction,
    parent: Option<Layout>,
    inferred: Option<&JsString>,
    constants: &mut HashMap<CodeUnitId, Vec<CoreValue>>,
) -> Result<CodeUnitId, LowerError> {
    let mut analysis = tree.analyses.pop().unwrap_or_default();
    analyze_into(function.code(), &mut analysis, &mut tree.analysis_scratch)?;
    let layout = Layout::of(function, &analysis)?;
    let names = inferred_names(function, &analysis);
    let mut children = Vec::new();
    for (index, constant) in function.constants().iter().enumerate() {
        if let Some(child) = constant.as_child() {
            let name = names.get(index).copied().flatten();
            children.push(lower_tree(
                runtime,
                tree,
                child,
                Some(layout),
                name,
                constants,
            )?);
        }
    }
    let named = names.iter().map(Option::is_some).collect();
    let (unit, pool) = lower_function(
        runtime, tree, function, analysis, layout, parent, children, inferred, named,
    )?;
    let id = runtime.publish(unit)?;
    constants.insert(id, pool);
    Ok(id)
}

/// The names `function`'s code gives its anonymous children where it creates
/// them, indexed by constant (empty when it creates none): a child without a name of its own whose every
/// `FClosure` is followed, at no join, by a `SetName` of a string constant or
/// a plain method's `DefineMethod`. Its unit carries the name, so a new
/// closure is named without `SetFunctionName` and its properties stay
/// pending.
fn inferred_names<'f>(
    function: &'f UnlinkedFunction,
    analysis: &Analysis,
) -> Vec<Option<&'f JsString>> {
    let code = function.code();
    let constants = function.constants();
    if !constants
        .iter()
        .any(|constant| constant.as_child().is_some())
    {
        return Vec::new();
    }
    let string = |index: u32| match constants.get(index as usize)?.as_primitive()? {
        PrimitiveValue::String(string) => Some(string),
        _ => None,
    };
    // Per constant: unseen, a name every `FClosure` so far agrees on, or
    // `Some(None)` once one does not.
    let mut names: Vec<Option<Option<&'f JsString>>> = vec![None; constants.len()];
    for (pc, instruction) in code.iter().enumerate() {
        let Instruction::FClosure(child) = *instruction else {
            continue;
        };
        let anonymous = constants
            .get(child as usize)
            .and_then(|constant| constant.as_child())
            .is_some_and(|child| child.func_name().is_none_or(|name| name.is_empty()));
        let name = match code.get(pc + 1) {
            _ if !anonymous || analysis.label.get(pc + 1).copied().unwrap_or(true) => None,
            Some(&Instruction::SetName(name)) => string(name),
            Some(&Instruction::DefineMethod {
                key,
                kind: DefineMethodKind::Method,
                ..
            }) => string(key),
            _ => None,
        };
        let Some(entry) = names.get_mut(child as usize) else {
            continue;
        };
        match entry {
            None => *entry = Some(name),
            Some(seen) if *seen != name => *seen = None,
            Some(_) => {}
        }
    }
    names.into_iter().map(Option::flatten).collect()
}

/// Lowers one function whose children are already published, given its
/// analysis and the layout that analysis gives it. `inferred` is the name its
/// parent gives it, `named` the children it gives one ([`inferred_names`]).
#[allow(clippy::too_many_arguments)]
fn lower_function(
    runtime: &mut Compiler<'_>,
    tree: &mut TreeCache,
    function: &UnlinkedFunction,
    analysis: Analysis,
    layout: Layout,
    parent: Option<Layout>,
    children: Vec<CodeUnitId>,
    inferred: Option<&JsString>,
    named: Vec<bool>,
) -> Result<(CodeUnit, Vec<CoreValue>), LowerError> {
    check_surface(function)?;
    let is_script = parent.is_none() && !function.metadata().is_module;
    let mut lowerer = Lowerer::new(runtime, tree, function, analysis, layout, is_script)?;
    lowerer.inferred_name = inferred.cloned();
    lowerer.named_children = named;
    lowerer.prologue()?;
    lowerer.body()?;
    lowerer.finish(parent, children)
}

/// What the lowering of one tree keeps from one function to the next.
#[derive(Default)]
struct TreeCache {
    /// The key and the atom of each string of the tree, by its address; the
    /// string is kept, so the address is not reused while the entry is.
    keys: HashMap<usize, (JsString, PropertyKey)>,
    atoms: HashMap<usize, (JsString, StringId)>,
    /// The tree's file name, converted once.
    filename: Option<(JsString, std::rc::Rc<str>)>,
    /// Buffers kept for the next function: analyses not in use (a parent's
    /// stays in use while its children lower) and the lowering's own; one
    /// function lowers at a time.
    analyses: Vec<Analysis>,
    analysis_scratch: analysis::Scratch,
    lowering: LowerScratch,
}

/// A [`Lowerer`]'s buffers between functions, empty.
#[derive(Default)]
struct LowerScratch {
    labels: Vec<Option<u32>>,
    fixups: Vec<(u32, usize)>,
    origins: Vec<usize>,
    stack: Vec<Entry>,
    name_keys: Vec<Option<PropertyKey>>,
    local_keys: Vec<Option<PropertyKey>>,
}

/// Where a function's locals and temporaries live. The window of 256
/// registers holds `this`, the parameters, the locals and the temporaries in
/// that order. A function whose locals do not all fit keeps the ones past
/// `window_locals` in the slots of a spill scope, held in the register after
/// the window's locals; its temporaries stay above every local, so a call's
/// window never covers one.
///
/// A function whose operand stack does not fit either keeps its first
/// `temp_registers` depths in registers and the deeper ones in the spill
/// scope's slots after the spilled locals, reached through [`SCRATCH`]
/// registers above the temporaries.
#[derive(Clone, Copy, Debug)]
struct Layout {
    first_local: u32,
    window_locals: u32,
    spill: Option<(u32, u32)>,
    temp_base: u32,
    /// The depths whose temporaries are registers: all of them (`u32::MAX`)
    /// unless the operand stack passes the window.
    temp_registers: u32,
    /// The spill slot of depth `temp_registers`.
    first_temp_slot: u32,
    /// The register a construction writes `new.target` into: a hidden local right after the parameters.
    new_target: Option<u32>,
}

/// Whether a function reads `new.target`: the prologue's pseudo binding, a
/// class constructor's check, or a derived constructor's implicit `super`.
fn uses_new_target(function: &UnlinkedFunction) -> bool {
    function.code().iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::PushNewTarget
                | Instruction::CheckCtor
                | Instruction::InitDerivedConstructor
        )
    })
}

/// Local registers past the window are named from here: `SPILLED + slot`.
const SPILLED: u32 = 1 << 20;

/// The registers above the temporaries of a function whose operand stack
/// passes the window: the first carries a value between a slot and a
/// constant or another slot, the others hold the operands and results of
/// the instruction being built, taken in turn.
const SCRATCH: u32 = 8;

impl Layout {
    fn of(function: &UnlinkedFunction, analysis: &Analysis) -> Result<Layout, LowerError> {
        let metadata = function.metadata();
        let new_target = uses_new_target(function).then(|| 1 + u32::from(metadata.argument_count));
        let first_local = 1 + u32::from(metadata.argument_count) + u32::from(new_target.is_some());
        let locals = u32::from(metadata.local_count);
        // One temporary above the deepest stack for scratch values.
        let temps = analysis.max_depth + 1;
        let window = oxide_core::bytecode::WINDOW;
        let stress = cfg!(feature = "register-stress");
        if !stress && first_local + locals + temps <= window {
            return Ok(Layout {
                first_local,
                window_locals: locals,
                spill: None,
                temp_base: first_local + locals,
                temp_registers: u32::MAX,
                first_temp_slot: 0,
                new_target,
            });
        }
        if let Some(window_locals) = window
            .checked_sub(first_local + 1 + temps)
            .filter(|_| !stress)
        {
            let spill = first_local + window_locals;
            return Ok(Layout {
                first_local,
                window_locals,
                spill: Some((spill, locals - window_locals)),
                temp_base: spill + 1,
                temp_registers: u32::MAX,
                first_temp_slot: 0,
                new_target,
            });
        }
        // The operand stack passes the window: a quarter of it at most for
        // locals, the spill register and the scratch registers at the top,
        // the temporaries that fit in between.
        let available = window
            .checked_sub(first_local + 1 + SCRATCH)
            .ok_or_else(|| LowerError::unsupported("functions with more than 256 registers"))?;
        let window_locals = locals.min(window / 4).min(available);
        let mut temp_registers = available - window_locals;
        if stress {
            temp_registers = temp_registers.min(2);
        }
        let spilled_locals = locals - window_locals;
        let spill = first_local + window_locals;
        Ok(Layout {
            first_local,
            window_locals,
            spill: Some((spill, spilled_locals + temps.saturating_sub(temp_registers))),
            temp_base: spill + 1,
            temp_registers,
            first_temp_slot: spilled_locals,
            new_target,
        })
    }

    fn local(&self, local: u16) -> u32 {
        let local = u32::from(local);
        if local < self.window_locals {
            self.first_local + local
        } else {
            SPILLED + local - self.window_locals
        }
    }
}

/// Rejects whole-function features outside the surface.
fn check_surface(function: &UnlinkedFunction) -> Result<(), LowerError> {
    // `with` is a dropped feature: an early `SyntaxError`, as in strict code.
    if function
        .local_definitions()
        .iter()
        .any(|local| local.kind == ClosureVariableKind::WithObject)
    {
        return Err(LowerError::Syntax(
            "with statements are not supported".into(),
        ));
    }
    let metadata = function.metadata();
    let unsupported = if metadata.eval_kind == EvalKind::Direct {
        // Direct eval code: a direct `eval` runs as indirect eval (a dropped
        // feature), so only indirect eval code is compiled.
        Some("direct eval code")
    } else {
        None
    };
    unsupported.map_or(Ok(()), |construct| Err(LowerError::unsupported(construct)))
}

/// A symbolic operand-stack entry.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    /// In `T(d)` for the entry's own depth `d`.
    Canon,
    /// Still in this local, parameter or `this` register.
    Reg(u32),
    Int(i32),
    Undefined,
    Null,
    Bool(bool),
    /// A constant-pool index.
    Const(u32),
    /// A `Catch` marker; it has no register value.
    Marker,
}

/// Stack-code comparisons, for fusion with a following branch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Compare {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
    Neq,
    StrictEq,
    StrictNeq,
}

impl Compare {
    fn of(instruction: &Instruction) -> Option<Compare> {
        Some(match instruction {
            Instruction::Lt => Compare::Lt,
            Instruction::Lte => Compare::Lte,
            Instruction::Gt => Compare::Gt,
            Instruction::Gte => Compare::Gte,
            Instruction::Eq => Compare::Eq,
            Instruction::Neq => Compare::Neq,
            Instruction::StrictEq => Compare::StrictEq,
            Instruction::StrictNeq => Compare::StrictNeq,
            _ => return None,
        })
    }

    /// The comparison with its operands exchanged: `k < x` is `x > k`.
    fn mirrored(self) -> Compare {
        match self {
            Compare::Lt => Compare::Gt,
            Compare::Lte => Compare::Gte,
            Compare::Gt => Compare::Lt,
            Compare::Gte => Compare::Lte,
            symmetric => symmetric,
        }
    }

    /// The compare-jump taken when the comparison is `taken_when`. A negated
    /// relational form is `JN…`, not the opposite relation, because of NaN.
    fn jump(self, taken_when: bool, immediate: bool) -> Opcode {
        use Opcode as O;
        let (register, short) = match (self, taken_when) {
            (Compare::Lt, true) => (O::JLt, O::JLtI),
            (Compare::Lte, true) => (O::JLte, O::JLteI),
            (Compare::Gt, true) => (O::JGt, O::JGtI),
            (Compare::Gte, true) => (O::JGte, O::JGteI),
            (Compare::Lt, false) => (O::JNlt, O::JNltI),
            (Compare::Lte, false) => (O::JNlte, O::JNlteI),
            (Compare::Gt, false) => (O::JNgt, O::JNgtI),
            (Compare::Gte, false) => (O::JNgte, O::JNgteI),
            (Compare::Eq, true) | (Compare::Neq, false) => (O::JEq, O::JEqI),
            (Compare::Neq, true) | (Compare::Eq, false) => (O::JNeq, O::JNeqI),
            (Compare::StrictEq, true) | (Compare::StrictNeq, false) => {
                (O::JStrictEq, O::JStrictEqI)
            }
            (Compare::StrictNeq, true) | (Compare::StrictEq, false) => {
                (O::JStrictNeq, O::JStrictNeqI)
            }
        };
        if immediate { short } else { register }
    }

    fn value(self) -> Opcode {
        match self {
            Compare::Lt => Opcode::Lt,
            Compare::Lte => Opcode::Lte,
            Compare::Gt => Opcode::Gt,
            Compare::Gte => Opcode::Gte,
            Compare::Eq => Opcode::Eq,
            Compare::Neq => Opcode::Neq,
            Compare::StrictEq => Opcode::StrictEq,
            Compare::StrictNeq => Opcode::StrictNeq,
        }
    }
}

fn unary(instruction: &Instruction) -> Option<Opcode> {
    Some(match instruction {
        Instruction::Neg => Opcode::Neg,
        Instruction::Plus => Opcode::Plus,
        Instruction::Inc => Opcode::Inc,
        Instruction::Dec => Opcode::Dec,
        Instruction::BitNot => Opcode::BitNot,
        Instruction::Not => Opcode::Not,
        Instruction::TypeOf => Opcode::TypeOf,
        Instruction::ToPropKey => Opcode::ToPropertyKey,
        Instruction::IsUndefinedOrNull => Opcode::IsUndefinedOrNull,
        _ => return None,
    })
}

/// A binary operator and its `i32`-immediate form, if it has one.
fn binary(instruction: &Instruction) -> Option<(Opcode, Option<Opcode>)> {
    Some(match instruction {
        Instruction::Add => (Opcode::Add, Some(Opcode::AddI)),
        Instruction::Sub => (Opcode::Sub, Some(Opcode::SubI)),
        Instruction::Mul => (Opcode::Mul, Some(Opcode::MulI)),
        Instruction::Div => (Opcode::Div, None),
        Instruction::Mod => (Opcode::Mod, None),
        Instruction::Pow => (Opcode::Pow, None),
        Instruction::Shl => (Opcode::Shl, Some(Opcode::ShlI)),
        Instruction::Sar => (Opcode::Sar, Some(Opcode::SarI)),
        Instruction::Shr => (Opcode::Shr, Some(Opcode::ShrI)),
        Instruction::BitAnd => (Opcode::BitAnd, Some(Opcode::BitAndI)),
        Instruction::BitOr => (Opcode::BitOr, Some(Opcode::BitOrI)),
        Instruction::BitXor => (Opcode::BitXor, Some(Opcode::BitXorI)),
        Instruction::InstanceOf => (Opcode::InstanceOf, None),
        Instruction::In => (Opcode::In, None),
        _ => return None,
    })
}

/// What an instruction outside the surface belongs to, for `Unsupported`:
/// the construct when it is a named one, else the stack-code name.
fn unsupported_instruction(instruction: &Instruction) -> LowerError {
    use Instruction as I;
    let construct = match instruction {
        I::CheckCtor
        | I::DefineClass { .. }
        | I::InstallClassInstanceInitializer
        | I::CallClassInstanceInitializer
        | I::RunClassStaticInitializer
        | I::CallClassStaticBlock
        | I::PushHomeObject
        | I::GetSuper
        | I::GetSuperValue
        | I::GetSuperValueForCall
        | I::PutSuperValue => "classes",
        I::CopyDataProperties | I::CopyDataPropertiesExcluded { .. } => "object spread",
        I::ForInStart | I::ForInNext => "for-in",
        I::Import => "modules",
        _ => {
            let text = format!("{instruction:?}");
            let end = text.find(['(', ' ', '{']).unwrap_or(text.len());
            return LowerError::Unsupported(text[..end].to_owned());
        }
    };
    LowerError::unsupported(construct)
}

/// The `DefineMethod` flags of a stack-code method definition.
fn method_flags(kind: DefineMethodKind, enumerable: bool) -> u32 {
    use oxide_core::bytecode::opcode::method;
    let kind = match kind {
        DefineMethodKind::Method => 0,
        DefineMethodKind::Getter => method::GETTER,
        DefineMethodKind::Setter => method::SETTER,
    };
    kind | if enumerable { method::ENUMERABLE } else { 0 }
}

/// Whether QuickJS's `JS_NewAtom` makes `string` a tagged integer atom: the
/// canonical decimal spelling of an integer up to `JS_ATOM_MAX_INT`.
fn is_tagged_int_atom(string: &JsString) -> bool {
    let len = string.len();
    if len == 0 || len > 10 {
        return false;
    }
    let mut value: u64 = 0;
    for (index, unit) in string.utf16_units().enumerate() {
        let Some(digit) = unit
            .checked_sub(u16::from(b'0'))
            .filter(|digit| *digit <= 9)
        else {
            return false;
        };
        if index == 0 && digit == 0 && len > 1 {
            return false;
        }
        value = value * 10 + u64::from(digit);
    }
    value <= i32::MAX as u64
}

fn string_units(string: &JsString) -> Vec<u16> {
    string.utf16_units().collect()
}

/// The most operands one `ConcatN` joins.
const CONCAT_OPERANDS: usize = 16;

/// Marks a capture index in the set of bindings a check has proven
/// initialized since the last join point (a binding leaves the TDZ only
/// forward, and only `SetLocalUninitialized` re-enters it).
const CAPTURE_BIT: u32 = 1 << 31;

struct Lowerer<'a, 'c> {
    runtime: &'a mut Compiler<'c>,
    tree: &'a mut TreeCache,
    function: &'a UnlinkedFunction,
    code: &'a [Instruction],
    analysis: Analysis,
    is_script: bool,
    builder: CodeBuilder,
    stack: Vec<Entry>,
    temp_base: u32,
    layout: Layout,
    captured_arguments: Vec<bool>,
    captured_locals: Vec<bool>,
    /// Register PC of each stack PC that starts a label.
    labels: Vec<Option<u32>>,
    /// Jumps to patch: register PC, stack target.
    fixups: Vec<(u32, usize)>,
    /// The stack PC each register instruction came from.
    origins: Vec<usize>,
    pc: usize,
    /// The last instruction, if it only wrote a temporary: its PC and the
    /// temporary, for retargeting into a local.
    last_dest: Option<(u32, u32)>,
    checked: HashSet<u32>,
    /// Depth of temporaries used above the stack (TDZ checks of captures).
    max_scratch: u32,
    pool: Vec<CoreValue>,
    pool_index: HashMap<u32, u32>,
    /// The key of each string constant used as a name, and of each
    /// local's name.
    name_keys: Vec<Option<PropertyKey>>,
    local_keys: Vec<Option<PropertyKey>>,
    child_ordinal: HashMap<u32, u32>,
    this_keys: HashSet<u32>,
    /// Register PCs of the calls written as a direct `eval` (for the
    /// backtrace, which QuickJS gives no native frame there).
    direct_eval_sites: Vec<u32>,
    /// The next of the turn-taking scratch registers (see [`SCRATCH`]).
    next_scratch: u32,
    /// The scratch register the last [`Self::result_register`] gave for a
    /// depth in the spill scope.
    result_scratch: u8,
    /// Jumps that write a register on the way to a stack PC whose depth is
    /// in the spill scope, each through a stub after the code that stores
    /// it: the jump's register PC, the register and its temporary, the
    /// target.
    store_stubs: Vec<(u32, u8, u32, usize)>,
    /// The name the parent's code gives this function, for its unit when it
    /// has none of its own.
    inferred_name: Option<JsString>,
    /// The children whose units carry the name the `SetName` after their
    /// `FClosure` gives: that `SetName` emits nothing.
    named_children: Vec<bool>,
}

impl<'a, 'c> Lowerer<'a, 'c> {
    fn new(
        runtime: &'a mut Compiler<'c>,
        tree: &'a mut TreeCache,
        function: &'a UnlinkedFunction,
        analysis: Analysis,
        layout: Layout,
        is_script: bool,
    ) -> Result<Self, LowerError> {
        let metadata = function.metadata();
        let arguments = u32::from(metadata.argument_count);
        let locals = u32::from(metadata.local_count);
        let mut captured_arguments = vec![false; arguments as usize];
        let mut captured_locals = vec![false; locals as usize];
        let mut child_ordinal = HashMap::new();
        for (index, constant) in function.constants().iter().enumerate() {
            let Some(child) = constant.as_child() else {
                continue;
            };
            child_ordinal.insert(index as u32, child_ordinal.len() as u32);
            for variable in child.closure_variables() {
                let slot = match variable.source {
                    ClosureSource::ParentLocal(local) => captured_locals.get_mut(local as usize),
                    ClosureSource::ParentArgument(argument) => {
                        captured_arguments.get_mut(argument as usize)
                    }
                    _ => continue,
                };
                *slot.ok_or_else(|| LowerError::Internal("capture out of range".into()))? = true;
            }
        }
        let code = function.code();
        let scratch = &mut tree.lowering;
        let mut labels = std::mem::take(&mut scratch.labels);
        labels.resize(code.len(), None);
        let mut name_keys = std::mem::take(&mut scratch.name_keys);
        name_keys.resize(function.constants().len(), None);
        let mut local_keys = std::mem::take(&mut scratch.local_keys);
        local_keys.resize(function.local_definitions().len(), None);
        let fixups = std::mem::take(&mut scratch.fixups);
        let origins = std::mem::take(&mut scratch.origins);
        let stack = std::mem::take(&mut scratch.stack);
        debug_assert!(fixups.is_empty() && origins.is_empty() && stack.is_empty());
        // A mapped `arguments` object aliases the parameters
        // through cells, so every parameter of such a function is one.
        if code
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Arguments(ArgumentsKind::Mapped)))
        {
            captured_arguments.fill(true);
        }
        Ok(Lowerer {
            runtime,
            tree,
            function,
            code,
            is_script,
            builder: CodeBuilder::new(),
            stack,
            temp_base: layout.temp_base,
            layout,
            captured_arguments,
            captured_locals,
            labels,
            fixups,
            origins,
            direct_eval_sites: Vec::new(),
            next_scratch: 0,
            result_scratch: 0,
            store_stubs: Vec::new(),
            inferred_name: None,
            named_children: Vec::new(),
            pc: 0,
            last_dest: None,
            checked: HashSet::new(),
            max_scratch: 0,
            pool: Vec::new(),
            pool_index: HashMap::new(),
            name_keys,
            local_keys,
            child_ordinal,
            this_keys: HashSet::new(),
            analysis,
        })
    }

    // ---- registers ----

    /// The temporary of `depth`: a register, or past the window's
    /// temporaries a slot of the spill scope, named as a spilled local is.
    fn temp(&self, depth: usize) -> u32 {
        let depth = depth as u32;
        if depth < self.layout.temp_registers {
            self.temp_base + depth
        } else {
            SPILLED + self.layout.first_temp_slot + depth - self.layout.temp_registers
        }
    }

    /// Whether the temporary of `depth` is a spill slot.
    fn spilled(&self, depth: usize) -> bool {
        depth as u32 >= self.layout.temp_registers
    }

    /// The scratch register that carries a value between a slot and a
    /// constant or another slot within one helper.
    fn transfer(&self) -> Result<u8, LowerError> {
        Self::r8(self.temp_base + self.layout.temp_registers)
    }

    /// The next turn-taking scratch register.
    fn rotating(&mut self) -> Result<u8, LowerError> {
        let index = 1 + self.next_scratch % (SCRATCH - 1);
        self.next_scratch = index;
        Self::r8(self.temp_base + self.layout.temp_registers + index)
    }

    /// `count` consecutive turn-taking scratch registers.
    fn scratch_run(&mut self, count: usize) -> Result<u8, LowerError> {
        let count = count as u32;
        if count >= SCRATCH {
            return Err(LowerError::Internal("a scratch run past the pool".into()));
        }
        let mut start = self.next_scratch % (SCRATCH - 1);
        if start + count > SCRATCH - 1 {
            start = 0;
        }
        self.next_scratch = start + count;
        Self::r8(self.temp_base + self.layout.temp_registers + 1 + start)
    }

    fn argument_register(&self, argument: u16) -> u32 {
        1 + u32::from(argument)
    }

    fn local_register(&self, local: u16) -> u32 {
        self.layout.local(local)
    }

    /// A local past the window, in a slot of the spill scope.
    fn is_wide(register: u32) -> bool {
        register >= SPILLED
    }

    /// The spill register and the slot of a spilled local.
    fn spill_slot(&self, register: u32) -> Result<(u8, i32), LowerError> {
        let (spill, _) = self
            .layout
            .spill
            .ok_or_else(|| LowerError::Internal("a spilled local without a spill scope".into()))?;
        Ok((Self::r8(spill)?, (register - SPILLED) as i32))
    }

    /// A temporary above the stack, for shuttling a wide local.
    fn scratch(&mut self) -> Result<u8, LowerError> {
        let depth = self.stack.len();
        if self.spilled(depth) {
            return self.rotating();
        }
        self.max_scratch = self.max_scratch.max(depth as u32 + 1);
        Self::r8(self.temp(depth))
    }

    /// Emits `build` on local register `register`: directly in the window,
    /// or past it on a scratch temporary loaded from it (`load`) and stored
    /// back to it (`store`).
    fn on_local(
        &mut self,
        register: u32,
        load: bool,
        store: bool,
        build: impl FnOnce(&mut CodeBuilder, u8) -> u32,
    ) -> Result<(), LowerError> {
        if !Self::is_wide(register) {
            let a = Self::r8(register)?;
            self.emit(|b| build(b, a));
            return Ok(());
        }
        let scratch = self.scratch()?;
        let (spill, slot) = self.spill_slot(register)?;
        if load {
            self.emit(|b| b.ab_int(Opcode::LoadSlot, scratch, spill, slot));
        }
        self.emit(|b| build(b, scratch));
        if store {
            self.emit(|b| b.ab_int(Opcode::StoreSlot, scratch, spill, slot));
        }
        Ok(())
    }

    fn r8(register: u32) -> Result<u8, LowerError> {
        u8::try_from(register)
            .map_err(|_| LowerError::unsupported("functions with more than 256 registers"))
    }

    // ---- emission ----

    fn emit(&mut self, build: impl FnOnce(&mut CodeBuilder) -> u32) -> u32 {
        let pc = build(&mut self.builder);
        self.origins.push(self.pc);
        self.last_dest = None;
        pc
    }

    fn emit_jump(&mut self, build: impl FnOnce(&mut CodeBuilder) -> u32, target: usize) {
        let pc = self.emit(build);
        self.fixups.push((pc, target));
    }

    /// A jump that writes `register` for the depth just above the stack on
    /// the way to `target`: through a stub that stores it when that depth is
    /// in the spill scope.
    fn emit_storing_jump(
        &mut self,
        build: impl FnOnce(&mut CodeBuilder) -> u32,
        register: u8,
        target: usize,
    ) {
        let depth = self.stack.len();
        if self.spilled(depth) {
            let pc = self.emit(build);
            self.store_stubs
                .push((pc, register, self.temp(depth), target));
        } else {
            self.emit_jump(build, target);
        }
    }

    /// A back edge from `pc` to a loop whose first lowered instruction is
    /// a compare-jump out of it repeats that test, negated, as a jump to the
    /// loop's body: an iteration runs one branch instead of a jump and the
    /// test. The test reads registers only and the back edge is in the
    /// test's handler regions, so the copy behaves as the original;
    /// leaving the loop jumps to the test's exit unless that is next.
    fn invert_back_edge(&mut self, pc: usize, target: usize) -> bool {
        let Some(head) = self.labels[target] else {
            return false;
        };
        let Some(&test) = self.builder.code().get(head as usize) else {
            return false;
        };
        let Some(negated) = negated_jump(test.op) else {
            return false;
        };
        let Some(&(_, exit)) = self.fixups.iter().find(|&&(at, _)| at == head) else {
            return false;
        };
        if self.analysis.regions[pc] != self.analysis.regions[target] {
            return false;
        }
        let mut repeat = test;
        repeat.op = negated;
        repeat.imm = head + 1;
        self.emit(|b| b.raw(repeat));
        let next = (pc + 1..self.code.len()).find(|&at| self.analysis.depth[at].is_some());
        if next != Some(exit) {
            self.emit_jump(|b| b.jump(Opcode::Jmp, 0), exit);
        }
        true
    }

    /// `dst = src` between registers and spill slots.
    fn mov(&mut self, dst: u32, src: u32) -> Result<(), LowerError> {
        if dst == src {
            return Ok(());
        }
        match (Self::is_wide(dst), Self::is_wide(src)) {
            (false, false) => {
                let (dst, src) = (Self::r8(dst)?, Self::r8(src)?);
                self.emit(|b| b.ab(Opcode::Mov, dst, src));
            }
            (false, true) => {
                let dst = Self::r8(dst)?;
                let (spill, slot) = self.spill_slot(src)?;
                self.emit(|b| b.ab_int(Opcode::LoadSlot, dst, spill, slot));
            }
            (true, false) => {
                let src = Self::r8(src)?;
                let (spill, slot) = self.spill_slot(dst)?;
                self.emit(|b| b.ab_int(Opcode::StoreSlot, src, spill, slot));
            }
            (true, true) => {
                let transfer = self.transfer()?;
                self.mov(u32::from(transfer), src)?;
                self.mov(dst, u32::from(transfer))?;
            }
        }
        Ok(())
    }

    /// Exchanges two registers or spill slots.
    fn swap(&mut self, a: u32, b: u32) -> Result<(), LowerError> {
        let mut registers = [0; 2];
        for (register, location) in registers.iter_mut().zip([a, b]) {
            *register = if Self::is_wide(location) {
                let scratch = self.rotating()?;
                self.mov(u32::from(scratch), location)?;
                scratch
            } else {
                Self::r8(location)?
            };
        }
        let [ra, rb] = registers;
        self.emit(|builder| builder.ab(Opcode::Swap, ra, rb));
        self.mov(a, u32::from(ra))?;
        self.mov(b, u32::from(rb))
    }

    /// Loads a non-register entry into `dst`.
    fn load(&mut self, dst: u32, entry: Entry) -> Result<(), LowerError> {
        if let Entry::Reg(src) = entry {
            return self.mov(dst, src);
        }
        if Self::is_wide(dst) && !matches!(entry, Entry::Canon | Entry::Marker) {
            let transfer = self.transfer()?;
            self.load(u32::from(transfer), entry)?;
            return self.mov(dst, u32::from(transfer));
        }
        let a = Self::r8(dst)?;
        match entry {
            Entry::Canon | Entry::Marker | Entry::Reg(_) => {
                return Err(LowerError::Internal("load of a register entry".into()));
            }
            Entry::Int(value) => {
                self.emit(|b| b.a_int(Opcode::LoadInt, a, value));
            }
            Entry::Undefined => {
                self.emit(|b| b.a(Opcode::LoadUndefined, a));
            }
            Entry::Null => {
                self.emit(|b| b.a(Opcode::LoadNull, a));
            }
            Entry::Bool(true) => {
                self.emit(|b| b.a(Opcode::LoadTrue, a));
            }
            Entry::Bool(false) => {
                self.emit(|b| b.a(Opcode::LoadFalse, a));
            }
            Entry::Const(index) => {
                self.emit(|b| b.a_const(Opcode::LoadConst, a, index));
            }
        }
        Ok(())
    }

    /// Makes the entry at `depth` canonical.
    fn materialize(&mut self, depth: usize) -> Result<(), LowerError> {
        let entry = self.stack[depth];
        self.max_scratch = self.max_scratch.max(depth as u32 + 1);
        if !matches!(entry, Entry::Canon | Entry::Marker) {
            self.load(self.temp(depth), entry)?;
            self.stack[depth] = Entry::Canon;
        }
        Ok(())
    }

    /// Rule 1.
    fn materialize_all(&mut self) -> Result<(), LowerError> {
        for depth in 0..self.stack.len() {
            self.materialize(depth)?;
        }
        Ok(())
    }

    /// The register holding the entry at `depth`, loading a constant into
    /// its temporary first; a slot's value or a constant of a depth in the
    /// spill scope comes in a scratch register.
    fn operand(&mut self, depth: usize) -> Result<u8, LowerError> {
        match self.stack[depth] {
            Entry::Canon if self.spilled(depth) => {
                let scratch = self.rotating()?;
                self.mov(u32::from(scratch), self.temp(depth))?;
                Ok(scratch)
            }
            Entry::Canon => Self::r8(self.temp(depth)),
            Entry::Reg(register) => Self::r8(register),
            Entry::Marker => Err(LowerError::Internal("catch marker used as a value".into())),
            entry if self.spilled(depth) => {
                let scratch = self.rotating()?;
                self.load(u32::from(scratch), entry)?;
                Ok(scratch)
            }
            _ => {
                self.materialize(depth)?;
                Self::r8(self.temp(depth))
            }
        }
    }

    /// The register holding the entry at `depth` once it is canonical.
    fn canonical(&mut self, depth: usize) -> Result<u8, LowerError> {
        self.materialize(depth)?;
        self.operand(depth)
    }

    /// Loads the entry at `depth`, whatever its kind, into `dst`.
    fn copy_entry(&mut self, dst: u32, depth: usize) -> Result<(), LowerError> {
        match self.stack[depth] {
            Entry::Canon => self.mov(dst, self.temp(depth)),
            entry => self.load(dst, entry),
        }
    }

    /// The register an instruction writes the temporary of `depth` into: the
    /// temporary, or a scratch register that [`Self::written`] stores.
    fn dest(&mut self, depth: usize) -> Result<u8, LowerError> {
        if self.spilled(depth) {
            self.rotating()
        } else {
            Self::r8(self.temp(depth))
        }
    }

    /// Stores what an instruction wrote to the register of [`Self::dest`]
    /// or [`Self::temps_window`] into the temporary of `depth`.
    fn written(&mut self, depth: usize, register: u8) -> Result<(), LowerError> {
        self.mov(self.temp(depth), u32::from(register))
    }

    /// Consecutive registers for the canonical temporaries of
    /// `base..base + count`, which an instruction reads or writes as a
    /// range: the temporaries themselves, or once the range reaches the
    /// spill scope a run of scratch registers holding copies of the first
    /// `loaded`.
    fn temps_window(&mut self, base: usize, count: usize, loaded: usize) -> Result<u8, LowerError> {
        if !self.spilled(base + count - 1) {
            return Self::r8(self.temp(base));
        }
        let run = self.scratch_run(count)?;
        for index in 0..loaded {
            self.mov(u32::from(run) + index as u32, self.temp(base + index))?;
        }
        Ok(run)
    }

    /// Stores the members `range` of a run of [`Self::temps_window`] back.
    fn temps_written(
        &mut self,
        run: u8,
        base: usize,
        range: std::ops::Range<usize>,
    ) -> Result<(), LowerError> {
        for index in range {
            self.mov(self.temp(base + index), u32::from(run) + index as u32)?;
        }
        Ok(())
    }

    /// Marks a record copied out of the spill scope done in its slot before
    /// an operation that marks the copy done before what may throw
    /// (`IterNext`'s and `IterNextRaw`'s call of `next`, `IterResult`'s
    /// reads, `IterClose`'s call of `return`): a throw leaves the record
    /// done for the closing stub, which reads the slots. The operation's
    /// own value is stored back after it.
    fn mark_record_done(&mut self, run: u8, record: usize) -> Result<(), LowerError> {
        if u32::from(run) != self.temp(record) {
            let done = self.temp(record + 2);
            self.load(done, Entry::Bool(true))?;
        }
        Ok(())
    }

    fn pop(&mut self) -> Result<(usize, Entry), LowerError> {
        let entry = self
            .stack
            .pop()
            .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
        Ok((self.stack.len(), entry))
    }

    /// Reads the top `count` entries as registers and pops them; returns the
    /// depth of the lowest.
    fn pop_operands<const N: usize>(&mut self) -> Result<(usize, [u8; N]), LowerError> {
        let base = self
            .stack
            .len()
            .checked_sub(N)
            .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
        let mut registers = [0; N];
        for (index, register) in registers.iter_mut().enumerate() {
            *register = self.operand(base + index)?;
        }
        self.stack.truncate(base);
        Ok((base, registers))
    }

    /// The destination of a value pushed now.
    fn result_register(&mut self) -> Result<u8, LowerError> {
        let depth = self.stack.len();
        if self.spilled(depth) {
            self.result_scratch = self.rotating()?;
            return Ok(self.result_scratch);
        }
        Self::r8(self.temp(depth))
    }

    /// Records the value written by the instruction at `pc` as the new top.
    fn push_result(&mut self, pc: u32) -> Result<(), LowerError> {
        let depth = self.stack.len();
        let temp = self.temp(depth);
        if self.spilled(depth) {
            self.written(depth, self.result_scratch)?;
            self.stack.push(Entry::Canon);
            return Ok(());
        }
        self.stack.push(Entry::Canon);
        self.max_scratch = self.max_scratch.max(self.stack.len() as u32);
        self.last_dest = Some((pc, temp));
        Ok(())
    }

    /// Rule 2: entries aliasing `register` become canonical before it is
    /// written. Returns whether any did.
    fn spill_aliases(&mut self, register: u32) -> Result<bool, LowerError> {
        let mut spilled = false;
        for depth in 0..self.stack.len() {
            if self.stack[depth] == Entry::Reg(register) {
                self.materialize(depth)?;
                spilled = true;
            }
        }
        Ok(spilled)
    }

    /// Stores the popped `entry` (from `depth`) into a plain register,
    /// retargeting its producer when it only wrote the temporary.
    fn store_register(
        &mut self,
        register: u32,
        depth: usize,
        entry: Entry,
    ) -> Result<(), LowerError> {
        let producer = self.last_dest;
        let spilled = self.spill_aliases(register)?;
        match entry {
            Entry::Canon => {
                let temp = self.temp(depth);
                match producer {
                    Some((pc, written)) if written == temp && !spilled => {
                        self.builder.retarget(pc, Self::r8(register)?);
                        self.last_dest = None;
                    }
                    _ => self.mov(register, temp)?,
                }
            }
            Entry::Marker => return Err(LowerError::Internal("store of a catch marker".into())),
            other => self.load(register, other)?,
        }
        Ok(())
    }

    fn check_init(&mut self, register: u32, name: PropertyKey) -> Result<(), LowerError> {
        if self.checked.insert(register) {
            self.on_local(register, true, false, |b, a| {
                b.a_key(Opcode::CheckInit, a, name)
            })?;
        }
        Ok(())
    }

    fn check_cell_init(&mut self, cell: u32, name: PropertyKey) -> Result<(), LowerError> {
        if self.checked.insert(cell) {
            self.on_local(cell, true, false, |b, a| {
                b.a_key(Opcode::CheckCellInit, a, name)
            })?;
        }
        Ok(())
    }

    /// Checks capture `closure` through a scratch temporary above the stack.
    fn check_captured_init(&mut self, closure: u16) -> Result<(), LowerError> {
        if !self.checked.insert(CAPTURE_BIT | u32::from(closure)) {
            return Ok(());
        }
        let name = self.closure_key(closure)?;
        let a = self.scratch()?;
        self.emit(|b| b.a_capture(Opcode::LoadCaptured, a, u32::from(closure)));
        self.emit(|b| b.a_key(Opcode::CheckInit, a, name));
        Ok(())
    }

    // ---- names and constants ----

    fn constant_string(&self, index: u32) -> Result<&'a JsString, LowerError> {
        match self
            .function
            .constants()
            .get(index as usize)
            .and_then(|constant| constant.as_primitive())
        {
            Some(PrimitiveValue::String(string)) => Ok(string),
            _ => Err(LowerError::Internal(format!(
                "constant {index} is not a string"
            ))),
        }
    }

    /// The key `string` spells, once per string of the tree.
    fn key(&mut self, string: &JsString) -> Result<PropertyKey, LowerError> {
        if let Some(&(_, key)) = self.tree.keys.get(&string.address()) {
            return Ok(key);
        }
        let heap = self.runtime.heap_mut();
        let key = match string.flat_latin1() {
            Some(bytes) => heap.property_key_latin1(bytes)?,
            None => heap.property_key_units(&string_units(string))?,
        };
        self.tree
            .keys
            .insert(string.address(), (string.clone(), key));
        Ok(key)
    }

    /// The key of a message or other text of the lowering's own.
    fn static_key(&mut self, text: &'static str) -> Result<PropertyKey, LowerError> {
        Ok(self.runtime.heap_mut().property_key(text)?)
    }

    fn name_key(&mut self, constant: u32) -> Result<PropertyKey, LowerError> {
        if let Some(&Some(key)) = self.name_keys.get(constant as usize) {
            return Ok(key);
        }
        let string = self.constant_string(constant)?;
        let key = self.key(string)?;
        if let Some(slot) = self.name_keys.get_mut(constant as usize) {
            *slot = Some(key);
        }
        Ok(key)
    }

    fn closure_key(&mut self, closure: u16) -> Result<PropertyKey, LowerError> {
        let variable = self
            .function
            .closure_variables()
            .get(closure as usize)
            .ok_or_else(|| LowerError::Internal("closure variable out of range".into()))?;
        match variable.name {
            ClosureVariableName::Constant(constant) => self.name_key(constant),
            _ => self.static_key(""),
        }
    }

    fn local_key(&mut self, local: u16) -> Result<PropertyKey, LowerError> {
        if let Some(&Some(key)) = self.local_keys.get(local as usize) {
            return Ok(key);
        }
        let function = self.function;
        let key = match function
            .local_definitions()
            .get(local as usize)
            .and_then(|definition| definition.name.as_ref())
        {
            Some(name) => self.key(name)?,
            None => self.static_key("")?,
        };
        if let Some(slot) = self.local_keys.get_mut(local as usize) {
            *slot = Some(key);
        }
        Ok(key)
    }

    /// An atom value for a string of the tree, once per string (the
    /// compilation pins or holds the atoms it interns, so the pool needs no
    /// rooting until it is attached).
    fn atom_value(&mut self, string: &JsString) -> Result<CoreValue, LowerError> {
        if let Some(&(_, atom)) = self.tree.atoms.get(&string.address()) {
            return Ok(CoreValue::string(atom));
        }
        let heap = self.runtime.heap_mut();
        let atom = match string.flat_latin1() {
            Some(bytes) => heap.intern_latin1(bytes)?,
            None => heap.intern_units(&string_units(string))?,
        };
        self.tree
            .atoms
            .insert(string.address(), (string.clone(), atom));
        Ok(CoreValue::string(atom))
    }

    fn pool_entry(&mut self, index: u32) -> Result<Entry, LowerError> {
        if let Some(&slot) = self.pool_index.get(&index) {
            return Ok(Entry::Const(slot));
        }
        let constant = self
            .function
            .constants()
            .get(index as usize)
            .ok_or_else(|| LowerError::Internal(format!("constant {index} out of range")))?;
        if let Some((cooked, raw)) = constant.as_template_object() {
            let value = self.template_entry(cooked, raw)?;
            let slot = self.pool.len() as u32;
            self.pool.push(value);
            self.pool_index.insert(index, slot);
            return Ok(Entry::Const(slot));
        }
        let Some(primitive) = constant.as_primitive() else {
            return Err(if constant.as_regexp().is_some() {
                LowerError::unsupported("RegExp literals")
            } else {
                LowerError::unsupported("constant kind")
            });
        };
        let value = match primitive {
            PrimitiveValue::Int(value) => return Ok(Entry::Int(*value)),
            PrimitiveValue::Float(value) => CoreValue::number(*value),
            PrimitiveValue::String(string) => {
                if is_tagged_int_atom(string) {
                    // QuickJS keeps a literal spelling an index up to 2^31 - 1
                    // as its own constant-pool string, not an atom: two such
                    // literals are distinct values (`Array.prototype.sort`'s
                    // identity shortcut observes this).
                    let heap = self.runtime.heap_mut();
                    let value = CoreValue::string(heap.alloc_string_units(&string_units(string))?);
                    heap.roots().push(value);
                    value
                } else {
                    self.atom_value(string)?
                }
            }
            PrimitiveValue::BigInt(value) => {
                let (sign, limbs) = value.to_bigint().to_u64_digits();
                let heap = self.runtime.heap_mut();
                let id = heap.alloc_bigint(sign == num_bigint::Sign::Minus, &limbs)?;
                let value = CoreValue::bigint(id);
                heap.roots().push(value);
                value
            }
        };
        let slot = self.pool.len() as u32;
        self.pool.push(value);
        self.pool_index.insert(index, slot);
        Ok(Entry::Const(slot))
    }

    // ---- function entry ----

    fn prologue(&mut self) -> Result<(), LowerError> {
        // An async function returns its promise; the body runs
        // as a resumption from here.
        if self.function.metadata().function_kind == FunctionKind::Async {
            self.emit(|b| b.op(Opcode::AsyncStart));
        }
        if let Some((spill, slots)) = self.layout.spill {
            let spill = Self::r8(spill)?;
            self.emit(|b| b.a_count(Opcode::NewSpill, spill, slots));
        }
        if self.is_script {
            // Every name is checked before any binding is made,
            // in GlobalDeclarationInstantiation's passes (16.1.7 steps 3, 4,
            // 10 and 11): the lexical names, then every var and function
            // name against the lexical declarations, then whether the
            // global object can take each function, then each var.
            use oxide_core::bytecode::opcode::global;
            let mut declarations = Vec::new();
            let eval = self.global_flags();
            for index in 0..self.function.closure_variables().len() {
                let variable = self.function.closure_variables()[index];
                match variable.source {
                    ClosureSource::GlobalDeclaration => {
                        let key = self.closure_key(index as u16)?;
                        let declare = match (variable.is_lexical, variable.is_const) {
                            (false, _) => Opcode::DeclareGlobal,
                            (true, false) => Opcode::DeclareGlobalLet,
                            (true, true) => Opcode::DeclareGlobalConst,
                        };
                        let flags = if variable.kind == ClosureVariableKind::GlobalFunction {
                            eval | global::FUNCTION
                        } else {
                            eval
                        };
                        declarations.push((declare, flags, key));
                    }
                    ClosureSource::Global => {}
                    _ => return Err(LowerError::unsupported("script closure source")),
                }
            }
            let checks = [
                (|d: Opcode, _: u8| d != Opcode::DeclareGlobal) as fn(Opcode, u8) -> bool,
                |d, _| d == Opcode::DeclareGlobal,
                |d, f| d == Opcode::DeclareGlobal && f & global::FUNCTION != 0,
                |d, f| d == Opcode::DeclareGlobal && f & global::FUNCTION == 0,
            ];
            for (pass, selected) in checks.iter().enumerate() {
                for &(declare, flags, key) in &declarations {
                    if !selected(declare, flags) {
                        continue;
                    }
                    let (check, flags) = match pass {
                        0 => (Opcode::CheckGlobalLexical, flags),
                        1 => (Opcode::CheckGlobalVar, flags | global::LEXICAL_CONFLICT),
                        _ => (Opcode::CheckGlobalVar, flags | global::DEFINABLE),
                    };
                    self.emit(|b| b.raw(oxide_core::Insn::new(check, 0, flags, 0, key.raw())));
                }
            }
            for (declare, flags, key) in declarations {
                self.emit(|b| b.raw(oxide_core::Insn::new(declare, 0, flags, 0, key.raw())));
            }
        }
        let metadata = self.function.metadata();
        if let Some(local) = metadata.function_name_local {
            let register = self.local_register(local);
            self.on_local(register, false, true, |b, a| b.a(Opcode::LoadFunction, a))?;
        }
        for argument in 0..self.captured_arguments.len() {
            if self.captured_arguments[argument] {
                let register = Self::r8(self.argument_register(argument as u16))?;
                self.emit(|b| b.ab(Opcode::NewCell, register, register));
            }
        }
        for local in 0..self.captured_locals.len() {
            let register = self.local_register(local as u16);
            let lexical = self.function.local_definitions()[local].is_lexical;
            match (self.captured_locals[local], lexical) {
                (true, true) => {
                    self.on_local(register, false, true, |b, a| b.a(Opcode::NewCellEmpty, a))?
                }
                (true, false) => {
                    self.on_local(register, true, true, |b, a| b.ab(Opcode::NewCell, a, a))?
                }
                (false, true) => {
                    self.on_local(register, false, true, |b, a| b.a(Opcode::LoadEmpty, a))?
                }
                (false, false) => continue,
            };
        }
        Ok(())
    }

    // ---- the walk ----

    fn body(&mut self) -> Result<(), LowerError> {
        let mut pc = 0;
        while pc < self.code.len() {
            let Some(depth) = self.analysis.depth[pc] else {
                pc += 1;
                continue;
            };
            self.pc = pc;
            self.next_scratch = 0;
            if self.analysis.label[pc] {
                self.materialize_all()?;
                self.labels[pc] = Some(self.builder.pc());
                self.last_dest = None;
                self.checked.clear();
                // A Catch marker survives a join only above its region's
                // handler; every other entry is canonical.
                let markers: Vec<usize> = self
                    .analysis
                    .chain(pc)
                    .map(|(_, region)| region)
                    .filter(|region| !region.iterator)
                    .map(|region| region.marker_depth as usize)
                    .collect();
                self.stack.clear();
                for position in 0..depth as usize {
                    self.stack.push(if markers.contains(&position) {
                        Entry::Marker
                    } else {
                        Entry::Canon
                    });
                }
            } else if self.stack.len() != depth as usize {
                return Err(LowerError::Internal(format!(
                    "symbolic stack depth {} at {pc}, expected {depth}",
                    self.stack.len()
                )));
            }
            pc += self.instruction(pc)?;
        }
        Ok(())
    }

    /// Whether `pc` is reachable and not a join point, so the instruction
    /// before it may fuse with it.
    fn fusible(&self, pc: usize) -> bool {
        pc < self.code.len() && self.analysis.depth[pc].is_some() && !self.analysis.label[pc]
    }

    /// Lowers the instruction at `pc`; returns how many stack instructions
    /// it consumed.
    fn instruction(&mut self, pc: usize) -> Result<usize, LowerError> {
        use Instruction as I;
        let instruction = &self.code[pc];
        if let Some(compare) = Compare::of(instruction) {
            return self.compare(compare, pc);
        }
        if let Some(op) = unary(instruction) {
            if op == Opcode::IsUndefinedOrNull
                && self.fusible(pc + 1)
                && let I::IfTrue(target) = self.code[pc + 1]
            {
                let (_, [value]) = self.pop_operands::<1>()?;
                self.materialize_all()?;
                self.emit_jump(
                    |b| b.a_jump(Opcode::JUndefinedOrNull, value, 0),
                    target as usize,
                );
                return Ok(2);
            }
            let (_, [value]) = self.pop_operands::<1>()?;
            let dst = self.result_register()?;
            let at = self.emit(|b| b.ab(op, dst, value));
            self.push_result(at)?;
            return Ok(1);
        }
        if matches!(instruction, I::Add) {
            let extra = self.concat_chain(pc);
            if extra > 0 {
                return self.concat(pc, extra);
            }
        }
        if let Some((op, immediate)) = binary(instruction) {
            self.binary(op, immediate)?;
            return Ok(1);
        }
        match *instruction {
            I::Nop => {}

            // ---- pushes ----
            I::PushI32(value) => self.stack.push(Entry::Int(value)),
            I::PushConst(index) => {
                let entry = self.pool_entry(index)?;
                self.stack.push(entry);
            }
            I::Undefined => self.stack.push(Entry::Undefined),
            I::Null => self.stack.push(Entry::Null),
            I::PushFalse => self.stack.push(Entry::Bool(false)),
            I::PushTrue => self.stack.push(Entry::Bool(true)),
            I::PushThis => self.stack.push(Entry::Reg(0)),
            I::FClosure(index) => {
                let ordinal = *self
                    .child_ordinal
                    .get(&index)
                    .ok_or_else(|| LowerError::Internal("closure of a non-child".into()))?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_child(Opcode::MakeClosure, dst, ordinal));
                self.push_result(at)?;
            }
            I::Object => {
                let slots = self.literal_slots(pc);
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_count(Opcode::NewObject, dst, slots));
                self.push_result(at)?;
            }
            I::ArrayFrom(count) => {
                let count = usize::from(count);
                let base = self
                    .stack
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
                if count > 0 && self.spilled(base + count - 1) {
                    return self.array_from_slots(base).map(|()| 1);
                }
                for depth in base..self.stack.len() {
                    self.materialize(depth)?;
                }
                self.stack.truncate(base);
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab_range(Opcode::NewArray, dst, dst, count as u32));
                self.push_result(at)?;
            }

            // ---- locals and parameters ----
            I::GetLocal(local) => self.get_local(local, false)?,
            I::GetLocalCheck(local) => self.get_local(local, true)?,
            I::PutLocal(local) | I::InitializeLocal(local) => {
                self.put_local(local, false, false)?
            }
            I::SetLocal(local) => self.put_local(local, true, false)?,
            I::PutLocalCheck(local) => self.put_local(local, false, true)?,
            I::SetLocalCheck(local) => self.put_local(local, true, true)?,
            I::SetLocalUninitialized(local) => {
                let register = self.local_register(local);
                self.spill_aliases(register)?;
                self.checked.remove(&register);
                // QuickJS's `set_loc_uninitialized` resets the slot a closure
                // may already hold (a hoisted function's capture); a fresh
                // binding comes from `close_loc`.
                if self.captured_locals[local as usize] {
                    self.on_local(register, true, false, |b, a| b.a(Opcode::ClearCell, a))?;
                } else {
                    self.on_local(register, false, true, |b, a| b.a(Opcode::LoadEmpty, a))?;
                }
            }
            I::CloseLocal(local) => {
                if self.captured_locals[local as usize] {
                    let register = self.local_register(local);
                    self.on_local(register, true, true, |b, a| b.a(Opcode::CloseCell, a))?;
                }
            }
            I::GetArg(argument) => {
                let register = self.argument_register(argument);
                if self.captured_arguments[argument as usize] {
                    self.load_cell(register)?;
                } else {
                    self.stack.push(Entry::Reg(register));
                }
            }
            I::PutArg(argument) | I::SetArg(argument) => {
                let keep = matches!(instruction, I::SetArg(_));
                let register = self.argument_register(argument);
                let captured = self.captured_arguments[argument as usize];
                self.store(register, captured, keep)?;
            }

            // ---- captured bindings of enclosing functions ----
            I::GetVarRef(closure) => self.load_captured(closure)?,
            I::GetVarRefCheck(closure) => {
                self.load_captured(closure)?;
                if self.checked.insert(CAPTURE_BIT | u32::from(closure)) {
                    let name = self.closure_key(closure)?;
                    let value = self.operand(self.stack.len() - 1)?;
                    self.emit(|b| b.a_key(Opcode::CheckInit, value, name));
                }
            }
            I::PutVarRef(closure) | I::InitializeVarRef(closure) => {
                self.store_captured(closure, false)?
            }
            I::SetVarRef(closure) => self.store_captured(closure, true)?,
            I::PutVarRefCheck(closure) => {
                self.check_captured_init(closure)?;
                self.store_captured(closure, false)?;
            }

            // ---- globals ----
            I::GetVar(closure) => self.global_read(Opcode::GetGlobal, closure)?,
            I::GetVarUndef(closure) => self.global_read(Opcode::GetGlobalOrUndefined, closure)?,
            I::DeleteVar(closure) => self.global_read(Opcode::DeleteGlobal, closure)?,
            I::PutVar(closure) => self.global_write(Opcode::SetGlobal, closure)?,
            I::PutVarInit(closure) => self.global_write(Opcode::DefineGlobal, closure)?,

            // ---- properties ----
            I::GetField(name) => {
                let key = self.name_key(name)?;
                let (_, [object]) = self.pop_operands::<1>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab_key(Opcode::GetField, dst, object, key));
                self.push_result(at)?;
            }
            I::GetField2(name) => {
                let key = self.name_key(name)?;
                let object = self.operand(self.stack.len() - 1)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab_key(Opcode::GetField, dst, object, key));
                self.push_result(at)?;
            }
            I::GetArrayEl => {
                let (_, [object, key]) = self.pop_operands::<2>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::GetElem, dst, object, key));
                self.push_result(at)?;
            }
            I::GetArrayEl2 => {
                let (_, [key]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::GetElem, dst, object, key));
                self.push_result(at)?;
            }
            I::GetArrayEl3 => {
                let (_, [raw]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                let converted = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::ToPropertyKeyOf, converted, raw, object));
                self.push_result(at)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::GetElem, dst, object, converted));
                self.push_result(at)?;
            }
            I::PutField(name) => {
                let key = self.name_key(name)?;
                let (_, [object, value]) = self.pop_operands::<2>()?;
                if object == 0 {
                    self.this_keys.insert(key.raw());
                }
                self.emit(|b| b.ab_key(Opcode::SetField, object, value, key));
            }
            I::PutArrayEl => {
                let (_, [object, key, value]) = self.pop_operands::<3>()?;
                self.emit(|b| b.abc(Opcode::SetElem, object, key, value));
            }
            I::DefineField(name) => {
                let key = self.name_key(name)?;
                let (_, [value]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.ab_key(Opcode::DefineField, object, value, key));
            }
            I::DefineArrayEl => {
                let (_, [value]) = self.pop_operands::<1>()?;
                let index = self.operand(self.stack.len() - 1)?;
                let object = self.operand(self.stack.len() - 2)?;
                self.emit(|b| b.abc(Opcode::DefineElem, object, index, value));
            }
            I::DefineMethod {
                key,
                kind,
                enumerable,
            } => {
                let key = self.name_key(key)?;
                let (_, [function]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                let flags = method_flags(kind, enumerable);
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::DefineMethod,
                        object,
                        function,
                        flags as u8,
                        key.raw(),
                    ))
                });
            }
            I::DefineMethodComputed { kind, enumerable } => {
                let (_, [key, function]) = self.pop_operands::<2>()?;
                let object = self.operand(self.stack.len() - 1)?;
                let flags = method_flags(kind, enumerable);
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::DefineMethodComputed,
                        object,
                        key,
                        function,
                        flags,
                    ))
                });
            }
            I::DefineFieldComputed => {
                let (_, [key, value]) = self.pop_operands::<2>()?;
                let object = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.abc(Opcode::DefineElem, object, key, value));
            }
            I::SetName(_)
                if pc > 0
                    && matches!(self.code[pc - 1],
                        I::FClosure(child)
                            if self.named_children.get(child as usize) == Some(&true)) => {}
            I::SetName(name) => {
                let entry = self.pool_entry(name)?;
                let scratch = self.scratch()?;
                self.load(u32::from(scratch), entry)?;
                let function = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.ab(Opcode::SetFunctionName, function, scratch));
            }
            I::SetNameComputed => {
                let function = self.operand(self.stack.len() - 1)?;
                let key = self.operand(self.stack.len() - 2)?;
                self.emit(|b| b.ab(Opcode::SetFunctionName, function, key));
            }
            I::Delete => {
                let (_, [object, key]) = self.pop_operands::<2>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::DeleteElem, dst, object, key));
                self.push_result(at)?;
            }

            // ---- updates ----
            I::PostInc | I::PostDec => {
                let op = if matches!(instruction, I::PostInc) {
                    Opcode::Inc
                } else {
                    Opcode::Dec
                };
                // `x++` as a statement: the old value is stored nowhere and
                // dropped, and `Inc` converts its operand itself, so the
                // update is one instruction once the store retargets it.
                if self.fusible(pc + 1)
                    && self.fusible(pc + 2)
                    && matches!(
                        self.code[pc + 1],
                        I::PutLocal(_) | I::PutLocalCheck(_) | I::PutArg(_)
                    )
                    && matches!(self.code[pc + 2], I::Drop)
                {
                    let (_, [value]) = self.pop_operands::<1>()?;
                    let new = self.result_register()?;
                    let at = self.emit(|b| b.ab(op, new, value));
                    self.push_result(at)?;
                    self.pc = pc + 1;
                    self.instruction(pc + 1)?;
                    return Ok(3);
                }
                let (_, [value]) = self.pop_operands::<1>()?;
                let old = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::ToNumeric, old, value));
                self.push_result(at)?;
                let new = self.result_register()?;
                let at = self.emit(|b| b.ab(op, new, old));
                self.push_result(at)?;
            }

            // ---- stack shuffles ----
            I::Drop => {
                self.pop()?;
            }
            I::Nip => self.permute(2, &[1])?,
            I::Swap => self.permute(2, &[1, 0])?,
            I::Dup => self.permute(1, &[0, 0])?,
            I::Dup1 => self.permute(2, &[0, 0, 1])?,
            I::Dup3 => self.permute(3, &[0, 1, 2, 0, 1, 2])?,
            I::Insert2 => self.permute(2, &[1, 0, 1])?,
            // `o[k] = v` as a statement: the copy of `v` kept as the
            // assignment's value is dropped right after the store, so the
            // store takes the three operands where they are.
            I::Insert3
                if self.fusible(pc + 1)
                    && self.fusible(pc + 2)
                    && matches!(self.code[pc + 1], I::PutArrayEl)
                    && matches!(self.code[pc + 2], I::Drop) =>
            {
                self.pc = pc + 1;
                self.instruction(pc + 1)?;
                return Ok(3);
            }
            I::Insert3 => self.permute(3, &[2, 0, 1, 2])?,
            I::Insert4 => self.permute(4, &[3, 0, 1, 2, 3])?,
            I::Perm3 => self.permute(3, &[1, 0, 2])?,
            I::Perm4 => self.permute(4, &[2, 0, 1, 3])?,
            I::Perm5 => self.permute(5, &[3, 0, 1, 2, 4])?,
            I::Rot4Left => self.permute(4, &[1, 2, 3, 0])?,

            // ---- control ----
            I::Goto(target) => {
                self.materialize_all()?;
                // A jump over unreachable code only is a fallthrough.
                let target = target as usize;
                let skipped = pc + 1..target;
                if target <= pc && self.invert_back_edge(pc, target) {
                    // The loop's test repeated at its back edge.
                } else if target <= pc
                    || skipped.clone().any(|at| self.analysis.depth[at].is_some())
                {
                    self.emit_jump(|b| b.jump(Opcode::Jmp, 0), target);
                }
            }
            I::IfFalse(target) | I::IfTrue(target) => {
                let op = if matches!(instruction, I::IfTrue(_)) {
                    Opcode::JTrue
                } else {
                    Opcode::JFalse
                };
                let (_, [condition]) = self.pop_operands::<1>()?;
                self.materialize_all()?;
                self.emit_jump(|b| b.a_jump(op, condition, 0), target as usize);
            }
            I::Catch(_) => {
                self.materialize_all()?;
                self.stack.push(Entry::Marker);
            }
            I::DropCatch => {
                let (_, entry) = self.pop()?;
                if entry != Entry::Marker {
                    return Err(LowerError::Internal("DropCatch without a marker".into()));
                }
            }
            I::NipCatch => {
                let (_, region) = self
                    .analysis
                    .chain(pc)
                    .next()
                    .ok_or_else(|| LowerError::Internal("NipCatch outside a region".into()))?;
                let marker = region.marker_depth as usize;
                let (depth, value) = self.pop()?;
                self.stack.truncate(marker);
                if value == Entry::Canon {
                    self.mov(self.temp(marker), self.temp(depth))?;
                }
                self.stack.push(value);
            }
            I::Gosub(target) => {
                self.materialize_all()?;
                let dst = self.result_register()?;
                self.emit_storing_jump(|b| b.a_jump(Opcode::Gosub, dst, 0), dst, target as usize);
            }
            I::Ret => {
                let (_, [address]) = self.pop_operands::<1>()?;
                self.emit(|b| b.a(Opcode::Ret, address));
            }
            I::DropGosub => {
                self.pop()?;
            }
            I::Return => {
                let (_, [value]) = self.pop_operands::<1>()?;
                self.emit(|b| b.a(Opcode::Return, value));
            }
            I::Throw => {
                let (_, [value]) = self.pop_operands::<1>()?;
                self.emit(|b| b.a(Opcode::Throw, value));
            }
            I::Import => {
                let (_, [specifier, options]) = self.pop_operands::<2>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::ImportCall, dst, specifier, options));
                self.push_result(at)?;
            }
            I::InitializeModuleImportCollision(closure) => self.store_captured(closure, false)?,
            I::ThrowDeleteSuper => {
                let (_, _) = self.pop_operands::<3>()?;
                let message = self.static_key("unsupported reference to 'super'")?;
                self.emit(|b| b.key(Opcode::ThrowReference, message));
                self.stack.push(Entry::Undefined);
            }
            I::ThrowReadOnly(name) => {
                let key = self.name_key(name)?;
                self.emit(|b| b.key(Opcode::ThrowConstAssign, key));
            }

            // ---- arguments, rest, construction, for-in, spread and eval ----
            I::Arguments(kind) => {
                let dst = self.result_register()?;
                let mapped = i32::from(kind == ArgumentsKind::Mapped);
                let at = self.emit(|b| b.a_int(Opcode::NewArguments, dst, mapped));
                self.push_result(at)?;
            }
            I::Rest(start) => {
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_int(Opcode::Rest, dst, i32::from(start)));
                self.push_result(at)?;
            }
            I::PushNewTarget => {
                let register = self.layout.new_target.ok_or_else(|| {
                    LowerError::Internal("new.target without its register".into())
                })?;
                self.stack.push(Entry::Reg(register));
            }
            I::CheckCtor => {
                let register = self
                    .layout
                    .new_target
                    .ok_or_else(|| LowerError::Internal("CheckCtor without new.target".into()))?;
                let register = Self::r8(register)?;
                self.emit(|b| b.a(Opcode::CheckCtor, register));
            }
            I::ToObject => {
                let (_, [value]) = self.pop_operands::<1>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::ToObject, dst, value));
                self.push_result(at)?;
            }
            I::SetProto => {
                let (_, [value]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.ab(Opcode::SetProto, object, value));
            }
            I::PushActiveFunction => {
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a(Opcode::LoadFunction, dst));
                self.push_result(at)?;
            }
            I::PushHomeObject => {
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a(Opcode::LoadHomeObject, dst));
                self.push_result(at)?;
            }
            I::RegExp(index) => {
                let slot = self.regexp_constants(index)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_const(Opcode::NewRegExp, dst, slot));
                self.push_result(at)?;
            }
            I::ForInStart => {
                let (_, [source]) = self.pop_operands::<1>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::ForInStart, dst, source));
                self.push_result(at)?;
            }
            I::ForInNext => return self.for_in_next(pc),
            I::Append => {
                // `array index iterable -> array index`, in place.
                let base = self.stack.len() - 3;
                for depth in base..self.stack.len() {
                    self.materialize(depth)?;
                }
                let (array, index, source) = (
                    self.operand(base)?,
                    self.operand(base + 1)?,
                    self.operand(base + 2)?,
                );
                self.emit(|b| b.abc(Opcode::AppendSpread, array, index, source));
                self.written(base + 1, index)?;
                self.stack.truncate(base + 2);
            }
            I::Apply(kind) => {
                let op = match kind {
                    ApplyKind::Call => Opcode::CallSpread,
                    ApplyKind::Construct => Opcode::NewSpread,
                };
                self.call(op, 0, 3)?;
            }
            // A direct `eval` is a call of whatever `eval` resolves to, which
            // runs as indirect eval (a dropped feature).
            I::Eval { argument_count, .. } => {
                let at = self.call(Opcode::Call, argument_count, 1)?;
                self.direct_eval_sites.push(at);
            }
            I::ApplyEval { .. } => {
                // `CallSpread` takes the callee, `this` and the argument array.
                let base = self.stack.len() - 2;
                for depth in base..self.stack.len() {
                    self.materialize(depth)?;
                }
                self.mov(self.temp(base + 2), self.temp(base + 1))?;
                self.stack[base + 1] = Entry::Undefined;
                self.push_canon(1);
                self.call(Opcode::CallSpread, 0, 3)?;
            }
            // A function with a direct `eval` has no dynamic binding: the
            // eval throws before it could create one.
            I::VariableEnvironment | I::GetDynamicBinding { .. } => {
                self.stack.push(Entry::Undefined)
            }
            I::HasDynamicBinding { .. } => self.stack.push(Entry::Bool(false)),
            // Reached only past a `HasDynamicBinding`, which is false.
            I::PutDynamicBinding { .. } => {
                self.pop_operands::<1>()?;
            }
            I::DynamicEnvironmentObject(_) => self.stack.push(Entry::Undefined),
            I::DeleteDynamicBinding { .. } => self.stack.push(Entry::Bool(true)),

            // ---- iteration ----
            I::ForOfStart | I::IteratorStart => {
                let generic = i32::from(matches!(instruction, I::IteratorStart));
                let (depth, [iterable]) = self.pop_operands::<1>()?;
                let record = self.temps_window(depth, 3, 0)?;
                self.emit(|b| b.ab_int(Opcode::IterStart, record, iterable, generic));
                self.temps_written(record, depth, 0..3)?;
                self.push_canon(3);
            }
            I::ForOfNext(offset) => {
                let top = self.stack.len();
                let base = top
                    .checked_sub(3 + usize::from(offset))
                    .ok_or_else(|| LowerError::Internal("ForOfNext without a record".into()))?;
                let record = self.temps_window(base, 3, 3)?;
                let value = self.dest(top)?;
                let done = self.dest(top + 1)?;
                self.mark_record_done(record, base)?;
                self.emit(|b| b.abc(Opcode::IterNext, value, record, done));
                self.temps_written(record, base, 2..3)?;
                self.written(top, value)?;
                self.written(top + 1, done)?;
                self.push_canon(2);
            }
            I::IteratorClose => {
                let base = self.record_base(3)?;
                let record = self.temps_window(base, 3, 3)?;
                self.mark_record_done(record, base)?;
                self.emit(|b| b.a(Opcode::IterClose, record));
                self.stack.truncate(base);
            }
            I::IteratorClosePreserve | I::IteratorDropPreserve | I::IteratorDetachPreserve => {
                let region = self.innermost_iterator(pc)?;
                let value = self.operand(self.stack.len() - 1)?;
                let record = self.temps_window(region, 3, 3)?;
                if matches!(instruction, I::IteratorClosePreserve) {
                    self.mark_record_done(record, region)?;
                    self.emit(|b| b.a(Opcode::IterClose, record));
                }
                self.stack.truncate(region);
                if matches!(instruction, I::IteratorDetachPreserve) {
                    let moves = vec![
                        (self.temp(region), u32::from(value)),
                        (self.temp(region + 1), self.temp(region)),
                    ];
                    self.parallel_move(moves)?;
                    self.push_canon(2);
                } else {
                    self.mov(self.temp(region), u32::from(value))?;
                    self.push_canon(1);
                }
            }
            I::IteratorNext => {
                let top = self.stack.len();
                let value = self.operand(top - 1)?;
                let record = self.temps_window(top - 4, 3, 3)?;
                let dst = self.dest(top - 1)?;
                self.emit(|b| b.abc(Opcode::IterCallNext, dst, record, value));
                self.written(top - 1, dst)?;
                self.stack[top - 1] = Entry::Canon;
            }
            I::IteratorCall(kind) => {
                let top = self.stack.len();
                let value = self.operand(top - 1)?;
                let record = self.temps_window(top - 4, 3, 3)?;
                // The result and whether the method was missing.
                let dst = self.temps_window(top - 1, 2, 0)?;
                let kind = match kind {
                    IteratorCallKind::ReturnWithValue => 0,
                    IteratorCallKind::ThrowWithValue => 1,
                    IteratorCallKind::ReturnWithoutValue => 2,
                };
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::IterCall,
                        dst,
                        record,
                        value,
                        kind,
                    ))
                });
                self.temps_written(dst, top - 1, 0..2)?;
                self.stack[top - 1] = Entry::Canon;
                self.push_canon(1);
            }
            I::IteratorCheckObject => {
                let value = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.a(Opcode::IterCheckObject, value));
            }
            I::ThrowIteratorMissingThrow => {
                let message = self.static_key("iterator does not have a throw method")?;
                self.emit(|b| b.key(Opcode::ThrowType, message));
            }

            I::ForAwaitOfStart | I::AsyncIteratorStart => {
                let (depth, [iterable]) = self.pop_operands::<1>()?;
                let record = self.temps_window(depth, 3, 0)?;
                self.emit(|b| b.ab(Opcode::IterStartAsync, record, iterable));
                self.temps_written(record, depth, 0..3)?;
                self.push_canon(3);
            }
            I::ForAwaitOfNext => {
                let top = self.stack.len();
                let record = self.temps_window(top - 3, 3, 3)?;
                let dst = self.dest(top)?;
                self.mark_record_done(record, top - 3)?;
                self.emit(|b| b.ab(Opcode::IterNextRaw, dst, record));
                self.temps_written(record, top - 3, 2..3)?;
                self.written(top, dst)?;
                self.push_canon(1);
            }
            I::IteratorGetValueDone => {
                let top = self.stack.len();
                let result = self.canonical(top - 1)?;
                let record = self.temps_window(top - 4, 3, 3)?;
                let done = self.dest(top)?;
                self.mark_record_done(record, top - 4)?;
                self.emit(|b| b.abc(Opcode::IterResult, result, record, done));
                self.temps_written(record, top - 4, 2..3)?;
                self.written(top - 1, result)?;
                self.written(top, done)?;
                self.push_canon(1);
            }

            // ---- suspension ----
            I::InitialYield => {
                self.emit(|b| b.op(Opcode::InitialYield));
            }
            I::Yield | I::YieldStar | I::AsyncYieldStar => {
                let op = if matches!(instruction, I::Yield) {
                    Opcode::Yield
                } else {
                    Opcode::YieldStar
                };
                let top = self.stack.len();
                self.materialize(top - 1)?;
                // A resumption writes the value and its kind.
                let value = self.temps_window(top - 1, 2, 1)?;
                self.materialize_all()?;
                self.emit(|b| b.a(op, value));
                self.temps_written(value, top - 1, 0..2)?;
                self.push_canon(1);
            }
            I::Await => {
                let top = self.stack.len();
                let value = self.canonical(top - 1)?;
                self.materialize_all()?;
                self.emit(|b| b.a(Opcode::Await, value));
                self.written(top - 1, value)?;
                self.stack[top - 1] = Entry::Canon;
                self.last_dest = None;
            }

            // ---- classes ----
            I::DefineClass { name, has_heritage } => {
                let key = self.name_key(name)?;
                let base = self.stack.len() - 2;
                self.materialize(base)?;
                self.materialize(base + 1)?;
                let (parent, closure) = (self.operand(base)?, self.operand(base + 1)?);
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::MakeClass,
                        parent,
                        closure,
                        u8::from(has_heritage),
                        key.raw(),
                    ))
                });
                // The constructor and its prototype replace them.
                self.written(base, parent)?;
                self.written(base + 1, closure)?;
            }
            I::InstallClassInstanceInitializer => {
                let (_, [initializer]) = self.pop_operands::<1>()?;
                let top = self.stack.len();
                let constructor = self.operand(top - 2)?;
                let prototype = self.operand(top - 1)?;
                self.emit(|b| b.abc(Opcode::InstallFields, constructor, prototype, initializer));
            }
            I::CallClassInstanceInitializer => {
                let (_, [constructor]) = self.pop_operands::<1>()?;
                let receiver = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.ab(Opcode::InitFields, receiver, constructor));
            }
            I::RunClassStaticInitializer => {
                let (_, [initializer]) = self.pop_operands::<1>()?;
                let constructor = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.ab(Opcode::RunStaticInit, constructor, initializer));
            }
            I::CallClassStaticBlock => {
                let (_, [block]) = self.pop_operands::<1>()?;
                self.emit(|b| b.a(Opcode::CallStaticBlock, block));
            }
            I::GetSuper => {
                let (_, [home]) = self.pop_operands::<1>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::GetSuper, dst, home));
                self.push_result(at)?;
            }
            I::GetSuperValue | I::GetSuperValueForCall => {
                let base = self.stack.len() - 3;
                for depth in base..self.stack.len() {
                    self.materialize(depth)?;
                }
                let first = self.temps_window(base, 3, 3)?;
                let for_call = matches!(instruction, I::GetSuperValueForCall);
                let keep = if for_call { base + 1 } else { base };
                let dst = self.dest(keep)?;
                self.emit(|b| b.ab_int(Opcode::GetSuperValue, dst, first, i32::from(for_call)));
                self.written(keep, dst)?;
                self.stack.truncate(keep);
                self.stack.push(Entry::Canon);
            }
            I::PutSuperValue => {
                let base = self.stack.len() - 4;
                for depth in base..self.stack.len() {
                    self.materialize(depth)?;
                }
                let first = self.temps_window(base, 4, 4)?;
                self.emit(|b| b.a(Opcode::PutSuperValue, first));
                self.stack.truncate(base);
            }
            I::MarkSuperCall => {}
            I::ConstructSuper(argc) => {
                self.call(Opcode::New, argc, 2)?;
            }
            I::ApplySuper => {
                self.call(Opcode::NewSpread, 0, 3)?;
            }
            I::InitDerivedConstructor => {
                let new_target = self.new_target_register()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::InitDerived, dst, new_target));
                self.push_result(at)?;
            }
            I::InitializeDerivedLocal(local) => {
                let register = self.local_register(local);
                let (_, [value]) = self.pop_operands::<1>()?;
                if self.captured_locals[local as usize] {
                    let cell = self.cell_operand(register)?;
                    self.emit(|b| b.ab(Opcode::InitThisCell, cell, value));
                } else {
                    let register = Self::r8(register)?;
                    self.spill_aliases(u32::from(register))?;
                    self.emit(|b| b.ab(Opcode::InitThis, register, value));
                }
            }
            I::InitializeDerivedVarRef(closure) => {
                let (_, [value]) = self.pop_operands::<1>()?;
                self.emit(|b| b.a_capture(Opcode::InitThisCaptured, value, u32::from(closure)));
            }
            I::ReturnDerived(local) => {
                let top = self.stack.len();
                let value = self.canonical(top - 1)?;
                self.get_local(local, false)?;
                let this = self.operand(self.stack.len() - 1)?;
                self.stack.pop();
                self.emit(|b| b.ab(Opcode::CheckDerivedReturn, value, this));
                self.stack.pop();
                self.emit(|b| b.a(Opcode::Return, value));
            }

            // ---- private names ----
            I::InitializePrivateName(local) => {
                let name = self.local_key(local)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_key(Opcode::NewPrivateName, dst, name));
                self.push_result(at)?;
                self.put_local(local, false, false)?;
            }
            I::InitializePrivateMethod(local) | I::InitializePrivateAccessor(local) => {
                let top = self.stack.len();
                let home = self.operand(top - 2)?;
                let method = self.operand(top - 1)?;
                self.emit(|b| b.ab(Opcode::SetHomeObject, method, home));
                let name = self.local_key(local)?;
                let prefix = match self.function.local_definitions()[usize::from(local)].kind {
                    ClosureVariableKind::PrivateGetter
                    | ClosureVariableKind::PrivateGetterSetter => 1,
                    ClosureVariableKind::PrivateSetter => 2,
                    _ => 0,
                };
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::NamePrivateMethod,
                        method,
                        0,
                        prefix,
                        name.raw(),
                    ))
                });
                self.put_local(local, false, false)?;
            }
            I::GetPrivateField(source) | I::GetPrivateField2(source) => {
                let keep = matches!(instruction, I::GetPrivateField2(_));
                let kind = self.private_kind(source)?;
                self.load_private(source)?;
                let (_, [name]) = self.pop_operands::<1>()?;
                let target = self.operand(self.stack.len() - 1)?;
                if !keep {
                    self.stack.pop();
                }
                let dst = self.result_register()?;
                let op = match kind {
                    ClosureVariableKind::PrivateMethod => Opcode::GetPrivateMethod,
                    ClosureVariableKind::PrivateGetter
                    | ClosureVariableKind::PrivateGetterSetter => Opcode::CallPrivateGetter,
                    ClosureVariableKind::PrivateSetter => {
                        let message = self.static_key("private class field has no getter")?;
                        self.emit(|b| b.key(Opcode::ThrowType, message));
                        self.stack.push(Entry::Undefined);
                        return Ok(1);
                    }
                    _ => Opcode::GetPrivate,
                };
                let at = self.emit(|b| b.abc(op, dst, target, name));
                self.push_result(at)?;
            }
            I::PutPrivateField(source) | I::DefinePrivateField(source) => {
                let define = matches!(instruction, I::DefinePrivateField(_));
                let kind = self.private_kind(source)?;
                self.load_private(source)?;
                let (_, [value, name]) = self.pop_operands::<2>()?;
                let target = self.operand(self.stack.len() - 1)?;
                if !define {
                    self.stack.pop();
                }
                let op = match kind {
                    _ if define => Opcode::DefinePrivate,
                    ClosureVariableKind::PrivateSetter => Opcode::CallPrivateSetter,
                    ClosureVariableKind::PrivateField => Opcode::PutPrivate,
                    _ => {
                        let message = self.static_key("private class field is read-only")?;
                        self.emit(|b| b.key(Opcode::ThrowType, message));
                        return Ok(1);
                    }
                };
                self.emit(|b| b.abc(op, target, value, name));
            }
            I::PrivateIn(source) => {
                let kind = self.private_kind(source)?;
                self.load_private(source)?;
                let (_, [target, name]) = self.pop_operands::<2>()?;
                let dst = self.result_register()?;
                let op = if kind == ClosureVariableKind::PrivateField {
                    Opcode::PrivateIn
                } else {
                    Opcode::PrivateBrandIn
                };
                let at = self.emit(|b| b.abc(op, dst, target, name));
                self.push_result(at)?;
            }

            // ---- object spread and references ----
            I::CopyDataProperties => {
                let (_, [source]) = self.pop_operands::<1>()?;
                let target = self.operand(self.stack.len() - 1)?;
                self.emit(|b| b.ab(Opcode::CopyData, target, source));
            }
            I::CopyDataPropertiesExcluded {
                target_depth,
                source_depth,
                excluded_depth,
            } => {
                let top = self.stack.len();
                let at = |depth: u8| top - 1 - usize::from(depth);
                let target = self.operand(at(target_depth))?;
                let source = self.operand(at(source_depth))?;
                let excluded = self.operand(at(excluded_depth))?;
                self.emit(|b| b.abc(Opcode::CopyDataExcluded, target, source, excluded));
            }
            I::GlobalReference(closure) => {
                let key = self.closure_key(closure)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_key(Opcode::GlobalRef, dst, key));
                self.push_result(at)?;
            }
            I::GetRefValue(name) | I::GetRefValueUndef(name) => {
                let key = self.name_key(name)?;
                let undefined = u8::from(matches!(instruction, I::GetRefValueUndef(_)));
                let environment = self.operand(self.stack.len() - 1)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::GetRef,
                        dst,
                        environment,
                        undefined,
                        key.raw(),
                    ))
                });
                self.push_result(at)?;
            }
            I::PutRefValue(name) => {
                let key = self.name_key(name)?;
                let (_, [environment, value]) = self.pop_operands::<2>()?;
                self.emit(|b| b.ab_key(Opcode::PutRef, environment, value, key));
            }

            // ---- calls ----
            I::Call(argc) => {
                self.call(Opcode::Call, argc, 1)?;
            }
            I::CallMethod(argc) => {
                self.call(Opcode::CallMethod, argc, 2)?;
            }
            I::Construct(argc) => {
                self.call(Opcode::New, argc, 2)?;
            }

            _ => return Err(unsupported_instruction(instruction)),
        }
        Ok(1)
    }

    fn compare(&mut self, compare: Compare, pc: usize) -> Result<usize, LowerError> {
        let branch = if self.fusible(pc + 1) {
            match self.code[pc + 1] {
                Instruction::IfTrue(target) => Some((true, target as usize)),
                Instruction::IfFalse(target) => Some((false, target as usize)),
                _ => None,
            }
        } else {
            None
        };
        let Some((taken_when, target)) = branch else {
            let (_, [left, right]) = self.pop_operands::<2>()?;
            let dst = self.result_register()?;
            let at = self.emit(|b| b.abc(compare.value(), dst, left, right));
            self.push_result(at)?;
            return Ok(1);
        };
        let top = self.stack.len();
        let short = |entry: Entry| match entry {
            Entry::Int(value) => i16::try_from(value).ok(),
            _ => None,
        };
        let (left, right) = (self.stack[top - 2], self.stack[top - 1]);
        // Order of evaluation is unaffected: a constant operand converts
        // without side effects, so `k < x` may become `x > k`.
        let (compare, register_depth, immediate) = match (short(left), short(right)) {
            (_, Some(value)) => (compare, top - 2, Some(value)),
            (Some(value), None) => (compare.mirrored(), top - 1, Some(value)),
            (None, None) => (compare, top - 2, None),
        };
        if let Some(value) = immediate {
            let register = self.operand(register_depth)?;
            self.stack.truncate(top - 2);
            self.materialize_all()?;
            let op = compare.jump(taken_when, true);
            self.emit_jump(|b| b.a_short_jump(op, register, value, 0), target);
        } else {
            let (_, [left, right]) = self.pop_operands::<2>()?;
            self.materialize_all()?;
            let op = compare.jump(taken_when, false);
            self.emit_jump(|b| b.ab_jump(op, left, right, 0), target);
        }
        Ok(2)
    }

    /// How many more operands the `Add` at `pc` starts a chain of string
    /// concatenations over: pushes of a primitive constant, each followed by
    /// an `Add`. A constant converts without side effects and cannot throw,
    /// so every conversion that can run code or throw is the first `Add`'s
    /// and is reported at its position, as it is without the chain. Zero
    /// unless one of the first two operands is a string constant, so that
    /// every `+` of the chain concatenates.
    fn concat_chain(&self, pc: usize) -> usize {
        let top = self.stack.len();
        // The operands sit in their own temporaries, which the deepest
        // stack of the function already has; past the window the chain is
        // left as `Add`s.
        if self.layout.temp_registers != u32::MAX {
            return 0;
        }
        let room = (self.analysis.max_depth as usize).saturating_sub(top);
        let string = |entry: Entry| match entry {
            Entry::Const(slot) => self.pool[slot as usize].as_string().is_some(),
            _ => false,
        };
        if top < 2 || !(string(self.stack[top - 2]) || string(self.stack[top - 1])) {
            return 0;
        }
        let plain = |instruction: &Instruction| match *instruction {
            Instruction::PushI32(_) => true,
            Instruction::PushConst(index) => self
                .function
                .constants()
                .get(index as usize)
                .is_some_and(|constant| constant.as_primitive().is_some()),
            _ => false,
        };
        let mut extra = 0;
        let mut at = pc + 1;
        while extra < room.min(CONCAT_OPERANDS - 2)
            && self.fusible(at)
            && self.fusible(at + 1)
            && plain(&self.code[at])
            && matches!(self.code[at + 1], Instruction::Add)
        {
            extra += 1;
            at += 2;
        }
        extra
    }

    /// Lowers the `Add` at `pc` and the `extra` pushes and `Add`s after it
    /// as one `ConcatN`.
    fn concat(&mut self, pc: usize, extra: usize) -> Result<usize, LowerError> {
        for index in 0..extra {
            self.pc = pc + 1 + 2 * index;
            self.instruction(self.pc)?;
        }
        self.pc = pc;
        let count = extra + 2;
        let base = self.stack.len() - count;
        for depth in base..self.stack.len() {
            self.materialize(depth)?;
        }
        let first = Self::r8(self.temp(base))?;
        self.stack.truncate(base);
        let dst = self.result_register()?;
        let at = self.emit(|b| b.ab_range(Opcode::ConcatN, dst, first, count as u32));
        self.push_result(at)?;
        Ok(1 + 2 * extra)
    }

    fn binary(&mut self, op: Opcode, immediate: Option<Opcode>) -> Result<(), LowerError> {
        let top = self.stack.len();
        if let (Some(immediate), Entry::Int(value)) = (immediate, self.stack[top - 1]) {
            let left = self.operand(top - 2)?;
            self.stack.truncate(top - 2);
            let dst = self.result_register()?;
            let at = self.emit(|b| b.ab_int(immediate, dst, left, value));
            self.push_result(at)?;
            return Ok(());
        }
        let (_, [left, right]) = self.pop_operands::<2>()?;
        let dst = self.result_register()?;
        let at = self.emit(|b| b.abc(op, dst, left, right));
        self.push_result(at)?;
        Ok(())
    }

    /// Lowers a call of `fixed` operands and `argc` arguments; returns the
    /// call's register PC.
    fn call(&mut self, op: Opcode, argc: u16, fixed: usize) -> Result<u32, LowerError> {
        let count = fixed + usize::from(argc);
        let base = self
            .stack
            .len()
            .checked_sub(count)
            .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
        if self.spilled(base + count - 1) {
            return self.call_from_slots(op, argc, fixed, base);
        }
        let argc = u8::try_from(argc)
            .map_err(|_| LowerError::unsupported("calls with more than 255 arguments"))?;
        // The callee window overlaps the operands in place, so they must sit
        // in their own temporaries; entries below the call are untouched.
        for depth in base..self.stack.len() {
            self.materialize(depth)?;
        }
        self.stack.truncate(base);
        let callee = Self::r8(self.temp(base))?;
        let at = self.emit(|b| b.call(op, callee, callee, argc));
        self.push_result(at)?;
        Ok(at)
    }

    /// A call whose window reaches the spill scope: on a run of scratch registers when it
    /// fits, else as a spread call over an array of the arguments
    /// (`CallSpread` copies a dense array without the iterator protocol).
    fn call_from_slots(
        &mut self,
        op: Opcode,
        argc: u16,
        fixed: usize,
        base: usize,
    ) -> Result<u32, LowerError> {
        let count = fixed + usize::from(argc);
        let dst = if count < SCRATCH as usize {
            let run = self.scratch_run(count)?;
            for (index, depth) in (base..self.stack.len()).enumerate() {
                self.copy_entry(u32::from(run) + index as u32, depth)?;
            }
            run
        } else {
            let (spread, receiver) = match op {
                Opcode::Call => (Opcode::CallSpread, None),
                Opcode::CallMethod => (Opcode::CallSpread, Some(base)),
                Opcode::New => (Opcode::NewSpread, Some(base + 1)),
                _ => {
                    return Err(LowerError::Internal(format!(
                        "a {} past the window",
                        op.name()
                    )));
                }
            };
            let callee = if op == Opcode::CallMethod {
                base + 1
            } else {
                base
            };
            // Callee, receiver or new.target, then the array.
            let run = self.scratch_run(3)?;
            self.array_of(u32::from(run) + 2, base + fixed)?;
            self.copy_entry(u32::from(run), callee)?;
            match receiver {
                Some(depth) => self.copy_entry(u32::from(run) + 1, depth)?,
                None => self.load(u32::from(run) + 1, Entry::Undefined)?,
            }
            self.stack.truncate(base);
            let at = self.emit(|b| b.call(spread, run, run, 0));
            self.written(base, run)?;
            self.stack.push(Entry::Canon);
            return Ok(at);
        };
        self.stack.truncate(base);
        let argc = argc as u8;
        let at = self.emit(|b| b.call(op, dst, dst, argc));
        self.written(base, dst)?;
        self.stack.push(Entry::Canon);
        Ok(at)
    }

    /// An array literal whose elements reach the spill scope.
    fn array_from_slots(&mut self, base: usize) -> Result<(), LowerError> {
        let count = self.stack.len() - base;
        let dst = if count < SCRATCH as usize {
            let run = self.scratch_run(count)?;
            for (index, depth) in (base..self.stack.len()).enumerate() {
                self.copy_entry(u32::from(run) + index as u32, depth)?;
            }
            self.emit(|b| b.ab_range(Opcode::NewArray, run, run, count as u32));
            run
        } else {
            let run = self.scratch_run(1)?;
            self.array_of(u32::from(run), base)?;
            run
        };
        self.stack.truncate(base);
        self.written(base, dst)?;
        self.stack.push(Entry::Canon);
        Ok(())
    }

    /// Builds in `dst` an array of the entries from `first` to the top, one
    /// element at a time: an empty `NewArray`, then a `DefineElem` per
    /// entry through the two scratch registers after `dst`'s run.
    fn array_of(&mut self, dst: u32, first: usize) -> Result<(), LowerError> {
        let array = Self::r8(dst)?;
        self.emit(|b| b.ab_range(Opcode::NewArray, array, array, 0));
        let (index, value) = (self.rotating()?, self.rotating()?);
        if index == array || value == array {
            return Err(LowerError::Internal(
                "an array's scratch registers overlap".into(),
            ));
        }
        for (position, depth) in (first..self.stack.len()).enumerate() {
            let position = i32::try_from(position)
                .map_err(|_| LowerError::unsupported("arrays of more than 2^31 elements"))?;
            self.emit(|b| b.a_int(Opcode::LoadInt, index, position));
            self.copy_entry(u32::from(value), depth)?;
            self.emit(|b| b.abc(Opcode::DefineElem, array, index, value));
        }
        Ok(())
    }

    /// The constant-pool slots of a regular-expression literal: its source,
    /// then its flags, as atoms.
    fn regexp_constants(&mut self, index: u32) -> Result<u32, LowerError> {
        let (pattern, program) = self
            .function
            .constants()
            .get(index as usize)
            .and_then(|constant| constant.as_regexp())
            .ok_or_else(|| LowerError::Internal(format!("constant {index} is not a RegExp")))?;
        let source = self.atom_value(pattern)?;
        let flags = CoreValue::string(
            self.runtime
                .heap_mut()
                .intern(&program.flags().canonical_string())?,
        );
        let slot = self.pool.len() as u32;
        self.pool.push(source);
        self.pool.push(flags);
        Ok(slot)
    }

    /// `ForInNext` and the `IfFalse` that consumes its done flag: one
    /// register instruction that stores the key and jumps to the loop body,
    /// or falls through when the enumeration is exhausted.
    fn for_in_next(&mut self, pc: usize) -> Result<usize, LowerError> {
        let target = match self.code.get(pc + 1) {
            Some(Instruction::IfFalse(target)) if self.fusible(pc + 1) => *target as usize,
            _ => return Err(LowerError::Internal("ForInNext without its IfFalse".into())),
        };
        let iterator = self.operand(self.stack.len() - 1)?;
        self.materialize_all()?;
        let key = self.result_register()?;
        self.emit_storing_jump(
            |b| b.ab_jump(Opcode::ForInNext, key, iterator, 0),
            key,
            target,
        );
        self.stack.push(Entry::Canon);
        Ok(2)
    }

    // ---- iterator records, `new.target`, private names, templates ----

    /// Pushes `count` canonical entries (values an instruction wrote to the
    /// temporaries at their depths).
    fn push_canon(&mut self, count: usize) {
        for _ in 0..count {
            self.stack.push(Entry::Canon);
        }
        self.max_scratch = self.max_scratch.max(self.stack.len() as u32);
        self.last_dest = None;
    }

    /// The depth of the iterator record that occupies the top `count`
    /// entries.
    fn record_base(&self, count: usize) -> Result<usize, LowerError> {
        self.stack
            .len()
            .checked_sub(count)
            .ok_or_else(|| LowerError::Internal("iterator record underflow".into()))
    }

    /// The depth of the innermost iterator record active at stack PC `pc`.
    fn innermost_iterator(&self, pc: usize) -> Result<usize, LowerError> {
        let (_, region) = self
            .analysis
            .chain(pc)
            .next()
            .ok_or_else(|| LowerError::Internal("no iterator record".into()))?;
        if !region.iterator {
            return Err(LowerError::Internal("innermost region is a catch".into()));
        }
        Ok(region.marker_depth as usize)
    }

    fn new_target_register(&self) -> Result<u8, LowerError> {
        let register = self
            .layout
            .new_target
            .ok_or_else(|| LowerError::Internal("new.target without its register".into()))?;
        Self::r8(register)
    }

    /// What a private name binding holds: a field's name, a method, an
    /// accessor half.
    fn private_kind(&self, source: PrivateNameSource) -> Result<ClosureVariableKind, LowerError> {
        let kind = match source {
            PrivateNameSource::Local(local) => self
                .function
                .local_definitions()
                .get(usize::from(local))
                .map(|definition| definition.kind),
            PrivateNameSource::Closure(closure) => self
                .function
                .closure_variables()
                .get(usize::from(closure))
                .map(|variable| variable.kind),
        };
        kind.ok_or_else(|| LowerError::Internal("private name out of range".into()))
    }

    /// Pushes the value of a private name binding.
    fn load_private(&mut self, source: PrivateNameSource) -> Result<(), LowerError> {
        match source {
            PrivateNameSource::Local(local) => self.get_local(local, false),
            PrivateNameSource::Closure(closure) => self.load_captured(closure),
        }
    }

    /// The frozen template object of a tagged template site: the cooked
    /// strings (`undefined` for an invalid escape) with a frozen `raw` array
    /// of the raw strings, built once per site and kept in the constant pool.
    fn template_entry(
        &mut self,
        cooked: &[Option<JsString>],
        raw: &[JsString],
    ) -> Result<CoreValue, LowerError> {
        let mut cooked_values = Vec::with_capacity(cooked.len());
        for string in cooked {
            cooked_values.push(match string {
                Some(string) => self.atom_value(string)?,
                None => CoreValue::UNDEFINED,
            });
        }
        let mut raw_values = Vec::with_capacity(raw.len());
        for string in raw {
            raw_values.push(self.atom_value(string)?);
        }
        let object = self
            .runtime
            .template_object(&cooked_values, &raw_values)
            .map_err(LowerError::Heap)?;
        let value = CoreValue::object(object);
        self.runtime.heap_mut().roots().push(value);
        Ok(value)
    }

    // ---- bindings ----

    fn load_cell(&mut self, cell: u32) -> Result<(), LowerError> {
        let cell = self.cell_operand(cell)?;
        let dst = self.result_register()?;
        let at = self.emit(|b| b.ab(Opcode::LoadCell, dst, cell));
        self.push_result(at)?;
        Ok(())
    }

    /// A cell register as an operand: itself, or a scratch temporary holding
    /// it when it is past the window.
    fn cell_operand(&mut self, cell: u32) -> Result<u8, LowerError> {
        if !Self::is_wide(cell) {
            return Self::r8(cell);
        }
        let scratch = self.scratch()?;
        let (spill, slot) = self.spill_slot(cell)?;
        self.emit(|b| b.ab_int(Opcode::LoadSlot, scratch, spill, slot));
        Ok(scratch)
    }

    fn get_local(&mut self, local: u16, check: bool) -> Result<(), LowerError> {
        let register = self.local_register(local);
        if self.captured_locals[local as usize] {
            if check {
                let name = self.local_key(local)?;
                self.check_cell_init(register, name)?;
            }
            self.load_cell(register)
        } else {
            if check {
                let name = self.local_key(local)?;
                self.check_init(register, name)?;
            }
            if Self::is_wide(register) {
                let dst = self.result_register()?;
                let (spill, slot) = self.spill_slot(register)?;
                let at = self.emit(|b| b.ab_int(Opcode::LoadSlot, dst, spill, slot));
                self.push_result(at)?;
            } else {
                self.stack.push(Entry::Reg(register));
            }
            Ok(())
        }
    }

    fn put_local(&mut self, local: u16, keep: bool, check: bool) -> Result<(), LowerError> {
        let register = self.local_register(local);
        let captured = self.captured_locals[local as usize];
        if check {
            let name = self.local_key(local)?;
            // An emitted check ends the producer's eligibility for
            // retargeting, which would otherwise move the write before it.
            if captured {
                self.check_cell_init(register, name)?;
            } else {
                self.check_init(register, name)?;
            }
        }
        self.store(register, captured, keep)?;
        if !captured {
            // A plain register holding a value read before this point stays
            // initialized; stores of EMPTY go through SetLocalUninitialized.
            self.checked.insert(register);
        }
        Ok(())
    }

    /// Pops the top value into `register` (or its cell); pushes it back for
    /// the `Set*` forms.
    fn store(&mut self, register: u32, captured: bool, keep: bool) -> Result<(), LowerError> {
        if captured {
            let value = self.operand(self.stack.len() - 1)?;
            let (_, entry) = self.pop()?;
            // The scratch slot is the popped entry's: load the cell above it.
            self.stack.push(entry);
            let cell = self.cell_operand(register)?;
            self.stack.pop();
            self.emit(|b| b.ab(Opcode::StoreCell, cell, value));
            if keep {
                self.stack.push(entry);
            }
        } else if Self::is_wide(register) {
            let depth = self.stack.len() - 1;
            let value = self.canonical(depth)?;
            let (spill, slot) = self.spill_slot(register)?;
            self.emit(|b| b.ab_int(Opcode::StoreSlot, value, spill, slot));
            if !keep {
                self.pop()?;
            }
        } else {
            let (depth, entry) = self.pop()?;
            self.store_register(register, depth, entry)?;
            if keep {
                self.stack.push(Entry::Reg(register));
            }
        }
        Ok(())
    }

    fn load_captured(&mut self, closure: u16) -> Result<(), LowerError> {
        let dst = self.result_register()?;
        let at = self.emit(|b| b.a_capture(Opcode::LoadCaptured, dst, u32::from(closure)));
        self.push_result(at)?;
        Ok(())
    }

    fn store_captured(&mut self, closure: u16, keep: bool) -> Result<(), LowerError> {
        let value = self.operand(self.stack.len() - 1)?;
        let (_, entry) = self.pop()?;
        self.emit(|b| b.a_capture(Opcode::StoreCaptured, value, u32::from(closure)));
        if keep {
            self.stack.push(entry);
        }
        Ok(())
    }

    fn global_read(&mut self, op: Opcode, closure: u16) -> Result<(), LowerError> {
        let key = self.closure_key(closure)?;
        let dst = self.result_register()?;
        let at = self.emit(|b| b.a_key(op, dst, key));
        self.push_result(at)?;
        Ok(())
    }

    fn global_write(&mut self, op: Opcode, closure: u16) -> Result<(), LowerError> {
        let key = self.closure_key(closure)?;
        let (_, [value]) = self.pop_operands::<1>()?;
        if op == Opcode::DefineGlobal {
            let flags = self.global_flags();
            self.emit(|b| b.raw(oxide_core::Insn::new(op, value, flags, 0, key.raw())));
        } else {
            self.emit(|b| b.a_key(op, value, key));
        }
        Ok(())
    }

    /// The `global` flags of this code's declarations: `eval` code's
    /// bindings are configurable.
    fn global_flags(&self) -> u8 {
        if self.function.metadata().eval_kind == EvalKind::None {
            0
        } else {
            oxide_core::bytecode::opcode::global::EVAL
        }
    }

    // ---- shuffles ----

    /// Replaces the top `inputs` entries with `mapping.len()` entries, the
    /// `j`-th being the old entry `mapping[j]` (indices from the lowest of
    /// the inputs). Non-canonical entries move for free; canonical ones move
    /// between temporaries as one parallel move.
    fn permute(&mut self, inputs: usize, mapping: &[usize]) -> Result<(), LowerError> {
        let base = self
            .stack
            .len()
            .checked_sub(inputs)
            .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
        let old: Vec<Entry> = self.stack.split_off(base);
        if old.contains(&Entry::Marker) {
            return Err(LowerError::Internal("shuffle across a catch marker".into()));
        }
        let mut moves = Vec::new();
        for (position, &source) in mapping.iter().enumerate() {
            let entry = old[source];
            if entry == Entry::Canon && source != position {
                moves.push((self.temp(base + position), self.temp(base + source)));
            }
            self.stack.push(entry);
        }
        self.parallel_move(moves)
    }

    /// Performs `dst <- src` moves as if simultaneously; destinations are
    /// distinct. Cycles are broken with `Swap`.
    fn parallel_move(&mut self, mut moves: Vec<(u32, u32)>) -> Result<(), LowerError> {
        moves.retain(|&(dst, src)| dst != src);
        while !moves.is_empty() {
            let free = moves
                .iter()
                .position(|&(dst, _)| moves.iter().all(|&(_, src)| src != dst));
            if let Some(index) = free {
                let (dst, src) = moves.remove(index);
                self.mov(dst, src)?;
                continue;
            }
            // Every destination is still to be read: a cycle.
            let (dst, src) = moves.remove(0);
            self.swap(dst, src)?;
            for pending in &mut moves {
                if pending.1 == dst {
                    pending.1 = src;
                } else if pending.1 == src {
                    pending.1 = dst;
                }
            }
            moves.retain(|&(dst, src)| dst != src);
        }
        Ok(())
    }

    // ---- allocation forms ----

    /// The `DefineField`s that fill the object literal created at `pc`: the
    /// slot count `NewObject` reserves.
    fn literal_slots(&self, pc: usize) -> u32 {
        let Some(depth) = self.analysis.depth[pc] else {
            return 0;
        };
        let mut slots = 0;
        for next in pc + 1..self.code.len() {
            let Some(at) = self.analysis.depth[next] else {
                break;
            };
            if at <= depth {
                break;
            }
            if at == depth + 2 && matches!(self.code[next], Instruction::DefineField(_)) {
                slots += 1;
            }
        }
        slots
    }

    // ---- the unit ----

    fn finish(
        mut self,
        parent: Option<Layout>,
        children: Vec<CodeUnitId>,
    ) -> Result<(CodeUnit, Vec<CoreValue>), LowerError> {
        for &(at, target) in &self.fixups {
            let label = self.labels[target]
                .ok_or_else(|| LowerError::Internal(format!("jump to {target} has no label")))?;
            self.builder.patch_target(at, label);
        }
        // A jump that writes a depth in the spill scope reaches its target
        // through a stub that stores the register first.
        for index in 0..self.store_stubs.len() {
            let (at, register, temp, target) = self.store_stubs[index];
            let label = self.labels[target]
                .ok_or_else(|| LowerError::Internal(format!("jump to {target} has no label")))?;
            let origin = self.origins[at as usize];
            let start = self.builder.pc();
            let (spill, slot) = self.spill_slot(temp)?;
            self.builder
                .ab_int(Opcode::StoreSlot, register, spill, slot);
            self.builder.jump(Opcode::Jmp, label);
            self.origins.extend([origin, origin]);
            self.builder.patch_target(at, start);
        }
        // An exception leaving an iterator record's range closes
        // the iterator (unless it is done) and propagates. Each record gets a
        // stub after the code, reached through its own handler entry.
        let mut stubs = vec![None; self.analysis.catches.len()];
        let catches = self.analysis.catches.clone();
        for (region, catch) in catches.iter().enumerate() {
            if !catch.iterator {
                continue;
            }
            let depth = catch.marker_depth as usize;
            let start = self.builder.pc();
            // A record in the spill scope is copied into scratch registers.
            let record = if self.spilled(depth + 2) {
                let run = self.temp_base + self.layout.temp_registers + 1;
                self.pc = catch.opened_at;
                for index in 0..3 {
                    self.mov(run + index as u32, self.temp(depth + index))?;
                }
                Self::r8(run)?
            } else {
                Self::r8(self.temp(depth))?
            };
            let thrown = if self.spilled(depth + 3) {
                Self::r8(self.temp_base + self.layout.temp_registers + 4)?
            } else {
                self.max_scratch = self.max_scratch.max(catch.marker_depth + 4);
                Self::r8(self.temp(depth + 3))?
            };
            // The stub runs in the regions that enclose the record.
            self.builder.ab(Opcode::IterCloseThrow, record, thrown);
            self.origins.push(catch.opened_at);
            self.builder.a(Opcode::Throw, thrown);
            self.origins.push(catch.opened_at);
            stubs[region] = Some((start, u16::from(thrown)));
        }
        // A catch whose value lands in the spill scope is reached through a
        // stub that stores it.
        for (region, catch) in self.analysis.catches.iter().enumerate() {
            let depth = catch.marker_depth as usize;
            if catch.iterator || !self.spilled(depth) {
                continue;
            }
            let target = self.labels[catch.target]
                .ok_or_else(|| LowerError::Internal("catch target has no label".into()))?;
            let register = Self::r8(self.temp_base + self.layout.temp_registers + 1)?;
            let (spill, slot) = self.spill_slot(self.temp(depth))?;
            let start = self.builder.pc();
            self.builder
                .ab_int(Opcode::StoreSlot, register, spill, slot);
            self.builder.jump(Opcode::Jmp, target);
            self.origins.extend([catch.target, catch.target]);
            stubs[region] = Some((start, u16::from(register)));
        }
        let handlers = self.handlers(&stubs)?;
        for handler in handlers {
            self.builder.handler(handler);
        }

        let metadata = self.function.metadata();
        let register_count = if self.layout.temp_registers == u32::MAX {
            self.temp_base + self.analysis.max_depth.max(self.max_scratch)
        } else {
            self.temp_base + self.layout.temp_registers + SCRATCH
        };
        let register_count = u16::try_from(register_count)
            .map_err(|_| LowerError::unsupported("functions with more than 65535 registers"))?;
        let class_constructor = self
            .code
            .iter()
            .any(|instruction| matches!(instruction, Instruction::CheckCtor));
        let mut unit_flags = 0;
        if metadata.strict {
            unit_flags |= flags::STRICT;
        }
        match metadata.constructor_kind {
            ConstructorKind::Base => unit_flags |= flags::CONSTRUCTOR,
            ConstructorKind::Derived => unit_flags |= flags::CONSTRUCTOR | flags::DERIVED,
            ConstructorKind::None => {}
        }
        if class_constructor {
            unit_flags |= flags::CLASS_CONSTRUCTOR;
        }
        if self.layout.new_target.is_some() {
            unit_flags |= flags::NEW_TARGET;
        }
        match metadata.function_kind {
            FunctionKind::Normal => {}
            FunctionKind::Generator => unit_flags |= flags::GENERATOR,
            FunctionKind::Async => unit_flags |= flags::ASYNC,
            FunctionKind::AsyncGenerator => unit_flags |= flags::GENERATOR | flags::ASYNC,
        }
        if metadata.has_prototype && !class_constructor {
            unit_flags |= flags::PROTOTYPE;
        }
        if self.is_script {
            unit_flags |= flags::SCRIPT;
        }
        if metadata.is_module {
            unit_flags |= flags::MODULE;
        }
        if self.is_script && metadata.eval_kind != EvalKind::None {
            unit_flags |= flags::EVAL;
        }
        if self.code.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::Arguments(_)
                    | Instruction::Rest(_)
                    | Instruction::InitDerivedConstructor
            )
        }) {
            unit_flags |= flags::ARGUMENTS;
        }
        if metadata.class_private_brand {
            unit_flags |= flags::BRAND;
        }
        let this_slots = if metadata.constructor_kind == ConstructorKind::Base {
            self.this_keys.len() as u16
        } else {
            0
        };
        let name_atom = match self.function.func_name() {
            Some(name) if !name.is_empty() => Some(name.clone()),
            _ => self.inferred_name.clone(),
        };
        let name_atom = match name_atom {
            Some(name) => Some(self.atom_value(&name)?.as_string().ok_or_else(|| {
                LowerError::Internal("a function name atom is not a string".into())
            })?),
            _ => None,
        };
        let source = self
            .function
            .debug()
            .and_then(|debug| debug.source.as_ref())
            .filter(|_| !self.is_script)
            .map(|bytes| bytes.to_vec().into_boxed_slice());
        let captures = self.captures(parent)?;
        let name = self
            .function
            .func_name()
            .map(|name| match name.flat_latin1() {
                Some(bytes) if bytes.is_ascii() => {
                    String::from_utf8(bytes.to_vec()).expect("ASCII is UTF-8")
                }
                _ => String::from_utf16_lossy(&string_units(name)),
            })
            .unwrap_or_default();
        let filename = self
            .function
            .debug()
            .map(|debug| match &self.tree.filename {
                Some((string, converted)) if string.same_representation(&debug.filename) => {
                    converted.clone()
                }
                _ => {
                    let converted = std::rc::Rc::<str>::from(debug.filename.to_utf8_lossy());
                    self.tree.filename = Some((debug.filename.clone(), converted.clone()));
                    converted
                }
            });
        let positions = self.positions(self.builder.pc() as usize);
        self.builder.mark_back_edges();
        let mut unit = self.builder.finish(UnitLayout {
            register_count,
            parameter_count: metadata.argument_count,
            local_count: metadata.local_count + u16::from(self.layout.new_target.is_some()),
            flags: unit_flags,
            this_slots,
            constant_count: self.pool.len() as u32,
            spill_register: self.layout.spill.map(|(register, _)| register as u16),
            spill_slots: match self.layout.spill {
                Some((_, slots)) => u16::try_from(slots).map_err(|_| {
                    LowerError::unsupported("functions with more than 65535 spill slots")
                })?,
                None => 0,
            },
            children,
            captures,
            name,
            name_atom,
            length: metadata.defined_argument_count,
            source,
            filename,
        });
        unit.debug.positions = positions;
        unit.debug.definition = self
            .function
            .debug()
            .and_then(|debug| debug.pc2line.as_ref())
            .and_then(|table| table.definition.one_based());
        unit.direct_eval_sites = self.direct_eval_sites.into();
        let tree = &mut *self.tree;
        tree.analyses.push(self.analysis);
        let scratch = &mut tree.lowering;
        self.labels.clear();
        scratch.labels = self.labels;
        self.name_keys.clear();
        scratch.name_keys = self.name_keys;
        self.local_keys.clear();
        scratch.local_keys = self.local_keys;
        self.fixups.clear();
        scratch.fixups = self.fixups;
        self.origins.clear();
        scratch.origins = self.origins;
        self.stack.clear();
        scratch.stack = self.stack;
        Ok((unit, self.pool))
    }

    /// The source position of each emitted instruction, from the front
    /// end's table for the instruction it lowers, one entry per change.
    fn positions(&self, len: usize) -> Box<[oxide_core::bytecode::SourcePosition]> {
        let Some(table) = self
            .function
            .debug()
            .and_then(|debug| debug.pc2line.as_ref())
        else {
            return Box::new([]);
        };
        let mut positions: Vec<oxide_core::bytecode::SourcePosition> = Vec::new();
        // The entries are sorted by pc and the origins rise but for the
        // stubs at the end: walk forward, and search again on a step back.
        let entries = &table.entries;
        let mut after = 0;
        let mut previous = 0;
        for (pc, &origin) in self.origins.iter().take(len).enumerate() {
            let origin = origin as u32;
            if origin < previous {
                after = entries.partition_point(|entry| entry.pc <= origin);
            } else {
                while entries.get(after).is_some_and(|entry| entry.pc <= origin) {
                    after += 1;
                }
            }
            previous = origin;
            let position = after
                .checked_sub(1)
                .map_or(table.definition, |at| table.entries[at].position);
            let Some((line, column)) = position.one_based() else {
                continue;
            };
            if positions
                .last()
                .is_some_and(|last| (last.line, last.column) == (line, column))
            {
                continue;
            }
            positions.push(oxide_core::bytecode::SourcePosition {
                pc: pc as u32,
                line,
                column,
            });
        }
        positions.into()
    }

    fn captures(&self, parent: Option<Layout>) -> Result<Vec<Capture>, LowerError> {
        self.function
            .closure_variables()
            .iter()
            .map(|variable| {
                let register = |index: u32| {
                    u16::try_from(index).map_err(|_| {
                        LowerError::unsupported("functions with more than 65535 registers")
                    })
                };
                Ok(match variable.source {
                    ClosureSource::ParentLocal(local) => {
                        let parent = parent.ok_or_else(|| {
                            LowerError::Internal("a script local captured".into())
                        })?;
                        let local = parent.local(local);
                        if local >= SPILLED {
                            Capture::Slot(register(local - SPILLED)?)
                        } else {
                            Capture::Register(register(local)?)
                        }
                    }
                    ClosureSource::ParentArgument(argument) => {
                        Capture::Register(register(1 + u32::from(argument))?)
                    }
                    ClosureSource::ParentClosure(index) => Capture::Outer(index),
                    // A module's bindings are cells its record supplies.
                    ClosureSource::Global
                    | ClosureSource::GlobalDeclaration
                    | ClosureSource::ParentGlobal(_)
                    | ClosureSource::ModuleDeclaration
                    | ClosureSource::ModuleImport
                    | ClosureSource::ModuleImportCollision
                    | ClosureSource::ModuleImportMeta => Capture::None,
                    _ => return Err(LowerError::unsupported("direct eval bindings")),
                })
            })
            .collect()
    }

    /// One handler entry per run of register instructions inside a catch
    /// region, innermost regions first.
    ///
    /// One pass over the instructions keeps the regions open at the current
    /// one, outermost first, each with the instruction its run started at; a
    /// region's position in that stack is its nesting.
    fn handlers(&self, stubs: &[Option<(u32, u16)>]) -> Result<Vec<Handler>, LowerError> {
        let mut destinations = Vec::with_capacity(self.analysis.catches.len());
        for (region, catch) in self.analysis.catches.iter().enumerate() {
            destinations.push(match stubs[region] {
                Some(stub) => stub,
                None => {
                    let target = self.labels[catch.target]
                        .ok_or_else(|| LowerError::Internal("catch target has no label".into()))?;
                    let register =
                        u16::try_from(self.temp(catch.marker_depth as usize)).map_err(|_| {
                            LowerError::unsupported("functions with more than 65535 registers")
                        })?;
                    (target, register)
                }
            });
        }
        let mut entries = Vec::new();
        let mut open: Vec<(u32, u32)> = Vec::new();
        let mut close = |open: &mut Vec<(u32, u32)>, keep: usize, end: u32| {
            while open.len() > keep {
                let (region, start) = open.pop().expect("an open region");
                let (target, register) = destinations[region as usize];
                entries.push((
                    open.len(),
                    Handler {
                        start,
                        end,
                        target,
                        register,
                    },
                ));
            }
        };
        let mut entered = Vec::new();
        for (at, &origin) in self.origins.iter().enumerate() {
            let innermost = self
                .analysis
                .regions
                .get(origin)
                .copied()
                .unwrap_or(analysis::NO_REGION);
            if open.last().map(|&(region, _)| region) == Some(innermost)
                || (open.is_empty() && innermost == analysis::NO_REGION)
            {
                continue;
            }
            // The regions active here, innermost first, down to the first
            // one already open: those above it close, these open.
            entered.clear();
            let mut keep = 0;
            for (region, catch) in self.analysis.chain(origin) {
                let level = catch.level as usize;
                if open.get(level).is_some_and(|&(open, _)| open == region) {
                    keep = level + 1;
                    break;
                }
                entered.push(region);
            }
            close(&mut open, keep, at as u32);
            open.extend(entered.iter().rev().map(|&region| (region, at as u32)));
        }
        close(&mut open, 0, self.origins.len() as u32);
        entries.sort_by_key(|&(nesting, handler)| (std::cmp::Reverse(nesting), handler.start));
        Ok(entries.into_iter().map(|(_, handler)| handler).collect())
    }
}
