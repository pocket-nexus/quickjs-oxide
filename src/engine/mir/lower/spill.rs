//! Values past the window: when a function's values need more registers
//! than the window holds even with every variable in the spill scope, the
//! longest-lived values become variables of their own, written once after
//! their definition and read right before each use, and the layout puts
//! them in the spill scope too. An iterator record, which the iteration
//! instructions update in place, is written back after each use.

use super::alloc::{self, Layout, Order};
use crate::engine::mir::ir::{Args, Function, Imm, Insn, Value, VarKind};
use crate::engine::mir::op::Op;

/// `f` with its longest-lived instruction results moved to variables: as
/// many as its registers overflow the window, times `2^attempt`; `None`
/// when that would move them all already.
pub(super) fn spill(f: &Function, attempt: u32) -> Option<Function> {
    let layout = Layout::of(f, Some(0));
    let order = Order::of(f);
    let plan = alloc::plan(f, &layout, &order);
    let allocation = alloc::allocate(f, &layout, &order, &plan);
    let (_, top) = alloc::windows(&allocation, f, &layout, &order, &plan);
    let overflow = allocation
        .used
        .max(top)
        .saturating_sub(oxide_core::bytecode::WINDOW) as usize;
    let count = (overflow + 16).checked_shl(attempt)?;
    // Instruction results of one register; phis, catch values, return
    // addresses and terminator-defined values stay.
    let mut candidates: Vec<(u32, Value)> = Vec::new();
    for &block in &order.blocks {
        for insn in &f.block(block).insns {
            if matches!(insn.op, Op::GetVar | Op::SetVar) {
                continue;
            }
            for &result in insn.results.iter().flatten() {
                let length: u32 = allocation
                    .ranges
                    .of(result.index())
                    .iter()
                    .map(|&(start, end)| end - start)
                    .sum();
                if length > 2 {
                    candidates.push((length, result));
                }
            }
        }
    }
    if candidates.is_empty() || (attempt > 0 && count >> 1 >= candidates.len()) {
        return None;
    }
    candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    candidates.truncate(count);
    let mut g = f.clone();
    let mut var_of = vec![None; f.values.len()];
    for &(_, value) in &candidates {
        let var = g.new_var(VarKind::Local, None);
        g.vars[var as usize].width = f.values[value.index()].width.max(1);
        var_of[value.index()] = Some(var);
    }
    let get = |g: &mut Function, var: u32, pos| {
        let value = g.new_value(Default::default());
        g.values[value.index()].width = g.vars[var as usize].width;
        let insn = Insn {
            op: Op::GetVar,
            args: Args::default(),
            results: [Some(value), None],
            imm: Imm::Var(var),
            pos,
        };
        (value, insn)
    };
    for index in 0..g.blocks.len() {
        let block = std::mem::take(&mut g.blocks[index].insns);
        let mut insns = Vec::with_capacity(block.len());
        for mut insn in block {
            let mut loaded: Vec<(Value, Value)> = Vec::new();
            for arg in insn.args.as_mut_slice() {
                if let Some(var) = var_of[arg.index()] {
                    let value = match loaded.iter().find(|&&(from, _)| from == *arg) {
                        Some(&(_, value)) => value,
                        None => {
                            let (value, load) = get(&mut g, var, insn.pos);
                            insns.push(load);
                            loaded.push((*arg, value));
                            value
                        }
                    };
                    *arg = value;
                }
            }
            let stores: Vec<(u32, Value)> = insn
                .results
                .iter()
                .flatten()
                .filter_map(|&r| var_of[r.index()].map(|var| (var, r)))
                .collect();
            let pos = insn.pos;
            insns.push(insn);
            // A record the instruction may have updated goes back.
            for &(from, value) in &loaded {
                let var = var_of[from.index()].expect("a spilled value");
                if g.vars[var as usize].width > 1 {
                    insns.push(Insn {
                        op: Op::SetVar,
                        args: Args::from_slice(&[value]),
                        results: [None, None],
                        imm: Imm::Var(var),
                        pos,
                    });
                }
            }
            for (var, value) in stores {
                insns.push(Insn {
                    op: Op::SetVar,
                    args: Args::from_slice(&[value]),
                    results: [None, None],
                    imm: Imm::Var(var),
                    pos,
                });
            }
        }
        // The terminator's operands, read at the block's end.
        let mut term = std::mem::replace(
            &mut g.blocks[index].term,
            crate::engine::mir::ir::Terminator::Unreachable,
        );
        let term_pos = g.blocks[index].term_pos;
        for operand in term.uses_mut() {
            if let Some(var) = var_of[operand.index()] {
                let (value, load) = get(&mut g, var, term_pos);
                insns.push(load);
                *operand = value;
            }
        }
        g.blocks[index].term = term;
        g.blocks[index].insns = insns;
    }
    // Phi operands, read at the end of their predecessors.
    for index in 0..g.blocks.len() {
        for p in 0..g.blocks[index].phis.len() {
            for a in 0..g.blocks[index].phis[p].args.len() {
                let arg = g.blocks[index].phis[p].args[a];
                let Some(var) = var_of[arg.index()] else {
                    continue;
                };
                let pred = g.blocks[index].preds[a];
                let pos = g.blocks[pred.index()].term_pos;
                let (value, load) = get(&mut g, var, pos);
                g.blocks[pred.index()].insns.push(load);
                g.blocks[index].phis[p].args[a] = value;
            }
        }
    }
    Some(g)
}
