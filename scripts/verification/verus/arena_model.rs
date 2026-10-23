// Inductive bitmap model. This file does not assert full Arena refinement.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub struct Word {
    pub valid: u64,
    pub live: u64,
    pub marks: u64,
    pub quarantine: u64,
}
impl Word {
    pub open spec fn wf(self) -> bool {
        self.live & !self.valid == 0
        && self.quarantine & !self.valid == 0
        && self.live & self.quarantine == 0
    }
    pub open spec fn reusable(self) -> u64 {
        self.valid & !self.live & !self.quarantine
    }
}
pub enum Op {
    Append { mask: u64 },
    Reuse { mask: u64 },
    Free { mask: u64 },
    Mark { mask: u64 },
    ClearMarks,
    Sweep { stress: bool },
    ReleaseQuarantine,
    Trim { mask: u64 },
    Write,
}
pub open spec fn legal(s: Word, op: Op) -> bool {
    match op {
        Op::Append { mask } => mask & s.valid == 0,
        Op::Reuse { mask } => mask & !s.reusable() == 0,
        Op::Free { mask } | Op::Mark { mask } => mask & !s.live == 0,
        Op::Trim { mask } => mask & !s.reusable() == 0,
        _ => true,
    }
}
pub open spec fn step(s: Word, op: Op) -> Word {
    match op {
        Op::Append { mask } => Word { valid: s.valid | mask, live: s.live | mask, ..s },
        Op::Reuse { mask } => Word { live: s.live | mask, ..s },
        Op::Free { mask } => Word { live: s.live & !mask, ..s },
        Op::Mark { mask } => Word { marks: s.marks | mask, ..s },
        Op::ClearMarks => Word { marks: 0, ..s },
        Op::Sweep { stress } => Word {
            live: s.live & s.marks,
            quarantine: if stress { s.live & !s.marks } else { 0 },
            ..s
        },
        Op::ReleaseQuarantine => Word { quarantine: 0, ..s },
        Op::Trim { mask } => Word { valid: s.valid & !mask, ..s },
        Op::Write => s,
    }
}
#[verifier::bit_vector]
proof fn append(v: u64, l: u64, q: u64, b: u64)
    requires l & !v == 0, q & !v == 0, l & q == 0, b & v == 0,
    ensures (l | b) & !(v | b) == 0, q & !(v | b) == 0, (l | b) & q == 0,
{}
#[verifier::bit_vector]
proof fn reuse(v: u64, l: u64, q: u64, b: u64)
    requires l & !v == 0, q & !v == 0, l & q == 0, b & !(v & !l & !q) == 0,
    ensures (l | b) & !v == 0, (l | b) & q == 0,
{}
#[verifier::bit_vector]
proof fn free(v: u64, l: u64, q: u64, b: u64)
    requires l & !v == 0, l & q == 0,
    ensures (l & !b) & !v == 0, (l & !b) & q == 0,
{}
#[verifier::bit_vector]
proof fn sweep(v: u64, l: u64, m: u64)
    requires l & !v == 0,
    ensures (l & m) & !v == 0, (l & !m) & !v == 0, (l & m) & (l & !m) == 0, (l & m) & 0u64 == 0,
{}
#[verifier::bit_vector]
proof fn trim(v: u64, l: u64, q: u64, b: u64)
    requires l & !v == 0, q & !v == 0, b & !(v & !l & !q) == 0,
    ensures l & !(v & !b) == 0, q & !(v & !b) == 0,
{}
pub proof fn constructor()
    ensures (Word { valid: 0, live: 0, marks: 0, quarantine: 0 }).wf(),
{ assert(0u64 & !0u64 == 0 && 0u64 & 0u64 == 0) by(bit_vector); }
pub proof fn preserves(s: Word, op: Op)
    requires s.wf(), legal(s, op),
    ensures step(s, op).wf(),
{
    let v = s.valid; let l = s.live; let q = s.quarantine;
    assert(l & 0u64 == 0 && 0u64 & !v == 0) by(bit_vector);
    match op {
        Op::Append { mask } => { append(v, l, q, mask); },
        Op::Reuse { mask } => { reuse(v, l, q, mask); },
        Op::Free { mask } => { free(v, l, q, mask); },
        Op::Mark { mask } => {},
        Op::ClearMarks => {},
        Op::Sweep { stress } => { sweep(v, l, s.marks); },
        Op::ReleaseQuarantine => {},
        Op::Trim { mask } => { trim(v, l, q, mask); },
        Op::Write => {},
    }
}
pub open spec fn run(s: Word, ops: Seq<Op>) -> Word
    decreases ops.len(),
{
    if ops.len() == 0 { s } else { run(step(s, ops[0]), ops.drop_first()) }
}
pub open spec fn legal_run(s: Word, ops: Seq<Op>) -> bool
    decreases ops.len(),
{
    ops.len() == 0 || (legal(s, ops[0]) && legal_run(step(s, ops[0]), ops.drop_first()))
}
pub proof fn arbitrary_finite_sequence(s: Word, ops: Seq<Op>)
    requires s.wf(), legal_run(s, ops),
    ensures run(s, ops).wf(),
    decreases ops.len(),
{
    if ops.len() != 0 {
        preserves(s, ops[0]);
        arbitrary_finite_sequence(step(s, ops[0]), ops.drop_first());
    }
}
pub proof fn frame_other_words(words: Seq<Word>, index: int, op: Op)
    requires 0 <= index < words.len(),
        forall|i: int| 0 <= i < words.len() ==> words[i].wf(),
        legal(words[index], op),
    ensures
        forall|i: int| 0 <= i < words.len() ==> words.update(index, step(words[index], op))[i].wf(),
        forall|i: int| 0 <= i < words.len() && i != index ==> words.update(index, step(words[index], op))[i] == words[i],
{
    preserves(words[index], op);
}
pub proof fn sweep_preserves_marked(s: Word, stress: bool)
    requires s.wf(),
    ensures
        step(s, Op::Sweep { stress }).live == s.live & s.marks,
        step(s, Op::Sweep { stress }).quarantine == if stress { s.live & !s.marks } else { 0 },
{}
fn main() {}
}
