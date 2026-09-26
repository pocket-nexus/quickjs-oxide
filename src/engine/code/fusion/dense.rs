//! Published numeric spans over canonical instructions.
use super::super::bytecode::Instruction;
use super::super::function::metadata::{ClosureVariableKind, VariableDefinition};
use crate::engine::heap::{BytecodeConstant, RawValue};

#[cfg(test)]
thread_local! {
    static DISABLE_CANDIDATES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Build the same canonical bytecode and legacy fusion plan without new dense
/// candidates. The switch is thread-local so parallel crate tests cannot
/// change one another's publication. Production builds contain no switch.
#[cfg(test)]
pub(crate) fn with_dense_candidates_disabled<R>(run: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            DISABLE_CANDIDATES.with(|disabled| disabled.set(self.0));
        }
    }
    let previous = DISABLE_CANDIDATES.with(|disabled| disabled.replace(true));
    let _restore = Restore(previous);
    run()
}

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
    ReadPostInc = 3,
    ReadPreInc = 4,
    ReadBinary = 5,
    AccPut = 6,
    AccSetDrop = 7,
    AccIndexPut = 8,
    AccIndexSetDrop = 9,
    Store = 10,
    Copy = 11,
    StoreBinary = 12,
    UpdateElement = 13,
    ReadPostDec = 14,
    ReadPreDec = 15,
}

impl DenseSpanKind {
    pub(crate) const fn len(self) -> usize {
        match self {
            Self::Read => 3,
            Self::ReadIndexBinary
            | Self::ReadPostInc
            | Self::ReadPreInc
            | Self::ReadPostDec
            | Self::ReadPreDec
            | Self::ReadBinary => 5,
            Self::AccPut | Self::Store => 6,
            Self::AccSetDrop => 7,
            Self::AccIndexPut | Self::Copy | Self::StoreBinary | Self::UpdateElement => 8,
            Self::AccIndexSetDrop => 9,
        }
    }

    pub(crate) const fn peak(self) -> u8 {
        match self {
            Self::Read | Self::ReadPreInc | Self::ReadPreDec | Self::ReadBinary => 2,
            Self::ReadIndexBinary
            | Self::ReadPostInc
            | Self::ReadPostDec
            | Self::AccPut
            | Self::AccSetDrop => 3,
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
            | Self::ReadPostInc
            | Self::ReadPreInc
            | Self::ReadPostDec
            | Self::ReadPreDec
            | Self::ReadBinary => 1,
            _ => 0,
        }
    }

    pub(crate) const fn from_flag(flag: u8) -> Option<Self> {
        match flag {
            1 => Some(Self::Read),
            2 => Some(Self::ReadIndexBinary),
            3 => Some(Self::ReadPostInc),
            4 => Some(Self::ReadPreInc),
            5 => Some(Self::ReadBinary),
            6 => Some(Self::AccPut),
            7 => Some(Self::AccSetDrop),
            8 => Some(Self::AccIndexPut),
            9 => Some(Self::AccIndexSetDrop),
            10 => Some(Self::Store),
            11 => Some(Self::Copy),
            12 => Some(Self::StoreBinary),
            13 => Some(Self::UpdateElement),
            14 => Some(Self::ReadPostDec),
            15 => Some(Self::ReadPreDec),
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

fn writable_local(slot: DirectSlot, locals: &[VariableDefinition]) -> bool {
    let DirectSlot::Local(index) = slot else {
        return false;
    };
    locals
        .get(usize::from(index))
        .is_some_and(|local| local.kind == ClosureVariableKind::Normal && !local.is_const)
}

fn store_matches(
    instruction: &Instruction,
    slot: DirectSlot,
    locals: &[VariableDefinition],
    put: bool,
) -> bool {
    match (instruction, slot, put) {
        (
            Instruction::PutLocal(index) | Instruction::PutLocalCheck(index),
            DirectSlot::Local(target),
            true,
        )
        | (
            Instruction::SetLocal(index) | Instruction::SetLocalCheck(index),
            DirectSlot::Local(target),
            false,
        ) if index == &target => writable_local(slot, locals),
        (Instruction::PutArg(index), DirectSlot::Argument(target), true)
        | (Instruction::SetArg(index), DirectSlot::Argument(target), false) => index == &target,
        _ => false,
    }
}

fn index_binary(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Add | Instruction::Sub | Instruction::BitAnd
    )
}

fn number_binary(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Add
            | Instruction::Sub
            | Instruction::Mul
            | Instruction::Div
            | Instruction::BitAnd
            | Instruction::BitOr
            | Instruction::BitXor
            | Instruction::Shl
            | Instruction::Sar
            | Instruction::Shr
    )
}

fn matches_shape(
    kind: DenseSpanKind,
    code: &[Instruction],
    first: DirectSlot,
    locals: &[VariableDefinition],
    constants: &[BytecodeConstant],
) -> bool {
    let source = |at: usize| {
        code.get(at)
            .and_then(|instruction| numeric_source(instruction, locals, constants))
            .is_some()
    };
    let base = |at: usize| {
        code.get(at)
            .and_then(|instruction| direct_slot(instruction, locals))
            .is_some()
    };
    let is = |at: usize, expected: &Instruction| {
        code.get(at).is_some_and(|actual| {
            std::mem::discriminant(actual) == std::mem::discriminant(expected)
        })
    };
    match kind {
        DenseSpanKind::Read => source(1) && is(2, &Instruction::GetArrayEl),
        DenseSpanKind::ReadIndexBinary => {
            source(1)
                && source(2)
                && code.get(3).is_some_and(index_binary)
                && is(4, &Instruction::GetArrayEl)
        }
        DenseSpanKind::ReadPostInc
        | DenseSpanKind::ReadPreInc
        | DenseSpanKind::ReadPostDec
        | DenseSpanKind::ReadPreDec => {
            let Some(slot) = code.get(1).and_then(|i| direct_slot(i, locals)) else {
                return false;
            };
            let (operation, put) = match kind {
                DenseSpanKind::ReadPostInc => (is(2, &Instruction::PostInc), true),
                DenseSpanKind::ReadPreInc => (is(2, &Instruction::Inc), false),
                DenseSpanKind::ReadPostDec => (is(2, &Instruction::PostDec), true),
                DenseSpanKind::ReadPreDec => (is(2, &Instruction::Dec), false),
                _ => unreachable!(),
            };
            operation
                && code
                    .get(3)
                    .is_some_and(|instruction| store_matches(instruction, slot, locals, put))
                && is(4, &Instruction::GetArrayEl)
        }
        DenseSpanKind::ReadBinary => {
            source(1)
                && is(2, &Instruction::GetArrayEl)
                && source(3)
                && code.get(4).is_some_and(number_binary)
        }
        DenseSpanKind::AccPut
        | DenseSpanKind::AccSetDrop
        | DenseSpanKind::AccIndexPut
        | DenseSpanKind::AccIndexSetDrop => {
            let indexed = matches!(
                kind,
                DenseSpanKind::AccIndexPut | DenseSpanKind::AccIndexSetDrop
            );
            let put = matches!(kind, DenseSpanKind::AccPut | DenseSpanKind::AccIndexPut);
            let read_at = if indexed { 5 } else { 3 };
            let store_at = read_at + 2;
            writable_local(first, locals)
                && base(1)
                && source(2)
                && (!indexed || (source(3) && code.get(4).is_some_and(index_binary)))
                && is(read_at, &Instruction::GetArrayEl)
                && is(read_at + 1, &Instruction::Add)
                && code
                    .get(store_at)
                    .is_some_and(|instruction| store_matches(instruction, first, locals, put))
                && (put || is(store_at + 1, &Instruction::Drop))
        }
        DenseSpanKind::Store => {
            source(1)
                && source(2)
                && is(3, &Instruction::Insert3)
                && is(4, &Instruction::PutArrayEl)
                && is(5, &Instruction::Drop)
        }
        DenseSpanKind::Copy => {
            source(1)
                && base(2)
                && source(3)
                && is(4, &Instruction::GetArrayEl)
                && is(5, &Instruction::Insert3)
                && is(6, &Instruction::PutArrayEl)
                && is(7, &Instruction::Drop)
        }
        DenseSpanKind::StoreBinary => {
            source(1)
                && source(2)
                && source(3)
                && code.get(4).is_some_and(number_binary)
                && is(5, &Instruction::Insert3)
                && is(6, &Instruction::PutArrayEl)
                && is(7, &Instruction::Drop)
        }
        DenseSpanKind::UpdateElement => {
            source(1)
                && is(2, &Instruction::GetArrayEl3)
                && source(3)
                && code.get(4).is_some_and(number_binary)
                && is(5, &Instruction::Insert3)
                && is(6, &Instruction::PutArrayEl)
                && is(7, &Instruction::Drop)
        }
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
    #[cfg(test)]
    if DISABLE_CANDIDATES.with(std::cell::Cell::get) {
        return None;
    }
    // Ordinary bytecode PCs must not test dense candidate shapes.
    let first = direct_slot(rest.first()?, locals)?;
    const PRIORITY: [DenseSpanKind; 15] = [
        DenseSpanKind::AccIndexSetDrop,
        DenseSpanKind::AccIndexPut,
        DenseSpanKind::AccSetDrop,
        DenseSpanKind::AccPut,
        DenseSpanKind::Copy,
        DenseSpanKind::StoreBinary,
        DenseSpanKind::UpdateElement,
        DenseSpanKind::Store,
        DenseSpanKind::ReadBinary,
        DenseSpanKind::ReadPostInc,
        DenseSpanKind::ReadPostDec,
        DenseSpanKind::ReadPreInc,
        DenseSpanKind::ReadPreDec,
        DenseSpanKind::ReadIndexBinary,
        DenseSpanKind::Read,
    ];
    PRIORITY.into_iter().find(|kind| {
        rest.len() >= kind.len()
            && matches_shape(*kind, &rest[..kind.len()], first, locals, constants)
            && authenticated(rest, entries, *kind)
    })
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
        for flag in 1..=15 {
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
        for flag in [0, 16, 32, 34, 38, 64, 65, 128, 160, 167] {
            assert_eq!(DenseSpanKind::from_flag(flag), None);
        }
    }

    #[test]
    fn dense_manifest_matches_stack_contracts() {
        let cases: [(DenseSpanKind, Vec<Instruction>); 15] = [
            (
                DenseSpanKind::Read,
                vec![GetLocal(0), GetArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadIndexBinary,
                vec![GetLocal(0), GetArg(0), PushI32(1), Add, GetArrayEl],
            ),
            (
                DenseSpanKind::ReadPostInc,
                vec![GetLocal(0), GetArg(0), PostInc, PutArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadPreInc,
                vec![GetLocal(0), GetArg(0), Inc, SetArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadPostDec,
                vec![GetLocal(0), GetArg(0), PostDec, PutArg(0), GetArrayEl],
            ),
            (
                DenseSpanKind::ReadPreDec,
                vec![GetLocal(0), GetArg(0), Dec, SetArg(0), GetArrayEl],
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
            assert_eq!(plan(&code).dense_span(0), Some(kind), "{kind:?} published");
            for entry in 1..code.len() {
                let mut entries = vec![false; code.len()];
                entries[entry] = true;
                assert_ne!(
                    candidate(
                        &code,
                        &[local(ClosureVariableKind::Normal); 2],
                        &[],
                        &entries,
                    ),
                    Some(kind),
                    "{kind:?} admitted interior entry {entry}"
                );
            }
        }
    }

    #[test]
    fn dense_candidate_prefers_valid_longest() {
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

        let mut long = vec![GetLocal(0), GetArg(0), GetArrayEl, PushI32(1), Add];
        assert_eq!(plan(&long).dense_span(0), Some(DenseSpanKind::ReadBinary));
        long.push(Goto(4));
        assert_eq!(
            plan(&long).dense_span(0),
            Some(DenseSpanKind::Read),
            "invalid long span must not hide a valid short prefix"
        );
    }

    #[test]
    fn dense_canonical_only_keeps_legacy_fusion() {
        let dense = [GetLocal(0), GetArg(0), GetArrayEl];
        let legacy = [GetLocal(0), PushI32(1), Add, PutLocal(0)];
        assert_eq!(plan(&dense).dense_span(0), Some(DenseSpanKind::Read));
        super::super::with_dense_candidates_disabled(|| {
            assert_eq!(plan(&dense).dense_span(0), None);
            assert_eq!(plan(&legacy).local_add_span(0), Some(4));
        });
        assert_eq!(plan(&dense).dense_span(0), Some(DenseSpanKind::Read));
    }

    #[test]
    fn dense_producer_and_store_whitelist() {
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
            Some(DenseSpanKind::ReadBinary)
        );
        let index_binary = [GetLocal(0), GetArg(0), PushI32(1), Add, GetArrayEl];
        assert_eq!(
            FusionPlan::build(&index_binary, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::ReadIndexBinary)
        );

        let post = [
            GetArg(0),
            GetLocalCheck(1),
            PostInc,
            PutLocalCheck(1),
            GetArrayEl,
        ];
        assert_eq!(
            FusionPlan::build(&post, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::ReadPostInc)
        );
        for wrong_store in [PutArg(0), SetLocal(1), PutLocal(0), PutLocal(1)] {
            let code = [GetArg(0), GetArg(1), PostInc, wrong_store, GetArrayEl];
            assert_eq!(
                FusionPlan::build(&code, &locals, &constants).dense_span(0),
                None,
                "incorrect postfix store {code:?}"
            );
        }
        let pre = [GetArg(0), GetArg(1), Inc, SetArg(1), GetArrayEl];
        assert_eq!(
            FusionPlan::build(&pre, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::ReadPreInc)
        );
        let mut const_local = locals;
        const_local[1].is_const = true;
        assert_eq!(
            FusionPlan::build(&post, &const_local, &constants).dense_span(0),
            None,
            "const local cannot be an update target"
        );
        let acc = [
            GetLocal(1),
            GetLocal(0),
            GetArg(0),
            GetArrayEl,
            Add,
            PutLocal(1),
        ];
        assert_eq!(
            FusionPlan::build(&acc, &const_local, &constants).dense_span(0),
            None,
            "const local cannot be an accumulator"
        );
        let mut wrong_acc = acc;
        wrong_acc[5] = PutLocal(0);
        assert_ne!(
            FusionPlan::build(&wrong_acc, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::AccPut)
        );
        let mut bad_value = [
            GetArg(0),
            GetArg(1),
            PushConst(1),
            Insert3,
            PutArrayEl,
            Drop,
        ];
        assert_eq!(
            FusionPlan::build(&bad_value, &locals, &constants).dense_span(0),
            None
        );
        bad_value[2] = PushConst(0);
        assert_eq!(
            FusionPlan::build(&bad_value, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::Store)
        );
        let kept_assignment = [GetArg(0), GetArg(1), PushI32(1), Insert3, PutArrayEl];
        assert_eq!(
            FusionPlan::build(&kept_assignment, &locals, &constants).dense_span(0),
            None,
            "W0 must require trailing Drop"
        );
        let wrong_compound = [
            GetArg(0),
            GetArg(1),
            GetArrayEl,
            PushI32(1),
            Add,
            Insert3,
            PutArrayEl,
            Drop,
        ];
        assert_eq!(
            FusionPlan::build(&wrong_compound, &locals, &constants).dense_span(0),
            Some(DenseSpanKind::ReadBinary),
            "GetArrayEl cannot stand in for W3's GetArrayEl3"
        );
    }

    #[test]
    fn dense_update_direction_is_published_only_for_the_authenticated_shape() {
        let locals = [local(ClosureVariableKind::Normal); 2];
        for (kind, update, store, wrong_store, wrong_mode) in [
            (
                DenseSpanKind::ReadPostInc,
                PostInc,
                PutArg(1),
                PutArg(0),
                SetArg(1),
            ),
            (
                DenseSpanKind::ReadPreInc,
                Inc,
                SetArg(1),
                SetArg(0),
                PutArg(1),
            ),
            (
                DenseSpanKind::ReadPostDec,
                PostDec,
                PutArg(1),
                PutArg(0),
                SetArg(1),
            ),
            (
                DenseSpanKind::ReadPreDec,
                Dec,
                SetArg(1),
                SetArg(0),
                PutArg(1),
            ),
        ] {
            let correct = [
                GetArg(0),
                GetArg(1),
                update.clone(),
                store.clone(),
                GetArrayEl,
            ];
            assert_eq!(
                FusionPlan::build(&correct, &locals, &[]).dense_span(0),
                Some(kind),
                "{correct:?}"
            );
            for invalid in [
                [
                    GetArg(0),
                    GetArg(1),
                    update.clone(),
                    wrong_store,
                    GetArrayEl,
                ],
                [GetArg(0), GetArg(1), update.clone(), wrong_mode, GetArrayEl],
                [GetArg(0), GetArg(1), update, store.clone(), GetArrayEl3],
                [GetArg(0), GetArg(1), PushI32(1), store, GetArrayEl],
            ] {
                assert_eq!(
                    FusionPlan::build(&invalid, &locals, &[]).dense_span(0),
                    None,
                    "incorrect direction or writeback {invalid:?}"
                );
            }
        }
    }
}
