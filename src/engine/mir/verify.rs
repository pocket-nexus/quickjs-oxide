//! The MIR invariants every pass keeps: each value defined once, every use
//! dominated by its definition, phis matching predecessors, operand counts
//! matching their operations.
//!
//! A throw can leave a block before any of its instructions run, so each
//! block is two nodes for dominance: its entry (phis, the catch value and
//! the return address of a `finally` subroutine) and
//! its body (instructions and terminator). The entry precedes the body; a
//! normal edge leaves the body, the edge to the block's catch region leaves
//! the entry. A value defined in a region's body is therefore not available
//! in its catch block, and a value a terminator defines for one successor
//! belongs to that successor's entry, which must have no other predecessor.

use super::ir::{BlockId, Function, Value};
use super::op::Arity;

/// The first broken invariant, for the dump and the tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyError(pub String);

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MIR verification failed: {}", self.0)
    }
}

impl std::error::Error for VerifyError {}

fn fail<T>(message: String) -> Result<T, VerifyError> {
    Err(VerifyError(message))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Def {
    Undefined,
    /// A parameter: defined before the entry block.
    Param,
    /// A phi, the catch value or an edge definition: the block's entry node.
    Entry(u32),
    /// An instruction result: the block's body node, at this position.
    Body(u32, u32),
}

/// The dominator tree over the entry and body nodes (node `2b` is block
/// `b`'s entry, `2b + 1` its body).
struct Dominators {
    idom: Vec<Option<usize>>,
    order: Vec<usize>,
}

impl Dominators {
    fn new(f: &Function) -> Self {
        let n = 2 * f.blocks.len();
        let mut succs = vec![Vec::new(); n];
        let mut preds = vec![Vec::new(); n];
        let mut edge = |from: usize, to: usize| {
            succs[from].push(to);
            preds[to].push(from);
        };
        for (b, block) in f.blocks.iter().enumerate() {
            edge(2 * b, 2 * b + 1);
            for succ in block.term.successors() {
                edge(2 * b + 1, 2 * succ.index());
            }
            if let Some(h) = block.handler
                && let Some(handler) = f.handlers.get(h.index())
            {
                edge(2 * b, 2 * handler.catch.index());
            }
        }
        // Reverse postorder from the entry's entry node.
        let mut rpo = Vec::with_capacity(n);
        let mut seen = vec![false; n];
        let mut stack = vec![(0usize, 0usize)];
        seen[0] = true;
        while let Some((node, next)) = stack.pop() {
            if let Some(&succ) = succs[node].get(next) {
                stack.push((node, next + 1));
                if !seen[succ] {
                    seen[succ] = true;
                    stack.push((succ, 0));
                }
            } else {
                rpo.push(node);
            }
        }
        rpo.reverse();
        let mut order = vec![usize::MAX; n];
        for (i, &node) in rpo.iter().enumerate() {
            order[node] = i;
        }
        // Cooper, Harvey and Kennedy's iteration.
        let mut idom: Vec<Option<usize>> = vec![None; n];
        idom[0] = Some(0);
        let mut changed = true;
        while changed {
            changed = false;
            for &node in rpo.iter().skip(1) {
                let mut new = None;
                for &p in &preds[node] {
                    if idom[p].is_none() {
                        continue;
                    }
                    new = Some(match new {
                        None => p,
                        Some(q) => {
                            let (mut a, mut b) = (p, q);
                            while a != b {
                                while order[a] > order[b] {
                                    a = idom[a].expect("processed");
                                }
                                while order[b] > order[a] {
                                    b = idom[b].expect("processed");
                                }
                            }
                            a
                        }
                    });
                }
                if new.is_some() && idom[node] != new {
                    idom[node] = new;
                    changed = true;
                }
            }
        }
        Self { idom, order }
    }

    fn reachable(&self, node: usize) -> bool {
        self.idom[node].is_some()
    }

    fn dominates(&self, a: usize, mut b: usize) -> bool {
        loop {
            if a == b {
                return true;
            }
            match self.idom[b] {
                Some(up) if up != b && self.order[up] < self.order[b] => b = up,
                _ => return false,
            }
        }
    }
}

/// Checks `f`'s invariants.
///
/// # Errors
/// Returns the first invariant `f` breaks.
pub fn verify(f: &Function) -> Result<(), VerifyError> {
    let nblocks = f.blocks.len();
    if nblocks == 0 {
        return fail("no entry block".into());
    }
    let block_ok = |b: BlockId| b.index() < nblocks;
    let mut defs = vec![Def::Undefined; f.values.len()];
    let mut define = |v: Value, def: Def, what: &str| -> Result<(), VerifyError> {
        match defs.get_mut(v.index()) {
            None => fail(format!("{what} defines v{} past the value table", v.0)),
            Some(slot) if *slot != Def::Undefined => fail(format!("v{} is defined twice", v.0)),
            Some(slot) => {
                *slot = def;
                Ok(())
            }
        }
    };
    for &p in &f.params {
        define(p, Def::Param, "a parameter")?;
    }
    for (b, block) in f.blocks.iter().enumerate() {
        let bu = b as u32;
        for phi in &block.phis {
            define(phi.result, Def::Entry(bu), "a phi")?;
            if phi.args.len() != block.preds.len() {
                return fail(format!(
                    "a phi in b{b} has {} operands for {} predecessors",
                    phi.args.len(),
                    block.preds.len()
                ));
            }
        }
        if let Some(c) = block.catch {
            define(c, Def::Entry(bu), "a catch value")?;
        }
        if let Some(a) = block.ret_addr {
            define(a, Def::Entry(bu), "a return address")?;
        }
        for (i, insn) in block.insns.iter().enumerate() {
            let n = insn.args.len();
            let ok = match insn.op.arity() {
                Arity::Fixed(k) => n == usize::from(k),
                Arity::AtLeast(k) => n >= usize::from(k),
            };
            if !ok {
                return fail(format!("{} in b{b} has {n} operands", insn.op.name()));
            }
            let expected = usize::from(insn.op.results());
            let given = insn.results.iter().filter(|r| r.is_some()).count();
            if given != expected || (expected == 1 && insn.results[0].is_none()) {
                return fail(format!(
                    "{} in b{b} defines {given} values, not {expected}",
                    insn.op.name()
                ));
            }
            for r in insn.results.iter().flatten() {
                define(*r, Def::Body(bu, i as u32), "an instruction")?;
            }
        }
        for succ in block.term.successors() {
            if !block_ok(succ) {
                return fail(format!("b{b} jumps to the missing b{}", succ.0));
            }
            if !f.blocks[succ.index()].preds.contains(&BlockId(bu)) {
                return fail(format!("b{} does not list its predecessor b{b}", succ.0));
            }
        }
        for (v, succ) in block.term.defs() {
            if f.blocks[succ.index()].preds.len() != 1 {
                return fail(format!(
                    "b{} receives v{} from b{b} but has other predecessors",
                    succ.0, v.0
                ));
            }
            define(v, Def::Entry(succ.0), "a terminator")?;
        }
        for &p in &block.preds {
            if !block_ok(p) || !f.blocks[p.index()].term.successors().contains(&BlockId(bu)) {
                return fail(format!(
                    "b{b} lists b{} as a predecessor, which does not jump to it",
                    p.0
                ));
            }
        }
        if let Some(h) = block.handler
            && h.index() >= f.handlers.len()
        {
            return fail(format!("b{b} names the missing handler h{}", h.0));
        }
    }
    for (h, handler) in f.handlers.iter().enumerate() {
        if !block_ok(handler.catch) {
            return fail(format!(
                "handler h{h} catches at the missing b{}",
                handler.catch.0
            ));
        }
        if let Some(parent) = handler.parent
            && parent.index() >= h
        {
            return fail(format!(
                "handler h{h}'s parent h{} is not an earlier handler",
                parent.0
            ));
        }
    }

    let dom = Dominators::new(f);
    let available = |v: Value, node: usize, at: Option<u32>| -> Result<(), VerifyError> {
        let def = *defs
            .get(v.index())
            .ok_or_else(|| VerifyError(format!("v{} is past the value table", v.0)))?;
        let ok = match def {
            Def::Undefined => false,
            Def::Param => true,
            Def::Entry(b) => dom.dominates(2 * b as usize, node),
            Def::Body(b, i) => {
                let d = 2 * b as usize + 1;
                if d == node {
                    at.is_none_or(|at| i < at)
                } else {
                    dom.dominates(d, node)
                }
            }
        };
        if ok {
            Ok(())
        } else {
            fail(format!(
                "v{} is used where its definition does not dominate (node {node})",
                v.0
            ))
        }
    };
    for (b, block) in f.blocks.iter().enumerate() {
        let body = 2 * b + 1;
        if !dom.reachable(body) {
            continue;
        }
        for phi in &block.phis {
            for (arg, pred) in phi.args.iter().zip(&block.preds) {
                if dom.reachable(2 * pred.index() + 1) {
                    available(*arg, 2 * pred.index() + 1, None)?;
                }
            }
        }
        for (i, insn) in block.insns.iter().enumerate() {
            for &arg in insn.args.as_slice() {
                available(arg, body, Some(i as u32))?;
            }
        }
        for v in block.term.uses() {
            available(v, body, None)?;
        }
    }
    Ok(())
}
