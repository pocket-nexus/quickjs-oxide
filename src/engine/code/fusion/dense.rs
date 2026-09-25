//! Published numeric spans. Only `Read` is admitted until its complete
//! runtime path has been integrated and measured.
use super::super::bytecode::Instruction;
use super::super::function::metadata::{ClosureVariableKind, VariableDefinition};
use crate::engine::heap::{BytecodeConstant, RawValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectSlot {
    Local(u16),
    Argument(u16),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum NumericSource {
    Slot(DirectSlot),
    I32(i32),
    Constant(u32),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DenseSpanKind {
    Read = 1,
    ReadIndexBinary = 2,
    ReadPostUpdate = 3,
    ReadPreUpdate = 4,
    ReadBinary = 5,
    AccPut = 6,
    AccSetDrop = 7,
    AccIndexPut = 8,
    AccIndexSetDrop = 9,
    Store = 10,
    Copy = 11,
    StoreBinary = 12,
    UpdateElement = 13,
}

impl DenseSpanKind {
    pub(crate) const fn len(self) -> usize {
        match self {
            Self::Read => 3,
            Self::ReadIndexBinary
            | Self::ReadPostUpdate
            | Self::ReadPreUpdate
            | Self::ReadBinary => 5,
            Self::AccPut | Self::Store => 6,
            Self::AccSetDrop => 7,
            Self::AccIndexPut | Self::Copy | Self::StoreBinary | Self::UpdateElement => 8,
            Self::AccIndexSetDrop => 9,
        }
    }

    pub(crate) const fn peak(self) -> u8 {
        match self {
            Self::Read | Self::ReadPreUpdate | Self::ReadBinary => 2,
            Self::ReadIndexBinary | Self::ReadPostUpdate | Self::AccPut | Self::AccSetDrop => 3,
            Self::AccIndexPut
            | Self::AccIndexSetDrop
            | Self::Store
            | Self::Copy
            | Self::StoreBinary
            | Self::UpdateElement => 4,
        }
    }

    pub(crate) const fn delta(self) -> i8 {
        match self {
            Self::Read
            | Self::ReadIndexBinary
            | Self::ReadPostUpdate
            | Self::ReadPreUpdate
            | Self::ReadBinary => 1,
            _ => 0,
        }
    }

    pub(crate) const fn from_flag(flag: u8) -> Option<Self> {
        match flag {
            1 => Some(Self::Read),
            2 => Some(Self::ReadIndexBinary),
            3 => Some(Self::ReadPostUpdate),
            4 => Some(Self::ReadPreUpdate),
            5 => Some(Self::ReadBinary),
            6 => Some(Self::AccPut),
            7 => Some(Self::AccSetDrop),
            8 => Some(Self::AccIndexPut),
            9 => Some(Self::AccIndexSetDrop),
            10 => Some(Self::Store),
            11 => Some(Self::Copy),
            12 => Some(Self::StoreBinary),
            13 => Some(Self::UpdateElement),
            _ => None,
        }
    }
}

fn direct_slot(instruction: &Instruction, locals: &[VariableDefinition]) -> Option<DirectSlot> {
    match instruction {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index)
            if locals
                .get(usize::from(*index))
                .is_some_and(|local| local.kind == ClosureVariableKind::Normal) =>
        {
            Some(DirectSlot::Local(*index))
        }
        Instruction::GetArg(index) => Some(DirectSlot::Argument(*index)),
        _ => None,
    }
}

fn numeric_source(
    instruction: &Instruction,
    locals: &[VariableDefinition],
    constants: &[BytecodeConstant],
) -> Option<NumericSource> {
    match instruction {
        Instruction::PushI32(value) => Some(NumericSource::I32(*value)),
        Instruction::PushConst(index)
            if matches!(
                constants.get(*index as usize),
                Some(BytecodeConstant::Value(
                    RawValue::Int(_) | RawValue::Float(_)
                ))
            ) =>
        {
            Some(NumericSource::Constant(*index))
        }
        _ => direct_slot(instruction, locals).map(NumericSource::Slot),
    }
}

/// Check a candidate against the canonical stack effects before publishing.
/// `entries` starts at the candidate's first PC.
fn authenticated(rest: &[Instruction], entries: &[bool], kind: DenseSpanKind) -> bool {
    let length = kind.len();
    if rest.len() < length
        || entries
            .get(1..length)
            .is_none_or(|interior| interior.iter().any(|entry| *entry))
    {
        return false;
    }
    let mut depth = 0_usize;
    let mut peak = 0_usize;
    for instruction in &rest[..length] {
        let effect = instruction.stack_contract();
        if depth < effect.popped {
            return false;
        }
        depth = depth - effect.popped + effect.pushed;
        peak = peak.max(depth);
    }
    peak == usize::from(kind.peak()) && depth == kind.delta() as usize
}

pub(super) fn candidate(
    rest: &[Instruction],
    locals: &[VariableDefinition],
    constants: &[BytecodeConstant],
    entries: &[bool],
) -> Option<DenseSpanKind> {
    // This first-opcode filter avoids work for ordinary bytecode sites.
    direct_slot(rest.first()?, locals)?;
    if matches!(rest.get(2), Some(Instruction::GetArrayEl))
        && rest
            .get(1)
            .and_then(|instruction| numeric_source(instruction, locals, constants))
            .is_some()
        && authenticated(rest, entries, DenseSpanKind::Read)
    {
        return Some(DenseSpanKind::Read);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::FusionPlan;
    use super::*;
    use Instruction::*;

    fn local(kind: ClosureVariableKind) -> VariableDefinition {
        VariableDefinition {
            name: None,
            is_lexical: true,
            is_const: false,
            is_parameter_initializer: false,
            kind,
        }
    }

    fn plan(code: &[Instruction]) -> FusionPlan {
        FusionPlan::build(code, &[local(ClosureVariableKind::Normal); 2], &[])
    }

    #[test]
    fn dense_flags_do_not_alias_legacy_accessors() {
        for flag in 1..=13 {
            let kind = DenseSpanKind::from_flag(flag).expect("reserved dense flag");
            assert_eq!(kind as u8, flag);
            assert_eq!(flag & 16, 0);
            let plan = FusionPlan(Some(vec![flag].into()));
            assert_eq!(plan.dense_span(0), Some(kind));
            assert!(plan.update(0).is_none());
            assert!(!plan.compare_branch(0));
            assert!(plan.local_compare_branch(0).is_none());
            assert!(!plan.add_store(0));
            assert!(plan.local_add_span(0).is_none());
            assert!(plan.local_field_add_span(0).is_none());
            assert!(plan.const_add_span(0).is_none());
            assert!(plan.method_call(0).is_none());
        }
        for flag in [0, 14, 15, 16, 32, 34, 38, 64, 65, 128, 160, 167] {
            assert_eq!(DenseSpanKind::from_flag(flag), None);
        }
    }

    #[test]
    fn dense_manifest_matches_stack_contracts() {
        let cases: [(DenseSpanKind, Vec<Instruction>); 13] = [
            (
                DenseSpanKind::Read,
                vec![GetLocal(0), GetArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadIndexBinary,
                vec![GetLocal(0), GetArg(0), PushI32(1), Add, GetArrayEl],
            ),
            (
                DenseSpanKind::ReadPostUpdate,
                vec![GetLocal(0), GetArg(0), PostInc, PutArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadPreUpdate,
                vec![GetLocal(0), GetArg(0), Inc, SetArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadBinary,
                vec![GetLocal(0), GetArg(0), GetArrayEl, PushI32(1), Add],
            ),
            (
                DenseSpanKind::AccPut,
                vec![
                    GetLocal(1),
                    GetLocal(0),
                    GetArg(0),
                    GetArrayEl,
                    Add,
                    PutLocal(1),
                ],
            ),
            (
                DenseSpanKind::AccSetDrop,
                vec![
                    GetLocal(1),
                    GetLocal(0),
                    GetArg(0),
                    GetArrayEl,
                    Add,
                    SetLocal(1),
                    Drop,
                ],
            ),
            (
                DenseSpanKind::AccIndexPut,
                vec![
                    GetLocal(1),
                    GetLocal(0),
                    GetArg(0),
                    PushI32(1),
                    Add,
                    GetArrayEl,
                    Add,
                    PutLocal(1),
                ],
            ),
            (
                DenseSpanKind::AccIndexSetDrop,
                vec![
                    GetLocal(1),
                    GetLocal(0),
                    GetArg(0),
                    PushI32(1),
                    Add,
                    GetArrayEl,
                    Add,
                    SetLocal(1),
                    Drop,
                ],
            ),
            (
                DenseSpanKind::Store,
                vec![
                    GetLocal(0),
                    GetArg(0),
                    PushI32(1),
                    Insert3,
                    PutArrayEl,
                    Drop,
                ],
            ),
            (
                DenseSpanKind::Copy,
                vec![
                    GetLocal(0),
                    GetArg(0),
                    GetLocal(1),
                    GetArg(1),
                    GetArrayEl,
                    Insert3,
                    PutArrayEl,
                    Drop,
                ],
            ),
            (
                DenseSpanKind::StoreBinary,
                vec![
                    GetLocal(0),
                    GetArg(0),
                    PushI32(1),
                    PushI32(2),
                    Add,
                    Insert3,
                    PutArrayEl,
                    Drop,
                ],
            ),
            (
                DenseSpanKind::UpdateElement,
                vec![
                    GetLocal(0),
                    GetArg(0),
                    GetArrayEl3,
                    PushI32(1),
                    Add,
                    Insert3,
                    PutArrayEl,
                    Drop,
                ],
            ),
        ];
        for (kind, code) in cases {
            assert_eq!(kind.len(), code.len(), "{kind:?} length");
            assert!(
                authenticated(&code, &vec![false; code.len()], kind),
                "{kind:?} stack contract"
            );
        }
    }

    #[test]
    fn dense_r0_candidate_checks_all_interior_entries() {
        let original = vec![GetLocal(0), GetArg(0), GetArrayEl, Return];
        assert_eq!(plan(&original).dense_span(0), Some(DenseSpanKind::Read));
        for target in 1..3 {
            let mut code = original.clone();
            code.push(Goto(target));
            assert_eq!(plan(&code).dense_span(0), None, "entry at {target}");
        }
        let mut code = original;
        code.push(Goto(3));
        assert_eq!(plan(&code).dense_span(0), Some(DenseSpanKind::Read));
    }

    #[test]
    fn dense_r0_producer_whitelist_and_only_r0_is_published() {
        let locals = [local(ClosureVariableKind::Normal); 2];
        let constants = [
            BytecodeConstant::Value(RawValue::Float(2.0)),
            BytecodeConstant::Value(RawValue::Undefined),
        ];
        for base in [GetLocal(0), GetLocalCheck(0), GetArg(0)] {
            for index in [
                GetLocal(1),
                GetLocalCheck(1),
                GetArg(1),
                PushI32(1),
                PushConst(0),
            ] {
                let code = [base.clone(), index, GetArrayEl];
                assert_eq!(
                    FusionPlan::build(&code, &locals, &constants).dense_span(0),
                    Some(DenseSpanKind::Read),
                    "{code:?}"
                );
            }
        }
        for base in [PushThis, GetVarRef(0), GetVar(0), GetField(0), PushConst(0)] {
            let code = [base, GetArg(0), GetArrayEl];
            assert_eq!(
                FusionPlan::build(&code, &locals, &constants).dense_span(0),
                None
            );
        }
        for index in [
            PushConst(1),
            PushConst(2),
            PushThis,
            GetVarRef(0),
            GetField(0),
        ] {
            let code = [GetLocal(0), index, GetArrayEl];
            assert_eq!(
                FusionPlan::build(&code, &locals, &constants).dense_span(0),
                None
            );
        }
        let code = [GetLocal(0), GetArg(0), GetArrayEl];
        let invalid = [local(ClosureVariableKind::FunctionName)];
        assert_eq!(
            FusionPlan::build(&code, &invalid, &constants).dense_span(0),
            None
        );
        let read_binary = [GetLocal(0), GetArg(0), GetArrayEl, PushI32(1), Add];
        assert_eq!(
            FusionPlan::build(&read_binary, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::Read),
            "an unpublished R4 must leave its valid R0 prefix available"
        );
        let index_binary = [GetLocal(0), GetArg(0), PushI32(1), Add, GetArrayEl];
        assert_eq!(
            FusionPlan::build(&index_binary, &locals, &constants).dense_span(0),
            None
        );
    }
}
