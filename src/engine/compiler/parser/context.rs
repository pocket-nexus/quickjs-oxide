//! Temporary state discarded when a function builder finishes.

use crate::engine::compiler::BreakControlContext;
use crate::engine::compiler::model::scope::ScopeId;
use crate::engine::compiler::optional_chain::FinalizedOptionalChain;

#[derive(Debug)]
pub(in crate::engine::compiler) struct FunctionParseContext {
    /// Parser-only Reference marker for the final member getter. QuickJS uses
    /// `last_opcode_pos` for the same rewrite, but an explicit index prevents
    /// comma/conditional values from accidentally retaining a method receiver.
    pub(in crate::engine::compiler) last_member_reference: Option<usize>,
    /// Parser-only Reference marker for a final identifier read. This lets
    /// parenthesized IdentifierReferences remain assignment targets while
    /// composed values (comma, conditional, logical and binary forms) do not.
    pub(in crate::engine::compiler) last_identifier_reference: Option<usize>,
    /// A completed optional chain whose value has not yet been composed with
    /// an outer operation. Parentheses deliberately preserve this marker.
    pub(in crate::engine::compiler) last_optional_chain: Option<FinalizedOptionalChain>,
    pub(in crate::engine::compiler) break_controls: Vec<BreakControlContext>,
    pub(in crate::engine::compiler) stack_depth: usize,
    pub(in crate::engine::compiler) current_scope: ScopeId,
    /// YieldExpression is disabled while generator formal initializers parse
    /// and becomes active only after the InitialYield boundary is installed.
    pub(in crate::engine::compiler) in_function_body: bool,
}

impl FunctionParseContext {
    pub(super) fn new(body_scope: ScopeId) -> Self {
        Self {
            last_member_reference: None,
            last_identifier_reference: None,
            last_optional_chain: None,
            break_controls: Vec::new(),
            stack_depth: 0,
            current_scope: body_scope,
            in_function_body: false,
        }
    }
}
