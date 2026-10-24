//! HIR to MIR: one graph per function.
//!
//! The builder walks each function's HIR in the order QuickJS emits its
//! bytecode, so the operations, their evaluation order and their source
//! positions are the ones the old front end produces. A binding of the
//! function is a variable ([`Var`]): one register for the whole function,
//! read and written with `GetVar` and `SetVar`; a binding an inner function
//! captures holds its cell there. Everything else is an SSA value. Phis
//! appear only where an expression's branches join.
//!
//! Positions follow QuickJS's `emit_source_pos`: an operation either names
//! its own position or keeps the last one, in emission order. Each
//! instruction records the position in effect when it was emitted, so the
//! lowering's block order does not change them.

mod class;
mod entry;
mod expr;
mod pattern;
mod stmt;

use super::ir::{
    BlockId, Constant, Function, Handler, HandlerId, Imm, Insn, Phi, Pos, Terminator, Value,
    ValueInfo, VarKind,
};
use super::op::Op;
use crate::engine::hir::schema::{
    self as hir, DeclKind, DeclRef, FunctionKind, FunctionRef, StringRef,
};
use crate::engine::value::JsString;
use oxide_core::{FxHashMap, FxHashSet};
use std::rc::Rc;

/// The MIR of a whole program: one function per HIR function.
pub(crate) struct Program {
    pub(crate) functions: Vec<Function>,
    /// The HIR's strings, then the builder's own (messages).
    pub(crate) strings: Vec<JsString>,
    pub(crate) root: FunctionRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BuildError {
    /// An early error found here (`with`).
    Syntax(String),
    /// A construct the builder does not lower yet.
    Unsupported(String),
    Internal(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(message) => write!(f, "SyntaxError: {message}"),
            Self::Unsupported(what) => write!(f, "the MIR builder does not support {what}"),
            Self::Internal(message) => write!(f, "MIR builder: {message}"),
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, BuildError>;

pub(crate) fn unsupported<T>(what: impl Into<String>) -> Result<T> {
    Err(BuildError::Unsupported(what.into()))
}

pub(crate) fn internal<T>(message: impl Into<String>) -> Result<T> {
    Err(BuildError::Internal(message.into()))
}

/// What the builders of a program share.
pub(crate) struct Shared<'h> {
    pub(crate) hir: &'h hir::Program,
    pub(crate) regexps: &'h FxHashMap<u32, Rc<oxide_regexp::CompiledRegExp>>,
    pub(crate) strings: Vec<JsString>,
    statics: FxHashMap<&'static str, u32>,
    interned: FxHashMap<JsString, u32>,
    /// The function that creates each function, and the name its creation
    /// site gives it.
    pub(crate) parent: Vec<Option<FunctionRef>>,
    pub(crate) inferred: Vec<Option<u32>>,
    /// The aggregate initializers that brand their receiver.
    pub(crate) brand: Vec<bool>,
}

impl Shared<'_> {
    /// The index of `string` in the program's strings, added once.
    pub(crate) fn intern(&mut self, string: JsString) -> u32 {
        if let Some(&index) = self.interned.get(&string) {
            return index;
        }
        let index = self.strings.len() as u32;
        self.strings.push(string.clone());
        self.interned.insert(string, index);
        index
    }

    /// The index of a string of the builder's own.
    pub(crate) fn static_string(&mut self, text: &'static str) -> u32 {
        if let Some(&index) = self.statics.get(text) {
            return index;
        }
        let index = self.strings.len() as u32;
        self.strings.push(JsString::from_static(text));
        self.statics.insert(text, index);
        index
    }

    pub(crate) fn decl(&self, decl: DeclRef) -> &hir::Decl {
        &self.hir.decls[decl.index()]
    }

    pub(crate) fn function(&self, f: FunctionRef) -> &hir::Function {
        &self.hir.functions[f.index()]
    }
}

/// Builds the MIR of every function of `hir`.
///
/// # Errors
/// A construct outside the builder's surface, or broken HIR.
pub(crate) fn build(
    hir: &hir::Program,
    regexps: &FxHashMap<u32, Rc<oxide_regexp::CompiledRegExp>>,
    has_with: bool,
) -> Result<Program> {
    if has_with {
        return Err(BuildError::Syntax(
            "with statements are not supported".into(),
        ));
    }
    let root = match *hir.entry {
        hir::Entry::Script { top } | hir::Entry::Eval { top } => top,
        hir::Entry::Module { module } => hir.modules[module.index()].top,
    };
    let count = hir.functions.len();
    let mut parent = vec![None; count];
    for (index, function) in hir.functions.iter().enumerate() {
        for inner in &function.inner {
            parent[inner.index()] = Some(FunctionRef(index as u32));
        }
    }
    let mut shared = Shared {
        hir,
        regexps,
        strings: hir.strings.clone(),
        statics: FxHashMap::default(),
        interned: FxHashMap::default(),
        parent,
        inferred: vec![None; count],
        brand: vec![false; count],
    };
    let mut functions = Vec::with_capacity(count);
    for index in 0..count {
        let id = FunctionRef(index as u32);
        functions.push(Builder::new(&mut shared, id)?.build()?);
    }
    for (index, function) in functions.iter_mut().enumerate() {
        if function.unit.name_atom.is_none() {
            function.unit.name_atom = shared.inferred[index];
        }
        if shared.brand[index] {
            function.unit.flags |= oxide_core::bytecode::flags::BRAND;
        }
        if cfg!(debug_assertions)
            && let Err(error) = super::verify::verify(function)
        {
            return internal(format!(
                "{error} in function #{index}\n{}",
                super::dump::dump(function)
            ));
        }
    }
    Ok(Program {
        functions,
        strings: shared.strings,
        root,
    })
}

/// How a function reaches a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    /// A variable of this function; `cell` when it holds the binding's cell.
    Var { var: u32, cell: bool },
    /// Capture `index` of this function: a cell its creator supplies.
    Captured { index: u32 },
    /// A binding of the global object or the global lexical environment.
    Global { name: StringRef },
}

/// A statement that `break`, `continue` and `return` leave or unwind.
#[derive(Clone, Debug)]
pub(crate) enum Control {
    /// A loop, switch or labeled statement: `label` is the HIR's id.
    Breakable {
        label: u32,
        break_to: BlockId,
        continue_to: Option<BlockId>,
        /// What leaving it needs: a for-of's iterator record to close.
        iterator: Option<Value>,
        /// The lexical scopes inside it, which a jump out closes.
        scope_depth: usize,
    },
    /// A `try` or `catch` body with a `finally` subroutine to run on the
    /// way out (QuickJS gives every try statement one, maybe empty).
    Finally { finally: BlockId },
    /// A `finally` body: leaving it abandons the subroutine.
    FinallyBody,
    /// An iterator a destructuring holds open: a return closes it.
    Iterator { record: Value },
    /// A for-of assignment fragment: a return drops the record unclosed.
    Fragment,
}

/// The builder of one function.
pub(crate) struct Builder<'s, 'h> {
    pub(crate) cx: &'s mut Shared<'h>,
    pub(crate) id: FunctionRef,
    pub(crate) hf: &'h hir::Function,
    pub(crate) f: Function,
    /// The block being filled; `None` after a terminator, until the next
    /// reachable block starts.
    pub(crate) cur: Option<BlockId>,
    /// The catch region new blocks belong to.
    pub(crate) handler: Option<HandlerId>,
    pub(crate) loop_depth: u8,
    /// The position in effect and one an operation without its own takes.
    pub(crate) site: Pos,
    pub(crate) pending: Option<u32>,
    /// The last instruction emitted, for QuickJS's `return f()` position.
    pub(crate) last: Option<(BlockId, usize)>,
    pub(crate) vars: FxHashMap<DeclRef, u32>,
    /// The setter variable of each private accessor pair this function owns.
    pub(crate) setter_vars: FxHashMap<DeclRef, u32>,
    pub(crate) captures: FxHashMap<(DeclRef, bool), u32>,
    pub(crate) controls: Vec<Control>,
    /// The lexical scopes entered, innermost last: the captured bindings
    /// each closes when left.
    pub(crate) scopes: Vec<Vec<DeclRef>>,
    /// The completion value of script and eval code.
    pub(crate) completion: Option<u32>,
    /// Declarations checked for the TDZ since the block started.
    pub(crate) checked: FxHashSet<DeclRef>,
    /// The variable holding the function's own `this`, `new.target`, home
    /// object and active function, when read.
    pub(crate) children: FxHashMap<FunctionRef, u32>,
    pub(crate) consts: FxHashMap<u64, u32>,
    /// Blocks placed after the function's code (iterator-closing handlers).
    pub(crate) stubs: Vec<BlockId>,
    pub(crate) string_consts: FxHashMap<u32, u32>,
    pub(crate) this_keys: FxHashSet<u32>,
    /// Values read from register 0.
    pub(crate) this_values: FxHashSet<Value>,
}

impl<'s, 'h> Builder<'s, 'h> {
    fn new(cx: &'s mut Shared<'h>, id: FunctionRef) -> Result<Self> {
        let hf: &'h hir::Function = &cx.hir.functions[id.index()];
        let mut f = Function::new(id);
        f.unit.definition = Pos(hf.span.start);
        f.layout.push(BlockId::ENTRY);
        Ok(Self {
            cx,
            id,
            hf,
            f,
            cur: Some(BlockId::ENTRY),
            handler: None,
            loop_depth: 0,
            site: Pos::NONE,
            pending: None,
            last: None,
            vars: FxHashMap::default(),
            setter_vars: FxHashMap::default(),
            captures: FxHashMap::default(),
            controls: Vec::new(),
            scopes: Vec::new(),
            completion: None,
            checked: FxHashSet::default(),
            children: FxHashMap::default(),
            consts: FxHashMap::default(),
            stubs: Vec::new(),
            string_consts: FxHashMap::default(),
            this_keys: FxHashSet::default(),
            this_values: FxHashSet::default(),
        })
    }

    fn build(mut self) -> Result<Function> {
        self.entry()?;
        if self.f.unit.flags & oxide_core::bytecode::flags::CONSTRUCTOR != 0
            && self.f.unit.flags & oxide_core::bytecode::flags::DERIVED == 0
        {
            self.f.unit.this_slots = self.this_keys.len() as u16;
        }
        let stubs = std::mem::take(&mut self.stubs);
        self.f.layout.extend(stubs);
        let mut f = self.f;
        f.recompute_preds_keeping_phis();
        Ok(f)
    }

    // ---- blocks ----

    /// A new block in the current catch region and loop.
    pub(crate) fn block(&mut self) -> BlockId {
        let id = self.f.new_block();
        let block = self.f.block_mut(id);
        block.handler = self.handler;
        block.loop_depth = self.loop_depth;
        id
    }

    /// Ends the current block with `term`.
    pub(crate) fn end(&mut self, term: Terminator) {
        if let Some(cur) = self.cur.take() {
            let pos = self.mark(None);
            let block = self.f.block_mut(cur);
            block.term = term;
            block.term_pos = pos;
        }
    }

    /// Ends the current block with `term` at its own position.
    pub(crate) fn end_at(&mut self, term: Terminator, at: u32) {
        if let Some(cur) = self.cur.take() {
            let pos = self.mark(Some(at));
            let block = self.f.block_mut(cur);
            block.term = term;
            block.term_pos = pos;
        }
    }

    pub(crate) fn goto(&mut self, target: BlockId) {
        self.end(Terminator::Jump(target));
    }

    /// Continues in `block`, which the current block (if reachable) falls
    /// into.
    pub(crate) fn enter(&mut self, block: BlockId) {
        if self.cur.is_some() {
            self.goto(block);
        }
        self.start(block);
    }

    /// Continues in `block` without a fallthrough into it.
    pub(crate) fn start(&mut self, block: BlockId) {
        self.cur = Some(block);
        self.checked.clear();
        self.f.layout.push(block);
    }

    /// Fills `block` with `fill` out of line: it goes after the function's
    /// code, at position `at`, and the current block stays current.
    pub(crate) fn stub(&mut self, block: BlockId, at: Pos, fill: impl FnOnce(&mut Self)) {
        let saved = (self.cur, self.site, self.pending, self.handler, self.last);
        let checked = std::mem::take(&mut self.checked);
        self.cur = Some(block);
        self.site = at;
        self.pending = None;
        self.handler = self.f.block(block).handler;
        fill(self);
        (self.cur, self.site, self.pending, self.handler, self.last) = saved;
        self.checked = checked;
        self.stubs.push(block);
    }

    pub(crate) fn reachable(&self) -> bool {
        self.cur.is_some()
    }

    /// Ends the current block for a later [`Self::join`]: the block, if
    /// reachable.
    pub(crate) fn branch_end(&mut self) -> Option<BlockId> {
        self.cur.take()
    }

    /// Joins the branches of an expression, each ended by
    /// [`Self::branch_end`] with its value: a new block, with a phi when
    /// more than one branch reaches it.
    pub(crate) fn join(&mut self, incoming: Vec<(Option<BlockId>, Value)>) -> Value {
        let incoming: Vec<(BlockId, Value)> = incoming
            .into_iter()
            .filter_map(|(block, value)| block.map(|block| (block, value)))
            .collect();
        if incoming.is_empty() {
            return self.f.new_value(ValueInfo::default());
        }
        let block = self.block();
        let pos = self.site;
        for &(pred, _) in &incoming {
            let b = self.f.block_mut(pred);
            b.term = Terminator::Jump(block);
            b.term_pos = pos;
        }
        self.start(block);
        if incoming.len() == 1 {
            self.f.block_mut(block).preds = vec![incoming[0].0];
            return incoming[0].1;
        }
        let result = self.f.new_value(ValueInfo::default());
        let b = self.f.block_mut(block);
        b.preds = incoming.iter().map(|&(pred, _)| pred).collect();
        b.phis.push(Phi {
            result,
            args: incoming.iter().map(|&(_, value)| value).collect(),
        });
        result
    }

    /// Joins branches that carry no value.
    pub(crate) fn join_empty(&mut self, incoming: Vec<Option<BlockId>>) {
        let incoming: Vec<BlockId> = incoming.into_iter().flatten().collect();
        if incoming.is_empty() {
            return;
        }
        let block = self.block();
        let pos = self.site;
        for &pred in &incoming {
            let b = self.f.block_mut(pred);
            b.term = Terminator::Jump(block);
            b.term_pos = pos;
        }
        self.start(block);
    }

    // ---- positions ----

    /// The position of an operation: its own, else the pending one, else the
    /// one in effect.
    pub(crate) fn mark(&mut self, own: Option<u32>) -> Pos {
        if let Some(at) = own {
            self.site = Pos(at);
            self.pending = None;
        } else if let Some(at) = self.pending.take() {
            self.site = Pos(at);
        }
        self.site
    }

    /// QuickJS's statement-entry position: the next operation takes it
    /// unless it has its own.
    pub(crate) fn seed(&mut self, at: u32) {
        self.pending = Some(at);
    }

    /// Gives the last instruction position `at`, as QuickJS moves a call's
    /// position to the `return` that folds it into a tail call.
    pub(crate) fn remark_last_call(&mut self, at: u32) {
        let Some((block, index)) = self.last else {
            return;
        };
        let insn = &mut self.f.blocks[block.index()].insns[index];
        if matches!(
            insn.op,
            Op::Call | Op::CallMethod | Op::CallEval | Op::CallSpread
        ) {
            insn.pos = Pos(at);
            self.site = Pos(at);
        }
    }

    // ---- instructions ----

    fn push(&mut self, op: Op, args: &[Value], imm: Imm, own: Option<u32>) -> [Option<Value>; 2] {
        let mut results = [None, None];
        for slot in results.iter_mut().take(usize::from(op.results())) {
            *slot = Some(self.f.new_value(ValueInfo::default()));
        }
        let pos = self.mark(own);
        // Unreachable code still evaluates its positions, as QuickJS's
        // emission does; its instructions go nowhere.
        let Some(cur) = self.cur else {
            return results;
        };
        let insns = &mut self.f.blocks[cur.index()].insns;
        self.last = Some((cur, insns.len()));
        insns.push(Insn {
            op,
            args: super::ir::Args::from_slice(args),
            results,
            imm,
            pos,
        });
        results
    }

    /// Emits `op`, returning its result.
    pub(crate) fn op(&mut self, op: Op, args: &[Value], imm: Imm) -> Value {
        debug_assert_eq!(op.results(), 1, "{} has one result", op.name());
        self.push(op, args, imm, None)[0].expect("one result")
    }

    /// Emits `op` at its own position.
    pub(crate) fn op_at(&mut self, op: Op, args: &[Value], imm: Imm, at: u32) -> Value {
        debug_assert_eq!(op.results(), 1, "{} has one result", op.name());
        self.push(op, args, imm, Some(at))[0].expect("one result")
    }

    /// Emits `op`, which has no result.
    pub(crate) fn effect(&mut self, op: Op, args: &[Value], imm: Imm) {
        debug_assert_eq!(op.results(), 0, "{} has no result", op.name());
        self.push(op, args, imm, None);
    }

    pub(crate) fn effect_at(&mut self, op: Op, args: &[Value], imm: Imm, at: u32) {
        debug_assert_eq!(op.results(), 0, "{} has no result", op.name());
        self.push(op, args, imm, Some(at));
    }

    /// Emits `op`, which has two results.
    pub(crate) fn op2(&mut self, op: Op, args: &[Value], imm: Imm) -> (Value, Value) {
        let [a, b] = self.push(op, args, imm, None);
        (a.expect("first result"), b.expect("second result"))
    }

    pub(crate) fn op2_at(&mut self, op: Op, args: &[Value], imm: Imm, at: u32) -> (Value, Value) {
        let [a, b] = self.push(op, args, imm, Some(at));
        (a.expect("first result"), b.expect("second result"))
    }

    pub(crate) fn undefined(&mut self) -> Value {
        self.op(Op::Undefined, &[], Imm::None)
    }

    pub(crate) fn int(&mut self, value: i32) -> Value {
        self.op(Op::ConstInt, &[], Imm::Int(value))
    }

    /// Marks a value as three registers wide: an iterator record.
    pub(crate) fn record(&mut self, value: Value) {
        self.f.values[value.index()].width = 3;
    }

    // ---- constants ----

    pub(crate) fn number(&mut self, value: f64) -> Value {
        if value.fract() == 0.0
            && value >= f64::from(i32::MIN)
            && value <= f64::from(i32::MAX)
            && !(value == 0.0 && value.is_sign_negative())
        {
            return self.int(value as i32);
        }
        let bits = value.to_bits();
        let index = match self.consts.get(&bits) {
            Some(&index) => index,
            None => {
                let index = self.constant(Constant::Number(value));
                self.consts.insert(bits, index);
                index
            }
        };
        self.op(Op::Const, &[], Imm::Const(index))
    }

    pub(crate) fn constant(&mut self, constant: Constant) -> u32 {
        let index = self.f.consts.len() as u32;
        self.f.consts.push(constant);
        index
    }

    /// The pool index of string `string`.
    pub(crate) fn string_const(&mut self, string: u32) -> u32 {
        if let Some(&index) = self.string_consts.get(&string) {
            return index;
        }
        let index = self.constant(Constant::String(string));
        self.string_consts.insert(string, index);
        index
    }

    pub(crate) fn string(&mut self, string: u32) -> Value {
        let index = self.string_const(string);
        self.op(Op::Const, &[], Imm::Const(index))
    }

    pub(crate) fn static_key(&mut self, text: &'static str) -> u32 {
        self.cx.static_string(text)
    }

    // ---- children ----

    /// The child index of inner function `inner`.
    pub(crate) fn child(&mut self, inner: FunctionRef) -> u32 {
        if let Some(&index) = self.children.get(&inner) {
            return index;
        }
        let index = self.f.children.len() as u32;
        self.f.children.push(inner);
        self.children.insert(inner, index);
        index
    }

    /// A closure of inner function `inner`.
    pub(crate) fn closure(&mut self, inner: FunctionRef) -> Value {
        let child = self.child(inner);
        self.op(Op::MakeClosure, &[], Imm::Child(child))
    }

    // ---- handlers ----

    /// Opens a catch region whose throws reach `catch`.
    pub(crate) fn open_handler(&mut self, catch: BlockId) -> HandlerId {
        let id = HandlerId(self.f.handlers.len() as u32);
        self.f.handlers.push(Handler {
            parent: self.handler,
            catch,
        });
        self.handler = Some(id);
        id
    }

    pub(crate) fn close_handler(&mut self, id: HandlerId) {
        self.handler = self.f.handlers[id.index()].parent;
    }

    /// Continues in a fresh block of the current catch region, when the
    /// current block belongs to another one.
    pub(crate) fn rebase(&mut self) {
        if let Some(cur) = self.cur
            && self.f.block(cur).handler != self.handler
        {
            let next = self.block();
            self.enter(next);
        }
    }

    // ---- bindings ----

    pub(crate) fn decl(&self, decl: DeclRef) -> &'h hir::Decl {
        &self.cx.hir.decls[decl.index()]
    }

    pub(crate) fn captured(&self, decl: DeclRef) -> bool {
        !self.decl(decl).captured_by.is_empty()
    }

    /// How this function reaches `decl`.
    pub(crate) fn access(&mut self, decl: DeclRef) -> Result<Access> {
        if let Some(&var) = self.vars.get(&decl) {
            let cell = self.var_is_cell(decl);
            return Ok(Access::Var { var, cell });
        }
        let d = self.decl(decl);
        if d.kind == DeclKind::Global || self.is_global(decl) {
            return Ok(Access::Global { name: d.name });
        }
        if let Some(&index) = self.captures.get(&(decl, false)) {
            return Ok(Access::Captured { index });
        }
        internal(format!(
            "function #{} reaches decl #{} ({:?} of #{}) neither as a variable nor a capture",
            self.id.0, decl.0, d.kind, d.owner.0
        ))
    }

    /// Whether `decl` is a binding of the global object or global lexical
    /// environment: a script's top-level declarations, and sloppy eval
    /// code's `var` and function declarations.
    pub(crate) fn is_global(&self, decl: DeclRef) -> bool {
        let d = self.decl(decl);
        let owner = self.cx.function(d.owner);
        match owner.kind {
            FunctionKind::Script => match d.kind {
                DeclKind::Var | DeclKind::Function => true,
                DeclKind::Let | DeclKind::Const | DeclKind::Class => {
                    owner.body.decls.contains(&decl)
                }
                _ => false,
            },
            FunctionKind::Eval => {
                !owner.strict && matches!(d.kind, DeclKind::Var | DeclKind::Function)
            }
            _ => false,
        }
    }

    pub(crate) fn var_is_cell(&self, decl: DeclRef) -> bool {
        self.captured(decl) || self.mapped_param(decl)
    }

    /// Whether `decl` is a parameter a mapped `arguments` object aliases.
    pub(crate) fn mapped_param(&self, decl: DeclRef) -> bool {
        self.mapped_arguments()
            && self.decl(decl).kind == DeclKind::Param
            && self.param_index(decl).is_some()
    }

    pub(crate) fn mapped_arguments(&self) -> bool {
        self.hf.arguments_decl.is_some() && !self.hf.strict && self.hf.simple_params
    }

    /// The argument register a simple parameter's binding lives in.
    pub(crate) fn param_index(&self, decl: DeclRef) -> Option<u32> {
        if !self.hf.simple_params {
            return None;
        }
        self.hf.params.iter().position(|param| {
            matches!(param.pattern.kind, hir::PatternKind::Decl { decl: d } if d == decl)
        })
        .map(|index| index as u32)
    }

    pub(crate) fn new_var(&mut self, kind: VarKind, decl: Option<DeclRef>) -> u32 {
        self.f.new_var(kind, decl)
    }

    pub(crate) fn get_var(&mut self, var: u32) -> Value {
        let value = self.op(Op::GetVar, &[], Imm::Var(var));
        if self.f.vars[var as usize].kind == VarKind::This {
            self.this_values.insert(value);
        }
        value
    }

    pub(crate) fn set_var(&mut self, var: u32, value: Value) {
        self.effect(Op::SetVar, &[value], Imm::Var(var));
    }

    /// The key that names `decl` in a TDZ error.
    pub(crate) fn decl_key(&self, decl: DeclRef) -> u32 {
        self.decl(decl).name.0
    }
}

impl Function {
    /// [`Function::recompute_preds`] for a graph whose join blocks already
    /// list their predecessors in their phis' order: those keep the order.
    pub fn recompute_preds_keeping_phis(&mut self) {
        let mut preds: Vec<Vec<BlockId>> = vec![Vec::new(); self.blocks.len()];
        for index in 0..self.blocks.len() {
            for succ in self.blocks[index].term.successors() {
                let from = BlockId(index as u32);
                let list = &mut preds[succ.index()];
                if !list.contains(&from) {
                    list.push(from);
                }
            }
        }
        for (index, block) in self.blocks.iter_mut().enumerate() {
            if block.phis.is_empty() {
                block.preds = std::mem::take(&mut preds[index]);
            } else {
                // A join's predecessors are the ones its phis name; the ones
                // that turned out unreachable stay listed.
                debug_assert!(
                    preds[index].iter().all(|p| block.preds.contains(p)),
                    "a join gained a predecessor"
                );
            }
        }
    }
}
