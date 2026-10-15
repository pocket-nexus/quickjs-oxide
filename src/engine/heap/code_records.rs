use super::*;

/// Constant-pool entry owned by a [`FunctionBytecodeData`] node.
#[derive(Clone, Debug)]
pub enum BytecodeConstant {
    Value(RawValue),
    /// Compile-once RegExp literal payload. These reference-counted Rust
    /// leaves own no arena or atom edge; executing `Instruction::RegExp`
    /// clones them into a fresh realm-local RegExp object.
    RegExp {
        pattern: JsString,
        program: Rc<CompiledRegExp>,
    },
    Function(FunctionBytecodeId),
}

/// Input to publication. Compiler instructions are consumed by the allocator;
/// no `FunctionBytecodeDraft` is retained by a published function.
#[derive(Debug)]
pub struct FunctionBytecodeDraft {
    pub code: Rc<[Instruction]>,
    pub numeric_regions: Box<[crate::engine::code::region::NumericRegion]>,
    pub constants: Rc<[BytecodeConstant]>,
    pub property_key_atoms: Option<Rc<[Atom]>>,
    pub realm: ContextId,
    pub metadata: FunctionMetadata,
    pub parameter_environment: Option<ParameterEnvironmentLayout>,
    pub func_name: Option<JsString>,
    pub argument_definitions: Rc<[VariableDefinition]>,
    pub local_definitions: Rc<[VariableDefinition]>,
    pub closure_variables: Rc<[ClosureVariable]>,
    pub eval_environments: Rc<[EvalEnvironment<Atom>]>,
    pub debug: Option<FunctionDebugInfo>,
    pub auxiliary_atoms: Box<[Atom]>,
}

/// Runtime-owned immutable execution words, constant pool, and function realm.
#[derive(Debug)]
pub struct FunctionBytecodeData {
    /// Lazily shared read-only execution projection. It contains no GC roots:
    /// each external snapshot separately retains this bytecode node. Allocation
    /// clears any incoming cache before sealing the authenticated node.
    pub(crate) executable:
        std::cell::OnceCell<Rc<crate::engine::code::runtime::PublishedFunctionData>>,

    pub exec: crate::engine::code::exec::ExecCode,
    pub constants: Rc<[BytecodeConstant]>,
    /// Constant-indexed static names linked by the runtime publisher. Null
    /// entries are unused; functions without static names have no table.
    /// These identities borrow the references owned by `auxiliary_atoms`.
    pub property_key_atoms: Option<Rc<[Atom]>>,
    pub realm: ContextId,
    pub metadata: FunctionMetadata,
    pub parameter_environment: Option<ParameterEnvironmentLayout>,
    /// Intrinsic source-level name. Contextual `SetName` inference remains a
    /// separate opcode and is only emitted for anonymous definitions.
    pub func_name: Option<JsString>,
    pub argument_definitions: Rc<[VariableDefinition]>,
    pub local_definitions: Rc<[VariableDefinition]>,
    pub closure_variables: Rc<[ClosureVariable]>,
    pub eval_environments: Rc<[EvalEnvironment<Atom>]>,
    pub debug: Option<FunctionDebugInfo>,
    /// Atom references owned by bytecode metadata/opcode operands.
    ///
    /// Symbol constants are excluded: every `RawValue::Symbol` occurrence owns
    /// and releases its own separate atom reference.
    pub auxiliary_atoms: Box<[Atom]>,
}

/// Runtime-owned debug metadata for one bytecode function.
///
/// `filename` is backed by one distinct reference in `auxiliary_atoms`; it is
/// intentionally not released separately when the bytecode node dies.
#[derive(Clone, Debug)]
pub struct FunctionDebugInfo {
    pub filename: Atom,
    pub pc2line: Option<Pc2LineTable>,
    pub source: Option<Box<[u8]>>,
}
