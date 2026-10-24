//! Liveness and register allocation for one function.
//!
//! The variables keep the registers the core's convention gives them:
//! `this` in 0, the parameters after it, `new.target` after them, then the
//! other variables. The SSA values take the registers above, by linear scan
//! over live ranges: each value the lowest registers free for all of its
//! ranges. A value an instruction can read in a variable's register (an
//! unchanged `GetVar`) or write straight into one (the value a `SetVar`
//! stores) takes that register instead.
//!
//! Instruction slots are numbered in layout order; slot `k` reads at `2k`
//! and writes at `2k + 1`, so a result may take the register of an operand
//! the same instruction last reads. A block's entry is a slot of its own,
//! defining its phis, catch value, return address and the values its
//! predecessor's terminator defines for it.

use crate::engine::mir::ir::{BlockId, Function, Terminator, Value, VarKind};
use crate::engine::mir::op::Op;
use oxide_core::{FxHashMap, FxHashSet};

/// No register: the value is never live.
pub(super) const NONE: u32 = u32::MAX;

/// Where a function's variables live.
#[derive(Clone, Debug)]
pub(super) struct Layout {
    /// The register of each variable, or [`SLOT`] plus its spill slot.
    pub(super) var_regs: Vec<u32>,
    /// The register that receives `new.target`, when the unit reads it.
    pub(super) new_target: Option<u32>,
    pub(super) param_count: u32,
    /// Registers the variables other than `this`, the parameters and
    /// `new.target` take.
    pub(super) locals: u32,
    /// The register holding the spill scope and its slot count, when some
    /// variables live past the window.
    pub(super) spill: Option<(u32, u32)>,
    pub(super) temp_base: u32,
}

/// A variable in a slot of the spill scope: `SLOT + slot`.
pub(super) const SLOT: u32 = 1 << 30;

impl Layout {
    /// The layout keeping the first `budget` local variables in registers
    /// and the others in the spill scope (`None`: all in registers).
    pub(super) fn of(f: &Function, budget: Option<u32>) -> Layout {
        let param_count = u32::from(f.unit.param_count);
        let has_new_target = f.vars.iter().any(|var| var.kind == VarKind::NewTarget);
        let new_target = has_new_target.then_some(1 + param_count);
        let first_local = 1 + param_count + u32::from(has_new_target);
        let mut next = first_local;
        let mut slots = 0;
        let mut locals_seen = 0;
        let var_regs = f
            .vars
            .iter()
            .map(|var| match var.kind {
                VarKind::This => 0,
                VarKind::Param(index) => 1 + index,
                VarKind::NewTarget => 1 + param_count,
                VarKind::Local => {
                    locals_seen += 1;
                    if budget.is_some_and(|budget| locals_seen > budget) {
                        let slot = slots;
                        slots += 1;
                        SLOT + slot
                    } else {
                        let register = next;
                        next += 1;
                        register
                    }
                }
            })
            .collect();
        let locals = next - first_local;
        let spill = (slots > 0).then_some((next, slots));
        let temp_base = if spill.is_some() { next + 1 } else { next };
        Layout {
            var_regs,
            new_target,
            param_count,
            locals,
            spill,
            temp_base,
        }
    }

    /// The first register of the locals.
    pub(super) fn first_local(&self) -> u32 {
        1 + self.param_count + u32::from(self.new_target.is_some())
    }

    pub(super) fn in_slot(&self, var: u32) -> bool {
        self.var_regs[var as usize] >= SLOT
    }
}

/// The blocks of a function in emission order, and the slot numbering.
pub(super) struct Order {
    pub(super) blocks: Vec<BlockId>,
    /// The entry slot of each block, by block index; `u32::MAX` if left out.
    pub(super) entry: Vec<u32>,
    /// The slot after each block's terminator.
    pub(super) end: Vec<u32>,
}

impl Order {
    pub(super) fn of(f: &Function) -> Order {
        let reachable = {
            let mut seen = vec![false; f.blocks.len()];
            for block in f.reverse_postorder() {
                seen[block.index()] = true;
            }
            seen
        };
        let mut placed = vec![false; f.blocks.len()];
        let mut blocks = Vec::with_capacity(f.blocks.len());
        let candidates: Box<dyn Iterator<Item = BlockId>> = if f.layout.is_empty() {
            Box::new((0..f.blocks.len() as u32).map(BlockId))
        } else {
            Box::new(
                f.layout
                    .iter()
                    .copied()
                    .chain((0..f.blocks.len() as u32).map(BlockId)),
            )
        };
        for block in candidates {
            if reachable[block.index()] && !placed[block.index()] {
                placed[block.index()] = true;
                blocks.push(block);
            }
        }
        let mut entry = vec![u32::MAX; f.blocks.len()];
        let mut end = vec![u32::MAX; f.blocks.len()];
        let mut slot = 0;
        for &block in &blocks {
            entry[block.index()] = slot;
            slot += 1 + f.block(block).insns.len() as u32 + 1;
            end[block.index()] = slot;
        }
        Order { blocks, entry, end }
    }
}

/// The successors liveness flows from: the terminator's, the block's catch
/// region's entry, and for a `finally` subroutine's `Ret` the blocks its
/// `Gosub`s continue at.
pub(super) fn successors(
    f: &Function,
    conts: &FxHashMap<Value, Vec<BlockId>>,
    block: BlockId,
) -> Vec<BlockId> {
    let b = f.block(block);
    let mut succs = b.term.successors();
    if let Terminator::Ret(addr) = b.term
        && let Some(conts) = conts.get(&addr)
    {
        succs.extend(conts.iter().copied());
    }
    succs
}

/// The blocks each `finally` subroutine's return address returns to.
pub(super) fn ret_conts(f: &Function) -> FxHashMap<Value, Vec<BlockId>> {
    let mut by_entry: FxHashMap<BlockId, Vec<BlockId>> = FxHashMap::default();
    for block in &f.blocks {
        if let Terminator::Gosub { finally, cont } = block.term {
            by_entry.entry(finally).or_default().push(cont);
        }
    }
    let mut conts = FxHashMap::default();
    for (index, block) in f.blocks.iter().enumerate() {
        if let Some(addr) = block.ret_addr {
            conts.insert(
                addr,
                by_entry.remove(&BlockId(index as u32)).unwrap_or_default(),
            );
        }
    }
    conts
}

/// What the allocation decided.
pub(super) struct Allocation {
    /// The register of each value, [`NONE`] if it never lives.
    pub(super) regs: Vec<u32>,
    /// The registers in use: the frame's register count before windows.
    pub(super) used: u32,
    /// The ranges of each allocated value, for window placement.
    pub(super) ranges: Ranges,
}

/// Each value's live ranges, ascending: one array, with each value's
/// offset into it.
pub(super) struct Ranges {
    offsets: Vec<u32>,
    data: Vec<(u32, u32)>,
}

impl Ranges {
    /// The ranges of value `index`.
    pub(super) fn of(&self, index: usize) -> &[(u32, u32)] {
        &self.data[self.offsets[index] as usize..self.offsets[index + 1] as usize]
    }

    pub(super) fn len(&self) -> usize {
        self.offsets.len() - 1
    }
}

/// Which instructions the lowering skips, which values read a variable's
/// register and which write into one.
pub(super) struct Plan {
    /// Instructions not emitted: `dead[block][insn]`.
    pub(super) dead: Vec<Vec<bool>>,
    /// The register each value is bound to, [`NONE`] if any: an unchanged
    /// variable's, or the one a `SetVar` right after the definition
    /// stores to.
    pub(super) fixed: Vec<u32>,
    /// `SetVar`s whose value is already written in place.
    pub(super) folded: Vec<Vec<bool>>,
    /// The uses of each value.
    pub(super) uses: Vec<u32>,
    /// Binary operations whose right operand is an immediate, by block and
    /// instruction.
    pub(super) imm: FxHashMap<(u32, u32), i32>,
    /// A block's branch on the compare that is its last instruction, made
    /// one compare-jump.
    pub(super) fused: FxHashMap<u32, Fusion>,
    /// The last `+` of a chain of string concatenations, made one
    /// `ConcatN` over all the chain's operands.
    pub(super) chains: FxHashMap<(u32, u32), Vec<Value>>,
    /// Each chain's position: its first `+`'s.
    pub(super) chain_pos: FxHashMap<(u32, u32), crate::engine::mir::ir::Pos>,
    /// Steps whose operand replaces a `ToNumeric` they read: their
    /// `chains` entry holds the operand, not a concatenation.
    pub(super) converted: FxHashSet<(u32, u32)>,
    /// The instructions with a `chains` or `imm` entry: `special[block][insn]`.
    pub(super) special: Vec<Vec<bool>>,
    /// Branches on a constant, by block: the way they always go.
    pub(super) constant: FxHashMap<u32, bool>,
}

/// A compare-jump: the compare's operands, the right one maybe an
/// immediate; `mirrored` when the constant was on the left.
#[derive(Clone, Debug)]
pub(super) struct Fusion {
    pub(super) op: Op,
    pub(super) left: Value,
    pub(super) right: Option<Value>,
    pub(super) imm: Option<i16>,
}

impl Plan {
    /// The values instruction `index` of `block` reads as emitted.
    pub(super) fn args<'a>(
        &'a self,
        block: BlockId,
        index: usize,
        insn: &'a crate::engine::mir::ir::Insn,
    ) -> std::borrow::Cow<'a, [Value]> {
        if !self.special[block.index()][index] {
            return std::borrow::Cow::Borrowed(insn.args.as_slice());
        }
        let key = (block.0, index as u32);
        if let Some(chain) = self.chains.get(&key) {
            return std::borrow::Cow::Borrowed(chain);
        }
        let args = insn.args.as_slice();
        if self.imm.contains_key(&key) {
            return std::borrow::Cow::Borrowed(&args[..1]);
        }
        std::borrow::Cow::Borrowed(args)
    }

    /// The values a block's terminator reads as emitted.
    pub(super) fn term_uses(&self, block: BlockId, term: &Terminator) -> Vec<Value> {
        if self.constant.contains_key(&block.0) {
            return Vec::new();
        }
        match self.fused.get(&block.0) {
            Some(fusion) => std::iter::once(fusion.left).chain(fusion.right).collect(),
            None => term.uses(),
        }
    }
}

/// The compares a branch can fuse with.
fn compare(op: Op) -> bool {
    matches!(
        op,
        Op::Lt | Op::Lte | Op::Gt | Op::Gte | Op::Eq | Op::Neq | Op::StrictEq | Op::StrictNeq
    )
}

/// The operations with an immediate form for their right operand.
fn has_immediate(op: Op) -> bool {
    matches!(
        op,
        Op::Add
            | Op::Sub
            | Op::Mul
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Sar
            | Op::Shr
    )
}

/// The same comparison with its operands swapped.
pub(super) fn mirrored(op: Op) -> Op {
    match op {
        Op::Lt => Op::Gt,
        Op::Lte => Op::Gte,
        Op::Gt => Op::Lt,
        Op::Gte => Op::Lte,
        op => op,
    }
}

/// Counts uses, removes unused pure instructions, and binds variable reads
/// and writes to the variables' registers where that is safe.
pub(super) fn plan(f: &Function, layout: &Layout, order: &Order) -> Plan {
    // Branches on `true` or `false` in their own block: plain jumps, so a
    // constant loop test leaves a back edge that polls for interrupts.
    let mut constant = FxHashMap::default();
    for &block in &order.blocks {
        let b = f.block(block);
        let Terminator::Branch { cond, .. } = b.term else {
            continue;
        };
        let def = b
            .insns
            .iter()
            .find(|insn| insn.results.first().copied().flatten() == Some(cond));
        match def.map(|insn| insn.op) {
            Some(Op::True) => constant.insert(block.0, true),
            Some(Op::False) => constant.insert(block.0, false),
            _ => None,
        };
    }
    let mut uses = vec![0u32; f.values.len()];
    let count = |uses: &mut Vec<u32>, v: Value| uses[v.index()] += 1;
    for &block in &order.blocks {
        let b = f.block(block);
        for insn in &b.insns {
            for &arg in insn.args.as_slice() {
                count(&mut uses, arg);
            }
        }
        if !constant.contains_key(&block.0) {
            for v in b.term.uses() {
                count(&mut uses, v);
            }
        }
        for phi in &b.phis {
            for &arg in &phi.args {
                count(&mut uses, arg);
            }
        }
    }
    let mut dead: Vec<Vec<bool>> = f
        .blocks
        .iter()
        .map(|b| vec![false; b.insns.len()])
        .collect();
    // Unused pure results, backwards so a chain goes at once.
    for &block in order.blocks.iter().rev() {
        let b = f.block(block);
        for (index, insn) in b.insns.iter().enumerate().rev() {
            let removable = insn.op.removable()
                && insn.op != Op::SetVar
                && insn.results.iter().flatten().all(|r| uses[r.index()] == 0)
                && insn.results.iter().any(Option::is_some);
            if removable {
                dead[block.index()][index] = true;
                for &arg in insn.args.as_slice() {
                    uses[arg.index()] -= 1;
                }
            }
        }
    }
    let mut plan = Plan {
        dead,
        fixed: Vec::new(),
        folded: Vec::new(),
        uses,
        imm: FxHashMap::default(),
        fused: FxHashMap::default(),
        chains: FxHashMap::default(),
        chain_pos: FxHashMap::default(),
        converted: FxHashSet::default(),
        constant,
        special: Vec::new(),
    };
    fuse(f, order, &mut plan);
    let mut special: Vec<Vec<bool>> = f
        .blocks
        .iter()
        .map(|b| vec![false; b.insns.len()])
        .collect();
    for &(block, index) in plan.chains.keys().chain(plan.imm.keys()) {
        special[block as usize][index as usize] = true;
    }
    plan.special = special;
    let Plan {
        dead,
        uses,
        imm,
        fused,
        chains,
        chain_pos,
        converted,
        constant,
        special,
        ..
    } = plan;
    let mut fixed = vec![NONE; f.values.len()];
    let mut folded: Vec<Vec<bool>> = f
        .blocks
        .iter()
        .map(|b| vec![false; b.insns.len()])
        .collect();
    let view = Plan {
        dead,
        fixed: Vec::new(),
        folded: Vec::new(),
        uses,
        imm,
        fused,
        chains,
        chain_pos,
        converted,
        constant,
        special,
    };
    // The last instruction index each value is used at in the block, the
    // terminator counting as after every instruction, and its uses there.
    let mut last_use: Vec<usize> = vec![0; f.values.len()];
    let mut local_uses: Vec<u32> = vec![0; f.values.len()];
    let mut touched: Vec<Value> = Vec::new();
    for &block in &order.blocks {
        let b = f.block(block);
        let dead = &view.dead;
        let uses = &view.uses;
        for v in touched.drain(..) {
            local_uses[v.index()] = 0;
        }
        for (index, insn) in b.insns.iter().enumerate() {
            if dead[block.index()][index] {
                continue;
            }
            for &arg in view.args(block, index, insn).iter() {
                last_use[arg.index()] = index;
                if local_uses[arg.index()] == 0 {
                    touched.push(arg);
                }
                local_uses[arg.index()] += 1;
            }
        }
        let term_index = b.insns.len();
        for v in view.term_uses(block, &b.term) {
            last_use[v.index()] = term_index;
            if local_uses[v.index()] == 0 {
                touched.push(v);
            }
            local_uses[v.index()] += 1;
        }
        for (index, insn) in b.insns.iter().enumerate() {
            if dead[block.index()][index] {
                continue;
            }
            match insn.op {
                Op::GetVar => {
                    let r = insn.results[0].expect("a result");
                    if local_uses[r.index()] == 0 || local_uses[r.index()] != uses[r.index()] {
                        continue;
                    }
                    let last = last_use[r.index()];
                    let crate::engine::mir::ir::Imm::Var(var) = insn.imm else {
                        continue;
                    };
                    if layout.in_slot(var) {
                        continue;
                    }
                    // The variable must not change before the last use.
                    let changed = b.insns[index + 1..last.min(term_index)]
                        .iter()
                        .enumerate()
                        .any(|(offset, later)| {
                            !dead[block.index()][index + 1 + offset]
                                && later.op == Op::SetVar
                                && later.imm == crate::engine::mir::ir::Imm::Var(var)
                        });
                    // A use by a `SetVar` of the same variable at `last`
                    // reads before writing.
                    if !changed {
                        fixed[r.index()] = layout.var_regs[var as usize];
                    }
                }
                Op::SetVar => {
                    let value = insn.args.as_slice()[0];
                    let crate::engine::mir::ir::Imm::Var(var) = insn.imm else {
                        continue;
                    };
                    if layout.in_slot(var) || index == 0 || fixed[value.index()] != NONE {
                        continue;
                    }
                    // Other uses read the variable's register: they must be
                    // in this block, before the variable changes again.
                    if uses[value.index()] != 1 {
                        if local_uses[value.index()] != uses[value.index()] {
                            continue;
                        }
                        let last = last_use[value.index()];
                        if last <= index {
                            // A use before this write: not the definition
                            // right before it.
                            continue;
                        }
                        let changed = b.insns[index + 1..last.min(term_index)]
                            .iter()
                            .enumerate()
                            .any(|(offset, later)| {
                                !dead[block.index()][index + 1 + offset]
                                    && later.op == Op::SetVar
                                    && later.imm == crate::engine::mir::ir::Imm::Var(var)
                            });
                        if changed {
                            continue;
                        }
                    }
                    // The previous live instruction defines the value.
                    let Some(previous) = (0..index).rev().find(|&i| !dead[block.index()][i]) else {
                        continue;
                    };
                    let def = &b.insns[previous];
                    if def.results[0] != Some(value) || !retargetable(def.op) {
                        continue;
                    }
                    let key = (block.0, previous as u32);
                    if view.chains.contains_key(&key) && !view.converted.contains(&key) {
                        continue;
                    }
                    let register = layout.var_regs[var as usize];
                    // An operand of the definition may be an alias of a
                    // variable that this write changes while the alias is
                    // still to be read later: only the definition reads it.
                    fixed[value.index()] = register;
                    folded[block.index()][index] = true;
                }
                _ => {}
            }
        }
    }
    Plan {
        fixed,
        folded,
        ..view
    }
}

/// Finds the compare-jumps, immediate operands and concatenation chains.
fn fuse(f: &Function, order: &Order, plan: &mut Plan) {
    use crate::engine::mir::ir::{Constant, Imm};
    // The instruction defining each value.
    let mut def: Vec<Option<(BlockId, usize)>> = vec![None; f.values.len()];
    for &block in &order.blocks {
        for (index, insn) in f.block(block).insns.iter().enumerate() {
            for r in insn.results.iter().flatten() {
                def[r.index()] = Some((block, index));
            }
        }
    }
    let int_of = |v: Value| -> Option<i32> {
        let (block, index) = def[v.index()]?;
        let insn = &f.block(block).insns[index];
        match (insn.op, insn.imm) {
            (Op::ConstInt, Imm::Int(value)) => Some(value),
            _ => None,
        }
    };
    let constant_of = |v: Value| -> Option<&Constant> {
        let (block, index) = def[v.index()]?;
        let insn = &f.block(block).insns[index];
        match (insn.op, insn.imm) {
            (Op::Const, Imm::Const(c)) => f.consts.get(c as usize),
            _ => None,
        }
    };
    let kill_constant = |plan: &mut Plan, v: Value| {
        if let Some((block, index)) = def[v.index()] {
            plan.dead[block.index()][index] = true;
        }
    };
    // Branches on their block's last compare.
    for &block in &order.blocks {
        let b = f.block(block);
        let Terminator::Branch { cond, .. } = b.term else {
            continue;
        };
        let Some(last) = (0..b.insns.len())
            .rev()
            .find(|&i| !plan.dead[block.index()][i])
        else {
            continue;
        };
        let insn = &b.insns[last];
        if insn.results[0] != Some(cond) || plan.uses[cond.index()] != 1 {
            continue;
        }
        let args = insn.args.as_slice();
        let fusion = if compare(insn.op) {
            let (l, r) = (args[0], args[1]);
            let short = |v: Value| {
                int_of(v)
                    .and_then(|value| i16::try_from(value).ok())
                    .filter(|_| plan.uses[v.index()] == 1)
            };
            match (short(l), short(r)) {
                (_, Some(value)) => {
                    kill_constant(plan, r);
                    Fusion {
                        op: insn.op,
                        left: l,
                        right: None,
                        imm: Some(value),
                    }
                }
                (Some(value), None) => {
                    kill_constant(plan, l);
                    Fusion {
                        op: mirrored(insn.op),
                        left: r,
                        right: None,
                        imm: Some(value),
                    }
                }
                (None, None) => Fusion {
                    op: insn.op,
                    left: l,
                    right: Some(r),
                    imm: None,
                },
            }
        } else if insn.op == Op::IsUndefinedOrNull {
            Fusion {
                op: Op::IsUndefinedOrNull,
                left: args[0],
                right: None,
                imm: None,
            }
        } else {
            continue;
        };
        plan.dead[block.index()][last] = true;
        plan.fused.insert(block.0, fusion);
    }
    // Immediate right operands, when every use of the constant takes one.
    let mut foldable = vec![0u32; f.values.len()];
    for &block in &order.blocks {
        for (index, insn) in f.block(block).insns.iter().enumerate() {
            if plan.dead[block.index()][index] || !has_immediate(insn.op) {
                continue;
            }
            let right = insn.args.as_slice()[1];
            if int_of(right).is_some() {
                foldable[right.index()] += 1;
            }
        }
    }
    for &block in &order.blocks {
        for (index, insn) in f.block(block).insns.iter().enumerate() {
            if plan.dead[block.index()][index] || !has_immediate(insn.op) {
                continue;
            }
            let right = insn.args.as_slice()[1];
            if let Some(value) = int_of(right)
                && foldable[right.index()] == plan.uses[right.index()]
            {
                plan.imm.insert((block.0, index as u32), value);
                kill_constant(plan, right);
            }
        }
    }
    // A postfix update's `ToNumeric` read only by the step: `Inc` and `Dec`
    // convert their operand themselves, once, as the pair does.
    for &block in &order.blocks {
        let b = f.block(block);
        for index in 0..b.insns.len().saturating_sub(1) {
            let (convert, step) = (&b.insns[index], &b.insns[index + 1]);
            if plan.dead[block.index()][index]
                || convert.op != Op::ToNumeric
                || !matches!(step.op, Op::Inc | Op::Dec)
            {
                continue;
            }
            let numeric = convert.results[0].expect("a number");
            if plan.uses[numeric.index()] == 1 && step.args.as_slice() == [numeric] {
                plan.dead[block.index()][index] = true;
                plan.chains.insert(
                    (block.0, index as u32 + 1),
                    vec![convert.args.as_slice()[0]],
                );
                plan.converted.insert((block.0, index as u32 + 1));
            }
        }
    }
    // Chains of `+` from a string constant: `a + b + k1 + k2 ...` with one
    // of the first two a string constant and the rest primitive constants,
    // each `+` right after the constant it adds.
    const MAX_CHAIN: usize = 16;
    let primitive = |v: Value| {
        int_of(v).is_some()
            || matches!(
                constant_of(v),
                Some(Constant::Number(_) | Constant::String(_) | Constant::BigInt(_))
            )
    };
    let string = |v: Value| matches!(constant_of(v), Some(Constant::String(_)));
    for &block in &order.blocks {
        let b = f.block(block);
        let mut index = 0;
        while index < b.insns.len() {
            let insn = &b.insns[index];
            if plan.dead[block.index()][index]
                || insn.op != Op::Add
                || plan.imm.contains_key(&(block.0, index as u32))
            {
                index += 1;
                continue;
            }
            let args = insn.args.as_slice();
            if !(string(args[0]) || string(args[1])) {
                index += 1;
                continue;
            }
            let mut operands = vec![args[0], args[1]];
            let mut end = index;
            let mut value = insn.results[0].expect("a sum");
            loop {
                let (Some(k), Some(add)) = (b.insns.get(end + 1), b.insns.get(end + 2)) else {
                    break;
                };
                let Some(constant) = k.results[0] else { break };
                if operands.len() == MAX_CHAIN
                    || plan.dead[block.index()][end + 2]
                    || add.op != Op::Add
                    || plan.imm.contains_key(&(block.0, end as u32 + 2))
                    || add.args.as_slice() != [value, constant]
                    || !primitive(constant)
                    || plan.uses[value.index()] != 1
                {
                    break;
                }
                operands.push(constant);
                end += 2;
                value = add.results[0].expect("a sum");
            }
            if end > index {
                for at in (index..end).step_by(2) {
                    plan.dead[block.index()][at] = true;
                }
                plan.chains.insert((block.0, end as u32), operands);
                plan.chain_pos.insert((block.0, end as u32), insn.pos);
            }
            index = end + 1;
        }
    }
}

/// Whether an instruction's single result may be any register, read
/// operands first: the definitions a `SetVar` can retarget.
fn retargetable(op: Op) -> bool {
    !matches!(op, Op::GetVar | Op::SetVar | Op::Copy)
        && op.results() == 1
        && op.tied() == [None, None]
        && !early_def(op)
}

/// Operations whose results may not share an operand's register.
pub(super) fn early_def(op: Op) -> bool {
    matches!(
        op,
        Op::IterStart
            | Op::IterStartAsync
            | Op::IterNext
            | Op::IterCall
            | Op::IterResult
            | Op::MakeClass
            | Op::AppendSpread
            | Op::CloseCell
            | Op::InitThis
            | Op::CheckDerivedReturn
    )
}

/// Operations whose operands go in consecutive registers above every
/// value live across them.
pub(super) fn is_window(op: Op) -> bool {
    matches!(
        op,
        Op::Call
            | Op::CallEval
            | Op::CallMethod
            | Op::New
            | Op::CallSpread
            | Op::NewSpread
            | Op::NewArray
            | Op::ConcatN
            | Op::GetSuperValue
            | Op::PutSuperValue
    )
}

/// The live ranges of the values that need temporaries, and their
/// registers.
pub(super) fn allocate(f: &Function, layout: &Layout, order: &Order, plan: &Plan) -> Allocation {
    let cx = Liveness::new(f, order, plan);
    let live_in = cx.live_in();
    let ranges = cx.ranges(&live_in);
    let (regs, used) = scan(f, layout, order, plan, &ranges);
    Allocation { regs, used, ranges }
}

/// What the liveness passes read.
struct Liveness<'a> {
    f: &'a Function,
    order: &'a Order,
    plan: &'a Plan,
    conts: FxHashMap<Value, Vec<BlockId>>,
    /// Values a terminator defines for a successor are defined at that
    /// successor's entry.
    edge_defs: Vec<Vec<Value>>,
}

impl<'a> Liveness<'a> {
    fn new(f: &'a Function, order: &'a Order, plan: &'a Plan) -> Self {
        let mut edge_defs: Vec<Vec<Value>> = vec![Vec::new(); f.blocks.len()];
        for &block in &order.blocks {
            for (v, succ) in f.block(block).term.defs() {
                edge_defs[succ.index()].push(v);
            }
        }
        Self {
            f,
            order,
            plan,
            conts: ret_conts(f),
            edge_defs,
        }
    }

    fn needs(&self, v: Value) -> bool {
        self.plan.fixed[v.index()] == NONE
    }

    fn catch_of(&self, block: BlockId) -> Option<BlockId> {
        self.f
            .block(block)
            .handler
            .map(|h| self.f.handlers[h.index()].catch)
    }

    fn entry_defs(&self, block: BlockId) -> Vec<Value> {
        let b = self.f.block(block);
        let mut defs: Vec<Value> = b.phis.iter().map(|phi| phi.result).collect();
        defs.extend(b.catch);
        defs.extend(b.ret_addr);
        defs
    }

    /// Live-in sets by block, as sorted value lists.
    #[inline(never)]
    fn live_in(&self) -> Vec<Vec<Value>> {
        let (f, order, plan, conts) = (self.f, self.order, self.plan, &self.conts);
        let nvalues = f.values.len();
        let needs = |v: Value| self.needs(v);
        let catch_of = |block: BlockId| self.catch_of(block);
        let entry_defs = |block: BlockId| self.entry_defs(block);
        let edge_defs = &self.edge_defs;
        let mut live_in: Vec<Vec<Value>> = vec![Vec::new(); f.blocks.len()];
        // Without an edge back to a block at or before its source, one pass
        // in reverse layout order sees every successor's final set.
        let mut position = vec![u32::MAX; f.blocks.len()];
        for (at, &block) in order.blocks.iter().enumerate() {
            position[block.index()] = at as u32;
        }
        let backward = order.blocks.iter().any(|&block| {
            let at = position[block.index()];
            successors(f, conts, block)
                .into_iter()
                .chain(catch_of(block))
                .any(|succ| position[succ.index()] <= at)
        });
        let mut changed = true;
        let mut set = vec![false; nvalues];
        while changed {
            changed = false;
            for &block in order.blocks.iter().rev() {
                let b = f.block(block);
                let mut live: Vec<Value> = Vec::new();
                let add = |v: Value, live: &mut Vec<Value>, set: &mut Vec<bool>| {
                    if !set[v.index()] && needs(v) {
                        set[v.index()] = true;
                        live.push(v);
                    }
                };
                for succ in successors(f, &conts, block) {
                    for &v in &live_in[succ.index()] {
                        add(v, &mut live, &mut set);
                    }
                    let sb = f.block(succ);
                    if !sb.phis.is_empty()
                        && let Some(position) = sb.preds.iter().position(|&p| p == block)
                    {
                        for phi in &sb.phis {
                            add(phi.args[position], &mut live, &mut set);
                        }
                    }
                }
                if let Some(catch) = catch_of(block) {
                    for at in 0..live_in[catch.index()].len() {
                        add(live_in[catch.index()][at], &mut live, &mut set);
                    }
                }
                // The terminator's defs are its successors' entry defs.
                for v in plan.term_uses(block, &b.term) {
                    add(v, &mut live, &mut set);
                }
                for (index, insn) in b.insns.iter().enumerate().rev() {
                    if plan.dead[block.index()][index] {
                        continue;
                    }
                    // A definition ends the value's liveness; under SSA it is
                    // not read again above it, so the list drops it at the end.
                    for r in insn.results.iter().flatten() {
                        set[r.index()] = false;
                    }
                    for &arg in plan.args(block, index, insn).iter() {
                        add(arg, &mut live, &mut set);
                    }
                }
                for v in entry_defs(block)
                    .into_iter()
                    .chain(edge_defs[block.index()].iter().copied())
                {
                    set[v.index()] = false;
                }
                live.retain(|v| std::mem::take(&mut set[v.index()]));
                live.sort_unstable();
                if live != live_in[block.index()] {
                    live_in[block.index()] = live;
                    changed = backward;
                }
            }
        }

        live_in
    }

    /// The live ranges of the values that need temporaries.
    #[inline(never)]
    fn ranges(&self, live_in: &[Vec<Value>]) -> Ranges {
        let (f, order, plan, conts) = (self.f, self.order, self.plan, &self.conts);
        let nvalues = f.values.len();
        let needs = |v: Value| self.needs(v);
        let catch_of = |block: BlockId| self.catch_of(block);
        let entry_defs = |block: BlockId| self.entry_defs(block);
        let edge_defs = &self.edge_defs;
        // Ranges, block by block, backwards.
        // Built as lists from each value's earliest range, as the walk goes
        // backwards: a pool of (start, end, next) and each value's head.
        let mut lists = (
            vec![NONE; nvalues],
            Vec::<(u32, u32, u32)>::with_capacity(nvalues),
        );
        let mut open: Vec<u32> = vec![u32::MAX; nvalues];
        let add_range =
            |ranges: &mut (Vec<u32>, Vec<(u32, u32, u32)>), v: Value, start: u32, end: u32| {
                if start >= end {
                    return;
                }
                let (heads, pool) = ranges;
                let head = heads[v.index()];
                // Blocks are walked last to first: ranges arrive in descending order.
                if head != NONE && pool[head as usize].0 <= end {
                    let first = &mut pool[head as usize];
                    first.0 = first.0.min(start);
                    return;
                }
                heads[v.index()] = pool.len() as u32;
                pool.push((start, end, head));
            };
        // The values live into the block's catch region.
        let mut catching = vec![false; nvalues];
        for &block in order.blocks.iter().rev() {
            let b = f.block(block);
            let entry = order.entry[block.index()];
            let end_slot = order.end[block.index()];
            let block_start = 2 * entry + 1;
            let block_end = 2 * end_slot;
            let mut live: Vec<Value> = Vec::new();
            let set_live = |v: Value, at: u32, open: &mut Vec<u32>, live: &mut Vec<Value>| {
                if needs(v) && open[v.index()] == u32::MAX {
                    open[v.index()] = at;
                    live.push(v);
                }
            };
            for succ in successors(f, &conts, block) {
                for &v in &live_in[succ.index()] {
                    set_live(v, block_end, &mut open, &mut live);
                }
                let sb = f.block(succ);
                if !sb.phis.is_empty()
                    && let Some(position) = sb.preds.iter().position(|&p| p == block)
                {
                    for phi in &sb.phis {
                        set_live(phi.args[position], block_end, &mut open, &mut live);
                    }
                }
            }
            let catch_live: &[Value] = catch_of(block).map_or(&[], |catch| &live_in[catch.index()]);
            for &v in catch_live {
                set_live(v, block_end, &mut open, &mut live);
                catching[v.index()] = true;
            }
            let term_slot = end_slot - 1;
            for v in plan.term_uses(block, &b.term) {
                set_live(v, 2 * term_slot + 1, &mut open, &mut live);
            }
            for (index, insn) in b.insns.iter().enumerate().rev() {
                if plan.dead[block.index()][index] {
                    continue;
                }
                let slot = entry + 1 + index as u32;
                let def_at = if early_def(insn.op) || insn.op.results() == 2 {
                    2 * slot
                } else {
                    2 * slot + 1
                };
                for r in insn.results.iter().flatten() {
                    if !needs(*r) {
                        continue;
                    }
                    let end = open[r.index()];
                    if end == u32::MAX {
                        // Never read: it still takes a register for its write.
                        add_range(&mut lists, *r, def_at, 2 * slot + 2);
                    } else if !catching[r.index()] {
                        add_range(&mut lists, *r, def_at, end);
                        open[r.index()] = u32::MAX;
                    }
                }
                for &arg in plan.args(block, index, insn).iter() {
                    set_live(arg, 2 * slot + 1, &mut open, &mut live);
                }
            }
            for v in entry_defs(block)
                .into_iter()
                .chain(edge_defs[block.index()].iter().copied())
            {
                if !needs(v) {
                    continue;
                }
                let end = open[v.index()];
                if end == u32::MAX {
                    add_range(&mut lists, v, block_start, block_start + 1);
                } else {
                    add_range(&mut lists, v, block_start, end);
                    open[v.index()] = u32::MAX;
                }
            }
            for &v in catch_live {
                catching[v.index()] = false;
            }
            // The values still open are live into the block.
            for v in live {
                if open[v.index()] != u32::MAX {
                    add_range(&mut lists, v, 2 * entry, open[v.index()]);
                    open[v.index()] = u32::MAX;
                }
            }
        }
        let (heads, pool) = lists;
        let mut offsets = Vec::with_capacity(nvalues + 1);
        let mut data = Vec::with_capacity(pool.len());
        for &head in &heads {
            offsets.push(data.len() as u32);
            let mut at = head;
            while at != NONE {
                let (start, end, next) = pool[at as usize];
                data.push((start, end));
                at = next;
            }
        }
        offsets.push(data.len() as u32);
        let ranges = Ranges { offsets, data };

        ranges
    }
}

/// Linear scan over `ranges`: each value's register, and the registers
/// in use.
#[inline(never)]
fn scan(
    f: &Function,
    layout: &Layout,
    order: &Order,
    plan: &Plan,
    ranges: &Ranges,
) -> (Vec<u32>, u32) {
    let nvalues = f.values.len();
    let needs = |v: Value| plan.fixed[v.index()] == NONE;
    // Linear scan: values by their first range.
    let mut values: Vec<Value> = (0..nvalues as u32)
        .map(Value)
        .filter(|v| !ranges.of(v.index()).is_empty())
        .collect();
    values.sort_by_key(|v| ranges.of(v.index())[0].0);
    let mut regs = plan.fixed.clone();
    // The ranges each register holds, sorted.
    let mut occupied: Vec<Vec<(u32, u32)>> = Vec::new();
    let free = |occupied: &Vec<Vec<(u32, u32)>>, register: usize, want: &[(u32, u32)]| -> bool {
        let Some(held) = occupied.get(register) else {
            return true;
        };
        // Values arrive by start: most begin past all a register holds.
        if held.last().is_none_or(|&(_, end)| end <= want[0].0) {
            return true;
        }
        want.iter().all(|&(start, end)| {
            let at = held.partition_point(|&(_, e)| e <= start);
            held.get(at).is_none_or(|&(s, _)| s >= end)
        })
    };
    // A value read only as a window's operands: its slot and index there,
    // so it can be computed in place.
    let mut hints: FxHashMap<Value, (u32, u32)> = FxHashMap::default();
    for &block in &order.blocks {
        let b = f.block(block);
        let entry = order.entry[block.index()];
        for (index, insn) in b.insns.iter().enumerate() {
            if plan.dead[block.index()][index] {
                continue;
            }
            let key = (block.0, index as u32);
            let special = plan.special[block.index()][index];
            let operands: &[Value] =
                if let Some(chain) = special.then(|| plan.chains.get(&key)).flatten() {
                    if plan.converted.contains(&key) {
                        continue;
                    }
                    chain
                } else if is_window(insn.op) {
                    insn.args.as_slice()
                } else {
                    continue;
                };
            let slot = entry + 1 + index as u32;
            // A value read only here takes its first operand's slot.
            for (at, &arg) in operands.iter().enumerate() {
                let here = operands.iter().filter(|&&other| other == arg).count();
                if plan.uses[arg.index()] as usize == here && needs(arg) {
                    hints.entry(arg).or_insert((slot, at as u32));
                }
            }
        }
    }
    // The base a window at `slot` would take over what is allocated.
    let base_at = |occupied: &Vec<Vec<(u32, u32)>>, slot: u32| -> usize {
        let mut base = layout.temp_base as usize;
        for (register, held) in occupied.iter().enumerate().skip(base) {
            let at = held.partition_point(|&(start, _)| start <= 2 * slot);
            if at > 0 && held[at - 1].1 > 2 * slot + 1 {
                base = register + 1;
            }
        }
        base
    };
    let mut used = layout.temp_base;
    for value in values {
        let width = usize::from(f.values[value.index()].width.max(1));
        let want = ranges.of(value.index());
        let mut register = layout.temp_base as usize;
        let hinted = hints.get(&value).and_then(|&(slot, at)| {
            let register = base_at(&occupied, slot) + at as usize;
            (width == 1 && free(&occupied, register, want)).then_some(register)
        });
        if let Some(hinted) = hinted {
            register = hinted;
        }
        loop {
            if (0..width).all(|w| free(&occupied, register + w, want)) {
                break;
            }
            register += 1;
        }
        for w in 0..width {
            let r = register + w;
            if occupied.len() <= r {
                occupied.resize(r + 1, Vec::new());
            }
            let held = &mut occupied[r];
            for &(start, end) in want {
                if held.last().is_none_or(|&(s, _)| s < start) {
                    held.push((start, end));
                } else {
                    let at = held.partition_point(|&(s, _)| s < start);
                    held.insert(at, (start, end));
                }
            }
        }
        regs[value.index()] = register as u32;
        used = used.max((register + width) as u32);
    }
    (regs, used)
}

/// The slot and register count of each instruction or terminator whose
/// operands or results take a window, in order.
pub(super) fn window_slots(f: &Function, order: &Order, plan: &Plan) -> Vec<(u32, u32)> {
    use crate::engine::mir::ir::SuspendKind;
    let mut slots = Vec::new();
    for &block in &order.blocks {
        let b = f.block(block);
        let entry = order.entry[block.index()];
        for (index, insn) in b.insns.iter().enumerate() {
            if plan.dead[block.index()][index] {
                continue;
            }
            let count = match insn.op {
                Op::Inc | Op::Dec => continue,
                _ if plan.special[block.index()][index]
                    && plan.chains.contains_key(&(block.0, index as u32)) =>
                {
                    plan.chains[&(block.0, index as u32)].len() as u32
                }
                Op::GetSuperValue => 3,
                Op::PutSuperValue => 4,
                Op::IterCall => 2,
                op if is_window(op) => insn.args.len() as u32,
                _ => continue,
            };
            slots.push((entry + 1 + index as u32, count));
        }
        let count = match b.term {
            Terminator::Suspend {
                kind: SuspendKind::Yield | SuspendKind::YieldStar,
                ..
            } => 2,
            Terminator::Suspend {
                kind: SuspendKind::Await,
                ..
            } => 1,
            _ => continue,
        };
        slots.push((order.end[block.index()] - 1, count));
    }
    slots
}

/// The base of each window, by slot, and the registers the windows
/// reach past.
pub(super) fn windows(
    alloc: &Allocation,
    f: &Function,
    layout: &Layout,
    order: &Order,
    plan: &Plan,
) -> (FxHashMap<u32, u32>, u32) {
    let slots = window_slots(f, order, plan);
    let positions: Vec<u32> = slots.iter().map(|&(slot, _)| slot).collect();
    let bases = window_bases(alloc, f, layout, &positions);
    let top = slots
        .iter()
        .zip(&bases)
        .map(|(&(_, count), base)| base + count)
        .max()
        .unwrap_or(0);
    (positions.into_iter().zip(bases).collect(), top)
}

/// The first register above every value live across each slot in `slots`
/// (sorted), and above the variables.
pub(super) fn window_bases(
    alloc: &Allocation,
    f: &Function,
    layout: &Layout,
    slots: &[u32],
) -> Vec<u32> {
    // Every allocated range with its top register.
    let mut events: Vec<(u32, u32, u32)> = Vec::new();
    for index in 0..alloc.ranges.len() {
        let list = alloc.ranges.of(index);
        let register = alloc.regs[index];
        if register == NONE || register < layout.temp_base {
            continue;
        }
        let top = register + u32::from(f.values[index].width.max(1)) - 1;
        for &(start, end) in list {
            events.push((start, end, top));
        }
    }
    events.sort_unstable_by_key(|event| event.0);
    let mut result = Vec::with_capacity(slots.len());
    let mut next = 0;
    // Active ranges: a max-heap by top register, lazily expired.
    let mut active: std::collections::BinaryHeap<(u32, u32)> = std::collections::BinaryHeap::new();
    for &slot in slots {
        // Live across: started by the slot's reads, ending after its write.
        while next < events.len() && events[next].0 <= 2 * slot {
            let (_, end, top) = events[next];
            active.push((top, end));
            next += 1;
        }
        let mut base = layout.temp_base;
        let mut kept = Vec::new();
        while let Some((top, end)) = active.pop() {
            if end <= 2 * slot + 1 {
                // Ends here or before: no longer live across this or a
                // later slot.
                continue;
            }
            base = base.max(top + 1);
            kept.push((top, end));
            break;
        }
        for entry in kept {
            active.push(entry);
        }
        result.push(base);
    }
    result
}
