//! The MIR of one function: an SSA graph of basic blocks. Values are numbered
//! per function; a captured binding lives in a cell, which is an ordinary
//! value, so only uncaptured bindings become SSA values and phis.

use super::op::Op;
use crate::engine::hir::schema::{DeclRef, FunctionRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Value(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandlerId(pub u32);

impl Value {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl BlockId {
    pub const ENTRY: Self = Self(0);

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl HandlerId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A source position: the byte offset of the HIR node an instruction came
/// from, for the lowering's position table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos(pub u32);

impl Pos {
    pub const NONE: Self = Self(u32::MAX);
}

impl Default for Pos {
    fn default() -> Self {
        Self::NONE
    }
}

/// An instruction's immediate, as its operation's `ImmKind` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imm {
    None,
    Int(i32),
    Const(u32),
    Key(u32),
    KeyFlags(u32, u32),
    Flags(u32),
    Capture(u32),
    Child(u32),
    /// A variable of the function.
    Var(u32),
}

/// What the middle end knows about a value: the type lattice of M1, used
/// only to choose instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lattice {
    /// Nothing known.
    #[default]
    Any,
    /// An integer in `i32` range.
    Int32,
    Number,
    Boolean,
    String,
    Undefined,
    Null,
    /// A cell, an iterator record, a for-in enumeration or a private name:
    /// never a JavaScript value.
    Internal,
}

/// The operands of an instruction: up to three inline, more on the heap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Args {
    Inline(u8, [Value; 3]),
    Heap(Vec<Value>),
}

impl Default for Args {
    fn default() -> Self {
        Self::Inline(0, [Value(0); 3])
    }
}

impl Args {
    #[must_use]
    pub fn from_slice(values: &[Value]) -> Self {
        if values.len() <= 3 {
            let mut inline = [Value(0); 3];
            inline[..values.len()].copy_from_slice(values);
            Self::Inline(values.len() as u8, inline)
        } else {
            Self::Heap(values.to_vec())
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Value] {
        match self {
            Self::Inline(len, values) => &values[..usize::from(*len)],
            Self::Heap(values) => values,
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [Value] {
        match self {
            Self::Inline(len, values) => &mut values[..usize::from(*len)],
            Self::Heap(values) => values,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Insn {
    pub op: Op,
    pub args: Args,
    /// The values it defines: as many as `op.results()`.
    pub results: [Option<Value>; 2],
    pub imm: Imm,
    pub pos: Pos,
}

impl Insn {
    #[must_use]
    pub fn result(&self) -> Option<Value> {
        self.results[0]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phi {
    pub result: Value,
    /// One operand per predecessor, in the block's `preds` order.
    pub args: Vec<Value>,
}

/// The comparisons a fused compare-and-branch tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
    Neq,
    StrictEq,
    StrictNeq,
}

/// What a suspension waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuspendKind {
    /// A generator's first suspension, before its body.
    InitialYield,
    /// `yield v`: resumes with the sent value and the resumption kind.
    Yield,
    /// `yield*`'s suspension on the inner iterator's result.
    YieldStar,
    /// `await v`: resumes with the fulfilled value.
    Await,
}

/// The core errors a block can end by throwing, each naming a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowKind {
    /// An assignment to a constant.
    ConstAssign,
    Type,
    Reference,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        cond: Value,
        then: BlockId,
        else_: BlockId,
    },
    /// A comparison used only by the branch, fused by the lowering.
    BranchCmp {
        op: CmpOp,
        l: Value,
        r: Value,
        then: BlockId,
        else_: BlockId,
    },
    BranchNullish {
        value: Value,
        then: BlockId,
        else_: BlockId,
    },
    /// The next live key of enumeration `iter` to `key`, defined on the edge
    /// to `then`; an exhausted enumeration goes to `else_`.
    ForInNext {
        iter: Value,
        key: Value,
        then: BlockId,
        else_: BlockId,
    },
    Return(Value),
    ReturnUndefined,
    Throw(Value),
    ThrowError {
        kind: ThrowKind,
        key: u32,
    },
    /// Suspends with `value`; the resumption's value and kind are defined
    /// on the edge to `resume` (`kind` only for `Yield` and `YieldStar`).
    Suspend {
        kind: SuspendKind,
        value: Option<Value>,
        sent: Option<Value>,
        resume_kind: Option<Value>,
        resume: BlockId,
    },
    /// Runs the `finally` subroutine at `finally`, whose `Ret` on the
    /// block's return address comes back to `cont`.
    Gosub {
        finally: BlockId,
        cont: BlockId,
    },
    Ret(Value),
    Unreachable,
}

impl Terminator {
    /// The successors, in order.
    #[must_use]
    pub fn successors(&self) -> Vec<BlockId> {
        match *self {
            Self::Jump(b) => vec![b],
            Self::Branch { then, else_, .. }
            | Self::BranchCmp { then, else_, .. }
            | Self::BranchNullish { then, else_, .. }
            | Self::ForInNext { then, else_, .. } => vec![then, else_],
            Self::Suspend { resume, .. } => vec![resume],
            Self::Gosub { finally, cont, .. } => vec![finally, cont],
            Self::Return(_)
            | Self::ReturnUndefined
            | Self::Throw(_)
            | Self::ThrowError { .. }
            | Self::Ret(_)
            | Self::Unreachable => Vec::new(),
        }
    }

    /// The values it reads.
    #[must_use]
    pub fn uses(&self) -> Vec<Value> {
        match *self {
            Self::Branch { cond, .. } => vec![cond],
            Self::BranchCmp { l, r, .. } => vec![l, r],
            Self::BranchNullish { value, .. } | Self::Return(value) | Self::Throw(value) => {
                vec![value]
            }
            Self::ForInNext { iter, .. } => vec![iter],
            Self::Suspend { value, .. } => value.into_iter().collect(),
            Self::Ret(addr) => vec![addr],
            Self::Jump(_)
            | Self::ReturnUndefined
            | Self::ThrowError { .. }
            | Self::Gosub { .. }
            | Self::Unreachable => Vec::new(),
        }
    }

    /// The values it reads, to rewrite.
    pub fn uses_mut(&mut self) -> Vec<&mut Value> {
        match self {
            Self::Branch { cond, .. } => vec![cond],
            Self::BranchCmp { l, r, .. } => vec![l, r],
            Self::BranchNullish { value, .. } | Self::Return(value) | Self::Throw(value) => {
                vec![value]
            }
            Self::ForInNext { iter, .. } => vec![iter],
            Self::Suspend { value, .. } => value.iter_mut().collect(),
            Self::Ret(addr) => vec![addr],
            Self::Jump(_)
            | Self::ReturnUndefined
            | Self::ThrowError { .. }
            | Self::Gosub { .. }
            | Self::Unreachable => Vec::new(),
        }
    }

    /// The values it defines, with the successor they are defined for.
    #[must_use]
    pub fn defs(&self) -> Vec<(Value, BlockId)> {
        match *self {
            Self::ForInNext { key, then, .. } => vec![(key, then)],
            Self::Suspend {
                sent,
                resume_kind,
                resume,
                ..
            } => sent
                .into_iter()
                .chain(resume_kind)
                .map(|v| (v, resume))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn successors_mut(&mut self) -> Vec<&mut BlockId> {
        match self {
            Self::Jump(b) => vec![b],
            Self::Branch { then, else_, .. }
            | Self::BranchCmp { then, else_, .. }
            | Self::BranchNullish { then, else_, .. }
            | Self::ForInNext { then, else_, .. } => vec![then, else_],
            Self::Suspend { resume, .. } => vec![resume],
            Self::Gosub { finally, cont, .. } => vec![finally, cont],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub phis: Vec<Phi>,
    /// The terminator's source position.
    pub term_pos: Pos,
    /// The exception value, for a catch region's entry.
    pub catch: Option<Value>,
    /// The return address, for a `finally` subroutine's entry.
    pub ret_addr: Option<Value>,
    pub insns: Vec<Insn>,
    pub term: Terminator,
    /// The innermost catch region its instructions throw to.
    pub handler: Option<HandlerId>,
    pub preds: Vec<BlockId>,
    /// Loop nesting depth: register-allocation weight; a jump to a block at
    /// the same or a shallower position in a loop is a back edge.
    pub loop_depth: u8,
}

impl Block {
    #[must_use]
    pub fn new() -> Self {
        Self {
            phis: Vec::new(),
            term_pos: Pos::NONE,
            catch: None,
            ret_addr: None,
            insns: Vec::new(),
            term: Terminator::Unreachable,
            handler: None,
            preds: Vec::new(),
            loop_depth: 0,
        }
    }
}

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

/// A catch region: a throw inside it reaches `catch` with the exception.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub parent: Option<HandlerId>,
    pub catch: BlockId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueInfo {
    pub ty: Lattice,
    /// Registers it occupies: 3 for an iterator record, else 1.
    pub width: u8,
    /// The declaration it holds, for dumps.
    pub decl: Option<DeclRef>,
}

impl Default for ValueInfo {
    fn default() -> Self {
        Self {
            ty: Lattice::Any,
            width: 1,
            decl: None,
        }
    }
}

/// Where a variable lives: `this`, the parameters and `new.target` have
/// the registers the core's calling convention gives them, the other
/// variables any register the lowering chooses for the whole function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    This,
    Param(u32),
    NewTarget,
    Local,
}

/// A variable: a binding, or a value of the builder's own, that keeps one
/// register for the whole function and is read and written with `GetVar`
/// and `SetVar`, so a catch block or a resumption sees its latest value. A
/// captured binding's variable holds its cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Var {
    pub kind: VarKind,
    pub decl: Option<DeclRef>,
    /// The setter of a private accessor pair, whose getter is the
    /// declaration's other variable.
    pub setter: bool,
    /// The registers it takes: 1, or 3 for an iterator record moved out
    /// of the window.
    pub width: u8,
}

/// A constant-pool entry; strings are indexes into the program's strings.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Number(f64),
    String(u32),
    /// The decimal digits of a `BigInt` literal.
    BigInt(u32),
    /// A tagged template's strings: the template object of its site.
    Template {
        cooked: Vec<Option<u32>>,
        raw: Vec<u32>,
    },
    /// A regular expression literal: its source and canonical flags.
    RegExp {
        source: u32,
        flags: u32,
    },
}

/// The parts of the function's unit the builder decides.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct UnitInfo {
    /// The core's unit flags.
    pub flags: u16,
    /// The registers after `this` that hold the arguments.
    pub param_count: u16,
    /// `Function.length`.
    pub length: u16,
    /// The function's own name, for the backtrace.
    pub name: Option<u32>,
    /// The `name` property: its own name, or the one its creation site
    /// gives it.
    pub name_atom: Option<u32>,
    /// Distinct `this.x =` keys of a base constructor.
    pub this_slots: u16,
    /// The byte range of its source text, for `toString`.
    pub source: Option<(u32, u32)>,
    /// The position of its definition, for instructions without one.
    pub definition: Pos,
}

/// One function's graph; `blocks[0]` is the entry.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Function {
    /// The HIR function it was built from.
    pub source: FunctionRef,
    pub blocks: Vec<Block>,
    pub values: Vec<ValueInfo>,
    /// Values defined before the entry block (the tests' parameters).
    pub params: Vec<Value>,
    pub handlers: Vec<Handler>,
    /// The HIR functions its `MakeClosure` instructions create, by child index.
    pub children: Vec<FunctionRef>,
    pub vars: Vec<Var>,
    /// The bindings `LoadCaptured` indexes: a private accessor pair takes
    /// two, its setter second.
    pub captures: Vec<(DeclRef, bool)>,
    pub consts: Vec<Constant>,
    pub unit: UnitInfo,
    /// The blocks in the order the builder emitted them, which the lowering
    /// keeps; empty for block order.
    pub layout: Vec<BlockId>,
}

impl Function {
    pub fn new_var(&mut self, kind: VarKind, decl: Option<DeclRef>) -> u32 {
        let index = u32::try_from(self.vars.len()).expect("MIR variables are indexed by u32");
        self.vars.push(Var {
            kind,
            decl,
            setter: false,
            width: 1,
        });
        index
    }

    #[must_use]
    pub fn new(source: FunctionRef) -> Self {
        Self {
            source,
            blocks: vec![Block::new()],
            ..Self::default()
        }
    }

    pub fn new_value(&mut self, info: ValueInfo) -> Value {
        let value = Value(u32::try_from(self.values.len()).expect("MIR values are indexed by u32"));
        self.values.push(info);
        value
    }

    pub fn new_block(&mut self) -> BlockId {
        let id = BlockId(u32::try_from(self.blocks.len()).expect("MIR blocks are indexed by u32"));
        self.blocks.push(Block::new());
        id
    }

    #[must_use]
    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id.index()]
    }

    pub fn block_mut(&mut self, id: BlockId) -> &mut Block {
        &mut self.blocks[id.index()]
    }

    /// Recomputes every block's `preds` from the terminators, in block
    /// order. Phi operands are not reordered.
    pub fn recompute_preds(&mut self) {
        for block in &mut self.blocks {
            block.preds.clear();
        }
        for index in 0..self.blocks.len() {
            for succ in self.blocks[index].term.successors() {
                let from = BlockId(index as u32);
                let preds = &mut self.blocks[succ.index()].preds;
                if !preds.contains(&from) {
                    preds.push(from);
                }
            }
        }
    }

    /// The blocks control can reach from `id`: its terminator's successors,
    /// then its catch region's entry.
    #[must_use]
    pub fn successors(&self, id: BlockId) -> Vec<BlockId> {
        let block = self.block(id);
        let mut succs = block.term.successors();
        if let Some(h) = block.handler {
            succs.push(self.handlers[h.index()].catch);
        }
        succs
    }

    /// Blocks in reverse postorder from the entry, through normal and
    /// exceptional edges; unreachable blocks are left out.
    #[must_use]
    pub fn reverse_postorder(&self) -> Vec<BlockId> {
        let mut seen = vec![false; self.blocks.len()];
        let mut order = Vec::with_capacity(self.blocks.len());
        // An explicit stack of (block, next successor to visit).
        let mut stack = vec![(BlockId::ENTRY, 0usize)];
        seen[0] = true;
        while let Some((block, next)) = stack.pop() {
            let succs = self.successors(block);
            if let Some(&succ) = succs.get(next) {
                stack.push((block, next + 1));
                if !seen[succ.index()] {
                    seen[succ.index()] = true;
                    stack.push((succ, 0));
                }
            } else {
                order.push(block);
            }
        }
        order.reverse();
        order
    }
}
