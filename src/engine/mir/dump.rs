//! The text form of a MIR function, for `qjs --dump-mir` and the tests.

use super::ir::{Args, Block, Function, Imm, Insn, Lattice, Terminator, Value};
use std::fmt::Write;

fn values(args: &[Value]) -> String {
    args.iter()
        .map(|v| format!("v{}", v.0))
        .collect::<Vec<_>>()
        .join(", ")
}

fn imm(imm: Imm) -> String {
    match imm {
        Imm::None => String::new(),
        Imm::Int(n) => format!(" #{n}"),
        Imm::Const(c) => format!(" const:{c}"),
        Imm::Key(k) => format!(" key:{k}"),
        Imm::KeyFlags(k, f) => format!(" key:{k} flags:{f}"),
        Imm::Flags(f) => format!(" flags:{f}"),
        Imm::Capture(c) => format!(" capture:{c}"),
        Imm::Child(c) => format!(" child:{c}"),
    }
}

fn insn(out: &mut String, f: &Function, insn: &Insn) {
    out.push_str("  ");
    let results: Vec<Value> = insn.results.iter().flatten().copied().collect();
    if !results.is_empty() {
        let _ = write!(out, "{} = ", values(&results));
    }
    out.push_str(insn.op.name());
    let args = Args::as_slice(&insn.args);
    if !args.is_empty() {
        let _ = write!(out, " {}", values(args));
    }
    out.push_str(&imm(insn.imm));
    let typed: Vec<String> = results
        .iter()
        .filter(|v| f.values[v.index()].ty != Lattice::Any)
        .map(|v| format!("v{}:{:?}", v.0, f.values[v.index()].ty))
        .collect();
    if !typed.is_empty() {
        let _ = write!(out, "  ; {}", typed.join(" "));
    }
    out.push('\n');
}

fn term(t: &Terminator) -> String {
    match t {
        Terminator::Jump(b) => format!("jump b{}", b.0),
        Terminator::Branch { cond, then, else_ } => {
            format!("branch v{} b{} b{}", cond.0, then.0, else_.0)
        }
        Terminator::BranchCmp {
            op,
            l,
            r,
            then,
            else_,
        } => {
            format!("branch.{op:?} v{} v{} b{} b{}", l.0, r.0, then.0, else_.0)
        }
        Terminator::BranchNullish { value, then, else_ } => {
            format!("branch.nullish v{} b{} b{}", value.0, then.0, else_.0)
        }
        Terminator::ForInNext {
            iter,
            key,
            then,
            else_,
        } => {
            format!(
                "forin.next v{} -> v{} b{} b{}",
                iter.0, key.0, then.0, else_.0
            )
        }
        Terminator::Return(v) => format!("return v{}", v.0),
        Terminator::ReturnUndefined => "return".to_string(),
        Terminator::Throw(v) => format!("throw v{}", v.0),
        Terminator::ThrowError { kind, key } => format!("throw.{kind:?} key:{key}"),
        Terminator::Suspend {
            kind,
            value,
            sent,
            resume_kind,
            resume,
        } => {
            let mut s = format!("suspend.{kind:?}");
            if let Some(v) = value {
                let _ = write!(s, " v{}", v.0);
            }
            let defs: Vec<Value> = sent.iter().chain(resume_kind).copied().collect();
            if !defs.is_empty() {
                let _ = write!(s, " -> {}", values(&defs));
            }
            let _ = write!(s, " b{}", resume.0);
            s
        }
        Terminator::Gosub { finally, cont } => format!("gosub b{} b{}", finally.0, cont.0),
        Terminator::Ret(v) => format!("ret v{}", v.0),
        Terminator::Unreachable => "unreachable".to_string(),
    }
}

fn block(out: &mut String, f: &Function, id: usize, b: &Block) {
    let _ = write!(out, "b{id}:");
    if !b.preds.is_empty() {
        let preds: Vec<String> = b.preds.iter().map(|p| format!("b{}", p.0)).collect();
        let _ = write!(out, " preds {}", preds.join(" "));
    }
    if let Some(h) = b.handler {
        let _ = write!(out, " handler h{}", h.0);
    }
    if b.loop_depth > 0 {
        let _ = write!(out, " loop {}", b.loop_depth);
    }
    out.push('\n');
    if let Some(c) = b.catch {
        let _ = writeln!(out, "  v{} = catch", c.0);
    }
    if let Some(a) = b.ret_addr {
        let _ = writeln!(out, "  v{} = retaddr", a.0);
    }
    for phi in &b.phis {
        let _ = writeln!(out, "  v{} = phi {}", phi.result.0, values(&phi.args));
    }
    for i in &b.insns {
        insn(out, f, i);
    }
    let _ = writeln!(out, "  {}", term(&b.term));
}

/// The text form of `f`.
#[must_use]
pub fn dump(f: &Function) -> String {
    let mut out = String::new();
    let _ = write!(out, "mir fn #{} params({})", f.source.0, values(&f.params));
    if !f.children.is_empty() {
        let children: Vec<String> = f.children.iter().map(|c| format!("#{}", c.0)).collect();
        let _ = write!(out, " children({})", children.join(" "));
    }
    out.push('\n');
    for (h, handler) in f.handlers.iter().enumerate() {
        let _ = write!(out, "h{h}: catch b{}", handler.catch.0);
        if let Some(p) = handler.parent {
            let _ = write!(out, " in h{}", p.0);
        }
        out.push('\n');
    }
    for (id, b) in f.blocks.iter().enumerate() {
        block(&mut out, f, id, b);
    }
    out
}
