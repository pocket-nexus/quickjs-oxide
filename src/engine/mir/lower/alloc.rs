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
use oxide_core::FxHashMap;

/// No register: the value is never live.
pub(super) const NONE: u32 = u32::MAX;

/// Where a function's variables live.
#[derive(Clone, Debug)]
pub(super) struct Layout {
    /// The register of each variable.
    pub(super) var_regs: Vec<u32>,
    /// The register that receives `new.target`, when the unit reads it.
    pub(super) new_target: Option<u32>,
    pub(super) param_count: u32,
    /// Registers the variables other than `this`, the parameters and
    /// `new.target` take.
    pub(super) locals: u32,
    pub(super) temp_base: u32,
}

impl Layout {
    pub(super) fn of(f: &Function) -> Layout {
        let param_count = u32::from(f.unit.param_count);
        let has_new_target = f.vars.iter().any(|var| var.kind == VarKind::NewTarget);
        let new_target = has_new_target.then_some(1 + param_count);
        let first_local = 1 + param_count + u32::from(has_new_target);
        let mut next = first_local;
        let var_regs = f
            .vars
            .iter()
            .map(|var| match var.kind {
                VarKind::This => 0,
                VarKind::Param(index) => 1 + index,
                VarKind::NewTarget => 1 + param_count,
                VarKind::Local => {
                    let register = next;
                    next += 1;
                    register
                }
            })
            .collect();
        Layout {
            var_regs,
            new_target,
            param_count,
            locals: next - first_local,
            temp_base: next,
        }
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
    pub(super) ranges: Vec<Vec<(u32, u32)>>,
}

/// Which instructions the lowering skips, which values read a variable's
/// register and which write into one.
pub(super) struct Plan {
    /// Instructions not emitted: `dead[block][insn]`.
    pub(super) dead: Vec<Vec<bool>>,
    /// Values bound to a fixed register: an unchanged variable's, or the
    /// one a `SetVar` right after the definition stores to.
    pub(super) fixed: FxHashMap<Value, u32>,
    /// `SetVar`s whose value is already written in place.
    pub(super) folded: Vec<Vec<bool>>,
    /// The uses of each value.
    pub(super) uses: Vec<u32>,
}

/// Counts uses, removes unused pure instructions, and binds variable reads
/// and writes to the variables' registers where that is safe.
pub(super) fn plan(f: &Function, layout: &Layout, order: &Order) -> Plan {
    let mut uses = vec![0u32; f.values.len()];
    let count = |uses: &mut Vec<u32>, v: Value| uses[v.index()] += 1;
    for &block in &order.blocks {
        let b = f.block(block);
        for insn in &b.insns {
            for &arg in insn.args.as_slice() {
                count(&mut uses, arg);
            }
        }
        for v in b.term.uses() {
            count(&mut uses, v);
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
    let mut fixed = FxHashMap::default();
    let mut folded: Vec<Vec<bool>> = f
        .blocks
        .iter()
        .map(|b| vec![false; b.insns.len()])
        .collect();
    for &block in &order.blocks {
        let b = f.block(block);
        // The last instruction index each value is used at in this block,
        // and whether it is used outside (terminator use counts as after
        // every instruction).
        let mut last_use: FxHashMap<Value, usize> = FxHashMap::default();
        let mut local_uses: FxHashMap<Value, u32> = FxHashMap::default();
        for (index, insn) in b.insns.iter().enumerate() {
            if dead[block.index()][index] {
                continue;
            }
            for &arg in insn.args.as_slice() {
                last_use.insert(arg, index);
                *local_uses.entry(arg).or_default() += 1;
            }
        }
        let term_index = b.insns.len();
        for v in b.term.uses() {
            last_use.insert(v, term_index);
            *local_uses.entry(v).or_default() += 1;
        }
        for (index, insn) in b.insns.iter().enumerate() {
            if dead[block.index()][index] {
                continue;
            }
            match insn.op {
                Op::GetVar => {
                    let r = insn.results[0].expect("a result");
                    let Some(&last) = last_use.get(&r) else {
                        continue;
                    };
                    if local_uses.get(&r).copied().unwrap_or(0) != uses[r.index()] {
                        continue;
                    }
                    let crate::engine::mir::ir::Imm::Var(var) = insn.imm else {
                        continue;
                    };
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
                        fixed.insert(r, layout.var_regs[var as usize]);
                    }
                }
                Op::SetVar => {
                    let value = insn.args.as_slice()[0];
                    let crate::engine::mir::ir::Imm::Var(var) = insn.imm else {
                        continue;
                    };
                    if index == 0 || uses[value.index()] != 1 || fixed.contains_key(&value) {
                        continue;
                    }
                    // The previous live instruction defines the value.
                    let Some(previous) = (0..index).rev().find(|&i| !dead[block.index()][i]) else {
                        continue;
                    };
                    let def = &b.insns[previous];
                    if def.results[0] != Some(value) || !retargetable(def.op) {
                        continue;
                    }
                    let register = layout.var_regs[var as usize];
                    // An operand of the definition may be an alias of a
                    // variable that this write changes while the alias is
                    // still to be read later: only the definition reads it.
                    fixed.insert(value, register);
                    folded[block.index()][index] = true;
                }
                _ => {}
            }
        }
    }
    Plan {
        dead,
        fixed,
        folded,
        uses,
    }
}

/// Whether an instruction's single result may be any register, read
/// operands first: the definitions a `SetVar` can retarget.
fn retargetable(op: Op) -> bool {
    !matches!(op, Op::GetVar | Op::SetVar | Op::Copy)
        && op.results() == 1
        && op.tied() == [None, None]
        && !early_def(op)
        && !is_window(op)
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
    let nvalues = f.values.len();
    let conts = ret_conts(f);
    let needs = |v: Value| !plan.fixed.contains_key(&v);
    // Live-in sets by block, as sorted value lists.
    let nblocks = f.blocks.len();
    let mut live_in: Vec<Vec<Value>> = vec![Vec::new(); nblocks];
    let catch_of = |block: BlockId| -> Option<BlockId> {
        f.block(block).handler.map(|h| f.handlers[h.index()].catch)
    };
    let entry_defs = |block: BlockId| -> Vec<Value> {
        let b = f.block(block);
        let mut defs: Vec<Value> = b.phis.iter().map(|phi| phi.result).collect();
        defs.extend(b.catch);
        defs.extend(b.ret_addr);
        defs
    };
    // Values a terminator defines for a successor are defined at that
    // successor's entry.
    let mut edge_defs: Vec<Vec<Value>> = vec![Vec::new(); nblocks];
    for &block in &order.blocks {
        for (v, succ) in f.block(block).term.defs() {
            edge_defs[succ.index()].push(v);
        }
    }
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
            let catch_live: Vec<Value> = catch_of(block)
                .map(|catch| live_in[catch.index()].clone())
                .unwrap_or_default();
            for &v in &catch_live {
                add(v, &mut live, &mut set);
            }
            // The terminator's defs are its successors' entry defs.
            for v in b.term.uses() {
                add(v, &mut live, &mut set);
            }
            for (index, insn) in b.insns.iter().enumerate().rev() {
                if plan.dead[block.index()][index] {
                    continue;
                }
                for r in insn.results.iter().flatten() {
                    if set[r.index()] {
                        set[r.index()] = false;
                        live.retain(|v| v != r);
                    }
                }
                for &arg in insn.args.as_slice() {
                    add(arg, &mut live, &mut set);
                }
            }
            for v in entry_defs(block)
                .into_iter()
                .chain(edge_defs[block.index()].iter().copied())
            {
                if set[v.index()] {
                    set[v.index()] = false;
                    live.retain(|w| *w != v);
                }
            }
            for &v in &live {
                set[v.index()] = false;
            }
            live.sort_unstable();
            if live != live_in[block.index()] {
                live_in[block.index()] = live;
                changed = true;
            }
        }
    }

    // Ranges, block by block, backwards.
    let mut ranges: Vec<Vec<(u32, u32)>> = vec![Vec::new(); nvalues];
    let mut open: Vec<u32> = vec![u32::MAX; nvalues];
    let add_range = |ranges: &mut Vec<Vec<(u32, u32)>>, v: Value, start: u32, end: u32| {
        let list = &mut ranges[v.index()];
        if start >= end {
            return;
        }
        // Blocks are walked last to first: ranges arrive in descending order.
        if let Some(first) = list.last_mut()
            && first.0 <= end
        {
            first.0 = first.0.min(start);
            return;
        }
        list.push((start, end));
    };
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
        let catch_live: Vec<Value> = catch_of(block)
            .map(|catch| live_in[catch.index()].clone())
            .unwrap_or_default();
        for &v in &catch_live {
            set_live(v, block_end, &mut open, &mut live);
        }
        let term_slot = end_slot - 1;
        for v in b.term.uses() {
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
                    add_range(&mut ranges, *r, def_at, 2 * slot + 2);
                } else if !catch_live.contains(r) {
                    add_range(&mut ranges, *r, def_at, end);
                    open[r.index()] = u32::MAX;
                    live.retain(|v| v != r);
                }
            }
            for &arg in insn.args.as_slice() {
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
                add_range(&mut ranges, v, block_start, block_start + 1);
            } else {
                add_range(&mut ranges, v, block_start, end);
                open[v.index()] = u32::MAX;
                live.retain(|w| *w != v);
            }
        }
        for v in live {
            add_range(&mut ranges, v, 2 * entry, open[v.index()]);
            open[v.index()] = u32::MAX;
        }
    }
    for list in &mut ranges {
        list.reverse();
    }

    // Linear scan: values by their first range.
    let mut values: Vec<Value> = (0..nvalues as u32)
        .map(Value)
        .filter(|v| !ranges[v.index()].is_empty())
        .collect();
    values.sort_by_key(|v| ranges[v.index()][0].0);
    let mut regs = vec![NONE; nvalues];
    for (&value, &register) in &plan.fixed {
        regs[value.index()] = register;
    }
    // The ranges each register holds, sorted.
    let mut occupied: Vec<Vec<(u32, u32)>> = Vec::new();
    let free = |occupied: &Vec<Vec<(u32, u32)>>, register: usize, want: &[(u32, u32)]| -> bool {
        let Some(held) = occupied.get(register) else {
            return true;
        };
        want.iter().all(|&(start, end)| {
            let at = held.partition_point(|&(_, e)| e <= start);
            held.get(at).is_none_or(|&(s, _)| s >= end)
        })
    };
    let mut used = layout.temp_base;
    for value in values {
        let width = usize::from(f.values[value.index()].width.max(1));
        let want = &ranges[value.index()];
        let mut register = layout.temp_base as usize;
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
                let at = held.partition_point(|&(s, _)| s < start);
                held.insert(at, (start, end));
            }
        }
        regs[value.index()] = register as u32;
        used = used.max((register + width) as u32);
    }
    Allocation { regs, used, ranges }
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
    for (index, list) in alloc.ranges.iter().enumerate() {
        let register = alloc.regs[index];
        if register == NONE || register < layout.temp_base {
            continue;
        }
        let top = register + u32::from(f.values[index].width.max(1)) - 1;
        for &(start, end) in list {
            events.push((start, end, top));
        }
    }
    events.sort_unstable();
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
