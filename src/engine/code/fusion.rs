//! Authenticated execution spans over canonical, verified instruction PCs.
//!
//! The published bytecode and its source/relocation tables are never rewritten.
//! A span is entered only at its first instruction, contains no external entry,
//! and falls back to that original instruction before changing any VM state.
//! Legacy execution and serialization continue to consume canonical opcodes.
use super::bytecode::Instruction;
use super::function::metadata::{ClosureVariableKind, VariableDefinition};
use crate::engine::heap::{BytecodeConstant, RawValue};
use std::rc::Rc;

mod dense;
#[cfg(test)]
pub(crate) use dense::with_dense_candidates_disabled;
pub(crate) use dense::{DenseSpanKind, DirectSlot, NumericSource};

#[derive(Clone, Debug, Default)]
pub(crate) struct FusionPlan(Option<Rc<[u8]>>);

/// One flag load shared by all fusion candidates at a local-read PC.
#[derive(Clone, Copy)]
pub(crate) struct FusionEntry(u8);

impl FusionEntry {
    #[inline]
    pub(crate) fn dense_span(self) -> Option<DenseSpanKind> {
        DenseSpanKind::from_flag(self.0)
    }

    #[inline]
    pub(crate) fn update(self) -> Option<UpdateLocal> {
        let flag = self.0;
        (flag & 16 != 0).then_some(UpdateLocal {
            increment: flag & 1 != 0,
            postfix: flag & 2 != 0,
            discard: flag & 4 != 0,
            instructions: if flag & 8 != 0 { 4 } else { 3 },
        })
    }

    #[inline]
    pub(crate) fn local_compare_branch(self) -> Option<usize> {
        match self.0 {
            33 => Some(4),
            34 => Some(5),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn local_add_span(self) -> Option<usize> {
        match self.0 {
            35 | 128 | 130 => Some(4),
            36 | 129 | 131 => Some(5),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn local_field_add_span(self) -> Option<usize> {
        match self.0 {
            37 => Some(5),
            38 => Some(6),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct UpdateLocal {
    pub increment: bool,
    pub postfix: bool,
    pub discard: bool,
    pub instructions: usize,
}

impl FusionPlan {
    pub(crate) fn build(
        code: &[Instruction],
        locals: &[VariableDefinition],
        constants: &[BytecodeConstant],
    ) -> Self {
        #[cfg(feature = "profiling")]
        let _timer = crate::engine::api::profiling::PhaseTimer::start(
            crate::engine::api::profiling::CompilePhase::Fusion,
        );
        let mut entries = vec![false; code.len()];
        for (pc, instruction) in code.iter().enumerate() {
            let control = instruction.control_effect();
            if let Some(target) = control.target() {
                if let Some(entry) = entries.get_mut(target as usize) {
                    *entry = true;
                }
            }
            if control.ends_block() {
                if let Some(entry) = entries.get_mut(pc + 1) {
                    *entry = true;
                }
            }
        }
        let mut flags = Vec::new();
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_compiler_storage(
            crate::engine::api::profiling::CompilePhase::Fusion,
            (entries.capacity() + flags.capacity()) as u64,
        );
        let mut any = false;
        for pc in 0..code.len() {
            let rest = &code[pc..];
            // Dense publication already authenticated the whole span, including
            // interior entries. A match supersedes the legacy candidate at this
            // PC, so do not build and authenticate that unused candidate too.
            if let Some(kind) = dense::candidate(rest, locals, constants, &entries[pc..]) {
                if flags.is_empty() {
                    flags.resize(code.len(), 0);
                }
                flags[pc] = kind as u8;
                any = true;
                continue;
            }
            let update = match rest {
                [
                    Instruction::GetLocal(index) | Instruction::GetLocalCheck(index),
                    operation,
                    store,
                    ..,
                ] if locals
                    .get(usize::from(*index))
                    .is_some_and(|d| !d.is_const && d.kind == ClosureVariableKind::Normal) =>
                {
                    let postfix = matches!(operation, Instruction::PostInc | Instruction::PostDec);
                    let increment = matches!(operation, Instruction::Inc | Instruction::PostInc);
                    let operation_valid = matches!(
                        operation,
                        Instruction::Inc
                            | Instruction::Dec
                            | Instruction::PostInc
                            | Instruction::PostDec
                    );
                    let valid_store = match store {
                        Instruction::PutLocal(i) | Instruction::PutLocalCheck(i) => *i == *index,
                        Instruction::SetLocal(i) | Instruction::SetLocalCheck(i) => {
                            *i == *index && !postfix
                        }
                        _ => false,
                    };
                    if operation_valid && valid_store {
                        let keeps = postfix
                            || matches!(
                                store,
                                Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                            );
                        let drop = keeps
                            && matches!(rest.get(3), Some(Instruction::Drop))
                            && !entries[pc + 3];
                        Some((
                            16 | u8::from(increment)
                                | (u8::from(postfix) << 1)
                                | (u8::from(!keeps || drop) << 2)
                                | (u8::from(drop) << 3),
                            if drop { 4 } else { 3 },
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let local_add = match rest {
                [
                    Instruction::GetLocal(left) | Instruction::GetLocalCheck(left),
                    right,
                    Instruction::Add,
                    store,
                    ..,
                ] if locals
                    .get(usize::from(*left))
                    .is_some_and(|d| d.kind == ClosureVariableKind::Normal && !d.is_const)
                    && match right {
                        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => locals
                            .get(usize::from(*index))
                            .is_some_and(|d| d.kind == ClosureVariableKind::Normal),
                        Instruction::PushConst(_) => true,
                        _ => false,
                    } =>
                {
                    match store {
                        Instruction::PutLocal(index) | Instruction::PutLocalCheck(index)
                            if index == left =>
                        {
                            Some((128, 4))
                        }
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index)
                            if index == left && matches!(rest.get(4), Some(Instruction::Drop)) =>
                        {
                            Some((129, 5))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            // S2: one direct numeric producer, a numeric literal, one
            // addition, and a writeback. The literal domain is fixed at
            // publication, so no other constant kind can claim the span.
            let local_add_const = match rest {
                [
                    Instruction::GetLocal(left) | Instruction::GetLocalCheck(left),
                    immediate,
                    Instruction::Add,
                    store,
                    ..,
                ] if locals
                    .get(usize::from(*left))
                    .is_some_and(|d| !d.is_const && d.kind == ClosureVariableKind::Normal)
                    && numeric_immediate(immediate, constants) =>
                {
                    match store {
                        Instruction::PutLocal(index) | Instruction::PutLocalCheck(index)
                            if index == left =>
                        {
                            Some((35, 4))
                        }
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index)
                            if index == left && matches!(rest.get(4), Some(Instruction::Drop)) =>
                        {
                            Some((36, 5))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let method = method_call_count(rest).map(|count| (160 + count as u8, count + 2));
            // S3: one numeric accumulator and one direct object base whose
            // linked field read completes the addition. The IC peek is
            // non-owning; IC misses, accessors, proxies and non-number values
            // fall back canonically.
            let local_field_add = match rest {
                [
                    Instruction::GetLocal(left) | Instruction::GetLocalCheck(left),
                    Instruction::GetLocal(base) | Instruction::GetLocalCheck(base),
                    Instruction::GetField(_),
                    Instruction::Add,
                    store,
                    ..,
                ] if locals
                    .get(usize::from(*left))
                    .is_some_and(|d| !d.is_const && d.kind == ClosureVariableKind::Normal)
                    && locals
                        .get(usize::from(*base))
                        .is_some_and(|d| d.kind == ClosureVariableKind::Normal) =>
                {
                    match store {
                        Instruction::PutLocal(index) | Instruction::PutLocalCheck(index)
                            if index == left =>
                        {
                            Some((37, 5))
                        }
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index)
                            if index == left && matches!(rest.get(5), Some(Instruction::Drop)) =>
                        {
                            Some((38, 6))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let const_local_add = match rest {
                [
                    Instruction::PushConst(_constant),
                    Instruction::GetLocal(right) | Instruction::GetLocalCheck(right),
                    Instruction::Add,
                    store,
                    ..,
                ] if locals
                    .get(usize::from(*right))
                    .is_some_and(|d| d.kind == ClosureVariableKind::Normal && !d.is_const) =>
                {
                    match store {
                        Instruction::PutLocal(index) | Instruction::PutLocalCheck(index)
                            if index == right =>
                        {
                            Some((130, 4))
                        }
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index)
                            if index == right && matches!(rest.get(4), Some(Instruction::Drop)) =>
                        {
                            Some((131, 5))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            // S1: two direct producers feeding one comparison and its branch.
            // The trailing Goto collapses the not-taken edge; it stays out of
            // the interior-entry requirement because entering it directly is
            // still canonical.
            let local_compare = match rest {
                [
                    Instruction::GetLocal(_) | Instruction::GetLocalCheck(_),
                    Instruction::GetLocal(_)
                    | Instruction::GetLocalCheck(_)
                    | Instruction::GetArg(_),
                    Instruction::Lt
                    | Instruction::Lte
                    | Instruction::Gt
                    | Instruction::Gte
                    | Instruction::Eq
                    | Instruction::Neq
                    | Instruction::StrictEq
                    | Instruction::StrictNeq,
                    Instruction::IfTrue(_) | Instruction::IfFalse(_),
                    Instruction::Goto(_),
                    ..,
                ] => Some((34, 5)),
                [
                    Instruction::GetLocal(_) | Instruction::GetLocalCheck(_),
                    Instruction::GetLocal(_)
                    | Instruction::GetLocalCheck(_)
                    | Instruction::GetArg(_),
                    Instruction::Lt
                    | Instruction::Lte
                    | Instruction::Gt
                    | Instruction::Gte
                    | Instruction::Eq
                    | Instruction::Neq
                    | Instruction::StrictEq
                    | Instruction::StrictNeq,
                    Instruction::IfTrue(_) | Instruction::IfFalse(_),
                    ..,
                ] => Some((33, 4)),
                _ => None,
            };
            let old_candidate = local_compare
                .or(method)
                .or(local_field_add)
                .or(local_add_const)
                .or(local_add)
                .or(const_local_add)
                .or(update)
                .or_else(|| match rest {
                    [
                        Instruction::Lt
                        | Instruction::Lte
                        | Instruction::Gt
                        | Instruction::Gte
                        | Instruction::Eq
                        | Instruction::Neq
                        | Instruction::StrictEq
                        | Instruction::StrictNeq,
                        Instruction::IfTrue(_) | Instruction::IfFalse(_),
                        ..,
                    ] => Some((32, 2)),
                    [
                        Instruction::Add,
                        Instruction::PutLocal(index) | Instruction::PutLocalCheck(index),
                        ..,
                    ] if locals
                        .get(usize::from(*index))
                        .is_some_and(|d| !d.is_const && d.kind == ClosureVariableKind::Normal) =>
                    {
                        Some((64, 2))
                    }
                    [
                        Instruction::Add,
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index),
                        Instruction::Drop,
                        ..,
                    ] if locals
                        .get(usize::from(*index))
                        .is_some_and(|d| !d.is_const && d.kind == ClosureVariableKind::Normal) =>
                    {
                        Some((65, 3))
                    }
                    _ => None,
                });
            if let Some((flag, length)) = old_candidate {
                // The folded S1 Goto is an authenticated span tail: it may be
                // entered canonically on its own, so it is exempt from the
                // interior-entry rule.
                let interior_end = if flag == 34 {
                    pc + length - 1
                } else {
                    pc + length
                };
                if !entries[pc + 1..interior_end].iter().any(|v| *v) {
                    if flags.is_empty() {
                        flags.resize(code.len(), 0);
                    }
                    flags[pc] = flag;
                    any = true;
                }
            }
        }
        Self(any.then(|| flags.into()))
    }

    #[inline]
    fn flag(&self, pc: usize) -> u8 {
        self.0
            .as_ref()
            .and_then(|flags| flags.get(pc))
            .copied()
            .unwrap_or(0)
    }
    #[inline(always)]
    pub(crate) fn entry(&self, pc: usize) -> FusionEntry {
        FusionEntry(self.flag(pc))
    }
    // This lookup is on every direct local/argument read in `run`. Keeping it
    // outlined adds a call and caller spills even when no dense span exists.
    #[inline(always)]
    pub(crate) fn dense_span(&self, pc: usize) -> Option<DenseSpanKind> {
        self.entry(pc).dense_span()
    }
    #[inline]
    #[cfg(test)]
    pub(crate) fn update(&self, pc: usize) -> Option<UpdateLocal> {
        self.entry(pc).update()
    }
    #[inline]
    pub(crate) fn compare_branch(&self, pc: usize) -> bool {
        self.flag(pc) == 32
    }
    /// S1 `producer(a); producer(b); cmp; If*; [Goto]` span length. Admission
    /// is structural; the runtime guard requires both bindings to be direct
    /// numbers, so captured, TDZ and non-number operands fall back canonically.
    #[inline]
    #[cfg(test)]
    pub(crate) fn local_compare_branch(&self, pc: usize) -> Option<usize> {
        self.entry(pc).local_compare_branch()
    }
    #[inline]
    pub(crate) fn add_store(&self, pc: usize) -> bool {
        matches!(self.flag(pc), 64 | 65)
    }
    /// Only literals and direct binding reads may join a completed own read.
    /// Runtime guards retain canonical evaluation for TDZ/captured bindings.
    pub(crate) fn method_call(&self, pc: usize) -> Option<usize> {
        let flag = self.flag(pc);
        (160..=167).contains(&flag).then(|| usize::from(flag - 160))
    }
    /// Full borrowed-local addition begins before either operand copy. S2's
    /// numeric-literal writeback shares this span entry so each `GetLocal`
    /// pays one flag load for both shapes.
    pub(crate) fn local_add_span(&self, pc: usize) -> Option<usize> {
        self.entry(pc).local_add_span()
    }
    /// S3 `producer(acc); producer(base); GetField(key); Add; store(acc)[; Drop]`
    /// span length. Admission is structural; the runtime guard requires a
    /// direct number accumulator, a direct object base and a location-cache
    /// hit whose stored value is an immediate number.
    #[inline]
    #[cfg(test)]
    pub(crate) fn local_field_add_span(&self, pc: usize) -> Option<usize> {
        self.entry(pc).local_field_add_span()
    }
    /// Constant-left (prepend) LocalAdd span length. Admission is structural;
    /// the runtime still proves the constant is a String and the local is a
    /// direct, non-Object, domain-valid binding.
    pub(crate) fn const_add_span(&self, pc: usize) -> Option<usize> {
        match self.flag(pc) {
            130 => Some(4),
            131 => Some(5),
            _ => None,
        }
    }
    pub(crate) fn add_store_span(&self, pc: usize) -> usize {
        if self.flag(pc) == 65 { 3 } else { 2 }
    }
}

/// True when the operand is an immediate numeric literal: a raw `PushI32` or
/// a constant-pool entry holding an `Int`/`Float`. String and BigInt entries
/// stay on the canonical primitive-addition path.
fn numeric_immediate(instruction: &Instruction, constants: &[BytecodeConstant]) -> bool {
    match instruction {
        Instruction::PushI32(_) => true,
        Instruction::PushConst(index) => matches!(
            constants.get(*index as usize),
            Some(BytecodeConstant::Value(
                RawValue::Int(_) | RawValue::Float(_)
            ))
        ),
        _ => false,
    }
}

fn method_call_count(rest: &[Instruction]) -> Option<usize> {
    if !matches!(rest.first(), Some(Instruction::GetField2(_))) {
        return None;
    }
    for count in 0..=7 {
        match rest.get(count + 1)? {
            Instruction::CallMethod(arguments) if usize::from(*arguments) == count => {
                return Some(count);
            }
            Instruction::GetLocal(_)
            | Instruction::GetLocalCheck(_)
            | Instruction::GetArg(_)
            | Instruction::PushI32(_)
            | Instruction::Undefined
            | Instruction::Null
            | Instruction::PushTrue
            | Instruction::PushFalse => {}
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn local(constant: bool) -> VariableDefinition {
        VariableDefinition {
            name: None,
            is_lexical: true,
            is_const: constant,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        }
    }
    #[test]
    fn local_add_span_rejects_intermediate_entries_and_other_targets() {
        use Instruction::*;
        let code = [
            GetLocalCheck(0),
            GetLocalCheck(1),
            Add,
            PutLocalCheck(0),
            ReturnUndefined,
        ];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(false)], &[]).local_add_span(0),
            Some(4)
        );
        assert_eq!(
            FusionPlan::build(&code, &[local(true), local(false)], &[]).local_add_span(0),
            None
        );
        let code = [
            GetLocal(0),
            GetLocal(1),
            Add,
            SetLocal(0),
            Drop,
            ReturnUndefined,
        ];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(false)], &[]).local_add_span(0),
            Some(5)
        );
        for target in 1..5 {
            let mut code = code.to_vec();
            code.push(Goto(target));
            assert_eq!(
                FusionPlan::build(&code, &[local(false), local(false)], &[]).local_add_span(0),
                None
            );
        }
        let code = [GetLocal(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(false)], &[]).local_add_span(0),
            None
        );
    }

    #[test]
    fn local_field_add_span_requires_number_target_and_normal_base() {
        use Instruction::*;
        let code = [
            GetLocal(0),
            GetLocal(1),
            GetField(0),
            Add,
            PutLocal(0),
            ReturnUndefined,
        ];
        let plan = FusionPlan::build(&code, &[local(false), local(false)], &[]);
        assert_eq!(plan.local_field_add_span(0), Some(5));
        assert_eq!(plan.local_add_span(0), None);

        let code = [
            GetLocal(0),
            GetLocalCheck(1),
            GetField(0),
            Add,
            SetLocal(0),
            Drop,
            ReturnUndefined,
        ];
        let plan = FusionPlan::build(&code, &[local(false), local(false)], &[]);
        assert_eq!(plan.local_field_add_span(0), Some(6));

        // The store must target the accumulator local; a dropped or value-used
        // SetLocal is not part of the shape.
        for code in [
            [
                GetLocal(0),
                GetLocal(1),
                GetField(0),
                Add,
                PutLocal(1),
                ReturnUndefined,
            ],
            [
                GetLocal(0),
                GetLocal(1),
                GetField(0),
                Add,
                SetLocal(0),
                ReturnUndefined,
            ],
        ] {
            assert_eq!(
                FusionPlan::build(&code, &[local(false), local(false)], &[])
                    .local_field_add_span(0),
                None
            );
        }
        // A constant accumulator cannot be written.
        let code = [
            GetLocal(0),
            GetLocal(1),
            GetField(0),
            Add,
            PutLocal(0),
            ReturnUndefined,
        ];
        assert_eq!(
            FusionPlan::build(&code, &[local(true), local(false)], &[]).local_field_add_span(0),
            None
        );

        // Any interior control target rejects the span.
        let base = [
            GetLocal(0),
            GetLocal(1),
            GetField(0),
            Add,
            PutLocal(0),
            ReturnUndefined,
        ];
        for target in 1..5 {
            let mut code = base.to_vec();
            code.push(Goto(target));
            assert_eq!(
                FusionPlan::build(&code, &[local(false), local(false)], &[])
                    .local_field_add_span(0),
                None
            );
        }
    }

    #[test]
    fn constant_left_local_add_requires_normal_target_and_no_interior_entry() {
        use Instruction::*;
        let code = [
            PushConst(0),
            GetLocalCheck(1),
            Add,
            PutLocalCheck(1),
            ReturnUndefined,
        ];
        let plan = FusionPlan::build(&code, &[local(false), local(false)], &[]);
        assert_eq!(plan.const_add_span(0), Some(4));
        assert_eq!(plan.local_add_span(0), Some(4));
        assert_eq!(plan.const_add_span(1), None);

        let code = [
            PushConst(0),
            GetLocal(1),
            Add,
            SetLocal(1),
            Drop,
            ReturnUndefined,
        ];
        let plan = FusionPlan::build(&code, &[local(false), local(false)], &[]);
        assert_eq!(plan.const_add_span(0), Some(5));

        // Store must target the right local.
        let code = [PushConst(0), GetLocal(1), Add, PutLocal(0), ReturnUndefined];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(false)], &[]).const_add_span(0),
            None
        );
        // Constant target local is rejected.
        let code = [PushConst(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(true)], &[]).const_add_span(0),
            None
        );
        // Const-left shape is only tagged at the PushConst PC.
        let code = [PushConst(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        let plan = FusionPlan::build(&code, &[local(false), local(false)], &[]);
        assert_eq!(plan.local_add_span(1), None);
        // Any interior control target rejects the span.
        let base = [PushConst(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        for target in 1..4 {
            let mut code = base.to_vec();
            code.push(Goto(target));
            assert_eq!(
                FusionPlan::build(&code, &[local(false), local(false)], &[]).const_add_span(0),
                None
            );
        }
    }

    #[test]
    fn method_call_spans_reject_effectful_arguments_and_interior_entry() {
        use Instruction::*;
        let code = [GetField2(0), PushI32(1), PushFalse, CallMethod(2), Return];
        assert_eq!(FusionPlan::build(&code, &[], &[]).method_call(0), Some(2));
        let code = [GetField2(0), GetLocal(0), CallMethod(1), Return];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)], &[]).method_call(0),
            Some(1)
        );
        let code = [GetField2(0), PushI32(1), CallMethod(1), Goto(1), Return];
        assert_eq!(FusionPlan::build(&code, &[], &[]).method_call(0), None);
        let code = [GetField2(0), PushI32(1), TailCallMethod(1)];
        assert_eq!(FusionPlan::build(&code, &[], &[]).method_call(0), None);
    }
    #[test]
    fn add_store_requires_mutable_normal_target_and_no_interior_entry() {
        use Instruction::*;
        let code = [Add, PutLocal(0), ReturnUndefined];
        assert!(FusionPlan::build(&code, &[local(false)], &[]).add_store(0));
        assert!(!FusionPlan::build(&code, &[local(true)], &[]).add_store(0));
        let code = [Add, SetLocalCheck(0), Drop, ReturnUndefined];
        let plan = FusionPlan::build(&code, &[local(false)], &[]);
        assert!(plan.add_store(0));
        assert_eq!(plan.add_store_span(0), 3);
        let code = [Add, SetLocalCheck(0), Drop, Goto(2), ReturnUndefined];
        assert!(!FusionPlan::build(&code, &[local(false)], &[]).add_store(0));
        let code = [Add, PutLocal(0), Goto(1), ReturnUndefined];
        assert!(!FusionPlan::build(&code, &[local(false)], &[]).add_store(0));
    }
    #[test]
    fn finite_update_forms_preserve_their_logical_extent() {
        use Instruction::*;
        for (operation, store, postfix, discard) in [
            (Inc, SetLocal(0), false, false),
            (PostInc, PutLocal(0), true, false),
            (Dec, PutLocal(0), false, true),
        ] {
            let code = [GetLocalCheck(0), operation, store, Return];
            let plan = FusionPlan::build(&code, &[local(false)], &[]);
            let update = plan.update(0).unwrap();
            assert_eq!(
                (update.instructions, update.postfix, update.discard),
                (3, postfix, discard)
            );
            assert!(plan.update(1).is_none());
            assert!(
                FusionPlan::build(&code, &[local(true)], &[])
                    .update(0)
                    .is_none()
            );
        }
        let code = [GetLocal(0), PostDec, PutLocal(0), Drop, ReturnUndefined];
        let update = FusionPlan::build(&code, &[local(false)], &[])
            .update(0)
            .unwrap();
        assert_eq!((update.instructions, update.discard), (4, true));
    }
    #[test]
    fn local_compare_branch_spans_require_producers_and_fold_trailing_gotos() {
        use Instruction::*;
        let code = [
            GetLocal(0),
            GetArg(1),
            Lt,
            IfFalse(6),
            Goto(5),
            Nop,
            ReturnUndefined,
        ];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)], &[]).local_compare_branch(0),
            Some(5)
        );
        let code = [
            GetLocal(0),
            GetLocalCheck(1),
            Lte,
            IfTrue(4),
            ReturnUndefined,
        ];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)], &[]).local_compare_branch(0),
            Some(4)
        );
        // Any interior entry still rejects the unfused content.
        let code = [GetLocal(0), GetArg(1), Lt, IfFalse(5), Nop, Goto(1)];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)], &[]).local_compare_branch(0),
            None
        );
        // The first producer must be a local binding and the operator a
        // comparison; otherwise the canonical sequence stays in charge.
        let code = [GetArg(0), GetArg(1), Lt, IfFalse(4), Return];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)], &[]).local_compare_branch(0),
            None
        );
        let code = [GetLocal(0), GetArg(1), Add, IfFalse(4), Return];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)], &[]).local_compare_branch(0),
            None
        );
        assert_eq!(FusionPlan::default().local_compare_branch(0), None);
    }

    #[test]
    fn all_interior_control_targets_prevent_fusion() {
        use Instruction::*;
        for target in [1, 2] {
            for entry in [Goto(target), IfTrue(target), Catch(target), Gosub(target)] {
                let code = [GetLocal(0), Inc, PutLocal(0), entry, ReturnUndefined];
                assert!(
                    FusionPlan::build(&code, &[local(false)], &[])
                        .update(0)
                        .is_none()
                );
            }
        }
        let code = [Lt, IfFalse(3), Goto(1), ReturnUndefined];
        assert!(!FusionPlan::build(&code, &[], &[]).compare_branch(0));
        let code = [Lt, IfFalse(3), Nop, ReturnUndefined];
        assert!(FusionPlan::build(&code, &[], &[]).compare_branch(0));
    }
    #[test]
    fn branch_to_drop_keeps_the_drop_outside_the_span() {
        use Instruction::*;
        let code = [GetLocal(0), PostInc, PutLocal(0), Drop, Goto(3)];
        let update = FusionPlan::build(&code, &[local(false)], &[])
            .update(0)
            .unwrap();
        assert_eq!((update.instructions, update.discard), (3, false));
    }
}
