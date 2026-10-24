//! MIR to the core's register code: the layout, register allocation,
//! instruction selection, handler and position tables of each function,
//! published children first.

mod alloc;
mod spill;

use self::alloc::{Allocation, Layout, NONE, Order, Plan};
use super::build::Program;
use super::ir::{
    BlockId, CmpOp, Constant, Function, Imm, Insn, Pos, SuspendKind, Terminator, ThrowKind, Value,
};
use super::op::Op;
use crate::engine::compiler::register::LowerError;
use crate::engine::hir::schema::{self as hir, FunctionRef};
use crate::engine::value::JsString;
use crate::source::QuickJsSourceLocator;
use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{Capture, CodeBuilder, CodeUnit, CodeUnitId, Handler, Opcode};
use oxide_core::vm::compile::Compiler;
use oxide_core::{FxHashMap, ObjectId, PropertyKey, StringId, Value as CoreValue};

/// A program lowered and instantiated on the core.
pub(crate) struct Lowered {
    pub(crate) root: CodeUnitId,
    /// The root's function object. Unrooted: the caller holds it before
    /// its next allocation.
    pub(crate) function: ObjectId,
}

/// What the lowering keeps of the source.
pub(crate) struct Debug<'a> {
    /// The source text, for the position table; `None` strips positions.
    pub(crate) source: Option<&'a [u8]>,
    /// Whether functions keep their source text.
    pub(crate) keep_source: bool,
    pub(crate) filename: Option<std::rc::Rc<str>>,
}

/// Lowers and instantiates `program`.
///
/// # Errors
/// A function past the register window, or an allocation failure.
pub(crate) fn lower(
    compiler: &mut Compiler<'_>,
    program: &Program,
    hir: &hir::Program,
    debug: &Debug<'_>,
) -> Result<Lowered, LowerError> {
    let mark = compiler.heap_mut().roots().mark();
    let result = (|| {
        let locator = debug
            .source
            .map(|source| QuickJsSourceLocator::from_bytes(source).index())
            .transpose()
            .map_err(|error| LowerError::Internal(error.to_string()))?;
        let spilled: Vec<std::cell::OnceCell<Function>> = (0..program.functions.len())
            .map(|_| std::cell::OnceCell::new())
            .collect();
        let mut tree = Tree {
            program,
            spilled: &spilled,
            hir,
            debug,
            locator,
            keys: FxHashMap::default(),
            atoms: FxHashMap::default(),
            layouts: FxHashMap::default(),
            constants: FxHashMap::default(),
        };
        let root = tree.lower(compiler, program.root, None)?;
        let order = compiler.units().tree(root);
        let tables: Vec<&[CoreValue]> = order
            .iter()
            .map(|id| tree.constants[id].as_slice())
            .collect();
        let function = compiler.instantiate(root, &tables)?;
        Ok(Lowered { root, function })
    })();
    compiler.heap_mut().roots().truncate(mark);
    result
}

/// What the lowering of one program keeps across its functions.
struct Tree<'a> {
    program: &'a Program,
    /// The functions whose values were moved to variables to fit.
    spilled: &'a [std::cell::OnceCell<Function>],
    hir: &'a hir::Program,
    debug: &'a Debug<'a>,
    locator: Option<crate::source::coordinates::QuickJsSourceIndex<'a>>,
    keys: FxHashMap<u32, PropertyKey>,
    atoms: FxHashMap<u32, StringId>,
    /// Each lowered function's variable layout, for its children's captures.
    layouts: FxHashMap<u32, Layout>,
    constants: FxHashMap<CodeUnitId, Vec<CoreValue>>,
}

impl<'a> Tree<'a> {
    /// `f` with values moved to variables until it fits, kept for the
    /// rest of the lowering.
    fn spill(&self, id: FunctionRef, f: &Function) -> Option<(&'a Function, Fitted)> {
        for attempt in 0.. {
            let g = spill::spill(f, attempt)?;
            if let Ok(fitted) = fit(&g) {
                let cell = &self.spilled[id.index()];
                let _ = cell.set(g);
                return cell.get().map(|g| (g, fitted));
            }
        }
        None
    }

    fn lower(
        &mut self,
        compiler: &mut Compiler<'_>,
        id: FunctionRef,
        parent: Option<FunctionRef>,
    ) -> Result<CodeUnitId, LowerError> {
        let f: &'a Function = &self.program.functions[id.index()];
        let (f, fitted) = match fit(f) {
            Ok(fitted) => (f, fitted),
            Err(error) => self.spill(id, f).ok_or(error)?,
        };
        self.layouts.insert(id.0, fitted.layout.clone());
        let mut children = Vec::with_capacity(f.children.len());
        for &child in &f.children {
            children.push(self.lower(compiler, child, Some(id))?);
        }
        let captures = self.captures(id, parent)?;
        let (unit, pool) =
            FunctionLowering::new(self, compiler, f, fitted)?.run(children, captures)?;
        let unit_id = compiler.publish(unit)?;
        self.constants.insert(unit_id, pool);
        Ok(unit_id)
    }

    /// Where a closure over `id` finds each binding it captures, in the
    /// frame of `parent` that creates it.
    fn captures(
        &self,
        id: FunctionRef,
        parent: Option<FunctionRef>,
    ) -> Result<Vec<Capture>, LowerError> {
        let f = &self.program.functions[id.index()];
        let Some(parent) = parent else {
            return Ok(f.captures.iter().map(|_| Capture::None).collect());
        };
        let pf = &self.program.functions[parent.index()];
        let layout = &self.layouts[&parent.0];
        f.captures
            .iter()
            .map(|&(decl, setter)| {
                if let Some(var) = pf
                    .vars
                    .iter()
                    .position(|var| var.decl == Some(decl) && var.setter == setter)
                {
                    let register = layout.var_regs[var];
                    if register >= alloc::SLOT {
                        return Ok(Capture::Slot(register16(register - alloc::SLOT)?));
                    }
                    return Ok(Capture::Register(register16(register)?));
                }
                if let Some(index) = pf.captures.iter().position(|&c| c == (decl, setter)) {
                    return Ok(Capture::Outer(index as u16));
                }
                Ok(Capture::None)
            })
            .collect()
    }

    fn string(&self, index: u32) -> &JsString {
        &self.program.strings[index as usize]
    }

    fn key(&mut self, compiler: &mut Compiler<'_>, index: u32) -> Result<PropertyKey, LowerError> {
        if let Some(&key) = self.keys.get(&index) {
            return Ok(key);
        }
        let string = self.program.strings[index as usize].clone();
        let heap = compiler.heap_mut();
        let key = match string.flat_latin1() {
            Some(bytes) => heap.property_key_latin1(bytes)?,
            None => heap.property_key_units(&string.utf16_units().collect::<Vec<_>>())?,
        };
        self.keys.insert(index, key);
        Ok(key)
    }

    fn atom(&mut self, compiler: &mut Compiler<'_>, index: u32) -> Result<StringId, LowerError> {
        if let Some(&atom) = self.atoms.get(&index) {
            return Ok(atom);
        }
        let string = self.program.strings[index as usize].clone();
        let heap = compiler.heap_mut();
        let atom = match string.flat_latin1() {
            Some(bytes) => heap.intern_latin1(bytes)?,
            None => heap.intern_units(&string.utf16_units().collect::<Vec<_>>())?,
        };
        self.atoms.insert(index, atom);
        Ok(atom)
    }
}

/// A layout that fits the register window, with its plan and allocation.
struct Fitted {
    layout: Layout,
    order: Order,
    plan: Plan,
    alloc: Allocation,
    windows: FxHashMap<u32, u32>,
}

/// The layout of `f` that fits the register window: its local variables
/// spill to slots from the last until the values fit beside the others.
fn fit(f: &Function) -> Result<Fitted, LowerError> {
    let window = oxide_core::bytecode::WINDOW;
    let mut layout = Layout::of(f, None);
    let locals = layout.locals;
    let mut budget = None;
    loop {
        let order = Order::of(f);
        let plan = alloc::plan(f, &layout, &order);
        let allocation = alloc::allocate(f, &layout, &order, &plan);
        let (windows, top) = alloc::windows(&allocation, f, &layout, &order, &plan);
        let needed = allocation.used.max(top);
        if needed <= window {
            return Ok(Fitted {
                layout,
                order,
                plan,
                alloc: allocation,
                windows,
            });
        }
        let temps = needed - layout.temp_base;
        let fixed = layout.first_local() + 1;
        let next = window
            .saturating_sub(fixed + temps)
            .min(budget.unwrap_or(locals));
        if budget == Some(0) || (budget.is_some() && Some(next) >= budget) {
            if next == 0 && budget != Some(0) {
                budget = Some(0);
                layout = Layout::of(f, budget);
                continue;
            }
            return Err(LowerError::Unsupported(
                "functions whose values need more than 256 registers".into(),
            ));
        }
        budget = Some(next);
        layout = Layout::of(f, budget);
    }
}

fn register16(register: u32) -> Result<u16, LowerError> {
    u16::try_from(register)
        .map_err(|_| LowerError::Unsupported("functions with more than 65535 registers".into()))
}

fn r8(register: u32) -> Result<u8, LowerError> {
    u8::try_from(register).map_err(|_| {
        LowerError::Unsupported(format!(
            "functions with more than 256 registers ({register})"
        ))
    })
}

/// The lowering of one function.
struct FunctionLowering<'t, 'a, 'c, 'd> {
    tree: &'t mut Tree<'a>,
    compiler: &'c mut Compiler<'d>,
    f: &'a Function,
    layout: Layout,
    order: Order,
    plan: Plan,
    alloc: Allocation,
    /// The window base of each window instruction, by slot.
    windows: FxHashMap<u32, u32>,
    builder: CodeBuilder,
    /// The pc of each block's first instruction.
    labels: Vec<u32>,
    /// Jumps to patch: pc, target block.
    fixups: Vec<(u32, BlockId)>,
    /// The position and catch region of each emitted instruction.
    positions: Vec<Pos>,
    regions: Vec<Option<super::ir::HandlerId>>,
    pool: Vec<CoreValue>,
    /// The pool slot of each constant.
    pool_slots: Vec<u32>,
    /// The registers in use, including windows.
    register_count: u32,
    direct_eval_sites: Vec<u32>,
    /// The block being emitted and the next in the layout.
    next_block: Option<BlockId>,
    /// The block whose code each block's label starts: itself, or past
    /// the blocks that emit nothing and fall through.
    resolved: Vec<BlockId>,
    pos: Pos,
    region: Option<super::ir::HandlerId>,
}

impl<'t, 'a, 'c, 'd> FunctionLowering<'t, 'a, 'c, 'd> {
    fn new(
        tree: &'t mut Tree<'a>,
        compiler: &'c mut Compiler<'d>,
        f: &'a Function,
        fitted: Fitted,
    ) -> Result<Self, LowerError> {
        let Fitted {
            layout,
            order,
            plan,
            alloc,
            windows,
        } = fitted;
        let register_count = alloc.used;
        Ok(Self {
            tree,
            compiler,
            f,
            layout,
            plan,
            alloc,
            windows,
            builder: CodeBuilder::with_capacity(f.blocks.iter().map(|b| b.insns.len()).sum()),
            labels: vec![u32::MAX; f.blocks.len()],
            fixups: Vec::new(),
            positions: Vec::new(),
            regions: Vec::new(),
            pool: Vec::new(),
            pool_slots: vec![u32::MAX; f.consts.len()],
            register_count,
            direct_eval_sites: Vec::new(),
            next_block: None,
            resolved: Vec::new(),
            order,
            pos: Pos::NONE,
            region: None,
        })
    }

    fn run(
        mut self,
        children: Vec<CodeUnitId>,
        captures: Vec<Capture>,
    ) -> Result<(CodeUnit, Vec<CoreValue>), LowerError> {
        let blocks = self.order.blocks.clone();
        self.resolved = self.resolve(&blocks);
        for (index, &block) in blocks.iter().enumerate() {
            self.next_block = blocks.get(index + 1).copied();
            self.block(block)?;
        }
        if self.builder.code().is_empty()
            || !self
                .builder
                .code()
                .last()
                .is_some_and(|i| i.op.is_terminator())
        {
            // Control never reaches past the last block's terminator; the
            // validator still wants a terminator last.
            self.emit(|b| b.op(Opcode::ReturnUndefined));
        }
        for &(at, target) in &self.fixups {
            let label = self.labels[target.index()];
            if label == u32::MAX {
                return Err(LowerError::Internal(format!(
                    "a jump to the unplaced b{}",
                    target.0
                )));
            }
            self.builder.patch_target(at, label);
        }
        self.handlers()?;
        self.finish(children, captures)
    }

    // ---- registers ----

    fn reg(&self, value: Value) -> Result<u8, LowerError> {
        let register = self.alloc.regs[value.index()];
        if register == NONE {
            return Err(LowerError::Internal(format!(
                "v{} has no register",
                value.0
            )));
        }
        r8(register)
    }

    fn var_reg(&self, var: u32) -> Result<u8, LowerError> {
        r8(self.layout.var_regs[var as usize])
    }

    /// The spill register and slot of a variable past the window.
    fn slot(&self, var: u32) -> Result<Option<(u8, i32)>, LowerError> {
        let register = self.layout.var_regs[var as usize];
        if register < alloc::SLOT {
            return Ok(None);
        }
        let (spill, _) = self
            .layout
            .spill
            .ok_or_else(|| LowerError::Internal("a slot without a spill scope".into()))?;
        Ok(Some((r8(spill)?, (register - alloc::SLOT) as i32)))
    }

    fn uses(&mut self, top: u32) {
        self.register_count = self.register_count.max(top);
    }

    // ---- emission ----

    fn emit(&mut self, build: impl FnOnce(&mut CodeBuilder) -> u32) -> u32 {
        let pc = build(&mut self.builder);
        self.positions.push(self.pos);
        self.regions.push(self.region);
        pc
    }

    fn jump_to(&mut self, build: impl FnOnce(&mut CodeBuilder) -> u32, target: BlockId) {
        let pc = self.emit(build);
        self.fixups.push((pc, target));
    }

    fn mov(&mut self, dst: u8, src: u8) {
        if dst != src {
            self.emit(|b| b.ab(Opcode::Mov, dst, src));
        }
    }

    /// `dst <- src` moves as if simultaneous; cycles are broken with `Swap`.
    fn parallel_move(&mut self, mut moves: Vec<(u8, u8)>) {
        moves.retain(|&(dst, src)| dst != src);
        // A destination written twice keeps only its last source.
        let mut seen = Vec::new();
        moves.retain(|&(dst, _)| {
            if seen.contains(&dst) {
                false
            } else {
                seen.push(dst);
                true
            }
        });
        while !moves.is_empty() {
            if let Some(index) = moves
                .iter()
                .position(|&(dst, _)| moves.iter().all(|&(_, src)| src != dst))
            {
                let (dst, src) = moves.remove(index);
                self.mov(dst, src);
                continue;
            }
            let (dst, src) = moves.remove(0);
            self.emit(|b| b.ab(Opcode::Swap, dst, src));
            for pending in &mut moves {
                if pending.1 == dst {
                    pending.1 = src;
                } else if pending.1 == src {
                    pending.1 = dst;
                }
            }
            moves.retain(|&(dst, src)| dst != src);
        }
    }

    fn key(&mut self, index: u32) -> Result<PropertyKey, LowerError> {
        self.tree.key(self.compiler, index)
    }

    /// The name a TDZ error gives a binding: none once debug information
    /// is stripped, as QuickJS drops the names with it.
    fn binding_key(&mut self, index: u32) -> Result<PropertyKey, LowerError> {
        if self.tree.debug.source.is_none() {
            return Ok(self.compiler.heap_mut().property_key("")?);
        }
        self.key(index)
    }

    fn block(&mut self, block: BlockId) -> Result<(), LowerError> {
        let b = self.f.block(block);
        self.labels[block.index()] = self.builder.pc();
        self.region = b.handler;
        let entry = self.order.entry[block.index()];
        // The spill scope exists before anything reaches a slot; an async
        // function's start stays first.
        let spill_at = if block == BlockId::ENTRY {
            let leading = b
                .insns
                .first()
                .is_some_and(|insn| insn.op == Op::AsyncStart);
            self.layout.spill.map(|spill| (usize::from(leading), spill))
        } else {
            None
        };
        let mut spill_at = spill_at;
        for (index, insn) in b.insns.iter().enumerate() {
            if let Some((at, (spill, slots))) = spill_at
                && index == at
            {
                let spill = r8(spill)?;
                self.emit(|b| b.a_count(Opcode::NewSpill, spill, slots));
                spill_at = None;
            }
            if self.plan.dead[block.index()][index] {
                continue;
            }
            self.pos = insn.pos;
            if self.plan.folded[block.index()][index] {
                continue;
            }
            let slot = entry + 1 + index as u32;
            self.insn(insn, slot, block, index)?;
        }
        if let Some((_, (spill, slots))) = spill_at {
            let spill = r8(spill)?;
            self.emit(|b| b.a_count(Opcode::NewSpill, spill, slots));
        }
        self.pos = b.term_pos;
        self.terminator(block, self.order.end[block.index()] - 1)
    }

    // ---- instructions ----

    fn insn(
        &mut self,
        insn: &Insn,
        slot: u32,
        block: BlockId,
        index: usize,
    ) -> Result<(), LowerError> {
        let key = (block.0, index as u32);
        if matches!(insn.op, Op::Inc | Op::Dec)
            && let Some(operand) = self.plan.chains.get(&key).map(|chain| chain[0])
        {
            let (dst, src) = (
                self.reg(insn.results[0].expect("a result"))?,
                self.reg(operand)?,
            );
            let core = insn.op.core()[0];
            self.emit(|b| b.ab(core, dst, src));
            return Ok(());
        }
        if let Some(chain) = self.plan.chains.get(&key).cloned() {
            if let Some(&pos) = self.plan.chain_pos.get(&key) {
                self.pos = pos;
            }
            let dst = self.reg(insn.results[0].expect("a sum"))?;
            let count = chain.len() as u32;
            let base = self.window(slot, count)?;
            let moves = self.window_moves(base, &chain)?;
            self.parallel_move(moves);
            self.emit(|b| b.ab_range(Opcode::ConcatN, dst, base, count));
            return Ok(());
        }
        if let Some(&value) = self.plan.imm.get(&key) {
            let dst = self.reg(insn.results[0].expect("a result"))?;
            let left = self.reg(insn.args.as_slice()[0])?;
            let core = match insn.op {
                Op::Add => Opcode::AddI,
                Op::Sub => Opcode::SubI,
                Op::Mul => Opcode::MulI,
                Op::BitAnd => Opcode::BitAndI,
                Op::BitOr => Opcode::BitOrI,
                Op::BitXor => Opcode::BitXorI,
                Op::Shl => Opcode::ShlI,
                Op::Sar => Opcode::SarI,
                _ => Opcode::ShrI,
            };
            self.emit(|b| b.ab_int(core, dst, left, value));
            return Ok(());
        }
        let args = insn.args.as_slice();
        let a = |s: &Self, i: usize| s.reg(args[i]);
        let result = insn.results[0];
        let res = |s: &Self| s.reg(result.expect("a result"));
        let op = insn.op;
        match op {
            Op::GetVar => {
                let r = result.expect("a result");
                let Imm::Var(var) = insn.imm else {
                    unreachable!("a var")
                };
                if self.alloc.regs[r.index()] == NONE {
                    return Ok(());
                }
                let dst = self.reg(r)?;
                if let Some((spill, slot)) = self.slot(var)? {
                    self.emit(|b| b.ab_int(Opcode::LoadSlot, dst, spill, slot));
                } else {
                    let src = self.var_reg(var)?;
                    self.mov(dst, src);
                }
            }
            Op::SetVar => {
                let Imm::Var(var) = insn.imm else {
                    unreachable!("a var")
                };
                let src = a(self, 0)?;
                if let Some((spill, slot)) = self.slot(var)? {
                    self.emit(|b| b.ab_int(Opcode::StoreSlot, src, spill, slot));
                } else {
                    let dst = self.var_reg(var)?;
                    self.mov(dst, src);
                }
            }
            Op::Copy | Op::IterIterator => {
                let (dst, src) = (res(self)?, a(self, 0)?);
                self.mov(dst, src);
            }
            Op::Const => {
                let Imm::Const(index) = insn.imm else {
                    unreachable!("a const")
                };
                let dst = res(self)?;
                let slot = self.pool_slot(index)?;
                self.emit(|b| b.a_const(Opcode::LoadConst, dst, slot));
            }
            Op::ConstInt => {
                let Imm::Int(value) = insn.imm else {
                    unreachable!("an int")
                };
                let dst = res(self)?;
                self.emit(|b| b.a_int(Opcode::LoadInt, dst, value));
            }
            Op::Undefined
            | Op::Null
            | Op::True
            | Op::False
            | Op::Empty
            | Op::Function
            | Op::NewTarget
            | Op::HomeObject
            | Op::NewCellEmpty => {
                let dst = res(self)?;
                let core = op.core()[0];
                self.emit(|b| b.a(core, dst));
            }
            Op::Neg
            | Op::Plus
            | Op::ToNumeric
            | Op::Inc
            | Op::Dec
            | Op::BitNot
            | Op::Not
            | Op::TypeOf
            | Op::ToPropertyKey
            | Op::IsUndefinedOrNull
            | Op::IsUndefined
            | Op::IsNull
            | Op::TypeOfIsUndefined
            | Op::TypeOfIsFunction
            | Op::ToObject
            | Op::NewCell
            | Op::LoadCell
            | Op::ForInStart
            | Op::IterNextRaw
            | Op::GetSuper
            | Op::InitDerived => {
                let (dst, src) = (res(self)?, a(self, 0)?);
                let core = op.core()[0];
                self.emit(|b| b.ab(core, dst, src));
            }
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::Shl
            | Op::Sar
            | Op::Shr
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Eq
            | Op::Neq
            | Op::StrictEq
            | Op::StrictNeq
            | Op::Lt
            | Op::Lte
            | Op::Gt
            | Op::Gte
            | Op::InstanceOf
            | Op::In
            | Op::ToPropertyKeyOf
            | Op::GetElem
            | Op::DeleteElem
            | Op::ImportCall
            | Op::IterCallNext
            | Op::GetPrivate
            | Op::PrivateIn
            | Op::GetPrivateMethod
            | Op::CallPrivateGetter
            | Op::PrivateBrandIn => {
                let (dst, l, r) = (res(self)?, a(self, 0)?, a(self, 1)?);
                let core = op.core()[0];
                self.emit(|b| b.abc(core, dst, l, r));
            }
            Op::CheckInit => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.binding_key(key)?;
                let value = a(self, 0)?;
                self.emit(|b| b.a_key(Opcode::CheckInit, value, key));
            }
            Op::CheckCellInit => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.binding_key(key)?;
                let cell = a(self, 0)?;
                self.emit(|b| b.a_key(Opcode::CheckCellInit, cell, key));
            }
            Op::GetField | Op::DeleteField => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let (dst, obj) = (res(self)?, a(self, 0)?);
                let core = op.core()[0];
                self.emit(|b| b.ab_key(core, dst, obj, key));
            }
            Op::SetField | Op::DefineField | Op::DefineGetter | Op::DefineSetter => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let (obj, value) = (a(self, 0)?, a(self, 1)?);
                let core = op.core()[0];
                self.emit(|b| b.ab_key(core, obj, value, key));
            }
            Op::SetElem
            | Op::DefineElem
            | Op::CopyDataExcluded
            | Op::PutPrivate
            | Op::DefinePrivate
            | Op::CallPrivateSetter => {
                let (x, y, z) = (a(self, 0)?, a(self, 1)?, a(self, 2)?);
                let core = op.core()[0];
                self.emit(|b| b.abc(core, x, y, z));
            }
            Op::GetGlobal | Op::GetGlobalOrUndefined | Op::DeleteGlobal | Op::GlobalRef => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let dst = res(self)?;
                let core = op.core()[0];
                self.emit(|b| b.a_key(core, dst, key));
            }
            Op::PutRef => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let (env, value) = (a(self, 0)?, a(self, 1)?);
                self.emit(|b| b.ab_key(Opcode::PutRef, env, value, key));
            }
            Op::SetGlobal => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let value = a(self, 0)?;
                self.emit(|b| b.a_key(Opcode::SetGlobal, value, key));
            }
            Op::DefineGlobal => {
                let Imm::KeyFlags(key, flags) = insn.imm else {
                    unreachable!("key and flags")
                };
                let key = self.key(key)?;
                let value = a(self, 0)?;
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::DefineGlobal,
                        value,
                        flags as u8,
                        0,
                        key.raw(),
                    ))
                });
            }
            Op::DeclareGlobal | Op::CheckGlobalVar => {
                let Imm::KeyFlags(key, flags) = insn.imm else {
                    unreachable!("key and flags")
                };
                let key = self.key(key)?;
                let core = op.core()[0];
                self.emit(|b| b.raw(oxide_core::Insn::new(core, 0, flags as u8, 0, key.raw())));
            }
            Op::DeclareGlobalLet | Op::DeclareGlobalConst | Op::CheckGlobalLexical => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let flags = if self.f.unit.flags & oxide_core::bytecode::flags::EVAL != 0 {
                    oxide_core::bytecode::opcode::global::EVAL
                } else {
                    0
                };
                let core = op.core()[0];
                self.emit(|b| b.raw(oxide_core::Insn::new(core, 0, flags, 0, key.raw())));
            }
            Op::CloseCell => {
                let (dst, src) = (res(self)?, a(self, 0)?);
                self.mov(dst, src);
                self.emit(|b| b.a(Opcode::CloseCell, dst));
            }
            Op::StoreCell
            | Op::SetProto
            | Op::SetFunctionName
            | Op::CopyData
            | Op::SetHomeObject
            | Op::IterCloseThrow
            | Op::InitFields
            | Op::RunStaticInit
            | Op::InitThisCell => {
                let (x, y) = (a(self, 0)?, a(self, 1)?);
                let core = op.core()[0];
                self.emit(|b| b.ab(core, x, y));
            }
            Op::ClearCell
            | Op::IterClose
            | Op::IterCheckObject
            | Op::CallStaticBlock
            | Op::CheckCtor => {
                let x = a(self, 0)?;
                let core = op.core()[0];
                self.emit(|b| b.a(core, x));
            }
            Op::LoadCaptured => {
                let Imm::Capture(index) = insn.imm else {
                    unreachable!("a capture")
                };
                let dst = res(self)?;
                self.emit(|b| b.a_capture(Opcode::LoadCaptured, dst, index));
            }
            Op::StoreCaptured | Op::InitThisCaptured => {
                let Imm::Capture(index) = insn.imm else {
                    unreachable!("a capture")
                };
                let value = a(self, 0)?;
                let core = op.core()[0];
                self.emit(|b| b.a_capture(core, value, index));
            }
            Op::NewArguments | Op::Rest => {
                let Imm::Int(value) = insn.imm else {
                    unreachable!("an int")
                };
                let dst = res(self)?;
                let core = op.core()[0];
                self.emit(|b| b.a_int(core, dst, value));
            }
            Op::NewObject => {
                let Imm::Flags(slots) = insn.imm else {
                    unreachable!("a count")
                };
                let dst = res(self)?;
                self.emit(|b| b.a_count(Opcode::NewObject, dst, slots));
            }
            Op::NewArray | Op::ConcatN => {
                let dst = res(self)?;
                let count = args.len() as u32;
                let base = self.window(slot, count)?;
                let moves = self.window_moves(base, args)?;
                self.parallel_move(moves);
                let core = op.core()[0];
                self.emit(|b| b.ab_range(core, dst, base, count));
            }
            Op::MakeClosure => {
                let Imm::Child(child) = insn.imm else {
                    unreachable!("a child")
                };
                let dst = res(self)?;
                self.emit(|b| b.a_child(Opcode::MakeClosure, dst, child));
            }
            Op::NewRegExp => {
                let Imm::Const(index) = insn.imm else {
                    unreachable!("a const")
                };
                let dst = res(self)?;
                let slot = self.pool_slot(index)?;
                self.emit(|b| b.a_const(Opcode::NewRegExp, dst, slot));
            }
            Op::AppendSpread => {
                // array, index (in place), source.
                let (dst, array, index, source) =
                    (res(self)?, a(self, 0)?, a(self, 1)?, a(self, 2)?);
                self.mov(dst, index);
                self.emit(|b| b.abc(Opcode::AppendSpread, array, dst, source));
            }
            Op::DefineMethod => {
                let Imm::KeyFlags(key, flags) = insn.imm else {
                    unreachable!("key and flags")
                };
                let key = self.key(key)?;
                let (obj, function) = (a(self, 0)?, a(self, 1)?);
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::DefineMethod,
                        obj,
                        function,
                        flags as u8,
                        key.raw(),
                    ))
                });
            }
            Op::DefineMethodComputed => {
                let Imm::Flags(flags) = insn.imm else {
                    unreachable!("flags")
                };
                let (obj, key, function) = (a(self, 0)?, a(self, 1)?, a(self, 2)?);
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::DefineMethodComputed,
                        obj,
                        key,
                        function,
                        flags,
                    ))
                });
            }
            Op::Call | Op::CallEval | Op::CallMethod | Op::New | Op::CallSpread | Op::NewSpread => {
                let dst = res(self)?;
                let count = args.len() as u32;
                let base = self.window(slot, count)?;
                let moves = self.window_moves(base, args)?;
                self.parallel_move(moves);
                let (core, argc) = match op {
                    Op::Call | Op::CallEval => (Opcode::Call, count - 1),
                    Op::CallMethod => (Opcode::CallMethod, count - 2),
                    Op::New => (Opcode::New, count - 2),
                    Op::CallSpread => (Opcode::CallSpread, 0),
                    _ => (Opcode::NewSpread, 0),
                };
                let argc = u8::try_from(argc).map_err(|_| {
                    LowerError::Unsupported("calls with more than 255 arguments".into())
                })?;
                let at = self.emit(|b| b.call(core, dst, base, argc));
                if op == Op::CallEval {
                    self.direct_eval_sites.push(at);
                }
            }
            Op::IterStart => {
                let Imm::Int(generic) = insn.imm else {
                    unreachable!("an int")
                };
                let (dst, iterable) = (res(self)?, a(self, 0)?);
                self.uses(u32::from(dst) + 3);
                self.emit(|b| b.ab_int(Opcode::IterStart, dst, iterable, generic));
            }
            Op::IterStartAsync => {
                let (dst, iterable) = (res(self)?, a(self, 0)?);
                self.uses(u32::from(dst) + 3);
                self.emit(|b| b.ab(Opcode::IterStartAsync, dst, iterable));
            }
            Op::IterNext => {
                let record = a(self, 0)?;
                let value = self.reg(insn.results[0].expect("a value"))?;
                let done = self.reg(insn.results[1].expect("a done flag"))?;
                self.emit(|b| b.abc(Opcode::IterNext, value, record, done));
            }
            Op::IterResult => {
                // result (in place), record → value, done.
                let (result_reg, record) = (a(self, 0)?, a(self, 1)?);
                let value = self.reg(insn.results[0].expect("a value"))?;
                let done = self.reg(insn.results[1].expect("a done flag"))?;
                self.mov(value, result_reg);
                self.emit(|b| b.abc(Opcode::IterResult, value, record, done));
            }
            Op::IterCall => {
                let Imm::Int(kind) = insn.imm else {
                    unreachable!("an int")
                };
                let (record, value) = (a(self, 0)?, a(self, 1)?);
                let pair = self.window(slot, 2)?;
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::IterCall,
                        pair,
                        record,
                        value,
                        kind as u32,
                    ))
                });
                let result = self.reg(insn.results[0].expect("a result"))?;
                let missing = self.reg(insn.results[1].expect("a missing flag"))?;
                self.parallel_move(vec![(result, pair), (missing, pair + 1)]);
            }
            Op::AsyncStart => {
                self.emit(|b| b.op(Opcode::AsyncStart));
            }
            Op::MakeClass => {
                let Imm::KeyFlags(key, heritage) = insn.imm else {
                    unreachable!("key and flags")
                };
                let key = self.key(key)?;
                let (parent, ctor) = (a(self, 0)?, a(self, 1)?);
                let class = self.reg(insn.results[0].expect("a class"))?;
                let proto = self.reg(insn.results[1].expect("a prototype"))?;
                self.parallel_move(vec![(class, parent), (proto, ctor)]);
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::MakeClass,
                        class,
                        proto,
                        heritage as u8,
                        key.raw(),
                    ))
                });
            }
            Op::InitThis | Op::CheckDerivedReturn => {
                let (dst, x, y) = (res(self)?, a(self, 0)?, a(self, 1)?);
                if dst == y && dst != x {
                    // The operand the result takes is read first.
                    return Err(LowerError::Internal(
                        "a tied result over the other operand".into(),
                    ));
                }
                self.mov(dst, x);
                let core = op.core()[0];
                self.emit(|b| b.ab(core, dst, y));
            }
            Op::GetSuperValue => {
                let Imm::Int(call) = insn.imm else {
                    unreachable!("an int")
                };
                let dst = res(self)?;
                let base = self.window(slot, 3)?;
                let moves = self.window_moves(base, args)?;
                self.parallel_move(moves);
                self.emit(|b| b.ab_int(Opcode::GetSuperValue, dst, base, call));
            }
            Op::PutSuperValue => {
                let base = self.window(slot, 4)?;
                let moves = self.window_moves(base, args)?;
                self.parallel_move(moves);
                self.emit(|b| b.a(Opcode::PutSuperValue, base));
            }
            Op::NewPrivateName => {
                let Imm::Key(key) = insn.imm else {
                    unreachable!("a key")
                };
                let key = self.key(key)?;
                let dst = res(self)?;
                self.emit(|b| b.a_key(Opcode::NewPrivateName, dst, key));
            }
            Op::NamePrivateMethod => {
                let Imm::KeyFlags(key, prefix) = insn.imm else {
                    unreachable!("key and flags")
                };
                let key = self.key(key)?;
                let function = a(self, 0)?;
                self.emit(|b| {
                    b.raw(oxide_core::Insn::new(
                        Opcode::NamePrivateMethod,
                        function,
                        0,
                        prefix as u8,
                        key.raw(),
                    ))
                });
            }
            Op::InstallFields => {
                let (x, y, z) = (a(self, 0)?, a(self, 1)?, a(self, 2)?);
                self.emit(|b| b.abc(Opcode::InstallFields, x, y, z));
            }
            _ => {
                return Err(LowerError::Unsupported(format!(
                    "the MIR operation {}",
                    op.name()
                )));
            }
        }
        Ok(())
    }

    /// The base of the window of `count` consecutive registers at `slot`,
    /// above every value live across it.
    fn window(&mut self, slot: u32, count: u32) -> Result<u8, LowerError> {
        let base = self
            .windows
            .get(&slot)
            .copied()
            .ok_or_else(|| LowerError::Internal("a window without a base".into()))?;
        self.uses(base + count);
        r8(base + count.saturating_sub(1)).map(|_| base as u8)
    }

    fn window_moves(&self, base: u8, args: &[Value]) -> Result<Vec<(u8, u8)>, LowerError> {
        let mut moves = Vec::with_capacity(args.len());
        for (index, &arg) in args.iter().enumerate() {
            let dst = r8(u32::from(base) + index as u32)?;
            moves.push((dst, self.reg(arg)?));
        }
        Ok(moves)
    }

    // ---- terminators ----

    fn terminator(&mut self, block: BlockId, slot: u32) -> Result<(), LowerError> {
        let b = self.f.block(block);
        match b.term {
            Terminator::Jump(target) => {
                self.phi_moves(block, target)?;
                if !self.rotate(target) {
                    self.jump(target);
                }
            }
            Terminator::Branch { then, else_, .. } if self.plan.constant.contains_key(&block.0) => {
                self.jump(if self.plan.constant[&block.0] {
                    then
                } else {
                    else_
                });
            }
            Terminator::Branch { then, else_, .. } if self.plan.fused.contains_key(&block.0) => {
                let fusion = self.plan.fused[&block.0].clone();
                self.fused_branch(&fusion, then, else_)?;
            }
            Terminator::Branch { cond, then, else_ } => {
                let cond = self.reg(cond)?;
                if self.falls_to(then) {
                    self.jump_to(|b| b.a_jump(Opcode::JFalse, cond, 0), else_);
                } else {
                    self.jump_to(|b| b.a_jump(Opcode::JTrue, cond, 0), then);
                    self.jump(else_);
                }
            }
            Terminator::BranchCmp {
                op,
                l,
                r,
                then,
                else_,
            } => {
                let (l, r) = (self.reg(l)?, self.reg(r)?);
                let (taken, negated) = compare_jumps(op);
                if self.falls_to(then) {
                    self.jump_to(|b| b.ab_jump(negated, l, r, 0), else_);
                } else {
                    self.jump_to(|b| b.ab_jump(taken, l, r, 0), then);
                    self.jump(else_);
                }
            }
            Terminator::BranchNullish { value, then, else_ } => {
                let value = self.reg(value)?;
                self.jump_to(|b| b.a_jump(Opcode::JUndefinedOrNull, value, 0), then);
                self.jump(else_);
            }
            Terminator::ForInNext {
                iter,
                key,
                then,
                else_,
            } => {
                let (iter, key) = (self.reg(iter)?, self.reg(key)?);
                self.jump_to(|b| b.ab_jump(Opcode::ForInNext, key, iter, 0), then);
                self.jump(else_);
            }
            Terminator::Return(value) => {
                let value = self.reg(value)?;
                self.emit(|b| b.a(Opcode::Return, value));
            }
            Terminator::ReturnUndefined => {
                self.emit(|b| b.op(Opcode::ReturnUndefined));
            }
            Terminator::Throw(value) => {
                let value = self.reg(value)?;
                self.emit(|b| b.a(Opcode::Throw, value));
            }
            Terminator::ThrowError { kind, key } => {
                let key = self.key(key)?;
                let core = match kind {
                    ThrowKind::ConstAssign => Opcode::ThrowConstAssign,
                    ThrowKind::Type => Opcode::ThrowType,
                    ThrowKind::Reference => Opcode::ThrowReference,
                    ThrowKind::Internal => Opcode::ThrowInternal,
                };
                self.emit(|b| b.key(core, key));
            }
            Terminator::Suspend {
                kind,
                value,
                sent,
                resume_kind,
                resume,
            } => {
                match kind {
                    SuspendKind::InitialYield => {
                        self.emit(|b| b.op(Opcode::InitialYield));
                    }
                    SuspendKind::Yield | SuspendKind::YieldStar => {
                        let at = self.window(slot, 2)?;
                        let value = self.reg(value.expect("a value"))?;
                        self.mov(at, value);
                        let core = if kind == SuspendKind::Yield {
                            Opcode::Yield
                        } else {
                            Opcode::YieldStar
                        };
                        self.emit(|b| b.a(core, at));
                        let mut moves = Vec::new();
                        if let Some(sent) = sent {
                            moves.push((self.reg(sent)?, at));
                        }
                        if let Some(resume_kind) = resume_kind {
                            moves.push((self.reg(resume_kind)?, at + 1));
                        }
                        self.parallel_move(moves);
                    }
                    SuspendKind::Await => {
                        let at = self.window(slot, 1)?;
                        let value = self.reg(value.expect("a value"))?;
                        self.mov(at, value);
                        self.emit(|b| b.a(Opcode::Await, at));
                        if let Some(sent) = sent {
                            let sent = self.reg(sent)?;
                            self.mov(sent, at);
                        }
                    }
                }
                self.jump(resume);
            }
            Terminator::Gosub { finally, cont } => {
                let addr = self.f.block(finally).ret_addr.expect("a return address");
                let addr = self.reg(addr)?;
                self.jump_to(|b| b.a_jump(Opcode::Gosub, addr, 0), finally);
                // `Ret` comes back to the instruction after the `Gosub`.
                if self.falls_to(cont) {
                    // Falls through.
                } else {
                    self.jump_to(|b| b.jump(Opcode::Jmp, 0), cont);
                }
            }
            Terminator::Ret(addr) => {
                let addr = self.reg(addr)?;
                self.emit(|b| b.a(Opcode::Ret, addr));
            }
            Terminator::Unreachable => {
                let message = self.tree.program.strings.len();
                let _ = message;
                self.emit(|b| b.op(Opcode::ReturnUndefined));
            }
        }
        Ok(())
    }

    /// A branch on a compare it fuses with: the compare-jump to `then`, or
    /// its negation to `else_` when `then` is next.
    fn fused_branch(
        &mut self,
        fusion: &alloc::Fusion,
        then: BlockId,
        else_: BlockId,
    ) -> Result<(), LowerError> {
        let left = self.reg(fusion.left)?;
        if fusion.op == Op::IsUndefinedOrNull {
            self.jump_to(|b| b.a_jump(Opcode::JUndefinedOrNull, left, 0), then);
            self.jump(else_);
            return Ok(());
        }
        let cmp = match fusion.op {
            Op::Lt => CmpOp::Lt,
            Op::Lte => CmpOp::Lte,
            Op::Gt => CmpOp::Gt,
            Op::Gte => CmpOp::Gte,
            Op::Eq => CmpOp::Eq,
            Op::Neq => CmpOp::Neq,
            Op::StrictEq => CmpOp::StrictEq,
            _ => CmpOp::StrictNeq,
        };
        let (taken, negated) = match fusion.imm {
            Some(_) => immediate_compare_jumps(cmp),
            None => compare_jumps(cmp),
        };
        let (op, target, other) = if self.falls_to(then) {
            (negated, else_, None)
        } else {
            (taken, then, Some(else_))
        };
        match (fusion.imm, fusion.right) {
            (Some(value), _) => self.jump_to(|b| b.a_short_jump(op, left, value, 0), target),
            (None, Some(right)) => {
                let right = self.reg(right)?;
                self.jump_to(|b| b.ab_jump(op, left, right, 0), target);
            }
            (None, None) => {
                return Err(LowerError::Internal(
                    "a compare without its right side".into(),
                ));
            }
        }
        if let Some(other) = other {
            self.jump(other);
        }
        Ok(())
    }

    /// A jump to `target`, left out when it is the next block.
    /// Whether a jump to `target` from the end of the current block is the
    /// fall-through.
    fn falls_to(&self, target: BlockId) -> bool {
        self.next_block
            .is_some_and(|next| self.resolved[next.index()] == self.resolved[target.index()])
    }

    /// [`Self::resolved`] for the blocks in layout order: a block other
    /// than the entry emits nothing when its instructions are dead and it
    /// jumps to the next block with no phi moves.
    fn resolve(&self, blocks: &[BlockId]) -> Vec<BlockId> {
        let mut resolved: Vec<BlockId> = (0..self.f.blocks.len() as u32).map(BlockId).collect();
        for (index, &block) in blocks.iter().enumerate().rev() {
            let b = self.f.block(block);
            let Terminator::Jump(target) = b.term else {
                continue;
            };
            let empty = block != BlockId::ENTRY
                && b.catch.is_none()
                && b.ret_addr.is_none()
                && self.plan.dead[block.index()].iter().all(|&dead| dead)
                && blocks.get(index + 1) == Some(&target);
            if !empty {
                continue;
            }
            let t = self.f.block(target);
            let moves = t.phis.iter().any(|phi| {
                let position = t.preds.iter().position(|&p| p == block);
                let result = self.alloc.regs[phi.result.index()];
                result != NONE
                    && position.is_none_or(|at| self.alloc.regs[phi.args[at].index()] != result)
            });
            if !moves {
                resolved[block.index()] = resolved[target.index()];
            }
        }
        resolved
    }

    /// A back edge to a loop whose first instruction is a conditional jump
    /// out of it repeats that test, negated, as a jump to the loop's body:
    /// an iteration runs one branch instead of a jump and the test. The
    /// test reads registers that hold the same values here, in the same
    /// catch region; leaving the loop jumps to the test's target.
    fn rotate(&mut self, target: BlockId) -> bool {
        let head = self.labels[target.index()];
        if head == u32::MAX {
            return false;
        }
        let Some(&test) = self.builder.code().get(head as usize) else {
            return false;
        };
        let Some(negated) = crate::engine::compiler::register::negated_jump(test.op) else {
            return false;
        };
        let Some(&(_, exit)) = self.fixups.iter().find(|&&(at, _)| at == head) else {
            return false;
        };
        if self.regions[head as usize] != self.region {
            return false;
        }
        let mut repeat = test;
        repeat.op = negated;
        repeat.imm = head + 1;
        let pos = std::mem::replace(&mut self.pos, self.positions[head as usize]);
        self.emit(|b| b.raw(repeat));
        self.pos = pos;
        self.jump(exit);
        true
    }

    fn jump(&mut self, target: BlockId) {
        if !self.falls_to(target) {
            self.jump_to(|b| b.jump(Opcode::Jmp, 0), target);
        }
    }

    /// The moves into `target`'s phis on the edge from `block`.
    fn phi_moves(&mut self, block: BlockId, target: BlockId) -> Result<(), LowerError> {
        let t = self.f.block(target);
        if t.phis.is_empty() {
            return Ok(());
        }
        let Some(position) = t.preds.iter().position(|&p| p == block) else {
            return Err(LowerError::Internal(
                "a phi edge from a block it does not list".into(),
            ));
        };
        let mut moves = Vec::with_capacity(t.phis.len());
        for phi in &t.phis {
            if self.alloc.regs[phi.result.index()] == NONE {
                continue;
            }
            moves.push((self.reg(phi.result)?, self.reg(phi.args[position])?));
        }
        self.parallel_move(moves);
        Ok(())
    }

    // ---- constants ----

    fn pool_slot(&mut self, index: u32) -> Result<u32, LowerError> {
        let slot = self.pool_slots[index as usize];
        if slot != u32::MAX {
            return Ok(slot);
        }
        let slot = self.pool.len() as u32;
        match &self.f.consts[index as usize] {
            Constant::Number(value) => self.pool.push(CoreValue::number(*value)),
            Constant::String(string) => {
                let string = *string;
                let text = self.tree.string(string).clone();
                if crate::engine::compiler::register::is_tagged_int_atom(&text) {
                    // QuickJS keeps a literal spelling an index as its own
                    // string, not an atom.
                    let heap = self.compiler.heap_mut();
                    let value = CoreValue::string(
                        heap.alloc_string_units(&text.utf16_units().collect::<Vec<_>>())?,
                    );
                    heap.roots().push(value);
                    self.pool.push(value);
                } else {
                    let atom = self.tree.atom(self.compiler, string)?;
                    self.pool.push(CoreValue::string(atom));
                }
            }
            Constant::BigInt(digits) => {
                let text = self.tree.string(*digits).to_utf8_lossy();
                let value = num_bigint::BigUint::parse_bytes(text.as_bytes(), 10)
                    .ok_or_else(|| LowerError::Internal("a BigInt literal's digits".into()))?;
                let limbs = value.to_u64_digits();
                let heap = self.compiler.heap_mut();
                let id = heap.alloc_bigint(false, &limbs)?;
                let value = CoreValue::bigint(id);
                heap.roots().push(value);
                self.pool.push(value);
            }
            Constant::Template { cooked, raw } => {
                let (cooked, raw) = (cooked.clone(), raw.clone());
                let mut cooked_values = Vec::with_capacity(cooked.len());
                for part in cooked {
                    cooked_values.push(match part {
                        Some(string) => CoreValue::string(self.tree.atom(self.compiler, string)?),
                        None => CoreValue::UNDEFINED,
                    });
                }
                let mut raw_values = Vec::with_capacity(raw.len());
                for part in raw {
                    raw_values.push(CoreValue::string(self.tree.atom(self.compiler, part)?));
                }
                let object = self.compiler.template_object(&cooked_values, &raw_values)?;
                let value = CoreValue::object(object);
                self.compiler.heap_mut().roots().push(value);
                self.pool.push(value);
            }
            Constant::RegExp { source, flags } => {
                let (source, flags) = (*source, *flags);
                let source = CoreValue::string(self.tree.atom(self.compiler, source)?);
                let flags = CoreValue::string(self.tree.atom(self.compiler, flags)?);
                self.pool.push(source);
                self.pool.push(flags);
            }
        }
        self.pool_slots[index as usize] = slot;
        Ok(slot)
    }

    // ---- tables ----

    /// One handler entry per run of instructions inside a catch region,
    /// innermost regions first.
    fn handlers(&mut self) -> Result<(), LowerError> {
        let f = self.f;
        let level = |mut h: Option<super::ir::HandlerId>| {
            let mut n = 0usize;
            while let Some(id) = h {
                n += 1;
                h = f.handlers[id.index()].parent;
            }
            n
        };
        let chain = |h: Option<super::ir::HandlerId>| {
            let mut out = Vec::new();
            let mut h = h;
            while let Some(id) = h {
                out.push(id);
                h = f.handlers[id.index()].parent;
            }
            out.reverse();
            out
        };
        let mut entries: Vec<(usize, Handler)> = Vec::new();
        // The regions open at the current instruction, outermost first, with
        // the pc their run started at.
        let mut open: Vec<(super::ir::HandlerId, u32)> = Vec::new();
        let len = self.regions.len() as u32;
        let destination = |s: &Self, id: super::ir::HandlerId| -> Result<(u32, u16), LowerError> {
            let catch = f.handlers[id.index()].catch;
            let target = s.labels[catch.index()];
            let value = f
                .block(catch)
                .catch
                .ok_or_else(|| LowerError::Internal("a catch block without its value".into()))?;
            Ok((target, register16(s.alloc.regs[value.index()])?))
        };
        let close = |s: &Self,
                     open: &mut Vec<(super::ir::HandlerId, u32)>,
                     keep: usize,
                     end: u32,
                     entries: &mut Vec<(usize, Handler)>|
         -> Result<(), LowerError> {
            while open.len() > keep {
                let (id, start) = open.pop().expect("an open region");
                if start < end {
                    let (target, register) = destination(s, id)?;
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
            }
            Ok(())
        };
        for at in 0..len {
            let wanted = chain(self.regions[at as usize]);
            let common = open
                .iter()
                .zip(&wanted)
                .take_while(|((id, _), w)| id == *w)
                .count();
            if common == open.len() && common == wanted.len() {
                continue;
            }
            close(self, &mut open, common, at, &mut entries)?;
            for &id in &wanted[common..] {
                open.push((id, at));
            }
        }
        close(self, &mut open, 0, len, &mut entries)?;
        let _ = level;
        entries.sort_by_key(|&(nesting, handler)| (std::cmp::Reverse(nesting), handler.start));
        for (_, handler) in entries {
            self.builder.handler(handler);
        }
        Ok(())
    }

    fn finish(
        mut self,
        children: Vec<CodeUnitId>,
        captures: Vec<Capture>,
    ) -> Result<(CodeUnit, Vec<CoreValue>), LowerError> {
        let f = self.f;
        let unit = &f.unit;
        let positions = self.positions_table();
        let name_atom = match unit.name_atom {
            Some(name) if !self.tree.string(name).is_empty() => {
                Some(self.tree.atom(self.compiler, name)?)
            }
            _ => None,
        };
        let name = unit
            .name
            .map(|name| self.tree.string(name).to_utf8_lossy())
            .unwrap_or_default();
        let source = if self.tree.debug.keep_source {
            unit.source.and_then(|(start, end)| {
                self.tree
                    .debug
                    .source
                    .and_then(|text| text.get(start as usize..end as usize))
                    .map(Box::from)
            })
        } else {
            None
        };
        let definition = self.locate(unit.definition);
        self.builder.mark_back_edges();
        let register_count = register16(self.register_count.max(self.layout.temp_base))?;
        let mut core_unit = self.builder.finish(UnitLayout {
            register_count,
            parameter_count: unit.param_count,
            local_count: self.layout.locals as u16 + u16::from(self.layout.new_target.is_some()),
            flags: unit.flags,
            this_slots: unit.this_slots,
            constant_count: self.pool.len() as u32,
            spill_register: self.layout.spill.map(|(register, _)| register as u16),
            spill_slots: self.layout.spill.map_or(0, |(_, slots)| slots as u16),
            children,
            captures,
            name,
            name_atom,
            length: unit.length,
            source,
            filename: self.tree.debug.filename.clone(),
        });
        core_unit.debug.positions = positions;
        core_unit.debug.definition = definition;
        core_unit.direct_eval_sites = self.direct_eval_sites.into();
        Ok((core_unit, self.pool))
    }

    fn locate(&self, pos: Pos) -> Option<(u32, u32)> {
        let locator = self.tree.locator.as_ref()?;
        let offset = if pos == Pos::NONE { 0 } else { pos.0 };
        locator
            .locate_byte_offset(offset as usize)
            .ok()?
            .one_based()
    }

    /// The position of each emitted instruction, one entry per change.
    fn positions_table(&self) -> Box<[oxide_core::bytecode::SourcePosition]> {
        let Some(locator) = &self.tree.locator else {
            return Box::new([]);
        };
        let mut cursor = locator.cursor();
        let definition = self.f.unit.definition;
        let mut table: Vec<oxide_core::bytecode::SourcePosition> = Vec::new();
        let mut last_pos = None;
        for (pc, &pos) in self.positions.iter().enumerate() {
            let pos = if pos == Pos::NONE { definition } else { pos };
            if last_pos == Some(pos) {
                continue;
            }
            last_pos = Some(pos);
            let offset = if pos == Pos::NONE { 0 } else { pos.0 };
            let Some((line, column)) = cursor
                .locate_byte_offset(offset as usize)
                .ok()
                .and_then(|at| at.one_based())
            else {
                continue;
            };
            if table
                .last()
                .is_some_and(|last| (last.line, last.column) == (line, column))
            {
                continue;
            }
            table.push(oxide_core::bytecode::SourcePosition {
                pc: pc as u32,
                line,
                column,
            });
        }
        table.into()
    }
}

/// [`compare_jumps`] against an `i16` immediate.
fn immediate_compare_jumps(op: CmpOp) -> (Opcode, Opcode) {
    match op {
        CmpOp::Lt => (Opcode::JLtI, Opcode::JNltI),
        CmpOp::Lte => (Opcode::JLteI, Opcode::JNlteI),
        CmpOp::Gt => (Opcode::JGtI, Opcode::JNgtI),
        CmpOp::Gte => (Opcode::JGteI, Opcode::JNgteI),
        CmpOp::Eq => (Opcode::JEqI, Opcode::JNeqI),
        CmpOp::Neq => (Opcode::JNeqI, Opcode::JEqI),
        CmpOp::StrictEq => (Opcode::JStrictEqI, Opcode::JStrictNeqI),
        CmpOp::StrictNeq => (Opcode::JStrictNeqI, Opcode::JStrictEqI),
    }
}

/// The compare-jump taken when `op` holds, and the one taken when it does
/// not.
fn compare_jumps(op: CmpOp) -> (Opcode, Opcode) {
    match op {
        CmpOp::Lt => (Opcode::JLt, Opcode::JNlt),
        CmpOp::Lte => (Opcode::JLte, Opcode::JNlte),
        CmpOp::Gt => (Opcode::JGt, Opcode::JNgt),
        CmpOp::Gte => (Opcode::JGte, Opcode::JNgte),
        CmpOp::Eq => (Opcode::JEq, Opcode::JNeq),
        CmpOp::Neq => (Opcode::JNeq, Opcode::JEq),
        CmpOp::StrictEq => (Opcode::JStrictEq, Opcode::JStrictNeq),
        CmpOp::StrictNeq => (Opcode::JStrictNeq, Opcode::JStrictEq),
    }
}
