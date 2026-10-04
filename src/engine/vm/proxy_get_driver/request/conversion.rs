//! Canonical raw conversion phases retain their request owners until selection.
use super::{NumberStep, Step};

impl TryFrom<NumberStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: NumberStep) -> Result<Self, Self::Error> {
        Ok(match step {
            NumberStep::CyclePublished(result) => Self::CyclePublishedNumber(Some(result)),
            NumberStep::Complete(result) => Self::NumberComplete(Some(result)),
            step => Self::NumberProgress(Some(step)),
        })
    }
}
impl TryFrom<crate::engine::value::conversion::primitive::PrimitiveStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::value::conversion::primitive::PrimitiveStep,
    ) -> Result<Self, Self::Error> {
        Ok(match step {
            crate::engine::value::conversion::primitive::PrimitiveStep::CyclePublished(result) => {
                Self::CyclePublishedComplete(Some(result))
            }
            crate::engine::value::conversion::primitive::PrimitiveStep::Complete(result) => {
                Self::Complete(Some(result))
            }
            step => Self::PrimitiveProgress(Some(step)),
        })
    }
}
