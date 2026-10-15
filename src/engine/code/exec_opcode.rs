//! Numeric execution opcodes shared by the publisher, disassembler and VM.
//! The numbering is declared by the opcode table in `instruction.rs`.
pub(crate) use super::instruction::Opcode;

#[cfg(test)]
mod tests {
    use super::Opcode;

    #[test]
    fn numeric_opcode_conversion_rejects_unassigned_discriminants() {
        // Keep the existing execution-word assignments stable while changing
        // how a numeric opcode is authenticated.
        assert_eq!(Opcode::BorrowedFieldThis as u16, 234);
        for raw in 0..=u16::MAX {
            match Opcode::from_raw(raw) {
                Some(opcode) => {
                    assert_eq!(opcode as u16, raw);
                    assert!(raw <= 234);
                }
                None => assert!(raw > 234),
            }
        }
    }
}
