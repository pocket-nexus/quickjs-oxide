//! Lowering from the compiler's stack code to the new core's register code
//! (`docs/performance/new-core-phase2.md`, section 3).
//!
//! The input is the [`UnlinkedFunction`] tree the current engine publishes:
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
mod run_tests;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt;

use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{
    Capture, CodeBuilder, CodeUnit, CodeUnitId, Handler, Opcode, ValidateError, flags,
};
use oxide_core::{HeapError, ObjectId, PropertyKey, Runtime, Value as CoreValue};

use self::analysis::{Analysis, analyze};
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::function::UnlinkedFunction;
use crate::engine::code::function::metadata::{
    ClosureSource, ClosureVariableName, ConstructorKind, EvalKind, FunctionKind,
};
use crate::engine::value::{JsString, PrimitiveValue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LowerError {
    /// A construct outside the phase 2 surface (section 3.4).
    Unsupported(String),
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

/// A script lowered and instantiated on the new core.
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
    // BigInt constants are heap records but not atoms: they stay on the root
    // stack until `instantiate` attaches the constant tables.
    let mark = runtime.heap_mut().roots().mark();
    let result = (|| {
        let mut constants = HashMap::new();
        let root = lower_tree(runtime, script, None, &mut constants)?;
        let order = runtime.code().tree(root);
        let tables: Vec<&[CoreValue]> = order.iter().map(|id| constants[id].as_slice()).collect();
        let function = runtime.instantiate(root, &tables)?;
        Ok(LoweredScript { root, function })
    })();
    runtime.heap_mut().roots().truncate(mark);
    result
}

/// Lowers and publishes `function` after its children. `parent_arguments` is
/// the parent's parameter count (`None` for the script), which places the
/// parent's locals for this function's captures.
fn lower_tree(
    runtime: &mut Runtime,
    function: &UnlinkedFunction,
    parent_arguments: Option<u16>,
    constants: &mut HashMap<CodeUnitId, Vec<CoreValue>>,
) -> Result<CodeUnitId, LowerError> {
    let arguments = function.metadata().argument_count;
    let mut children = Vec::new();
    for constant in function.constants() {
        if let Some(child) = constant.as_child() {
            children.push(lower_tree(runtime, child, Some(arguments), constants)?);
        }
    }
    let (unit, pool) = lower_function(runtime, function, parent_arguments, children)?;
    let id = runtime.publish(unit)?;
    constants.insert(id, pool);
    Ok(id)
}

/// Lowers one function whose children are already published.
pub(super) fn lower_function(
    runtime: &mut Runtime,
    function: &UnlinkedFunction,
    parent_arguments: Option<u16>,
    children: Vec<CodeUnitId>,
) -> Result<(CodeUnit, Vec<CoreValue>), LowerError> {
    check_surface(function)?;
    let analysis = analyze(function.code())?;
    let mut lowerer = Lowerer::new(runtime, function, analysis, parent_arguments.is_none())?;
    lowerer.prologue()?;
    lowerer.body()?;
    lowerer.finish(parent_arguments, children)
}

/// Rejects whole-function features outside the phase 2 surface.
fn check_surface(function: &UnlinkedFunction) -> Result<(), LowerError> {
    let metadata = function.metadata();
    let unsupported = if metadata.function_kind != FunctionKind::Normal {
        Some("generators and async functions")
    } else if metadata.constructor_kind == ConstructorKind::Derived
        || metadata.class_initializer_kind.is_some()
        || metadata.derived_this_local.is_some()
        || metadata.needs_home_object
    {
        Some("classes")
    } else if metadata.rest_parameter.is_some() || metadata.rest_pattern_start.is_some() {
        Some("rest parameters")
    } else if function.parameter_environment().is_some() || metadata.pattern_argument_count > 0 {
        Some("non-simple parameters")
    } else if metadata.eval_kind != EvalKind::None
        || metadata.eval_variable_object_local.is_some()
        || !function.eval_environments().is_empty()
    {
        Some("eval")
    } else if metadata.is_module {
        Some("modules")
    } else if metadata.active_function_local.is_some() {
        Some("super")
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
        Instruction::IsUndefined => Opcode::IsUndefined,
        Instruction::IsNull => Opcode::IsNull,
        Instruction::TypeOfIsUndefined => Opcode::TypeOfIsUndefined,
        Instruction::TypeOfIsFunction => Opcode::TypeOfIsFunction,
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
/// the construct when it is one of section 3.4's, else the stack-code name.
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
        I::RegExp(_) => "RegExp literals",
        I::Arguments(_) => "Arguments objects",
        I::Rest(_) => "rest parameters",
        I::Append | I::Apply(_) | I::CopyDataProperties => "spread",
        I::ForInStart | I::ForInNext => "for-in",
        I::DefineMethod { .. } | I::DefineMethodComputed { .. } => "object literal methods",
        I::Eval { .. } | I::ApplyEval { .. } => "eval",
        I::Import => "modules",
        I::PushNewTarget => "new.target",
        _ => {
            let text = format!("{instruction:?}");
            let end = text.find(['(', ' ', '{']).unwrap_or(text.len());
            return LowerError::Unsupported(text[..end].to_owned());
        }
    };
    LowerError::unsupported(construct)
}

fn string_units(string: &JsString) -> Vec<u16> {
    string.utf16_units().collect()
}

/// Marks a capture index in the set of bindings a check has proven
/// initialized since the last join point (a binding leaves the TDZ only
/// forward, and only `SetLocalUninitialized` re-enters it).
const CAPTURE_BIT: u32 = 1 << 31;

struct Lowerer<'a> {
    runtime: &'a mut Runtime,
    function: &'a UnlinkedFunction,
    code: &'a [Instruction],
    analysis: Analysis,
    is_script: bool,
    builder: CodeBuilder,
    stack: Vec<Entry>,
    arguments: u32,
    temp_base: u32,
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
    decimal_index: HashMap<u32, u32>,
    child_ordinal: HashMap<u32, u32>,
    this_keys: HashSet<u32>,
}

impl<'a> Lowerer<'a> {
    fn new(
        runtime: &'a mut Runtime,
        function: &'a UnlinkedFunction,
        analysis: Analysis,
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
        Ok(Lowerer {
            runtime,
            function,
            code,
            is_script,
            builder: CodeBuilder::new(),
            stack: Vec::new(),
            arguments,
            temp_base: 1 + arguments + locals,
            captured_arguments,
            captured_locals,
            labels: vec![None; code.len()],
            fixups: Vec::new(),
            origins: Vec::new(),
            pc: 0,
            last_dest: None,
            checked: HashSet::new(),
            max_scratch: 0,
            pool: Vec::new(),
            pool_index: HashMap::new(),
            decimal_index: HashMap::new(),
            child_ordinal,
            this_keys: HashSet::new(),
            analysis,
        })
    }

    // ---- registers ----

    fn temp(&self, depth: usize) -> u32 {
        self.temp_base + depth as u32
    }

    fn argument_register(&self, argument: u16) -> u32 {
        1 + u32::from(argument)
    }

    fn local_register(&self, local: u16) -> u32 {
        1 + self.arguments + u32::from(local)
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

    fn mov(&mut self, dst: u32, src: u32) -> Result<(), LowerError> {
        if dst != src {
            let (dst, src) = (Self::r8(dst)?, Self::r8(src)?);
            self.emit(|b| b.ab(Opcode::Mov, dst, src));
        }
        Ok(())
    }

    /// Loads a non-register entry into `dst`.
    fn load(&mut self, dst: u32, entry: Entry) -> Result<(), LowerError> {
        let a = Self::r8(dst)?;
        match entry {
            Entry::Canon | Entry::Marker => {
                return Err(LowerError::Internal("load of a register entry".into()));
            }
            Entry::Reg(src) => self.mov(dst, src)?,
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
    /// its temporary first.
    fn operand(&mut self, depth: usize) -> Result<u8, LowerError> {
        match self.stack[depth] {
            Entry::Canon => Self::r8(self.temp(depth)),
            Entry::Reg(register) => Self::r8(register),
            Entry::Marker => Err(LowerError::Internal("catch marker used as a value".into())),
            _ => {
                self.materialize(depth)?;
                Self::r8(self.temp(depth))
            }
        }
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
    fn result_register(&self) -> Result<u8, LowerError> {
        Self::r8(self.temp(self.stack.len()))
    }

    /// Records the value written by the instruction at `pc` as the new top.
    fn push_result(&mut self, pc: u32) {
        let temp = self.temp(self.stack.len());
        self.stack.push(Entry::Canon);
        self.last_dest = Some((pc, temp));
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
            let a = Self::r8(register)?;
            self.emit(|b| b.a_key(Opcode::CheckInit, a, name));
        }
        Ok(())
    }

    fn check_cell_init(&mut self, cell: u32, name: PropertyKey) -> Result<(), LowerError> {
        if self.checked.insert(cell) {
            let a = Self::r8(cell)?;
            self.emit(|b| b.a_key(Opcode::CheckCellInit, a, name));
        }
        Ok(())
    }

    /// Checks capture `closure` through a scratch temporary above the stack.
    fn check_captured_init(&mut self, closure: u16) -> Result<(), LowerError> {
        if !self.checked.insert(CAPTURE_BIT | u32::from(closure)) {
            return Ok(());
        }
        let name = self.closure_key(closure)?;
        let scratch = self.stack.len();
        self.max_scratch = self.max_scratch.max(scratch as u32 + 1);
        let a = Self::r8(self.temp(scratch))?;
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

    fn key(&mut self, string: &JsString) -> Result<PropertyKey, LowerError> {
        Ok(self
            .runtime
            .heap_mut()
            .property_key_units(&string_units(string))?)
    }

    fn name_key(&mut self, constant: u32) -> Result<PropertyKey, LowerError> {
        let string = self.constant_string(constant)?;
        self.key(string)
    }

    fn closure_key(&mut self, closure: u16) -> Result<PropertyKey, LowerError> {
        let variable = self
            .function
            .closure_variables()
            .get(closure as usize)
            .ok_or_else(|| LowerError::Internal("closure variable out of range".into()))?;
        match variable.name {
            ClosureVariableName::Constant(constant) => self.name_key(constant),
            _ => self.key(&JsString::from_static("")),
        }
    }

    fn local_key(&mut self, local: u16) -> Result<PropertyKey, LowerError> {
        let name = self
            .function
            .local_definitions()
            .get(local as usize)
            .and_then(|definition| definition.name.clone())
            .unwrap_or_else(|| JsString::from_static(""));
        self.key(&name)
    }

    /// An atom value for a string constant (atoms are strong roots, so the
    /// pool needs no rooting until it is attached).
    fn atom_value(&mut self, units: &[u16]) -> Result<CoreValue, LowerError> {
        Ok(CoreValue::string(
            self.runtime.heap_mut().intern_units(units)?,
        ))
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
        let Some(primitive) = constant.as_primitive() else {
            return Err(if constant.as_regexp().is_some() {
                LowerError::unsupported("RegExp literals")
            } else {
                LowerError::unsupported("template literals")
            });
        };
        let value = match primitive {
            PrimitiveValue::Undefined => return Ok(Entry::Undefined),
            PrimitiveValue::Null => return Ok(Entry::Null),
            PrimitiveValue::Bool(value) => return Ok(Entry::Bool(*value)),
            PrimitiveValue::Int(value) => return Ok(Entry::Int(*value)),
            PrimitiveValue::Float(value) => CoreValue::number(*value),
            PrimitiveValue::String(string) => self.atom_value(&string_units(string))?,
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

    fn decimal_entry(&mut self, index: u32) -> Result<Entry, LowerError> {
        if let Some(&slot) = self.decimal_index.get(&index) {
            return Ok(Entry::Const(slot));
        }
        let units: Vec<u16> = index.to_string().encode_utf16().collect();
        let value = self.atom_value(&units)?;
        let slot = self.pool.len() as u32;
        self.pool.push(value);
        self.decimal_index.insert(index, slot);
        Ok(Entry::Const(slot))
    }

    // ---- function entry ----

    fn prologue(&mut self) -> Result<(), LowerError> {
        if self.is_script {
            for index in 0..self.function.closure_variables().len() {
                let variable = self.function.closure_variables()[index];
                match variable.source {
                    ClosureSource::GlobalDeclaration => {
                        let key = self.closure_key(index as u16)?;
                        self.emit(|b| b.key(Opcode::DeclareGlobal, key));
                    }
                    ClosureSource::Global => {}
                    _ => return Err(LowerError::unsupported("script closure source")),
                }
            }
        }
        let metadata = self.function.metadata();
        if let Some(local) = metadata.function_name_local {
            let register = Self::r8(self.local_register(local))?;
            self.emit(|b| b.a(Opcode::LoadFunction, register));
        }
        for argument in 0..self.captured_arguments.len() {
            if self.captured_arguments[argument] {
                let register = Self::r8(self.argument_register(argument as u16))?;
                self.emit(|b| b.ab(Opcode::NewCell, register, register));
            }
        }
        for local in 0..self.captured_locals.len() {
            let register = Self::r8(self.local_register(local as u16))?;
            let lexical = self.function.local_definitions()[local].is_lexical;
            match (self.captured_locals[local], lexical) {
                (true, true) => self.emit(|b| b.a(Opcode::NewCellEmpty, register)),
                (true, false) => self.emit(|b| b.ab(Opcode::NewCell, register, register)),
                (false, true) => self.emit(|b| b.a(Opcode::LoadEmpty, register)),
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
            if self.analysis.label[pc] {
                self.materialize_all()?;
                self.labels[pc] = Some(self.builder.pc());
                self.last_dest = None;
                self.checked.clear();
                // A Catch marker survives a join only above its region's
                // handler; every other entry is canonical.
                let markers: Vec<usize> = self.analysis.regions[pc]
                    .iter()
                    .map(|&region| self.analysis.catches[region as usize].marker_depth as usize)
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
            self.push_result(at);
            return Ok(1);
        }
        if let Some((op, immediate)) = binary(instruction) {
            self.binary(op, immediate)?;
            return Ok(1);
        }
        match *instruction {
            I::Nop | I::SetName(_) | I::SetNameComputed => {}

            // ---- pushes ----
            I::PushI32(value) => self.stack.push(Entry::Int(value)),
            I::PushConst(index) => {
                let entry = self.pool_entry(index)?;
                self.stack.push(entry);
            }
            I::PushAtomValueIndex(index) => {
                let entry = self.decimal_entry(index)?;
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
                self.push_result(at);
            }
            I::Object => {
                let slots = self.literal_slots(pc);
                let dst = self.result_register()?;
                let at = self.emit(|b| b.a_count(Opcode::NewObject, dst, slots));
                self.push_result(at);
            }
            I::ArrayFrom(count) => {
                let count = usize::from(count);
                let base = self
                    .stack
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
                for depth in base..self.stack.len() {
                    self.materialize(depth)?;
                }
                self.stack.truncate(base);
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab_range(Opcode::NewArray, dst, dst, count as u32));
                self.push_result(at);
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
                let a = Self::r8(register)?;
                if self.captured_locals[local as usize] {
                    self.emit(|b| b.a(Opcode::NewCellEmpty, a));
                } else {
                    self.emit(|b| b.a(Opcode::LoadEmpty, a));
                }
            }
            I::CloseLocal(local) => {
                if self.captured_locals[local as usize] {
                    let a = Self::r8(self.local_register(local))?;
                    self.emit(|b| b.a(Opcode::CloseCell, a));
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
                    let value = Self::r8(self.temp(self.stack.len() - 1))?;
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
                self.push_result(at);
            }
            I::GetField2(name) => {
                let key = self.name_key(name)?;
                let object = self.operand(self.stack.len() - 1)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.ab_key(Opcode::GetField, dst, object, key));
                self.push_result(at);
            }
            I::GetArrayEl => {
                let (_, [object, key]) = self.pop_operands::<2>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::GetElem, dst, object, key));
                self.push_result(at);
            }
            I::GetArrayEl2 => {
                let (_, [key]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::GetElem, dst, object, key));
                self.push_result(at);
            }
            I::GetArrayEl3 => {
                let (_, [raw]) = self.pop_operands::<1>()?;
                let object = self.operand(self.stack.len() - 1)?;
                let converted = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::ToPropertyKey, converted, raw));
                self.push_result(at);
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::GetElem, dst, object, converted));
                self.push_result(at);
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
            I::Delete => {
                let (_, [object, key]) = self.pop_operands::<2>()?;
                let dst = self.result_register()?;
                let at = self.emit(|b| b.abc(Opcode::DeleteElem, dst, object, key));
                self.push_result(at);
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
                    self.push_result(at);
                    self.pc = pc + 1;
                    self.instruction(pc + 1)?;
                    return Ok(3);
                }
                let (_, [value]) = self.pop_operands::<1>()?;
                let old = self.result_register()?;
                let at = self.emit(|b| b.ab(Opcode::ToNumeric, old, value));
                self.push_result(at);
                let new = self.result_register()?;
                let at = self.emit(|b| b.ab(op, new, old));
                self.push_result(at);
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
                if target <= pc || skipped.clone().any(|at| self.analysis.depth[at].is_some()) {
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
                let region = *self.analysis.regions[pc]
                    .last()
                    .ok_or_else(|| LowerError::Internal("NipCatch outside a region".into()))?;
                let marker = self.analysis.catches[region as usize].marker_depth as usize;
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
                self.emit_jump(|b| b.a_jump(Opcode::Gosub, dst, 0), target as usize);
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
            I::ReturnUndefined => {
                self.emit(|b| b.op(Opcode::ReturnUndefined));
            }
            I::Throw => {
                let (_, [value]) = self.pop_operands::<1>()?;
                self.emit(|b| b.a(Opcode::Throw, value));
            }
            I::ThrowReadOnly(name) => {
                let key = self.name_key(name)?;
                self.emit(|b| b.key(Opcode::ThrowConstAssign, key));
            }

            // ---- calls ----
            I::Call(argc) => self.call(Opcode::Call, argc, 1, true)?,
            I::CallMethod(argc) => self.call(Opcode::CallMethod, argc, 2, true)?,
            I::Construct(argc) => self.call(Opcode::New, argc, 2, true)?,
            I::TailCall(argc) => self.call(Opcode::TailCall, argc, 1, false)?,
            I::TailCallMethod(argc) => self.call(Opcode::TailCallMethod, argc, 2, false)?,

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
            self.push_result(at);
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

    fn binary(&mut self, op: Opcode, immediate: Option<Opcode>) -> Result<(), LowerError> {
        let top = self.stack.len();
        if let (Some(immediate), Entry::Int(value)) = (immediate, self.stack[top - 1]) {
            let left = self.operand(top - 2)?;
            self.stack.truncate(top - 2);
            let dst = self.result_register()?;
            let at = self.emit(|b| b.ab_int(immediate, dst, left, value));
            self.push_result(at);
            return Ok(());
        }
        let (_, [left, right]) = self.pop_operands::<2>()?;
        let dst = self.result_register()?;
        let at = self.emit(|b| b.abc(op, dst, left, right));
        self.push_result(at);
        Ok(())
    }

    fn call(
        &mut self,
        op: Opcode,
        argc: u16,
        fixed: usize,
        returns: bool,
    ) -> Result<(), LowerError> {
        let argc = u8::try_from(argc)
            .map_err(|_| LowerError::unsupported("calls with more than 255 arguments"))?;
        let base = self
            .stack
            .len()
            .checked_sub(fixed + usize::from(argc))
            .ok_or_else(|| LowerError::Internal("symbolic stack underflow".into()))?;
        // The callee window overlaps the operands in place, so they must sit
        // in their own temporaries; entries below the call are untouched.
        for depth in base..self.stack.len() {
            self.materialize(depth)?;
        }
        self.stack.truncate(base);
        let callee = Self::r8(self.temp(base))?;
        if returns {
            let at = self.emit(|b| b.call(op, callee, callee, argc));
            self.push_result(at);
        } else {
            self.emit(|b| b.call(op, 0, callee, argc));
        }
        Ok(())
    }

    // ---- bindings ----

    fn load_cell(&mut self, cell: u32) -> Result<(), LowerError> {
        let cell = Self::r8(cell)?;
        let dst = self.result_register()?;
        let at = self.emit(|b| b.ab(Opcode::LoadCell, dst, cell));
        self.push_result(at);
        Ok(())
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
            self.stack.push(Entry::Reg(register));
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
            let cell = Self::r8(register)?;
            self.emit(|b| b.ab(Opcode::StoreCell, cell, value));
            if keep {
                self.stack.push(entry);
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
        self.push_result(at);
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
        self.push_result(at);
        Ok(())
    }

    fn global_write(&mut self, op: Opcode, closure: u16) -> Result<(), LowerError> {
        let key = self.closure_key(closure)?;
        let (_, [value]) = self.pop_operands::<1>()?;
        self.emit(|b| b.a_key(op, value, key));
        Ok(())
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
            let (a, b) = (Self::r8(dst)?, Self::r8(src)?);
            self.emit(|builder| builder.ab(Opcode::Swap, a, b));
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
        parent_arguments: Option<u16>,
        children: Vec<CodeUnitId>,
    ) -> Result<(CodeUnit, Vec<CoreValue>), LowerError> {
        for &(at, target) in &self.fixups {
            let label = self.labels[target]
                .ok_or_else(|| LowerError::Internal(format!("jump to {target} has no label")))?;
            self.builder.patch_target(at, label);
        }
        let handlers = self.handlers()?;
        for handler in handlers {
            self.builder.handler(handler);
        }

        let metadata = self.function.metadata();
        let register_count = self.temp_base + self.analysis.max_depth.max(self.max_scratch);
        let register_count = u16::try_from(register_count)
            .map_err(|_| LowerError::unsupported("functions with more than 65535 registers"))?;
        let mut unit_flags = 0;
        if metadata.strict {
            unit_flags |= flags::STRICT;
        }
        if metadata.constructor_kind == ConstructorKind::Base {
            unit_flags |= flags::CONSTRUCTOR;
        }
        if self.is_script {
            unit_flags |= flags::SCRIPT;
        }
        let this_slots = if metadata.constructor_kind == ConstructorKind::Base {
            self.this_keys.len() as u16
        } else {
            0
        };
        let captures = self.captures(parent_arguments)?;
        let name = self
            .function
            .func_name()
            .map(|name| String::from_utf16_lossy(&string_units(name)))
            .unwrap_or_default();
        let unit = self.builder.finish(UnitLayout {
            register_count,
            parameter_count: metadata.argument_count,
            local_count: metadata.local_count,
            flags: unit_flags,
            this_slots,
            constant_count: self.pool.len() as u32,
            children,
            captures,
            name,
        });
        Ok((unit, self.pool))
    }

    fn captures(&self, parent_arguments: Option<u16>) -> Result<Vec<Capture>, LowerError> {
        let parent_arguments = u32::from(parent_arguments.unwrap_or(0));
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
                        Capture::Register(register(1 + parent_arguments + u32::from(local))?)
                    }
                    ClosureSource::ParentArgument(argument) => {
                        Capture::Register(register(1 + u32::from(argument))?)
                    }
                    ClosureSource::ParentClosure(index) => Capture::Outer(index),
                    ClosureSource::Global
                    | ClosureSource::GlobalDeclaration
                    | ClosureSource::ParentGlobal(_) => Capture::None,
                    _ => return Err(LowerError::unsupported("modules")),
                })
            })
            .collect()
    }

    /// One handler entry per run of register instructions inside a catch
    /// region, innermost regions first.
    fn handlers(&self) -> Result<Vec<Handler>, LowerError> {
        let mut entries = Vec::new();
        for (region, catch) in self.analysis.catches.iter().enumerate() {
            let region = region as u32;
            let target = self.labels[catch.target]
                .ok_or_else(|| LowerError::Internal("catch target has no label".into()))?;
            let register = u16::try_from(self.temp(catch.marker_depth as usize))
                .map_err(|_| LowerError::unsupported("functions with more than 65535 registers"))?;
            let mut start = None;
            let mut nesting = 0;
            for (at, &origin) in self.origins.iter().enumerate() {
                let regions = &self.analysis.regions[origin];
                let inside = regions.iter().position(|&active| active == region);
                match (inside, start) {
                    (Some(level), None) => {
                        start = Some(at as u32);
                        nesting = level;
                    }
                    (None, Some(first)) => {
                        entries.push((
                            nesting,
                            Handler {
                                start: first,
                                end: at as u32,
                                target,
                                register,
                            },
                        ));
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(first) = start {
                entries.push((
                    nesting,
                    Handler {
                        start: first,
                        end: self.origins.len() as u32,
                        target,
                        register,
                    },
                ));
            }
        }
        entries.sort_by_key(|&(nesting, handler)| (std::cmp::Reverse(nesting), handler.start));
        Ok(entries.into_iter().map(|(_, handler)| handler).collect())
    }
}
