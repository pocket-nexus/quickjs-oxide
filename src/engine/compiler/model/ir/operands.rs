//! Function-owned cold operands. Ordinary opcodes carry compact typed indexes.
use super::IrOp;
use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::code::bytecode::DynamicEnvironmentSource;
use crate::engine::compiler::lexer::Span;

#[derive(Clone, Copy, Debug)]
pub(in crate::engine::compiler) struct SpanId(u32);

#[derive(Clone, Copy, Debug)]
pub(in crate::engine::compiler) struct DynamicId(pub(in crate::engine::compiler) u32);

#[derive(Clone, Copy, Debug)]
pub(in crate::engine::compiler) struct DynamicReferenceId(pub(in crate::engine::compiler) u32);

#[derive(Debug)]
pub(in crate::engine::compiler) struct DynamicOperand {
    pub(in crate::engine::compiler) name: u32,
    pub(in crate::engine::compiler) sources: Box<[DynamicEnvironmentSource]>,
    pub(in crate::engine::compiler) fallback: IrOp,
}

#[derive(Debug)]
pub(in crate::engine::compiler) struct DynamicReferenceOperand {
    pub(in crate::engine::compiler) name: u32,
    pub(in crate::engine::compiler) sources: Box<[DynamicEnvironmentSource]>,
    pub(in crate::engine::compiler) late_sources: Box<[DynamicEnvironmentSource]>,
    pub(in crate::engine::compiler) fallback: IrOp,
    pub(in crate::engine::compiler) syntactic_with: bool,
    pub(in crate::engine::compiler) fallback_readonly: bool,
}

#[derive(Debug, Default)]
pub(in crate::engine::compiler) struct IrOperands {
    pub(in crate::engine::compiler) spans: Vec<Span>,
    pub(in crate::engine::compiler) dynamic: Vec<Option<DynamicOperand>>,
    pub(in crate::engine::compiler) references: Vec<Option<DynamicReferenceOperand>>,
}

fn index(length: usize) -> Result<u32, Error> {
    u32::try_from(length)
        .map_err(|_| Error::new(ErrorKind::JsInternal, "too many compiler operands"))
}

impl IrOperands {
    pub(in crate::engine::compiler) fn add_span(&mut self, span: Span) -> Result<SpanId, Error> {
        // Reference rewrites commonly emit several operations at the same site.
        if self.spans.last() == Some(&span) {
            return Ok(SpanId(index(self.spans.len() - 1)?));
        }
        let id = SpanId(index(self.spans.len())?);
        self.spans.push(span);
        Ok(id)
    }

    pub(in crate::engine::compiler) fn span(&self, id: SpanId) -> Span {
        self.spans[id.0 as usize]
    }

    pub(in crate::engine::compiler) fn add_dynamic(
        &mut self,
        value: DynamicOperand,
    ) -> Result<DynamicId, Error> {
        let id = DynamicId(index(self.dynamic.len())?);
        self.dynamic.push(Some(value));
        Ok(id)
    }

    pub(in crate::engine::compiler) fn add_reference(
        &mut self,
        value: DynamicReferenceOperand,
    ) -> Result<DynamicReferenceId, Error> {
        let id = DynamicReferenceId(index(self.references.len())?);
        self.references.push(Some(value));
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::compiler::model::ir::SpannedIrOp;

    #[test]
    fn cold_operands_do_not_widen_the_instruction_stream() {
        // This is a footprint contract for the hot parser write stream, not
        // an ABI promise. Keep new variable-sized payloads in the operand table.
        assert!(size_of::<SpannedIrOp>() <= 40);
        eprintln!(
            "IR layout: IrOp={}, SpannedIrOp={}",
            size_of::<IrOp>(),
            size_of::<SpannedIrOp>()
        );
    }
}
