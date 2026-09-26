//! Static frame storage and binding classifications borrowed from one executable.

use super::metadata::{ClosureVariable, FunctionMetadata, VariableDefinition};

/// A frame layout does not own runtime values or duplicate published metadata.
/// Actual argument count belongs to the invocation. Writable arguments retain
/// enough slots for both declared parameters and extra supplied arguments;
/// preserving the original argument snapshot remains the caller's responsibility.
/// Lexical, captured and private locals keep their typed binding definitions.
pub(crate) struct FrameLayout<'a> {
    metadata: &'a FunctionMetadata,
    arguments: &'a [VariableDefinition],
    locals: &'a [VariableDefinition],
    closures: &'a [ClosureVariable],
    // Published once with the executable. No local can require TDZ or a
    // function-name owner when this is true.
    plain_local_initializers: bool,
}

impl<'a> FrameLayout<'a> {
    pub(in crate::engine::code) fn new(
        metadata: &'a FunctionMetadata,
        arguments: &'a [VariableDefinition],
        locals: &'a [VariableDefinition],
        closures: &'a [ClosureVariable],
        plain_local_initializers: bool,
    ) -> Self {
        Self {
            metadata,
            arguments,
            locals,
            closures,
            plain_local_initializers,
        }
    }

    pub(crate) fn argument_slots(&self, actual_count: usize) -> usize {
        actual_count.max(usize::from(self.metadata.argument_count))
    }

    pub(crate) fn operand_capacity(&self) -> usize {
        usize::from(self.metadata.max_stack)
    }

    pub(crate) fn is_strict(&self) -> bool {
        self.metadata.strict
    }

    pub(crate) fn arguments(&self) -> &'a [VariableDefinition] {
        self.arguments
    }

    pub(crate) fn locals(&self) -> &'a [VariableDefinition] {
        self.locals
    }

    pub(crate) fn plain_local_initializers(&self) -> bool {
        self.plain_local_initializers
    }

    pub(crate) fn function_name_local(&self) -> Option<u16> {
        self.metadata.function_name_local
    }

    pub(crate) fn closures(&self) -> &'a [ClosureVariable] {
        self.closures
    }
}
