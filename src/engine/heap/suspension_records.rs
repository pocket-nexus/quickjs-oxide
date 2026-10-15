use super::*;

/// ECMAScript-visible lifecycle of a branded synchronous generator object.
#[derive(Clone, Copy, Debug, Hash, PartialEq)]
pub enum GeneratorState {
    SuspendedStart,
    SuspendedYield,
    SuspendedYieldStar,
    Executing,
    Completed,
}

/// Heap-native representation of one argument or local binding retained by a
/// dormant generator frame. Runtime-owning root wrappers must never enter this
/// structure: every GC identity is stored as a raw arena edge instead.
#[derive(Clone, Debug)]
pub enum GeneratorFrameBinding {
    Direct(RawValue),
    Private(AtomIdx),
    PrivateCallable(ObjectId),
    Uninitialized,
    Captured(VarRefId),
}

/// Raw VM fields retained across a synchronous-generator suspension.
#[derive(Clone, Debug)]
pub struct GeneratorVmActivation {
    pub stack: Vec<RawValue>,
    pub regions: Vec<crate::engine::vm::VmUnwindRegion>,
    pub pc: usize,
    pub callee_realm: ContextId,
    pub current_function: ObjectId,
    pub this_value: RawValue,
    pub normalized_this: Option<RawValue>,
    pub new_target: RawValue,
    pub strict: bool,
    pub callee_global: ObjectId,
}

/// Complete dormant execution state owned by one generator object.
#[derive(Clone, Debug)]
pub struct GeneratorActivationData {
    pub bytecode: FunctionBytecodeId,
    pub vm: GeneratorVmActivation,
    pub actual_argument_count: usize,
    /// Immutable call operands, independent of writable parameter bindings.
    pub original_arguments: Vec<RawValue>,
    pub arguments: Vec<GeneratorFrameBinding>,
    pub locals: Vec<GeneratorFrameBinding>,
    pub reusable_captured_locals: Vec<bool>,
}

/// ECMAScript-visible lifecycle of a branded async-generator object.
#[derive(Clone, Copy, Debug, Hash, PartialEq)]
pub enum AsyncGeneratorState {
    SuspendedStart,
    SuspendedYield,
    SuspendedYieldStar,
    Executing,
    AwaitingReturn,
    Completed,
}

/// One queued `.next`, `.return`, or `.throw` request and its Promise
/// capability. Every identity is stored as a raw traced edge.
#[derive(Clone, Debug)]
pub struct AsyncGeneratorRequestData {
    pub completion: GeneratorResumeKind,
    pub result: RawValue,
    pub promise: ObjectId,
    pub resolve: ObjectId,
    pub reject: ObjectId,
}

/// Complete hidden state of one genuine AsyncGenerator.
#[derive(Clone, Debug)]
pub struct AsyncGeneratorData {
    pub state: AsyncGeneratorState,
    pub activation: Option<Box<GeneratorActivationData>>,
    pub queue: VecDeque<AsyncGeneratorRequestData>,
    /// Realm whose intrinsic Promise machinery owns the currently installed
    /// await/return reaction callbacks.
    pub resume_realm: Option<ContextId>,
}

/// Settlement branch selected by an internal async-function resume callback.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AsyncFunctionResumeKind {
    Fulfill,
    Reject,
}

/// Settlement branch selected by an internal async-generator reaction.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AsyncGeneratorResumeKind {
    AwaitFulfill,
    AwaitReject,
    ReturnFulfill,
    ReturnReject,
}

/// Heap-visible lifecycle of one async-function driver.
///
/// The active VM frame is rooted by the runtime while `Executing`. At an
/// `await`, ownership transfers into the state object and the phase becomes
/// `Awaiting`; `Completed` is absorbing.
#[derive(Clone, Copy, Debug, Hash, PartialEq)]
pub enum AsyncFunctionPhase {
    Executing,
    Awaiting,
    Completed,
}

/// Hidden driver state shared by the two callbacks installed for one `await`.
///
/// QuickJS keeps this as a separate GC object so the pending Promise reaction
/// owns the dormant activation without exposing it through authored
/// properties. `driver_realm` is the original caller realm which supplies the
/// returned Promise and await jobs; it may differ from the bytecode activation
/// realm. Every arena identity here is a raw, traced edge.
#[derive(Clone, Debug)]
pub struct AsyncFunctionStateData {
    pub driver_realm: ContextId,
    pub outer_resolve: ObjectId,
    pub outer_reject: ObjectId,
    pub activation: Option<Box<GeneratorActivationData>>,
    pub phase: AsyncFunctionPhase,
}
