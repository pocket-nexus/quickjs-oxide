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
    UpdateElement {
        delta: NumberSource,
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
