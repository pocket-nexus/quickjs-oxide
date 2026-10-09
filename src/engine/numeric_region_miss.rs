//! Small admission outcomes shared by numeric regions and their Array probes.
//! Diagnostic names are only materialized by profiling builds.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericRegionMiss {
    OperandCapacity,
    IndexNotNumber,
    IndexNotNumericInteger,
    IndexNotImmediate,
    ScaleNotNumber,
    DeltaNotNumber,
    AccumulatorNotNumber,
    RhsNotNumber,
    DestinationOrReceiverUnavailable,
    ReceiverBindingUnavailable,
    ReceiverNotObject,
    ReceiverNotArray,
    OwnElementNotNumber,
    MissingOwnElement,
    UnsupportedMaterializedOwnElement,
    UnsupportedOwnElement,
    ArrayStorageUnavailable,
    ArrayShapeUnavailable,
    OwnElementNotWritable,
    PropertyGenerationOverflow,
}

#[cfg(feature = "profiling")]
impl NumericRegionMiss {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::OperandCapacity => "operand_capacity",
            Self::IndexNotNumber => "index_not_number",
            Self::IndexNotNumericInteger => "index_not_numeric_integer",
            Self::IndexNotImmediate => "index_not_immediate",
            Self::ScaleNotNumber => "scale_not_number",
            Self::DeltaNotNumber => "delta_not_number",
            Self::AccumulatorNotNumber => "accumulator_not_number",
            Self::RhsNotNumber => "rhs_not_number",
            Self::DestinationOrReceiverUnavailable => "destination_or_receiver_unavailable",
            Self::ReceiverBindingUnavailable => "receiver_binding_unavailable",
            Self::ReceiverNotObject => "receiver_not_object",
            Self::ReceiverNotArray => "receiver_not_array",
            Self::OwnElementNotNumber => "own_element_not_number",
            Self::MissingOwnElement => "missing_own_element",
            Self::UnsupportedMaterializedOwnElement => "unsupported_materialized_own_element",
            Self::UnsupportedOwnElement => "unsupported_own_element",
            Self::ArrayStorageUnavailable => "array_storage_unavailable",
            Self::ArrayShapeUnavailable => "array_shape_unavailable",
            Self::OwnElementNotWritable => "own_element_not_writable",
            Self::PropertyGenerationOverflow => "property_generation_overflow",
        }
    }
}
