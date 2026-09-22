//! Authenticated execution spans over canonical, verified instruction PCs.
//!
//! The published bytecode and its source/relocation tables are never rewritten.
//! A span is entered only at its first instruction, contains no external entry,
//! and falls back to that original instruction before changing any VM state.
//! Legacy execution and serialization continue to consume canonical opcodes.
use super::bytecode::Instruction;
use super::function::metadata::{ClosureVariableKind, VariableDefinition};
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct FusionPlan(Option<Rc<[u8]>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UpdateLocal {
    pub increment: bool,
    pub postfix: bool,
    pub discard: bool,
    pub instructions: usize,
}

#[cfg(any(test, oxide_store_drop_fusion))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreDrop {
    Local,
    Argument,
}

/// Decoded descriptors are temporary. Published plans still store one byte per
/// canonical PC, and the established tag values remain unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FusionKind {
    UpdateLocal(UpdateLocal),
    CompareBranch,
    AddStore {
        discard: bool,
    },
    LocalAdd {
        constant_left: bool,
        discard: bool,
    },
    MethodCall {
        arguments: u8,
    },
    #[cfg(any(test, oxide_store_drop_fusion))]
    StoreDrop(StoreDrop),
}

impl FusionKind {
    #[inline]
    fn encode(self) -> u8 {
        match self {
            Self::UpdateLocal(update) => {
                16 | u8::from(update.increment)
                    | (u8::from(update.postfix) << 1)
                    | (u8::from(update.discard) << 2)
                    | (u8::from(update.instructions == 4) << 3)
            }
            Self::CompareBranch => 32,
            Self::AddStore { discard } => 64 | u8::from(discard),
            Self::LocalAdd {
                constant_left,
                discard,
            } => 128 | (u8::from(constant_left) << 1) | u8::from(discard),
            Self::MethodCall { arguments } => 160 + arguments,
            #[cfg(any(test, oxide_store_drop_fusion))]
            Self::StoreDrop(StoreDrop::Local) => 96,
            #[cfg(any(test, oxide_store_drop_fusion))]
            Self::StoreDrop(StoreDrop::Argument) => 97,
        }
    }

    #[cfg(test)]
    fn decode(tag: u8) -> Option<Self> {
        Some(match tag {
            16..=31 => Self::UpdateLocal(UpdateLocal {
                increment: tag & 1 != 0,
                postfix: tag & 2 != 0,
                discard: tag & 4 != 0,
                instructions: if tag & 8 != 0 { 4 } else { 3 },
            }),
            32 => Self::CompareBranch,
            64 | 65 => Self::AddStore { discard: tag == 65 },
            128..=131 => Self::LocalAdd {
                constant_left: tag & 2 != 0,
                discard: tag & 1 != 0,
            },
            160..=167 => Self::MethodCall {
                arguments: tag - 160,
            },
            #[cfg(any(test, oxide_store_drop_fusion))]
            96 => Self::StoreDrop(StoreDrop::Local),
            #[cfg(any(test, oxide_store_drop_fusion))]
            97 => Self::StoreDrop(StoreDrop::Argument),
            _ => return None,
        })
    }

    fn instructions(self) -> usize {
        match self {
            Self::UpdateLocal(update) => update.instructions,
            Self::CompareBranch => 2,
            Self::AddStore { discard } => {
                if discard {
                    3
                } else {
                    2
                }
            }
            Self::LocalAdd { discard, .. } => {
                if discard {
                    5
                } else {
                    4
                }
            }
            Self::MethodCall { arguments } => usize::from(arguments) + 2,
            #[cfg(any(test, oxide_store_drop_fusion))]
            Self::StoreDrop(_) => 2,
        }
    }
}

impl FusionPlan {
    pub(crate) fn build(code: &[Instruction], locals: &[VariableDefinition]) -> Self {
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
                        Some(FusionKind::UpdateLocal(UpdateLocal {
                            increment,
                            postfix,
                            discard: !keeps || drop,
                            instructions: if drop { 4 } else { 3 },
                        }))
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
                            Some(FusionKind::LocalAdd {
                                constant_left: false,
                                discard: false,
                            })
                        }
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index)
                            if index == left && matches!(rest.get(4), Some(Instruction::Drop)) =>
                        {
                            Some(FusionKind::LocalAdd {
                                constant_left: false,
                                discard: true,
                            })
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let method = method_call_count(rest).map(|count| FusionKind::MethodCall {
                arguments: count as u8,
            });
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
                            Some(FusionKind::LocalAdd {
                                constant_left: true,
                                discard: false,
                            })
                        }
                        Instruction::SetLocal(index) | Instruction::SetLocalCheck(index)
                            if index == right && matches!(rest.get(4), Some(Instruction::Drop)) =>
                        {
                            Some(FusionKind::LocalAdd {
                                constant_left: true,
                                discard: true,
                            })
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let candidate = method
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
                    ] => Some(FusionKind::CompareBranch),
                    [
                        Instruction::Add,
                        Instruction::PutLocal(index) | Instruction::PutLocalCheck(index),
                        ..,
                    ] if locals
                        .get(usize::from(*index))
                        .is_some_and(|d| !d.is_const && d.kind == ClosureVariableKind::Normal) =>
                    {
                        Some(FusionKind::AddStore { discard: false })
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
                        Some(FusionKind::AddStore { discard: true })
                    }
                    _ => None,
                });
            #[cfg(any(test, oxide_store_drop_fusion))]
            let candidate = candidate.or_else(|| store_drop_kind(rest, locals));
            if let Some(kind) = candidate {
                let length = kind.instructions();
                if !entries[pc + 1..pc + length].iter().any(|v| *v) {
                    if flags.is_empty() {
                        flags.resize(code.len(), 0);
                    }
                    flags[pc] = kind.encode();
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
    // Each hot query reads only its own tag family. Decoding the whole
    // descriptor union here repeats unrelated range checks on every opcode.
    #[inline]
    pub(crate) fn update(&self, pc: usize) -> Option<UpdateLocal> {
        let tag = self.flag(pc);
        if !(16..=31).contains(&tag) {
            return None;
        }
        Some(UpdateLocal {
            increment: tag & 1 != 0,
            postfix: tag & 2 != 0,
            discard: tag & 4 != 0,
            instructions: if tag & 8 != 0 { 4 } else { 3 },
        })
    }
    #[inline]
    pub(crate) fn compare_branch(&self, pc: usize) -> bool {
        self.flag(pc) == 32
    }
    #[inline]
    pub(crate) fn add_store(&self, pc: usize) -> bool {
        matches!(self.flag(pc), 64 | 65)
    }
    /// Only literals and direct binding reads may join a completed own read.
    /// Runtime guards retain canonical evaluation for TDZ/captured bindings.
    #[inline]
    pub(crate) fn method_call(&self, pc: usize) -> Option<usize> {
        let tag = self.flag(pc);
        (160..=167).contains(&tag).then(|| usize::from(tag - 160))
    }
    /// Full borrowed-local addition begins before either operand copy.
    #[inline]
    pub(crate) fn local_add_span(&self, pc: usize) -> Option<usize> {
        let tag = self.flag(pc);
        (128..=131).contains(&tag).then(|| 4 + usize::from(tag & 1))
    }
    /// Constant-left (prepend) LocalAdd span length. Admission is structural;
    /// the runtime still proves the constant and binding before execution.
    #[inline]
    pub(crate) fn const_add_span(&self, pc: usize) -> Option<usize> {
        let tag = self.flag(pc);
        matches!(tag, 130 | 131).then(|| 4 + usize::from(tag & 1))
    }
    #[inline]
    pub(crate) fn add_store_span(&self, pc: usize) -> usize {
        if self.flag(pc) == 65 { 3 } else { 2 }
    }

    /// Structural certificate only; execution still proves direct scalar
    /// source and displaced bindings at the authenticated SetLocal/SetArg.
    #[cfg(any(test, oxide_store_drop_fusion))]
    #[inline]
    pub(crate) fn store_drop(&self, pc: usize) -> Option<StoreDrop> {
        match self.flag(pc) {
            96 => Some(StoreDrop::Local),
            97 => Some(StoreDrop::Argument),
            _ => None,
        }
    }
}

#[cfg(any(test, oxide_store_drop_fusion))]
fn store_drop_kind(rest: &[Instruction], locals: &[VariableDefinition]) -> Option<FusionKind> {
    let target = match rest {
        [Instruction::SetLocal(index), Instruction::Drop, ..]
            if locals.get(usize::from(*index)).is_some_and(|definition| {
                definition.kind == ClosureVariableKind::Normal && !definition.is_const
            }) =>
        {
            StoreDrop::Local
        }
        // Mapped arguments and physical argument bounds are runtime/verifier
        // obligations; local metadata cannot establish either property.
        [Instruction::SetArg(_), Instruction::Drop, ..] => StoreDrop::Argument,
        _ => return None,
    };
    Some(FusionKind::StoreDrop(target))
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
mod store_drop_tests;

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
            FusionPlan::build(&code, &[local(false), local(false)]).local_add_span(0),
            Some(4)
        );
        assert_eq!(
            FusionPlan::build(&code, &[local(true), local(false)]).local_add_span(0),
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
            FusionPlan::build(&code, &[local(false), local(false)]).local_add_span(0),
            Some(5)
        );
        for target in 1..5 {
            let mut code = code.to_vec();
            code.push(Goto(target));
            assert_eq!(
                FusionPlan::build(&code, &[local(false), local(false)]).local_add_span(0),
                None
            );
        }
        let code = [GetLocal(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(false)]).local_add_span(0),
            None
        );
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
        let plan = FusionPlan::build(&code, &[local(false), local(false)]);
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
        let plan = FusionPlan::build(&code, &[local(false), local(false)]);
        assert_eq!(plan.const_add_span(0), Some(5));

        // Store must target the right local.
        let code = [PushConst(0), GetLocal(1), Add, PutLocal(0), ReturnUndefined];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(false)]).const_add_span(0),
            None
        );
        // Constant target local is rejected.
        let code = [PushConst(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        assert_eq!(
            FusionPlan::build(&code, &[local(false), local(true)]).const_add_span(0),
            None
        );
        // Const-left shape is only tagged at the PushConst PC.
        let code = [PushConst(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        let plan = FusionPlan::build(&code, &[local(false), local(false)]);
        assert_eq!(plan.local_add_span(1), None);
        // Any interior control target rejects the span.
        let base = [PushConst(0), GetLocal(1), Add, PutLocal(1), ReturnUndefined];
        for target in 1..4 {
            let mut code = base.to_vec();
            code.push(Goto(target));
            assert_eq!(
                FusionPlan::build(&code, &[local(false), local(false)]).const_add_span(0),
                None
            );
        }
    }

    #[test]
    fn method_call_spans_reject_effectful_arguments_and_interior_entry() {
        use Instruction::*;
        let code = [GetField2(0), PushI32(1), PushFalse, CallMethod(2), Return];
        assert_eq!(FusionPlan::build(&code, &[]).method_call(0), Some(2));
        let code = [GetField2(0), GetLocal(0), CallMethod(1), Return];
        assert_eq!(
            FusionPlan::build(&code, &[local(false)]).method_call(0),
            Some(1)
        );
        let code = [GetField2(0), PushI32(1), CallMethod(1), Goto(1), Return];
        assert_eq!(FusionPlan::build(&code, &[]).method_call(0), None);
        let code = [GetField2(0), PushI32(1), TailCallMethod(1)];
        assert_eq!(FusionPlan::build(&code, &[]).method_call(0), None);
    }
    #[test]
    fn add_store_requires_mutable_normal_target_and_no_interior_entry() {
        use Instruction::*;
        let code = [Add, PutLocal(0), ReturnUndefined];
        assert!(FusionPlan::build(&code, &[local(false)]).add_store(0));
        assert!(!FusionPlan::build(&code, &[local(true)]).add_store(0));
        let code = [Add, SetLocalCheck(0), Drop, ReturnUndefined];
        let plan = FusionPlan::build(&code, &[local(false)]);
        assert!(plan.add_store(0));
        assert_eq!(plan.add_store_span(0), 3);
        let code = [Add, SetLocalCheck(0), Drop, Goto(2), ReturnUndefined];
        assert!(!FusionPlan::build(&code, &[local(false)]).add_store(0));
        let code = [Add, PutLocal(0), Goto(1), ReturnUndefined];
        assert!(!FusionPlan::build(&code, &[local(false)]).add_store(0));
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
            let plan = FusionPlan::build(&code, &[local(false)]);
            let update = plan.update(0).unwrap();
            assert_eq!(
                (update.instructions, update.postfix, update.discard),
                (3, postfix, discard)
            );
            assert!(plan.update(1).is_none());
            assert!(FusionPlan::build(&code, &[local(true)]).update(0).is_none());
        }
        let code = [GetLocal(0), PostDec, PutLocal(0), Drop, ReturnUndefined];
        let update = FusionPlan::build(&code, &[local(false)]).update(0).unwrap();
        assert_eq!((update.instructions, update.discard), (4, true));
    }
    #[test]
    fn all_interior_control_targets_prevent_fusion() {
        use Instruction::*;
        for target in [1, 2] {
            for entry in [Goto(target), IfTrue(target), Catch(target), Gosub(target)] {
                let code = [GetLocal(0), Inc, PutLocal(0), entry, ReturnUndefined];
                assert!(
                    FusionPlan::build(&code, &[local(false)])
                        .update(0)
                        .is_none()
                );
            }
        }
        let code = [Lt, IfFalse(3), Goto(1), ReturnUndefined];
        assert!(!FusionPlan::build(&code, &[]).compare_branch(0));
        let code = [Lt, IfFalse(3), Nop, ReturnUndefined];
        assert!(FusionPlan::build(&code, &[]).compare_branch(0));
    }
    #[test]
    fn branch_to_drop_keeps_the_drop_outside_the_span() {
        use Instruction::*;
        let code = [GetLocal(0), PostInc, PutLocal(0), Drop, Goto(3)];
        let update = FusionPlan::build(&code, &[local(false)]).update(0).unwrap();
        assert_eq!((update.instructions, update.discard), (3, false));
    }
}
