//! Operand metadata for one planned numeric operation in `ExecCode`.
//! Compiler use graphs are discarded after publication; this descriptor stays.
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::value::number::operations::Number;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectSource {
    Local(u16),
    CheckedLocal(u16),
    Argument(u16),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum NumberSource {
    Direct(DirectSource),
    Immediate(i32),
    Constant { index: u32, value: Number },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArrayProductSource {
    pub array: DirectSource,
    pub index: NumberSource,
    pub scale: NumberSource,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArrayReadSource {
    pub array: DirectSource,
    pub index: NumberSource,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum UpdateDelta {
    Number(NumberSource),
    ArrayProduct(ArrayProductSource),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum NumericOperation {
    Accumulate {
        destination: u16,
        scale: NumberSource,
        checked: bool,
    },
    StoreProduct {
        destination: u16,
        scale: NumberSource,
        checked: bool,
    },
    CopyElement {
        source: ArrayReadSource,
    },
    /// Add one Array Number to the existing top Number after ++local index.
    AddPreInc,
    /// Two adjacent preincremented reads feed the same stack accumulator.
    /// The second index names a new binding version when it aliases the first.
    AddPreIncPair {
        second_array: DirectSource,
        second_index: DirectSource,
        second_reads_updated_index: bool,
    },
    /// Consume array, index and Number from the operand stack, then store the
    /// Number into the array and an initialized direct local.
    StoreElementAndLocal {
        destination: u16,
        checked: bool,
    },
    UpdateElement {
        delta: UpdateDelta,
    },
    CompareBranch {
        rhs: NumberSource,
        comparison: Opcode,
        when_true: bool,
        target: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NumericRegion {
    /// Inclusive compiler-instruction start and exclusive end.
    pub start: u32,
    pub end: u32,
    pub array: DirectSource,
    pub index: NumberSource,
    pub operation: NumericOperation,
    /// Maximum operand depth above the depth at `start` in generic execution.
    pub peak: u16,
}

/// Operands retained after publication. The opcode is the sole operation tag;
/// source instruction ranges and compiler targets never reach execution.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PublishedNumericRegion {
    pub array: DirectSource,
    pub index: NumberSource,
    /// Scale, delta, or right-hand comparison operand, as selected by opcode.
    pub value: NumberSource,
    /// Index into the opcode-specific product or copy payload table.
    pub producer_index: Option<u32>,
    /// The update and product indices read the same stable direct binding.
    pub shared_update_index: bool,
    /// Used only by local-write opcodes.
    pub destination: u16,
    pub checked: bool,
    /// Used only by the comparison opcode.
    pub comparison: Opcode,
    pub when_true: bool,
    pub fallthrough_pc: u32,
    pub peak: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SecondPreIncSource {
    pub array: DirectSource,
    pub index: DirectSource,
    pub reads_updated_index: bool,
}

#[cfg(feature = "profiling")]
#[derive(Clone, Debug)]
pub(crate) struct RejectedNumericSite {
    /// Published execution word PC of the original generic entry.
    pub pc: u32,
    pub source_pc: u32,
    pub family: &'static str,
    pub reason: &'static str,
    /// Bounded instruction window for interpreting the rejection.
    pub lowered_window: String,
}
